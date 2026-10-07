//! Paints a swatch of every pigment in the paper's palette, plus one defined by its look over
//! white and black, and writes `out/swatches.png`.
//!
//!     cargo run --release --example swatches

use std::fs::{self, File};
use std::io::BufWriter;

use curtis_watercolor::{Glaze, Lattice, PALETTE, Paper, Params, Pigment, composite, to_srgb8};

const SIDE: usize = 128;
const PAPER: [f32; 3] = [0.96, 0.94, 0.89];

fn main() -> std::io::Result<()> {
    let paper = Paper::generate(128, 7, Lattice::square(SIDE as f32));
    let custom = Pigment::from_colours("Custom teal", [0.35, 0.75, 0.7], [0.05, 0.25, 0.25], 0.03, 2.0, 0.6);
    let pigments: Vec<Pigment> = PALETTE.iter().copied().chain([custom]).collect();
    let swatches: Vec<Vec<u8>> = pigments.iter().map(|p| swatch(&paper, p)).collect();

    let (cols, rows) = (7, pigments.len().div_ceil(7));
    let (w, h) = (cols * SIDE, rows * SIDE);
    let background = PAPER.map(to_srgb8);
    let mut image: Vec<u8> = (0..w * h).flat_map(|_| [background[0], background[1], background[2], 255]).collect();
    for (k, s) in swatches.iter().enumerate() {
        let (x0, y0) = ((k % cols) * SIDE, (k / cols) * SIDE);
        for j in 0..SIDE {
            let row = &s[j * SIDE * 4..(j + 1) * SIDE * 4];
            let at = ((y0 + j) * w + x0) * 4;
            image[at..at + SIDE * 4].copy_from_slice(row);
        }
    }
    fs::create_dir_all("out")?;
    let mut encoder = png::Encoder::new(BufWriter::new(File::create("out/swatches.png")?), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.write_header().map_err(std::io::Error::other)?.write_image_data(&image).map_err(std::io::Error::other)?;
    println!("wrote out/swatches.png ({} pigments)", pigments.len());
    Ok(())
}

/// A disc of wet paint left to dry: edge darkening, granulation and flow in one picture.
fn swatch(paper: &Paper, pigment: &Pigment) -> Vec<u8> {
    let n = SIDE;
    let point = |c: usize| [(c % n) as f32, (c / n) as f32];
    let height: Vec<f32> = (0..n * n).map(|c| paper.at(point(c))).collect();
    let mask = (0..n * n)
        .map(|c| {
            let [x, y] = point(c);
            (x - 64.0).hypot(y - 64.0) < 48.0 + (height[c] - 0.5) * 2.5
        })
        .collect();
    let mut glaze = Glaze::new(n, Params::default(), *pigment, mask, height.clone());
    glaze.load_pigment(0.5);
    for _ in 0..200 {
        glaze.step();
    }
    let x = glaze.thickness();
    (0..n * n)
        .flat_map(|c| {
            let paper_rgb = PAPER.map(|v| v * (0.93 + 0.07 * height[c]));
            let [r, g, b] = composite(paper_rgb, &[(pigment, x[c])]).map(to_srgb8);
            [r, g, b, 255]
        })
        .collect()
}
