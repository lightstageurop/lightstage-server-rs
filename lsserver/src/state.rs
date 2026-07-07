use std::sync::{Arc, RwLock};

use serde::Deserialize;

use crate::{LightStageFrame, renderer::Renderer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum StageMode {
    #[default]
    Demo,
    Manual,
    Playback,
}

/// Shared lightstage state
pub struct StageState {
    pub mode: StageMode,
    pub renderer: Renderer,
    pub current_frame: LightStageFrame,
    /// Sequence for [`StageMode::Playback`]
    pub sequence: Vec<LightStageFrame>,
    /// Current frame from sequence
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
