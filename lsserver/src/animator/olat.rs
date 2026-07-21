use crate::{animator::Animator, config::ServerConfig, renderer::Renderer};

/// One-Light-At-a-Time animator.
///
/// Loops through each individual white fixture and sets it full on for a tick.
#[derive(Debug)]
pub struct OlatAnimator {
    current_arc: usize,
    current_light: usize,
    num_arcs: usize,
    lights_per_arc: usize,
    done: bool,
}

impl OlatAnimator {
    pub fn new(config: &ServerConfig) -> Self {
        Self {
            current_arc: 0,
            current_light: 0,
            num_arcs: config.num_arcs,
            lights_per_arc: config.lights_per_arc,
            done: false,
        }
    }
}

impl Animator for OlatAnimator {
    fn tick(&mut self, renderer: &mut Renderer) -> bool {
        // clear all fixtures
        for arc in &mut renderer.rgb_fixtures {
            for light in arc {
                light.set_color(0, 0, 0);
            }
        }
        for arc in &mut renderer.white_fixtures {
            for light in arc {
                light.set_white(0, 0, 0);
            }
        }

        // If we finished already, return
        if self.done {
            return false;
        }

        // illuminate the current light
        renderer.white_fixtures[self.current_arc][self.current_light].set_white(
            u16::MAX,
            u16::MAX,
            u16::MAX,
        );

        // advance pattern
        self.current_light += 1;
        if self.current_light >= self.lights_per_arc {
            self.current_light = 0;
            self.current_arc += 1;
        }
        if self.current_arc >= self.num_arcs {
            self.current_arc = 0;
            self.done = true;
        }

        true
    }

    fn total_frames(&self) -> Option<usize> {
        Some(self.num_arcs * self.lights_per_arc)
    }
}
