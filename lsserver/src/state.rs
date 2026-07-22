//! Internal light stage state(s)

use std::{
    mem,
    sync::{Arc, RwLock},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use utoipa::ToSchema;

use crate::{
    LightStageFrame,
    animator::{ActiveAnimator, Animator, DemoAnimator, OlatAnimator, PlaybackAnimator},
    api::ModeRequest,
    config::ServerConfig,
    renderer::Renderer,
};

/// Defines the active operation mode of the light stage.
#[allow(clippy::upper_case_acronyms)]
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
    /// One Light At a Time
    OLAT,
}

/// Asynchronous state change events which can be emitted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageEvent {
    /// Emitted when stage transitions to a new [`StageMode`]
    ModeChanged(StageMode),
    /// Emitted when an active capture session completes.
    CaptureFinished,
}

/// Configuration parameters for a capturing session.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct CaptureConfig {
    pub capture_hz: f64,
}

impl CaptureConfig {
    /// Validates capture frequency against global [`ServerConfig`].
    pub fn validate(self, config: &ServerConfig) -> anyhow::Result<()> {
        let max_hz = 1_000.0 / config.refresh_rate_ms as f64;
        if !self.capture_hz.is_finite() || self.capture_hz <= 0.0 {
            anyhow::bail!("Capture rate must be a positive finite number");
        }
        if self.capture_hz > max_hz {
            anyhow::bail!(
                "Requested capture rate ({:.1} Hz) exceeds maximum supported rate ({:.1} Hz)",
                self.capture_hz,
                max_hz
            );
        }
        Ok(())
    }
}

/// Metadata about a capturing session (eg. an OLAT sequence)
#[derive(Debug, Clone)]
pub struct CaptureSession {
    pub current_frame_idx: usize,
    pub total_frames: usize,
    pub config: CaptureConfig,
    pub started_at: Instant,
}

impl CaptureSession {
    pub fn new(total_frames: usize, config: CaptureConfig) -> Self {
        Self {
            current_frame_idx: 0,
            total_frames,
            config,
            started_at: Instant::now(),
        }
    }
}

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
    pub tx: broadcast::Sender<StageEvent>,
    renderer: Renderer,
    /// Current frame for [`StageMode::Manual`]
    pub current_frame: LightStageFrame,
    /// Trigger queued for [`StageMode::Manual`]?
    pub manual_capture_requested: bool,
    /// Currently active capture session
    pub active_session: Option<CaptureSession>,
    /// Currently active animator
    animator: ActiveAnimator,
    config: ServerConfig,
}

impl StageState {
    pub fn new(
        renderer: Renderer,
        config: ServerConfig,
        tx: broadcast::Sender<StageEvent>,
    ) -> Self {
        Self {
            mode: StageMode::default(),
            tx,
            renderer,
            current_frame: LightStageFrame::black(),
            manual_capture_requested: false,
            active_session: None,
            animator: ActiveAnimator::Demo(DemoAnimator::new(0.2, &config)),
            config,
        }
    }

    fn emit_event(&self, event: StageEvent) {
        let _ = self.tx.send(event);
    }

    pub fn advance_tick(&mut self, dest: &mut LightStageFrame) -> TickResult {
        if self.mode == StageMode::Manual {
            dest.clone_from(&self.current_frame);
            if mem::take(&mut self.manual_capture_requested) {
                TickResult::TriggerCapture
            } else {
                TickResult::Continue
            }
        } else {
            let still_active = self.animator.tick(&mut self.renderer);
            self.commit_and_render();

            if let Some(session) = &mut self.active_session {
                session.current_frame_idx += 1;
            }

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
                self.emit_event(StageEvent::CaptureFinished);
                self.transition_to_demo();
                dest.clone_from(&self.current_frame);
                TickResult::Finished
            }
        }
    }

    /// Internal helper for transition to [`StageMode::Manual`]. Can never fail.
    fn transition_to_manual(&mut self) {
        self.mode = StageMode::Manual;
        self.active_session = None;
        self.animator = ActiveAnimator::None;
        self.emit_event(StageEvent::ModeChanged(StageMode::Manual));
    }

    /// Internal helper for transition to [`StageMode::Demo`]. Can never fail.
    fn transition_to_demo(&mut self) {
        self.mode = StageMode::Demo;
        self.active_session = None;
        self.animator = ActiveAnimator::Demo(DemoAnimator::new(0.2, &self.config));
        self.emit_event(StageEvent::ModeChanged(StageMode::Demo));
    }

    /// Transition to a new state
    pub fn try_transition_to(&mut self, mode_req: ModeRequest) -> anyhow::Result<()> {
        let new_mode = match mode_req {
            ModeRequest::Demo => {
                self.active_session = None;
                self.animator = ActiveAnimator::Demo(DemoAnimator::new(0.2, &self.config));
                StageMode::Demo
            }
            ModeRequest::Manual => {
                self.active_session = None;
                self.animator = ActiveAnimator::None;
                StageMode::Manual
            }
            ModeRequest::Playback { config } => {
                config.validate(&self.config)?;
                let anim = PlaybackAnimator::new();
                self.active_session = Some(CaptureSession::new(
                    anim.total_frames().unwrap_or(0),
                    config,
                ));
                self.animator = ActiveAnimator::Playback(anim);
                StageMode::Playback
            }
            ModeRequest::OLAT { config } => {
                config.validate(&self.config)?;
                let anim = OlatAnimator::new(&self.config);
                self.active_session = Some(CaptureSession::new(
                    anim.total_frames().unwrap_or(0),
                    config,
                ));
                self.animator = ActiveAnimator::OLAT(anim);
                StageMode::OLAT
            }
        };

        if self.mode != new_mode {
            self.mode = new_mode;
            self.emit_event(StageEvent::ModeChanged(new_mode));
        }

        Ok(())
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
        self.transition_to_manual();
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
        self.transition_to_manual();
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
        self.transition_to_manual();
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
        self.transition_to_manual();
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
