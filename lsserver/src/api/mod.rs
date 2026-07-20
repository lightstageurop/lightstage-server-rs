//! # Light Stage API(s)
//!
//! Curently we provide two ways to interact with the server:
//! [WebSocket][crate::api::ws] and [REST][crate::api::rest]

mod rest;
mod ws;

pub use rest::start_server;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    config::ServerConfig,
    state::{CaptureConfig, SharedState, StageMode},
};

/// Generic colour of a 3-channel fixture.
///
/// Eg. [`crate::fixtures::RgbFixture`] or [`crate::fixtures::WhiteFixture`]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct FixtureColour(u16, u16, u16);

impl From<FixtureColour> for (u16, u16, u16) {
    fn from(c: FixtureColour) -> Self {
        (c.0, c.1, c.2)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateColourRequest {
    rgb: Option<FixtureColour>,
    white: Option<FixtureColour>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateFixturesRequest {
    arc_idx: usize,
    light_idx: usize,
    colour: UpdateColourRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum ModeRequest {
    Demo,
    Manual,
    OLAT { config: CaptureConfig },
    Playback { config: CaptureConfig },
}

/// An application service layer if you will to handle updating state.
///
/// Decouples the hardware ([`StageState`]) from api transport protocols. eg. REST, websocket.
#[derive(Clone)]
pub struct ApiState {
    /// The underlying [`StageState`]
    state: SharedState,
    config: ServerConfig,
}

impl ApiState {
    /// Retrieve current operation mode of the light stage.
    pub fn get_mode(&self) -> StageMode {
        { self.state.read().unwrap() }.mode
    }

    /// Update the current operation mode of the light stage.
    pub fn set_mode(&self, mode: ModeRequest) -> anyhow::Result<()> {
        let mut lock = self.state.write().unwrap();
        lock.try_transition_to(mode)
    }

    /// Updates colour of a single specified fixture.
    ///
    /// Also sets the mode to manual.
    pub fn set_fixture(
        &self,
        arc_idx: usize,
        light_idx: usize,
        rgb: Option<FixtureColour>,
        white: Option<FixtureColour>,
    ) {
        self.state
            .write()
            .unwrap()
            .update_rgb_and_white_single_fixture(
                arc_idx,
                light_idx,
                rgb.map(Into::into),
                white.map(Into::into),
            );
    }

    /// Updates colour of an entire arc uniformly.
    ///
    /// Also sets the mode to manual.
    pub fn set_arc(
        &self,
        arc_idx: usize,
        rgb: Option<FixtureColour>,
        white: Option<FixtureColour>,
    ) {
        self.state.write().unwrap().update_rgb_and_white_arc(
            arc_idx,
            rgb.map(Into::into),
            white.map(Into::into),
        );
    }

    /// Updates entire light stage to one uniform colour.
    ///
    /// Also sets the mode to manual.
    pub fn set_lightstage(&self, rgb: Option<FixtureColour>, white: Option<FixtureColour>) {
        self.state
            .write()
            .unwrap()
            .update_rgb_and_white_stage(rgb.map(Into::into), white.map(Into::into));
    }

    /// Batch updates some fixtures.
    ///
    /// Also sets the mode to manual.
    pub fn set_fixtures(
        &self,
        fixtures: Vec<(usize, usize, Option<FixtureColour>, Option<FixtureColour>)>,
    ) {
        let mapped = fixtures
            .into_iter()
            .map(|(a, l, r, w)| (a, l, r.map(Into::into), w.map(Into::into)));

        self.state
            .write()
            .unwrap()
            .update_rgb_and_white_batch_fixtures(mapped);
    }

    /// Trigger a capture for manual mode.
    ///
    /// Will error if not in manual mode, or trigger already pending.
    pub fn trigger_manual(&self) -> anyhow::Result<()> {
        let mut lock = self.state.write().unwrap();

        if lock.mode != StageMode::Manual {
            anyhow::bail!("Manual trigger only available in manual mode");
        }
        if lock.manual_capture_requested {
            anyhow::bail!("Manual trigger already pending");
        }

        lock.manual_capture_requested = true;
        Ok(())
    }
}
