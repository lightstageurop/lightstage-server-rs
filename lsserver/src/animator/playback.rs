use crate::{LightStageFrame, animator::Animator};

#[derive(Debug, Default)]
pub struct PlaybackAnimator {
    /// Loaded animation sequence for [`crate::state::StageMode::Playback`]
    sequence: Vec<LightStageFrame>,
    /// Current frame index within sequence
    seq_index: usize,
}

impl PlaybackAnimator {
    pub fn new() -> Self {
        todo!()
    }
}

impl Animator for PlaybackAnimator {
    fn tick(&mut self, renderer: &mut crate::renderer::Renderer) -> bool {
        todo!()
    }

    fn total_frames(&self) -> Option<usize> {
        Some(self.sequence.len())
    }
}
