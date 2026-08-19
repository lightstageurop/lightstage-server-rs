//! # Light Stage State Machine
//!
//! Defines the central state management ([`StageState`]) for the light stage,
//! including mode transitions, events, animator frame stepping.
//!
//! [`StageState`] serves as the central source of truth for the server.
//! This connects incoming api requests (via [`ApiState`][crate::api::ApiState])
//! to the `KiNET` networking loop, [`NetworkManager`][crate::network::NetworkManager].

use std::{
    mem,
    sync::{Arc, RwLock},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use utoipa::ToSchema;

use crate::{
    animator::{Animator, DemoAnimator, OlatAnimator, PlaybackAnimator},
    api::PlaybackSequence,
    config::ServerConfig,
    renderer::{LightStageFrame, Renderer},
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

/// Requested transition sent from the API layer
#[derive(Debug, Clone)]
pub enum ModeTransition {
    /// Transition to [`StageMode::Demo`]
    Demo,
    /// Transition to [`StageMode::Manual`]
    Manual,
    /// Transition to [`StageMode::OLAT`]
    Olat(CaptureConfig),
    /// Transition to [`StageMode::Playback`]
    Playback(PlaybackSequence),
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

/// Metadata about an active capturing session (eg. for [`StageMode::OLAT`] or [`StageMode::Playback`]).
#[derive(Debug, Clone)]
pub struct CaptureSession {
    /// Index of the frame currently being processed.
    pub current_frame_idx: usize,
    /// Total frames in the capture session
    pub total_frames: usize,
    /// Capture timing configuration
    pub config: CaptureConfig,
    /// Timestamp when the capture was started
    pub started_at: Instant,
}

impl CaptureSession {
    /// Returns a new [`CaptureSession`], starting now.
    pub fn new(total_frames: usize, config: CaptureConfig) -> Self {
        Self {
            current_frame_idx: 0,
            total_frames,
            config,
            started_at: Instant::now(),
        }
    }
}

/// Result of [`StageState::advance_tick`]. Defines what [`crate::network::NetworkManager`] should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    /// Animator is still running, continue.
    Continue,
    /// Animator is running and requested cameras to fire
    TriggerCapture,
    /// Animator has ended
    Finished,
}

/// Internal execution states for [`StageState`].
#[derive(Debug)]
enum RuntimeMode {
    /// Animator for [`StageMode::Demo`]
    Demo { animator: DemoAnimator },
    /// State for [`StageMode::Manual`]
    Manual {
        /// Trigger queued for [`StageMode::Manual`]?
        capture_requested: bool,
    },
    /// Animator and capture session for [`StageMode::Playback`]
    Playback {
        animator: PlaybackAnimator,
        /// Currently active capture session
        session: CaptureSession,
    },
    /// Animator and capture session for [`StageMode::OLAT`]
    Olat {
        animator: OlatAnimator,
        /// Currently active capture session
        session: CaptureSession,
    },
}

impl RuntimeMode {
    /// Map internal runtime state to public-facing [`StageMode`]
    fn stage_mode(&self) -> StageMode {
        match self {
            Self::Demo { .. } => StageMode::Demo,
            Self::Manual { .. } => StageMode::Manual,
            Self::Playback { .. } => StageMode::Playback,
            Self::Olat { .. } => StageMode::OLAT,
        }
    }

    /// Returns target `capture_hz` if a [`CaptureSession`] is currently running.
    fn capture_hz(&self) -> Option<f64> {
        match self {
            Self::Demo { .. } | Self::Manual { .. } => None,
            Self::Playback { session, .. } | Self::Olat { session, .. } => {
                Some(session.config.capture_hz)
            }
        }
    }
}

/// Shared lightstage state. Single source of truth for rendering, API, network, etc.
///
/// Higher level [`crate::api::ApiState`] will call into this to mutate hardware states.
/// Additionally, [`crate::network::NetworkManager`] will also refer to this for frame data, timings, etc.
#[derive(Debug)]
pub struct StageState {
    /// Internal state
    runtime: RuntimeMode,
    /// Channel to send async events to.
    ///
    /// Can be subscribed to with [`Self::subscribe`].
    tx: broadcast::Sender<StageEvent>,
    /// Internal rendering engine to write fixture states into [`Self::current_frame`].
    renderer: Renderer,
    /// Current rendered frame (a number of DMX universes).
    current_frame: LightStageFrame,
    /// A copy of the server's config, used for some validation.
    config: ServerConfig,
}

impl StageState {
    /// Returns a new [`StageState`], running default demo pattern ([`StageMode::Demo`]).
    pub fn new(
        renderer: Renderer,
        config: ServerConfig,
        tx: broadcast::Sender<StageEvent>,
    ) -> Self {
        Self {
            tx,
            renderer,
            current_frame: LightStageFrame::black(config.num_arcs),
            runtime: RuntimeMode::Demo {
                animator: DemoAnimator::new(0.2, &config),
            },
            config,
        }
    }

    /// Helper to broadcast async [`StageEvent`]s.
    fn emit_event(&self, event: StageEvent) {
        let _ = self.tx.send(event);
    }

    /// Returns current [`StageMode`].
    pub fn mode(&self) -> StageMode {
        self.runtime.stage_mode()
    }

    /// Subscribe to [`StageEvent`] broadcasts
    pub fn subscribe(&self) -> broadcast::Receiver<StageEvent> {
        self.tx.subscribe()
    }

    /// Returns target capture freq. of active session, if any.
    pub fn capture_hz(&self) -> Option<f64> {
        self.runtime.capture_hz()
    }

    /// Advances the active animator, returns an outcome. See [`TickResult`].
    pub fn advance_tick(&mut self, dest: &mut LightStageFrame) -> TickResult {
        // manual mode doesn't need to be rendered again here.
        // simply copy the current frame and check if capture was requested
        if let RuntimeMode::Manual { capture_requested } = &mut self.runtime {
            dest.clone_from(&self.current_frame);
            return if mem::take(capture_requested) {
                TickResult::TriggerCapture
            } else {
                TickResult::Continue
            };
        }

        let (still_active, is_capture) = match &mut self.runtime {
            RuntimeMode::Demo { animator } => {
                Self::tick_animator(animator, None, &mut self.renderer)
            }
            RuntimeMode::Playback { animator, session } => {
                Self::tick_animator(animator, Some(session), &mut self.renderer)
            }
            RuntimeMode::Olat { animator, session } => {
                Self::tick_animator(animator, Some(session), &mut self.renderer)
            }
            RuntimeMode::Manual { .. } => unreachable!(),
        };

        self.commit_and_render();
        dest.clone_from(&self.current_frame);

        if still_active {
            if is_capture {
                TickResult::TriggerCapture
            } else {
                TickResult::Continue
            }
        } else {
            // sequence ended. transition to idle (demo mode)
            if is_capture {
                self.emit_event(StageEvent::CaptureFinished);
            }
            self.transition_to_demo();

            TickResult::Finished
        }
    }

    /// Helper to tick an animator and return `(animator still active, has active capture session)`.
    fn tick_animator<A: Animator>(
        animator: &mut A,
        session: Option<&mut CaptureSession>,
        renderer: &mut Renderer,
    ) -> (bool, bool) {
        let still_active = animator.tick(renderer);
        let has_session = session.is_some();
        if let Some(session) = session {
            session.current_frame_idx += 1;
        }
        (still_active, has_session)
    }

    /// Helper to set new runtime mode and emit a [`StageEvent::ModeChanged`] event on mode change.
    fn set_runtime(&mut self, new_runtime: RuntimeMode) {
        let old_mode = self.runtime.stage_mode();
        let new_mode = new_runtime.stage_mode();

        self.runtime = new_runtime;

        if old_mode != new_mode {
            self.emit_event(StageEvent::ModeChanged(new_mode));
        }
    }

    /// Internal helper for infallible transition to [`StageMode::Manual`].
    fn transition_to_manual(&mut self) {
        self.set_runtime(RuntimeMode::Manual {
            capture_requested: false,
        });
    }

    /// Internal helper for infallible transition to [`StageMode::Demo`].
    fn transition_to_demo(&mut self) {
        self.set_runtime(RuntimeMode::Demo {
            animator: DemoAnimator::new(0.2, &self.config),
        });
    }

    /// Fallible state transition. Validates requested configuration before modifying runtime mode.
    pub fn try_transition_to(&mut self, transition: ModeTransition) -> anyhow::Result<()> {
        match transition {
            ModeTransition::Demo => {
                self.transition_to_demo();
            }
            ModeTransition::Manual => {
                self.transition_to_manual();
            }
            ModeTransition::Playback(sequence) => {
                let config = CaptureConfig {
                    capture_hz: sequence.capture_hz,
                };
                config.validate(&self.config)?;

                let animator = PlaybackAnimator::new(sequence);
                let session = CaptureSession::new(animator.total_frames().unwrap_or(0), config);

                self.set_runtime(RuntimeMode::Playback { animator, session });
            }
            ModeTransition::Olat(config) => {
                config.validate(&self.config)?;

                let animator = OlatAnimator::new(&self.config);
                let session = CaptureSession::new(animator.total_frames().unwrap_or(0), config);

                self.set_runtime(RuntimeMode::Olat { animator, session });
            }
        }

        Ok(())
    }

    /// Helper to transition to [`StageMode::Manual`], mutate renderer, and render the result.
    fn with_manual_renderer<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Renderer),
    {
        self.transition_to_manual();
        f(&mut self.renderer);
        self.commit_and_render();
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
        self.with_manual_renderer(|renderer| {
            if let Some(rgb) = rgb {
                renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
            }
            if let Some(white) = white {
                renderer.white_fixtures[arc_idx][light_idx].set_white(white.0, white.1, white.2);
            }
        });
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
        self.with_manual_renderer(|renderer| {
            for (arc_idx, light_idx, rgb, white) in fixtures {
                if let Some(rgb) = rgb {
                    renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
                }
                if let Some(white) = white {
                    renderer.white_fixtures[arc_idx][light_idx]
                        .set_white(white.0, white.1, white.2);
                }
            }
        });
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
        self.with_manual_renderer(|renderer| {
            if let Some(rgb) = rgb {
                for light in &mut renderer.rgb_fixtures[arc_idx] {
                    light.set_color(rgb.0, rgb.1, rgb.2);
                }
            }
            if let Some(white) = white {
                for light in &mut renderer.white_fixtures[arc_idx] {
                    light.set_white(white.0, white.1, white.2);
                }
            }
        });
    }

    /// Update rgb and white for entire stage.
    ///
    /// Sets mode to manual.
    pub fn update_rgb_and_white_stage(
        &mut self,
        rgb: Option<(u16, u16, u16)>,
        white: Option<(u16, u16, u16)>,
    ) {
        self.with_manual_renderer(|renderer| {
            if let Some(rgb) = rgb {
                for arc in &mut renderer.rgb_fixtures {
                    for light in arc {
                        light.set_color(rgb.0, rgb.1, rgb.2);
                    }
                }
            }
            if let Some(white) = white {
                for arc in &mut renderer.white_fixtures {
                    for light in arc {
                        light.set_white(white.0, white.1, white.2);
                    }
                }
            }
        });
    }

    /// Queues triggering a manual capture.
    ///
    /// Fails if not in [`StageMode::Manual`] or if trigger is already pending.
    pub fn request_manual_capture(&mut self) -> anyhow::Result<()> {
        if let RuntimeMode::Manual { capture_requested } = &mut self.runtime {
            if *capture_requested {
                anyhow::bail!("Manual trigger already pending");
            }
            *capture_requested = true;
            Ok(())
        } else {
            anyhow::bail!("Manual trigger only available in manual mode");
        }
    }

    /// Commits all pending fixture changes and calls [`crate::renderer::Renderer::update`].
    fn commit_and_render(&mut self) {
        self.renderer.update(&mut self.current_frame);
    }
}

pub type SharedState = Arc<RwLock<StageState>>;
