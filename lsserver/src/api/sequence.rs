//! Storage of pre-loaded playback sequences.
//!
//! A [`PlaybackSequence`] is saved onto disk using (optionally zstd-compressed) CBOR representation.

use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use zstd::{Decoder, Encoder};

use crate::api::FixtureColour;

/// A single frame capturing the states ([`crate::api::FixtureColour`]) of all white and colour fixtures on the light stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageFrame {
    pub white_fixtures: Vec<Vec<FixtureColour>>,
    pub rgb_fixtures: Vec<Vec<FixtureColour>>,
}

/// A playback sequence
///
/// An ordered sequence of [`StageFrame`] data at a specific capture frequency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackSequence {
    pub name: String,
    pub capture_hz: f64,
    pub frames: Vec<StageFrame>,
}

/// Metadata about a [`PlaybackSequence`], without the frame data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceSummary {
    pub id: String,
    pub name: String,
    pub capture_hz: f64,
    pub total_frames: usize,
    pub duration_secs: f64,
}

impl PlaybackSequence {
    /// Generate a [`SequenceSummary`] for this sequence given an ID.
    pub fn summary(&self, id: String) -> SequenceSummary {
        let total_frames = self.frames.len();

        #[allow(clippy::cast_precision_loss)]
        let duration_secs = if self.capture_hz > 0.0 {
            total_frames as f64 / self.capture_hz
        } else {
            0.0
        };

        SequenceSummary {
            id,
            name: self.name.clone(),
            capture_hz: self.capture_hz,
            total_frames,
            duration_secs,
        }
    }
}

/// Storage handler for sequence files on disk.
///
/// Manages persistence, loading, deletion of [`PlaybackSequence`] representation on disk.
#[derive(Debug, Clone)]
pub struct SequenceStore {
    storage_dir: PathBuf,
}

impl SequenceStore {
    /// Returns a new [`SequenceStore`].
    ///
    /// Creates the storage directory if it does not already exist.
    pub fn new(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let storage_dir = dir.into();
        if !storage_dir.exists() {
            // try to create dir, with only one depth level.
            // purposely fails if parent directory also doesn't exist.
            fs::create_dir(&storage_dir).with_context(|| {
                format!(
                    "Sequence storage path could not be created: {}",
                    storage_dir.display()
                )
            })?;
        }
        if !storage_dir.is_dir() {
            anyhow::bail!(
                "Sequence storage path is not a directory: {}",
                storage_dir.display()
            );
        }
        Ok(Self { storage_dir })
    }

    /// Helper for cbor file paths for given ID.
    fn file_path(&self, id: &str) -> PathBuf {
        self.storage_dir.join(format!("{id}.cbor"))
    }

    /// Helper for cbor.zst file paths for given ID.
    fn file_path_zst(&self, id: &str) -> PathBuf {
        self.storage_dir.join(format!("{id}.cbor.zst"))
    }

    /// Saves a [`PlaybackSequence`] to disk, as zstd-compressed CBOR data.
    ///
    /// Generates a new [`Ulid`] ID and returns the resulting [`SequenceSummary`].
    pub fn save(&self, sequence: &PlaybackSequence) -> anyhow::Result<SequenceSummary> {
        let id = Ulid::generate().to_string();
        let path = self.file_path_zst(&id);

        let file = File::create(&path)?;
        let writer = BufWriter::new(file);
        let zstd_writer = Encoder::new(writer, 3)?.auto_finish();
        ciborium::into_writer(sequence, zstd_writer)?;

        Ok(sequence.summary(id))
    }

    /// Load a sequence from disk by its [`Ulid`] ID.
    ///
    /// Tries `.cbor.zst` first, then `.cbor`.
    pub fn load(&self, id: Ulid) -> anyhow::Result<PlaybackSequence> {
        let id_str = &id.to_string();

        // try compressed version first
        let zst_path = self.file_path_zst(id_str);
        if zst_path.exists() {
            return Self::read_sequence(&zst_path, true);
        }

        // fallback to uncompressed
        let cbor_path = self.file_path(id_str);
        if cbor_path.exists() {
            return Self::read_sequence(&cbor_path, false);
        }

        anyhow::bail!("Sequence id '{id}' not found");
    }

    /// Retrieves summaries for all available sequences in the storage directory [`Self::storage_dir`].
    pub fn list(&self) -> anyhow::Result<Vec<SequenceSummary>> {
        let mut summaries = Vec::new();
        let mut seen_ids = HashSet::new();

        for entry in fs::read_dir(&self.storage_dir)? {
            let path = entry?.path();
            if !path.is_file() {
                continue; // not a file
            }

            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();

            let (id_str, is_zst) = if let Some(id) = file_name.strip_suffix(".cbor.zst") {
                (id, true)
            } else if let Some(id) = file_name.strip_suffix(".cbor") {
                (id, false)
            } else {
                continue; // file doesn't end in .cbor or .cbor.zst
            };

            if !seen_ids.insert(id_str.to_owned()) {
                continue; // sequence id exists twice? ignore it.
            }

            if let Ok(seq) = Self::read_sequence(&path, is_zst) {
                summaries.push(seq.summary(id_str.to_owned()));
            }
        }

        Ok(summaries)
    }

    /// Deletes all sequence files (compressed and uncompressed) for a [`Ulid`], if it exists.
    pub fn delete(&self, id: Ulid) -> anyhow::Result<()> {
        let id_str = &id.to_string();
        let zst_path = self.file_path_zst(id_str);
        let cbor_path = self.file_path(id_str);

        let mut deleted = false;
        if zst_path.exists() {
            fs::remove_file(&zst_path).with_context(|| {
                format!(
                    "Failed to delete compressed sequence: {}",
                    zst_path.display()
                )
            })?;
            deleted = true;
        }
        if cbor_path.exists() {
            fs::remove_file(&cbor_path)
                .with_context(|| format!("Failed to delete sequence: {}", cbor_path.display()))?;
            deleted = true;
        }

        if !deleted {
            anyhow::bail!("Sequence id '{id}' not found")
        }

        Ok(())
    }

    /// Helper to deserialise a [`PlaybackSequence`] from a compressed or uncompressed file.
    fn read_sequence(path: &Path, is_zst: bool) -> anyhow::Result<PlaybackSequence> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        if is_zst {
            let zstd_reader = Decoder::new(reader)?;
            Ok(ciborium::from_reader(zstd_reader)?)
        } else {
            Ok(ciborium::from_reader(reader)?)
        }
    }
}
