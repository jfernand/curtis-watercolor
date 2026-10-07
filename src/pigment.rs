//! Pigments and Kubelka–Munk glazing (Curtis et al. §5).
//!
//! Each pigment has absorption `K` and scattering `S` coefficients per RGB channel. A layer of
//! thickness `x` reflects `R` and transmits `T` of the light; layers are composited with
//! Kubelka's equations, so a glaze over another darkens and shifts hue the way paint does instead
//! of blending like transparency.

/// A watercolor pigment: Kubelka–Munk absorption `k` and scattering `s` per RGB channel, plus the
/// physical properties that drive the simulation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pigment {
    pub name: &'static str,
    pub k: [f32; 3],
    pub s: [f32; 3],
    /// ρ: heavier pigments settle sooner.
    pub density: f32,
    /// ω: higher staining power resists lifting back into the water.
    pub staining: f32,
    /// γ: how strongly the paper's peaks and valleys sort the pigment.
    pub granulation: f32,
}

/// The palette of Figure 5 in the paper.
pub const PALETTE: [Pigment; 12] = [
    pigment("Quinacridone Rose", [0.22, 1.47, 0.57], [0.05, 0.003, 0.03], 0.02, 5.5, 0.81),
    pigment("Indian Red", [0.46, 1.07, 1.50], [1.28, 0.38, 0.21], 0.05, 7.0, 0.40),
    pigment("Cadmium Yellow", [0.10, 0.36, 3.45], [0.97, 0.65, 0.007], 0.05, 3.4, 0.81),
    pigment("Hookers Green", [1.62, 0.61, 1.64], [0.01, 0.012, 0.003], 0.09, 1.0, 0.41),
    pigment("Cerulean Blue", [1.52, 0.32, 0.25], [0.06, 0.26, 0.40], 0.01, 1.0, 0.31),
    pigment("Burnt Umber", [0.74, 1.54, 2.10], [0.09, 0.09, 0.004], 0.09, 9.3, 0.90),
    pigment("Cadmium Red", [0.14, 1.08, 1.68], [0.77, 0.015, 0.018], 0.02, 1.0, 0.63),
    pigment("Brilliant Orange", [0.13, 0.81, 3.45], [0.005, 0.009, 0.007], 0.01, 1.0, 0.14),
    pigment("Hansa Yellow", [0.06, 0.21, 1.78], [0.50, 0.88, 0.009], 0.06, 1.0, 0.08),
    pigment("Phthalo Green", [1.55, 0.47, 0.63], [0.01, 0.05, 0.035], 0.02, 1.0, 0.12),
    pigment("French Ultramarine", [0.86, 0.86, 0.06], [0.005, 0.005, 0.09], 0.01, 3.1, 0.91),
    pigment("Interference Lilac", [0.08, 0.11, 0.07], [1.25, 0.42, 1.43], 0.06, 1.0, 0.08),
];

const fn pigment(
    name: &'static str,
    k: [f32; 3],
    s: [f32; 3],
    density: f32,
    staining: f32,
    granulation: f32,
) -> Pigment {
    Pigment { name, k, s, density, staining, granulation }
}

/// A pigment from the paper's palette, by name (case-insensitive).
pub fn by_name(name: &str) -> Option<Pigment> {
    PALETTE.iter().copied().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// Reflectance and transmittance of a layer, per channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub r: [f32; 3],
    pub t: [f32; 3],
}

impl Layer {
    /// An opaque background of reflectance `r` (paper).
    pub fn opaque(r: [f32; 3]) -> Self {
        Self { r, t: [0.0; 3] }
    }

    /// Kubelka's compositing: `self` laid over `below`.
    pub fn over(self, below: Layer) -> Layer {
        let mut out = Layer { r: [0.0; 3], t: [0.0; 3] };
        for c in 0..3 {
            let denom = 1.0 - self.r[c] * below.r[c];
            out.r[c] = self.r[c] + self.t[c] * self.t[c] * below.r[c] / denom;
            out.t[c] = self.t[c] * below.t[c] / denom;
        }
        out
    }
}

