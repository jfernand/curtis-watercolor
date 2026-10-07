//! Physically based watercolor, after Curtis et al., *Computer-Generated Watercolor* (SIGGRAPH 1997).

pub mod lattice;
pub mod paper;
pub mod pigment;
pub mod render;
pub mod rng;

pub use lattice::{Lattice, Point};
pub use paper::Paper;
pub use pigment::{Layer, PALETTE, Pigment, by_name};
pub use render::{composite, to_srgb8};
