//! # Frame Rendering
//!
//! This module provides translation logic to convert logical fixture states
//! into raw DMX512 universes for each PDS.

use crate::{
    config::ServerConfig,
    fixtures::{Fixture, RgbFixture, WhiteFixture},
};

/// A collection of DMX universes for the entire light stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightStageFrame {
    /// List of RGB DMX512 universe buffers.
    pub rgb_universes: Vec<[u8; 512]>,
    /// List of white DMX512 universe buffers.
    pub white_universes: Vec<[u8; 512]>,
}

impl LightStageFrame {
    /// Returns a new, fully off [`LightStageFrame`].
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

/// Translates logical light fixture states into raw DMX universes.
///
/// Maintains logical fixture objects organised by arc and light index,
/// and bakes their 3-channel 16-bit intensities into a DMX512 buffer (`[u8; 512]`).
#[derive(Debug)]
pub struct Renderer {
    /// Vector of RGB fixtures per universe.
    pub rgb_fixtures: Vec<Vec<RgbFixture<u16>>>,
    /// Vector of white fixtures per universe.
    pub white_fixtures: Vec<Vec<WhiteFixture<u16>>>,
}

impl Renderer {
    /// Constructs a new [`Renderer`] based on the provided config.
    ///
    /// Initialises empty fixture vectors for `num_arcs`.
    pub fn new(config: &ServerConfig) -> Self {
        Self {
            rgb_fixtures: (0..config.num_arcs).map(|_| Vec::new()).collect(),
            white_fixtures: (0..config.num_arcs).map(|_| Vec::new()).collect(),
        }
    }

    /// Bake current logical state of all fixtures into provided target frame.
    ///
    /// Iterates through all registered fixtures and serializes their state
    /// into the corresponding universe buffer of `next_frame`.
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
