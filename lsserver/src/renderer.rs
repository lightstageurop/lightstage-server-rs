use crate::{
    config::ServerConfig,
    fixtures::{Fixture, RgbFixture, WhiteFixture},
};

/// A collection of DMX universes for the entire light stage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightStageFrame {
    pub rgb_universes: Vec<[u8; 512]>,
    pub white_universes: Vec<[u8; 512]>,
}

impl LightStageFrame {
    /// Returns a new, fully off frame.
    #[must_use]
    pub fn black(num_arcs: usize) -> Self {
        Self {
            rgb_universes: vec![[0u8; 512]; num_arcs],
            white_universes: vec![[0u8; 512]; num_arcs],
        }
    }

    /// Clears the frame
    pub fn clear(&mut self) {
        for u in &mut self.rgb_universes {
            u.fill(0u8);
        }
        for u in &mut self.white_universes {
            u.fill(0u8);
        }
    }
}

/// Translates logical light fixture states into raw DMX universes. (`[u8; 512]`)
#[derive(Debug)]
pub struct Renderer {
    pub rgb_fixtures: Vec<Vec<RgbFixture<u16>>>,
    pub white_fixtures: Vec<Vec<WhiteFixture<u16>>>,
}

impl Renderer {
    pub fn new(config: &ServerConfig) -> Self {
        Self {
            rgb_fixtures: (0..config.num_arcs).map(|_| Vec::new()).collect(),
            white_fixtures: (0..config.num_arcs).map(|_| Vec::new()).collect(),
        }
    }

    /// Bake current logical state of all fixtures into provided target frame.
    pub fn update(&mut self, next_frame: &mut LightStageFrame) {
        for (idx, universe_fixtures) in self.rgb_fixtures.iter().enumerate() {
            for fixture in universe_fixtures {
                fixture.write_to_universe(&mut next_frame.rgb_universes[idx]);
            }
        }

        for (idx, universe_fixtures) in self.white_fixtures.iter().enumerate() {
            for fixture in universe_fixtures {
                fixture.write_to_universe(&mut next_frame.white_universes[idx]);
            }
        }
    }
}
