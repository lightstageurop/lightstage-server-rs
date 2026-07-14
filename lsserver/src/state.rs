//! Internal light stage state(s)

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    LightStageFrame,
    animator::{Animator, DemoAnimator, OlatAnimator},
    config::ServerConfig,
    renderer::Renderer,
};

/// Defines the active operation mode of the light stage.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, ToSchema)]
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
    Playback { capture_fps: f64 },
    /// One Light At a Time
    OLAT { capture_hz: f64 },
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

    demo_animator: DemoAnimator,
    olat_animator: OlatAnimator,
}

impl StageState {
    pub fn new(renderer: Renderer, config: ServerConfig) -> Self {
        Self {
            mode: StageMode::default(),
            renderer,
            current_frame: LightStageFrame::black(),
            sequence: vec![],
            seq_index: 0,
            demo_animator: DemoAnimator::new(0.2, &config),
            olat_animator: OlatAnimator::new(&config),
        }
    }

    pub fn advance_tick(&mut self) -> (LightStageFrame, bool) {
        match self.mode {
            StageMode::Demo => {
                self.demo_animator.tick(&mut self.renderer);
                self.commit_and_render();
                (self.current_frame.clone(), false)
            }
            StageMode::Manual => (self.current_frame.clone(), false),
            StageMode::Playback { capture_fps } => todo!(),
            StageMode::OLAT { capture_hz } => {
                self.olat_animator.tick(&mut self.renderer);
                self.commit_and_render();
                (self.current_frame.clone(), true)
            }
        }
    }

    /// Update an rgb and a white fixture as a pair.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_single_fixture(
        &mut self,
        arc_idx: usize,
        light_idx: usize,
        rgb: (u16, u16, u16),
        white: (u16, u16, u16),
    ) {
        self.mode = StageMode::Manual;
        self.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
        self.renderer.white_fixtures[arc_idx][light_idx].set_white(white.0, white.1, white.2);
        self.commit_and_render();
    }

    /// Batch update a set of rgb and white fixture pairs.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_batch_fixtures(
        &mut self,
        fixtures: impl IntoIterator<Item = (usize, usize, (u16, u16, u16), (u16, u16, u16))>,
    ) {
        self.mode = StageMode::Manual;
        for (arc_idx, light_idx, rgb, white) in fixtures {
            self.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
            self.renderer.white_fixtures[arc_idx][light_idx].set_white(white.0, white.1, white.2);
        }
        self.commit_and_render();
    }

    /// Update rgb and white for an arc.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_arc(
        &mut self,
        arc_idx: usize,
        rgb: (u16, u16, u16),
        white: (u16, u16, u16),
    ) {
        self.mode = StageMode::Manual;
        for light in &mut self.renderer.rgb_fixtures[arc_idx] {
            light.set_color(rgb.0, rgb.1, rgb.2);
        }
        for light in &mut self.renderer.white_fixtures[arc_idx] {
            light.set_white(white.0, white.1, white.2);
        }
        self.commit_and_render();
    }

    /// Update rgb and white for entire stage.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_stage(&mut self, rgb: (u16, u16, u16), white: (u16, u16, u16)) {
        self.mode = StageMode::Manual;
        for arc in &mut self.renderer.rgb_fixtures {
            for light in arc {
                light.set_color(rgb.0, rgb.1, rgb.2);
            }
        }
        for arc in &mut self.renderer.white_fixtures {
            for light in arc {
                light.set_white(white.0, white.1, white.2);
            }
        }
        self.commit_and_render();
    }

    /// Commits all pending fixture changes and calls [`crate::renderer::Renderer::update`].
    fn commit_and_render(&mut self) {
        self.renderer.update(&mut self.current_frame);
    }
}

pub type SharedState = Arc<RwLock<StageState>>;
