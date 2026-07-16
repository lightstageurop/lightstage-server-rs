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

    fn start(&mut self);
}

#[derive(Debug)]
pub enum ActiveAnimator {
    Demo(demo::DemoAnimator),
    OLAT(olat::OlatAnimator),
    Playback(playback::PlaybackAnimator),
    None,
}

impl Animator for ActiveAnimator {
    fn tick(&mut self, renderer: &mut Renderer) -> bool {
        match self {
            Self::Demo(demo_animator) => demo_animator.tick(renderer),
            Self::OLAT(olat_animator) => olat_animator.tick(renderer),
            Self::Playback(playback_animator) => playback_animator.tick(renderer),
            Self::None => false,
        }
    }

    fn start(&mut self) {
        match self {
            Self::Demo(demo_animator) => demo_animator.start(),
            Self::OLAT(olat_animator) => olat_animator.start(),
            Self::Playback(playback_animator) => playback_animator.start(),
            Self::None => {}
        }
    }
}
