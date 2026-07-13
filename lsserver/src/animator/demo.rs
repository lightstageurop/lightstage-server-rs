use std::time::Instant;

use crate::{config::ServerConfig, renderer::Renderer};

#[derive(Debug)]
pub struct DemoAnimator {
    start_time: Instant,
    brightness: f32,
    config: ServerConfig,
}

impl DemoAnimator {
    pub fn new(brightness: f32, config: ServerConfig) -> Self {
        Self {
            start_time: Instant::now(),
            brightness,
            config,
        }
    }

    pub fn tick(&self, renderer: &mut Renderer) {
        // this following is chatgpt's doing not mine
        // it's pretty.
        // i don't trust it.

        let elapsed = self.start_time.elapsed().as_secs_f32();

        // Pre-calculate our sine wave boundaries based on the brightness
        let amplitude = 32767.5 * self.brightness;
        let center = 32767.5 * self.brightness;

        for arc in 0..self.config.num_arcs {
            for light in 0..self.config.lights_per_arc {
                let phase_offset = (arc as f32 * 0.5) + (light as f32 * 0.2);
                let t = elapsed * 2.0 + phase_offset;

                // Calculate RGB using the new dimmed amplitude and center
                let r = ((t).sin() * amplitude + center) as u16;
                let g = ((t + 2.094).sin() * amplitude + center) as u16;
                let b = ((t + 4.188).sin() * amplitude + center) as u16;

                renderer.rgb_fixtures[arc][light].set_color(r, g, b);

                // Apply the same dimming to the white fixtures
                let w_phase = elapsed * 1.5 - phase_offset;
                let w = ((w_phase.sin() * amplitude) + center) as u16;
                renderer.white_fixtures[arc][light].set_white(w, w, w);
            }
        }
    }
}
