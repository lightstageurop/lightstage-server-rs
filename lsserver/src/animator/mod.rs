//! Animators
//!
//! Things that generate frames (fixture states) to be rendered.
//! Eg. [`OlatAnimator`] for lighting a single fixture each frame, or
//! [`DemoAnimator`] that generates a smooth rainbow effect.

mod demo;
mod olat;

pub use demo::DemoAnimator;
pub use olat::OlatAnimator;
