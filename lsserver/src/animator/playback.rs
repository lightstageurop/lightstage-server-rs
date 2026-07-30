use crate::{
    animator::Animator,
    api::{PlaybackSequence, StageFrame},
};

#[derive(Debug, Default)]
pub struct PlaybackAnimator {
    /// Loaded animation sequence for [`crate::state::StageMode::Playback`]
    sequence: Vec<StageFrame>,
    /// Current frame index within sequence
    seq_index: usize,
}

impl PlaybackAnimator {
    pub fn new(playback: PlaybackSequence) -> Self {
        Self {
            sequence: playback.frames,
            seq_index: 0,
        }
    }
}

impl Animator for PlaybackAnimator {
    fn tick(&mut self, renderer: &mut crate::renderer::Renderer) -> bool {
        if self.seq_index >= self.sequence.len() {
            return false;
        }

        let frame = &self.sequence[self.seq_index];
        for (arc_idx, arc_data) in frame.rgb_fixtures.iter().enumerate() {
            if let Some(renderer_arc) = renderer.rgb_fixtures.get_mut(arc_idx) {
                for (light_idx, rgb) in arc_data.iter().enumerate() {
                    if let Some(light) = renderer_arc.get_mut(light_idx) {
                        light.set_color(rgb.0, rgb.1, rgb.2);
                    }
                }
            }
        }
        for (arc_idx, arc_data) in frame.white_fixtures.iter().enumerate() {
            if let Some(renderer_arc) = renderer.white_fixtures.get_mut(arc_idx) {
                for (light_idx, white) in arc_data.iter().enumerate() {
                    if let Some(light) = renderer_arc.get_mut(light_idx) {
                        light.set_white(white.0, white.1, white.2);
                    }
                }
            }
        }

        self.seq_index += 1;

        true
    }

    fn total_frames(&self) -> Option<usize> {
        Some(self.sequence.len())
    }
}
