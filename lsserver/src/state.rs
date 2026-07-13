use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{LightStageFrame, renderer::Renderer};

/// Defines the active operation mode of the light stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum StageMode {
    /// Runs a pleasing background animation
    #[default]
    Demo,
    /// Awaits explicitly defined frames via the API.
    ///
    /// Keeps refreshing the same frame if no new updates are sent.
    /// Intended to be used for slow, or no capture. Shutter synchronisation is not guaranteed.
    Manual,
    /// Plays back a pre-loaded sequence of frames. Used for capture.
    Playback,
}

/// Shared lightstage state
#[derive(Debug)]
pub struct StageState {
    pub mode: StageMode,
    pub renderer: Renderer,
    /// Current frame for [`StageMode::Manual`]
    pub current_frame: LightStageFrame,
    /// Loaded animation sequence for [`StageMode::Playback`]
    pub sequence: Vec<LightStageFrame>,
    /// Current frame index within sequence
    pub seq_index: usize,
}

impl StageState {
    pub fn new(renderer: Renderer) -> Self {
        Self {
            mode: StageMode::default(),
            renderer,
            current_frame: LightStageFrame::black(),
            sequence: vec![],
            seq_index: 0,
        }
    }
}

pub type SharedState = Arc<RwLock<StageState>>;
