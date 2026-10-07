//! How a paper texture repeats across the plane.
//!
//! Paper is generated on a torus: one *period*, wrapping in both directions. A [`Lattice`] maps
//! that period onto the plane through two basis vectors, so the same paper can tile a square
//! grid, a hexagonal board, or simply repeat far beyond the area being painted.
//!
//! When every cell of a tiled game board sees the same patch of paper, a glaze baked once per cell
//! agrees with the paper drawn under it at runtime, wherever the cell sits, and the paper is
//! seamless across cells.

pub type Point = [f32; 2];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lattice {
    /// The plane displacement of one period along the paper's first axis.
    pub a: Point,
    /// The plane displacement of one period along the paper's second axis.
    pub b: Point,
}

impl Lattice {
    pub fn new(a: Point, b: Point) -> Self {
        let det = a[0] * b[1] - a[1] * b[0];
        assert!(det.abs() > 1e-9, "lattice basis vectors must not be parallel");
        Self { a, b }
    }

    /// A square period of side `period`.
    pub fn square(period: f32) -> Self {
        Self::new([period, 0.0], [0.0, period])
    }

    /// The lattice of flat-top hexagon centres with circumradius `r`, y up: one period per
    /// hexagon, basis `(1.5r, −√3/2·r)` and `(0, −√3·r)` (the axial q and r steps).
    pub fn hex_flat_top(r: f32) -> Self {
        let s = 3f32.sqrt();
        Self::new([1.5 * r, -s / 2.0 * r], [0.0, -s * r])
    }

    /// Lattice coordinates of a plane point: integers at lattice points, fractional parts give
    /// the position within the period.
    pub fn coords(&self, p: Point) -> Point {
        let (a, b) = (self.a, self.b);
        let det = a[0] * b[1] - a[1] * b[0];
        [(p[0] * b[1] - p[1] * b[0]) / det, (a[0] * p[1] - a[1] * p[0]) / det]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_points_have_integer_coordinates() {
        let hex = Lattice::hex_flat_top(1.0);
        let s = 3f32.sqrt();
        for (p, want) in [([1.5, -s / 2.0], [1.0, 0.0]), ([0.0, -s], [0.0, 1.0]), ([0.0, 0.0], [0.0, 0.0])] {
            let c = hex.coords(p);
            assert!((c[0] - want[0]).abs() < 1e-5 && (c[1] - want[1]).abs() < 1e-5, "{c:?}");
        }
        assert_eq!(Lattice::square(2.0).coords([1.0, 3.0]), [0.5, 1.5]);
    }
}
