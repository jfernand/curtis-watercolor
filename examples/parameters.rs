//! Paints the same wash while varying one parameter at a time, and writes one strip per parameter
//! to `out/parameters/<name>.png`. Each strip includes the default value.
//!
//!     cargo run --release --example parameters

use std::fs::{self, File};
use std::io::BufWriter;

use curtis_watercolor::{Glaze, Lattice, Paper, Params, Pigment, by_name, composite, to_srgb8};

const SIDE: usize = 128;
const GAP: usize = 8;
const PAPER: [f32; 3] = [0.96, 0.94, 0.89];

/// One wash: the simulation's and the pigment's parameters, how much pigment the brush holds and
/// how long the wash stays wet.
#[derive(Clone, Copy)]
struct Wash {
    params: Params,
    pigment: Pigment,
    load: f32,
    steps: usize,
}

type Vary = fn(&mut Wash, f32);

fn main() -> std::io::Result<()> {
    let base =
        Wash { params: Params::default(), pigment: by_name("French Ultramarine").unwrap(), load: 0.5, steps: 200 };
    let d = Params::default();
    let p = base.pigment;
    let strips: [(&str, [f32; 5], Vary); 10] = [
        ("edge_darkening", [0.0, 0.015, d.edge_darkening, 0.06, 0.12], |w, x| w.params.edge_darkening = x),
        ("viscosity", [0.0, 0.03, d.viscosity, 0.3, 0.6], |w, x| w.params.viscosity = x),
        ("drag", [0.0, 0.003, d.drag, 0.05, 0.2], |w, x| w.params.drag = x),
        ("slope", [0.0, 0.3, d.slope, 3.0, 8.0], |w, x| w.params.slope = x),
        ("blur", [2.0, 5.0, d.blur as f32, 18.0, 30.0], |w, x| w.params.blur = x as usize),
        ("density", [0.003, 0.006, p.density, 0.03, 0.1], |w, x| w.pigment.density = x),
        ("staining", [0.3, 1.0, p.staining, 6.0, 12.0], |w, x| w.pigment.staining = x),
        ("granulation", [0.0, 0.3, 0.6, p.granulation, 1.0], |w, x| w.pigment.granulation = x),
        ("load", [0.1, 0.25, base.load, 0.75, 1.0], |w, x| w.load = x),
        ("steps", [20.0, 60.0, base.steps as f32, 400.0, 800.0], |w, x| w.steps = x as usize),
    ];

    let paper = Paper::generate(128, 7, Lattice::square(SIDE as f32));
    let height: Vec<f32> = (0..SIDE * SIDE).map(|c| paper.at([(c % SIDE) as f32, (c / SIDE) as f32])).collect();
    fs::create_dir_all("out/parameters")?;
    for (name, values, vary) in strips {
        let swatches: Vec<Vec<u8>> = values
            .iter()
            .map(|&x| {
                let mut wash = base;
                vary(&mut wash, x);
                swatch(&height, wash)
            })
            .collect();
        let path = format!("out/parameters/{name}.png");
        write_strip(&path, &height, &swatches)?;
        println!("{path}: {}", values.map(|v| format!("{v}")).join(" · "));
    }
    Ok(())
}

/// A disc of wet paint, its rim roughened by the paper, left to dry.
fn swatch(height: &[f32], wash: Wash) -> Vec<u8> {
    let n = SIDE;
    let mask = (0..n * n)
        .map(|c| {
            let (x, y) = ((c % n) as f32, (c / n) as f32);
            (x - 64.0).hypot(y - 64.0) < 50.0 + (height[c] - 0.5) * 2.5
        })
        .collect();
    let mut glaze = Glaze::new(n, wash.params, wash.pigment, mask, height.to_vec());
    glaze.load_pigment(wash.load);
    for _ in 0..wash.steps {
        glaze.step();
    }
    let x = glaze.thickness();
    (0..n * n)
        .flat_map(|c| {
            let paper_rgb = PAPER.map(|v| v * (0.93 + 0.07 * height[c]));
            let [r, g, b] = composite(paper_rgb, &[(&wash.pigment, x[c])]).map(to_srgb8);
            [r, g, b, 255]
        })
        .collect()
}

fn write_strip(path: &str, height: &[f32], swatches: &[Vec<u8>]) -> std::io::Result<()> {
    let (w, h) = (swatches.len() * SIDE + (swatches.len() + 1) * GAP, SIDE + 2 * GAP);
    // The paper's average tone, so the gaps match the paper around each swatch.
    let mean = height.iter().sum::<f32>() / height.len() as f32;
    let background = PAPER.map(|v| to_srgb8(v * (0.93 + 0.07 * mean)));
    let mut image: Vec<u8> = (0..w * h).flat_map(|_| [background[0], background[1], background[2], 255]).collect();
    for (k, s) in swatches.iter().enumerate() {
        let x0 = GAP + k * (SIDE + GAP);
        for j in 0..SIDE {
            let at = ((GAP + j) * w + x0) * 4;
            image[at..at + SIDE * 4].copy_from_slice(&s[j * SIDE * 4..(j + 1) * SIDE * 4]);
        }
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.write_header().map_err(std::io::Error::other)?.write_image_data(&image).map_err(std::io::Error::other)
}
