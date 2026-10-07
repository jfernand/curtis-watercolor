//! Physically based watercolor, after Curtis et al., *Computer-Generated Watercolor* (SIGGRAPH 1997).

pub mod lattice;
pub mod paper;
pub mod rng;

pub use lattice::{Lattice, Point};
pub use paper::Paper;
