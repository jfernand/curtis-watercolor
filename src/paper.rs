//! Procedural watercolor paper: a height field (Curtis et al. §4.1) of soft noise and fibres.
//!
//! The paper is generated over one period of a [`Lattice`] and wraps there, so it repeats
//! seamlessly across the plane. The height steers the water (it runs downhill), sorts granulating
//! pigments into the valleys, and roughens the rim of a brushstroke.

use crate::lattice::{Lattice, Point};
use crate::rng::Rng;

pub struct Paper {
    /// Texels per period, in each lattice direction.
    pub n: usize,
    /// Heights in `[0, 1]`, row-major in lattice coordinates.
    pub h: Vec<f32>,
    pub lattice: Lattice,
}

impl Paper {
    /// Generates `n` × `n` texels of paper per period of `lattice`.
    pub fn generate(n: usize, seed: u64, lattice: Lattice) -> Self {
        let mut rng = Rng::new(seed);
        let mut h = vec![0.0f32; n * n];
        // Soft undulation: periodic value noise, octaves whose periods divide the lattice period.
        for (period, amp) in [(3usize, 0.35f32), (6, 0.3), (12, 0.25), (24, 0.2), (48, 0.14), (96, 0.08)] {
            let knots: Vec<f32> = (0..period * period).map(|_| unit(&mut rng)).collect();
            for (y, row) in h.chunks_mut(n).enumerate() {
                for (x, v) in row.iter_mut().enumerate() {
                    *v += amp * value_noise(&knots, period, x as f32 / n as f32, y as f32 / n as f32);
                }
            }
        }
        // Fibres: short raised strands in random directions, wrapped at the period.
        for _ in 0..n {
            let (x0, y0) = (unit(&mut rng) * n as f32, unit(&mut rng) * n as f32);
            let angle = unit(&mut rng) * std::f32::consts::TAU;
            let len = (0.03 + 0.08 * unit(&mut rng)) * n as f32;
            let lift = 0.02 + 0.04 * unit(&mut rng);
            let steps = (len * 2.0) as usize;
            for s in 0..steps {
                let t = s as f32 / 2.0;
                let (x, y) = (x0 + t * angle.cos(), y0 + t * angle.sin());
                for (dx, dy, w) in [(0, 0, 1.0), (1, 0, 0.35), (-1, 0, 0.35), (0, 1, 0.35), (0, -1, 0.35)] {
                    let xi = (x as isize + dx).rem_euclid(n as isize) as usize;
                    let yi = (y as isize + dy).rem_euclid(n as isize) as usize;
                    h[yi * n + xi] += lift * w / 2.0;
                }
            }
        }
        let (lo, hi) = h.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        h.iter_mut().for_each(|v| *v = (*v - lo) / (hi - lo).max(1e-6));
        Self { n, h, lattice }
    }

    /// Paper from your own height field (`n` × `n`, values in `[0, 1]`, wrapping at the edges).
    pub fn from_heights(n: usize, h: Vec<f32>, lattice: Lattice) -> Self {
        assert_eq!(h.len(), n * n);
        Self { n, h, lattice }
    }

    /// Height at lattice coordinates, bilinear, wrapping.
    pub fn at_lattice(&self, l: Point) -> f32 {
        let n = self.n as f32;
        let (x, y) = (l[0].rem_euclid(1.0) * n - 0.5, l[1].rem_euclid(1.0) * n - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |xi: f32, yi: f32| {
            let xi = (xi as isize).rem_euclid(self.n as isize) as usize;
            let yi = (yi as isize).rem_euclid(self.n as isize) as usize;
            self.h[yi * self.n + xi]
        };
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// Height at a point in the plane.
    pub fn at(&self, p: Point) -> f32 {
        self.at_lattice(self.lattice.coords(p))
    }
}

fn unit(rng: &mut Rng) -> f32 {
    rng.unit()
}

/// Smooth value noise on a `period` × `period` torus of random knots, at `(x, y)` in `[0, 1)²`.
fn value_noise(knots: &[f32], period: usize, x: f32, y: f32) -> f32 {
    let (x, y) = (x * period as f32, y * period as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (fx, fy) = (smooth(x.fract()), smooth(y.fract()));
    let k = |i: usize, j: usize| knots[(j % period) * period + (i % period)];
    let top = k(x0, y0) * (1.0 - fx) + k(x0 + 1, y0) * fx;
    let bottom = k(x0, y0 + 1) * (1.0 - fx) + k(x0 + 1, y0 + 1) * fx;
    top * (1.0 - fy) + bottom * fy
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_repeats_with_its_lattice() {
        let s = 3f32.sqrt();
        let paper = Paper::generate(64, 3, Lattice::hex_flat_top(1.0));
        for p in [[0.1, 0.2], [-0.7, 0.4], [0.33, -0.81]] {
            let h = paper.at(p);
            for step in [[1.5, -s / 2.0], [0.0, -s], [-1.5, -s / 2.0]] {
                let q = [p[0] + step[0], p[1] + step[1]];
                assert!((paper.at(q) - h).abs() < 1e-4, "{p:?} vs {q:?}");
            }
        }
        assert!(paper.h.iter().all(|v| (0.0..=1.0).contains(v)));
        let square = Paper::generate(32, 3, Lattice::square(2.0));
        assert!((square.at([0.3, 0.4]) - square.at([2.3, -1.6])).abs() < 1e-4);
    }
}
