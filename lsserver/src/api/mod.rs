mod rest;
mod ws;

pub use rest::start_server;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    config::ServerConfig,
    state::{SharedState, StageMode, StageState},
};

/// Generic colour of a 3-channel fixture.
///
/// Eg. [`crate::fixtures::RgbFixture`] or [`crate::fixtures::WhiteFixture`]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct FixtureColour(u16, u16, u16);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateColourRequest {
    rgb: FixtureColour,
    white: FixtureColour,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
struct UpdateFixturesRequest {
    arc_idx: usize,
    light_idx: usize,
    colour: UpdateColourRequest,
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
    pub fn set_mode(&self, mode: StageMode) {
        let mut lock = self.state.write().unwrap();
        lock.mode = mode;
    }

    /// Updates colour of a single specified fixture.
    ///
    /// Also sets the mode to manual.
    pub fn set_fixture(
        &self,
        arc_idx: usize,
        light_idx: usize,
        rgb: FixtureColour,
        white: FixtureColour,
    ) {
        {
            let mut lock = self.state.write().unwrap();
            lock.mode = StageMode::Manual;

            lock.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
            lock.renderer.white_fixtures[arc_idx][light_idx].set_white(white.0, white.1, white.2);
        }

        self.commit_and_render();
    }

    /// Updates colour of an entire arc uniformly.
    ///
    /// Also sets the mode to manual.
    pub fn set_arc(&self, arc_idx: usize, rgb: FixtureColour, white: FixtureColour) {
        {
            let mut lock = self.state.write().unwrap();
            lock.mode = StageMode::Manual;

            for light in &mut lock.renderer.rgb_fixtures[arc_idx] {
                light.set_color(rgb.0, rgb.1, rgb.2);
            }
            for light in &mut lock.renderer.white_fixtures[arc_idx] {
                light.set_white(white.0, white.1, white.2);
            }
        }

        self.commit_and_render();
    }

    /// Updates entire light stage to one uniform colour.
    ///
    /// Also sets the mode to manual.
    pub fn set_lightstage(&self, rgb: FixtureColour, white: FixtureColour) {
        {
            let mut lock = self.state.write().unwrap();
            lock.mode = StageMode::Manual;

            for arc in &mut lock.renderer.rgb_fixtures {
                for light in arc {
                    light.set_color(rgb.0, rgb.1, rgb.2);
                }
            }
            for arc in &mut lock.renderer.white_fixtures {
                for light in arc {
                    light.set_white(white.0, white.1, white.2);
                }
            }
        }

        self.commit_and_render();
    }

    pub fn set_fixtures(&self, fixtures: Vec<(usize, usize, FixtureColour, FixtureColour)>) {
        {
            let mut lock = self.state.write().unwrap();
            lock.mode = StageMode::Manual;

            for (arc_idx, light_idx, rgb, white) in fixtures {
                lock.renderer.rgb_fixtures[arc_idx][light_idx].set_color(rgb.0, rgb.1, rgb.2);
                lock.renderer.white_fixtures[arc_idx][light_idx]
                    .set_white(white.0, white.1, white.2);
            }
        }

        self.commit_and_render();
    }

    /// Commits all pending fixture changes and calls [`crate::renderer::Renderer::update`].
    fn commit_and_render(&self) {
        // TODO should this be a method on StageState instead of doing this here?
        let mut lock = self.state.write().unwrap();
        let StageState {
            renderer,
            current_frame,
            ..
        } = &mut *lock;
        renderer.update(current_frame);
    }
}
