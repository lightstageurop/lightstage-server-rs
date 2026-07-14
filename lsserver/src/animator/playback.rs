use crate::{LightStageFrame, animator::Animator};

#[derive(Debug, Default)]
pub struct PlaybackAnimator {
    /// Loaded animation sequence for [`StageMode::Playback`]
    sequence: Vec<LightStageFrame>,
    /// Current frame index within sequence
    seq_index: usize,
}

impl Animator for PlaybackAnimator {
    fn tick(&mut self, renderer: &mut crate::renderer::Renderer) {
        todo!()
    }
}