impl Pigment {
    /// A pigment whose unit-thickness layer looks `over_white` on white and `over_black` on black
    /// (linear RGB reflectances, `0 < over_black < over_white < 1` per channel), by inverting the
    /// KM equations as in §5.1. Opaque paints look alike on both; transparent ones are dark on
    /// black; interference paints are pale on white and coloured on black.
    pub fn from_colours(
        name: &'static str,
        over_white: [f32; 3],
        over_black: [f32; 3],
        density: f32,
        staining: f32,
        granulation: f32,
    ) -> Pigment {
        let mut k = [0.0; 3];
        let mut s = [0.0; 3];
        for c in 0..3 {
            let (rw, rb) = (over_white[c], over_black[c]);
            assert!(0.0 < rb && rb < rw && rw < 1.0, "need 0 < over_black < over_white < 1 (channel {c})");
            let a = 0.5 * (rw + (rb - rw + 1.0) / rb);
            let b = (a * a - 1.0).sqrt();
            let x = (b * b - (a - rw) * (a - 1.0)) / (b * (1.0 - rw));
            s[c] = 0.5 * libm::logf((x + 1.0) / (x - 1.0)) / b;
            k[c] = s[c] * (a - 1.0);
        }
        Pigment { name, k, s, density, staining, granulation }
    }

    /// The layer formed by thickness `x` of this pigment (the KM equations of §5.2).
    pub fn layer(&self, x: f32) -> Layer {
        let mut out = Layer { r: [0.0; 3], t: [1.0; 3] };
        for c in 0..3 {
            let (k, s) = (self.k[c], self.s[c].max(1e-4));
            let a = 1.0 + k / s;
            let b = (a * a - 1.0).sqrt();
            let bsx = (b * s * x).min(20.0);
            let (sh, ch) = (libm::sinhf(bsx), libm::coshf(bsx));
            let cc = a * sh + b * ch;
            out.r[c] = sh / cc;
            out.t[c] = b / cc;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pigment_is_clear() {
        let l = by_name("french ultramarine").unwrap().layer(0.0);
        assert_eq!(l.r, [0.0; 3]);
        assert!(l.t.iter().all(|t| (t - 1.0).abs() < 1e-6));
        let paper = Layer::opaque([0.9, 0.88, 0.8]);
        let over = l.over(paper);
        assert!((0..3).all(|c| (over.r[c] - paper.r[c]).abs() < 1e-6));
    }

    #[test]
    fn pigments_can_be_chosen_by_their_look() {
        let (white, black) = ([0.8, 0.5, 0.3], [0.3, 0.1, 0.05]);
        let p = Pigment::from_colours("Test", white, black, 0.05, 2.0, 0.5);
        let layer = p.layer(1.0);
        let on_white = layer.over(Layer::opaque([1.0; 3]));
        let on_black = layer.over(Layer::opaque([0.0; 3]));
        for c in 0..3 {
            assert!((on_white.r[c] - white[c]).abs() < 1e-3, "{on_white:?}");
            assert!((on_black.r[c] - black[c]).abs() < 1e-3, "{on_black:?}");
        }
    }

    #[test]
    fn glazes_darken_and_colour_the_paper() {
        let white = Layer::opaque([1.0; 3]);
        let blue = by_name("French Ultramarine").unwrap();
        let thin = blue.layer(0.3).over(white);
        let thick = blue.layer(1.0).over(white);
        assert!(thick.r[0] < thin.r[0], "more pigment absorbs more");
        assert!(thick.r[2] > thick.r[0], "ultramarine stays blue: {:?}", thick.r);
        // One layer over white matches R + T²/(1 − R).
        let l = blue.layer(0.5);
        let expect = l.r[1] + l.t[1] * l.t[1] / (1.0 - l.r[1]);
        assert!((l.over(white).r[1] - expect).abs() < 1e-6);
    }
}
