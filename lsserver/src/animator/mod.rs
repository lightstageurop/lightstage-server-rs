//! Animators
//!
//! Things that generate frames (fixture states) to be rendered.
//! Eg. [`OlatAnimator`] for lighting a single fixture each frame, or
//! [`DemoAnimator`] that generates a smooth rainbow effect.

mod demo;
mod olat;

pub use demo::DemoAnimator;
pub use olat::OlatAnimator;

use crate::renderer::Renderer;

pub trait Animator {
    /// Updates the state of the renderer with the next frame to display.
    fn tick(&mut self, renderer: &mut Renderer);
}
