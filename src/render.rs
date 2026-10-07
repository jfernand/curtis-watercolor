//! Turning glazes into colours.

use crate::pigment::{Layer, Pigment};

/// The colour of one point: `glazes` (pigment and thickness, bottom first) over paper of linear
/// reflectance `paper`.
pub fn composite(paper: [f32; 3], glazes: &[(&Pigment, f32)]) -> [f32; 3] {
    glazes.iter().fold(Layer::opaque(paper), |below, (pigment, x)| pigment.layer(*x).over(below)).r
}

/// Linear reflectance to an 8-bit sRGB channel.
pub fn to_srgb8(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let v = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * libm::powf(c, 1.0 / 2.4) - 0.055 };
    (v * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pigment::by_name;

    #[test]
    fn no_glaze_shows_the_paper() {
        assert_eq!(composite([0.9, 0.8, 0.7], &[]), [0.9, 0.8, 0.7]);
        let rose = by_name("Quinacridone Rose").unwrap();
        let c = composite([1.0; 3], &[(&rose, 1.0)]);
        assert!(c[0] > c[1], "rose is red more than green: {c:?}");
        assert_eq!(to_srgb8(1.0), 255);
        assert_eq!(to_srgb8(0.0), 0);
    }
}
