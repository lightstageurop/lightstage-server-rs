//! Animators
//!
//! Things that generate frames (fixture states) to be rendered.
//! Eg. [`OlatAnimator`] for lighting a single fixture each frame, or
//! [`DemoAnimator`] that generates a smooth rainbow effect.

mod demo;
mod olat;
mod playback;

pub use demo::DemoAnimator;
pub use olat::OlatAnimator;
pub use playback::PlaybackAnimator;

use crate::renderer::Renderer;

pub trait Animator {
    /// Updates the state of the renderer with the next frame to display.
    ///
    /// Returns `true` if the sequence is still active, or `false` if completed.
    fn tick(&mut self, renderer: &mut Renderer) -> bool;

    /// Returns the total frame count for a fixed-length sequence or none for an infinite loop.
    fn total_frames(&self) -> Option<usize>;
}

#[derive(Debug)]
pub enum ActiveAnimator {
    Demo(demo::DemoAnimator),
    Olat(olat::OlatAnimator),
    Playback(playback::PlaybackAnimator),
    None,
}

impl Animator for ActiveAnimator {
    fn tick(&mut self, renderer: &mut Renderer) -> bool {
        match self {
            Self::Demo(a) => a.tick(renderer),
            Self::Olat(a) => a.tick(renderer),
            Self::Playback(a) => a.tick(renderer),
            Self::None => false,
        }
    }

    fn total_frames(&self) -> Option<usize> {
        match self {
            ActiveAnimator::Demo(a) => a.total_frames(),
            ActiveAnimator::Olat(a) => a.total_frames(),
            ActiveAnimator::Playback(a) => a.total_frames(),
            ActiveAnimator::None => None,
        }
    }
}
