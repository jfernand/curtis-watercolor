//! Physically based watercolor: a Rust implementation of Cassidy J. Curtis, Sean E. Anderson,
//! Joshua E. Seims, Kurt W. Fleischer and David H. Salesin, *Computer-Generated Watercolor*
//! (SIGGRAPH 1997).
//!
//! A painting is a stack of **glazes**. Each glaze is one wash of one pigment, simulated on its own
//! ([`sim`]) over a sheet of procedural [`paper`]:
//!
//! - **Water** flows inside the wet area by the shallow-water equations on a staggered grid,
//!   damped, pushed downhill by the paper's slope and relaxed to be divergence-free.
//! - **Pigment** rides the water, settles onto the paper and lifts off again, at rates set by its
//!   density, staining power and granulation, so granulating pigments collect in the valleys.
//! - **Edge darkening**: water drains near the rim of the wet area, so it flows outward and
//!   leaves the dark edge real watercolor has.
//!
//! Glazes are then layered optically with the **Kubelka–Munk** model ([`pigment`], [`render`]):
//! a blue glaze over umber darkens and shifts hue, as paint does, instead of averaging.
//!
//! The crate has no engine dependency: it turns masks and pigment amounts into pigment
//! thicknesses and colours. Use it offline to bake textures, or in a tool to paint images. The
//! same compositing is available for shaders in `shaders/kubelka_munk.wgsl`.
//!
//! The paper's capillary layer (backruns, §4.6) is not implemented.
//!
//! ```
//! use curtis_watercolor::{Glaze, Lattice, Paper, Params, by_name, composite};
//!
//! // 64 × 64 cells; paper repeating every 64 cells.
//! let n = 64;
//! let paper = Paper::generate(64, 1, Lattice::square(n as f32));
//! let height: Vec<f32> = (0..n * n).map(|c| paper.at([(c % n) as f32, (c / n) as f32])).collect();
//! // A wet disc loaded with pigment.
//! let mask: Vec<bool> = (0..n * n)
//!     .map(|c| ((c % n) as f32 - 32.0).hypot((c / n) as f32 - 32.0) < 20.0)
//!     .collect();
//! let rose = by_name("Quinacridone Rose").unwrap();
//! let mut glaze = Glaze::new(n, Params::default(), rose, mask, height);
//! glaze.load_pigment(0.5);
//! for _ in 0..50 {
//!     glaze.step();
//! }
//! let thickness = glaze.thickness();
//! let centre = composite([0.96, 0.94, 0.89], &[(&rose, thickness[32 * n + 32])]);
//! assert!(centre[0] > centre[1]);
//! ```

pub mod lattice;
pub mod paper;
pub mod pigment;
pub mod render;
pub mod rng;
pub mod sim;

pub use lattice::{Lattice, Point};
pub use paper::Paper;
pub use pigment::{Layer, PALETTE, Pigment, by_name};
pub use render::{composite, to_srgb8};
pub use sim::{Glaze, Params};

/// The crate version, for caches keyed on the simulator's behaviour.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
