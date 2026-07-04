use crate::{
    LightStageFrame, NUM_ARCS,
    fixtures::{Fixture, RgbFixture, WhiteFixture},
};

pub struct Renderer {
    pub rgb_fixtures: Vec<Vec<RgbFixture<u16>>>,
    pub white_fixtures: Vec<Vec<WhiteFixture<u16>>>,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            rgb_fixtures: (0..NUM_ARCS).map(|_| Vec::new()).collect(),
            white_fixtures: (0..NUM_ARCS).map(|_| Vec::new()).collect(),
        }
    }

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
