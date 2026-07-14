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
    fn tick(&mut self, renderer: &mut Renderer);
}
