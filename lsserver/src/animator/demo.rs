use std::{f32, time::Instant};

use crate::{animator::Animator, config::ServerConfig, renderer::Renderer};

const HALF: f32 = u16::MAX as f32 / 2.0;
const TAU_THIRD: f32 = f32::consts::TAU / 3.0;

#[derive(Debug)]
pub struct DemoAnimator {
    start_time: Instant,
    brightness: f32,
    num_arcs: usize,
    lights_per_arc: usize,
}

impl DemoAnimator {
    pub fn new(brightness: f32, config: &ServerConfig) -> Self {
        Self {
            start_time: Instant::now(),
            brightness,
            num_arcs: config.num_arcs,
            lights_per_arc: config.lights_per_arc,
        }
    }
}

impl Animator for DemoAnimator {
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn tick(&mut self, renderer: &mut Renderer) -> bool {
        // this following is chatgpt's doing not mine
        // it's pretty.
        // i don't trust it.

        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Pre-calculate our sine wave boundaries based on the brightness
        let amplitude = HALF * self.brightness;
        let center = HALF * self.brightness;

        for arc in 0..self.num_arcs {
            for light in 0..self.lights_per_arc {
                let phase_offset = (arc as f32 * 0.5) + (light as f32 * 0.2);
                let time = elapsed * 2.0 + phase_offset;

                // Calculate RGB using the new dimmed amplitude and center
                let r = ((time).sin() * amplitude + center) as u16;
                let g = ((time + TAU_THIRD).sin() * amplitude + center) as u16;
                let b = ((time + 2.0 * TAU_THIRD).sin() * amplitude + center) as u16;

                renderer.rgb_fixtures[arc][light].set_color(r, g, b);

                // Apply the same dimming to the white fixtures
                let w_phase = elapsed * 1.5 - phase_offset;
                let w = ((w_phase.sin() * amplitude) + center) as u16;
                renderer.white_fixtures[arc][light].set_white(w, w, w);
            }
        }

        true // demo never dies
    }

    fn total_frames(&self) -> Option<usize> {
        None
    }
}
