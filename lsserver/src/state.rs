//! Internal light stage state(s)

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    LightStageFrame,
    animator::{ActiveAnimator, Animator, DemoAnimator, OlatAnimator},
    config::ServerConfig,
    renderer::Renderer,
};

/// Defines the active operation mode of the light stage.
#[allow(clippy::upper_case_acronyms)]
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

/// Metadata about a capturing session (eg. an OLAT sequence)
#[derive(Debug, Clone)]
pub struct CaptureSession {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    Continue,
    TriggerCapture,
    Finished,
}

/// Shared lightstage state
#[derive(Debug)]
pub struct StageState {
    pub mode: StageMode,
    renderer: Renderer,
    /// Current frame for [`StageMode::Manual`]
    pub current_frame: LightStageFrame,
    /// Currently active capture session
    pub active_session: Option<CaptureSession>,

    animator: ActiveAnimator,

    config: ServerConfig,
}

impl StageState {
    pub fn new(renderer: Renderer, config: ServerConfig) -> Self {
        Self {
            mode: StageMode::default(),
            renderer,
            current_frame: LightStageFrame::black(),
            active_session: None,
            animator: ActiveAnimator::Demo(DemoAnimator::new(0.2, &config)),
            config,
        }
    }

    pub fn advance_tick(&mut self, dest: &mut LightStageFrame) -> TickResult {
        if self.mode == StageMode::Manual {
            dest.clone_from(&self.current_frame);
            TickResult::Continue
        } else {
            let still_active = self.animator.tick(&mut self.renderer);
            self.commit_and_render();
            if still_active {
                let trigger = self.mode != StageMode::Demo;
                dest.clone_from(&self.current_frame);
                if trigger {
                    TickResult::TriggerCapture
                } else {
                    TickResult::Continue
                }
            } else {
                // sequence ended. transition to idle
                self.transition_to(StageMode::Demo);
                dest.clone_from(&self.current_frame);
                TickResult::Finished
            }
        }
    }

    /// Transition to a new state
    pub fn transition_to(&mut self, new_mode: StageMode) {
        self.mode = new_mode;
        match new_mode {
            StageMode::Demo => {
                let anim = DemoAnimator::new(0.2, &self.config);
                self.animator = ActiveAnimator::Demo(anim);
            }
            StageMode::Manual => {
                self.animator = ActiveAnimator::None;
            }
            StageMode::Playback { .. } => {
                todo!()
            }
            StageMode::OLAT { .. } => {
                let anim = OlatAnimator::new(&self.config);
                self.animator = ActiveAnimator::OLAT(anim);
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
        rgb: Option<(u16, u16, u16)>,
        white: Option<(u16, u16, u16)>,
    ) {
        self.transition_to(StageMode::Manual);
        if let Some(rgb) = rgb {
            self.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
        }
        if let Some(white) = white {
            self.renderer.white_fixtures[arc_idx][light_idx].set_white(white.0, white.1, white.2);
        }
        self.commit_and_render();
    }

    /// Batch update a set of rgb and white fixture pairs.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_batch_fixtures(
        &mut self,
        fixtures: impl IntoIterator<
            Item = (
                usize,
                usize,
                Option<(u16, u16, u16)>,
                Option<(u16, u16, u16)>,
            ),
        >,
    ) {
        self.transition_to(StageMode::Manual);
        for (arc_idx, light_idx, rgb, white) in fixtures {
            if let Some(rgb) = rgb {
                self.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
            }
            if let Some(white) = white {
                self.renderer.white_fixtures[arc_idx][light_idx]
                    .set_white(white.0, white.1, white.2);
            }
        }
        self.commit_and_render();
    }

    /// Update rgb and white for an arc.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_arc(
        &mut self,
        arc_idx: usize,
        rgb: Option<(u16, u16, u16)>,
        white: Option<(u16, u16, u16)>,
    ) {
        self.transition_to(StageMode::Manual);
        if let Some(rgb) = rgb {
            for light in &mut self.renderer.rgb_fixtures[arc_idx] {
                light.set_color(rgb.0, rgb.1, rgb.2);
            }
        }
        if let Some(white) = white {
            for light in &mut self.renderer.white_fixtures[arc_idx] {
                light.set_white(white.0, white.1, white.2);
            }
        }
        self.commit_and_render();
    }

    /// Update rgb and white for entire stage.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_stage(
        &mut self,
        rgb: Option<(u16, u16, u16)>,
        white: Option<(u16, u16, u16)>,
    ) {
        self.transition_to(StageMode::Manual);
        if let Some(rgb) = rgb {
            for arc in &mut self.renderer.rgb_fixtures {
                for light in arc {
                    light.set_color(rgb.0, rgb.1, rgb.2);
                }
            }
        }
        if let Some(white) = white {
            for arc in &mut self.renderer.white_fixtures {
                for light in arc {
                    light.set_white(white.0, white.1, white.2);
                }
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
