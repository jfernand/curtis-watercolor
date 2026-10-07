// Kubelka–Munk glazing for shaders: the same optics as `curtis_watercolor::pigment`.
// Copy these functions into your WGSL (or translate them; they are plain arithmetic).
//
//   var layer = KmLayer(paper_rgb, vec3(0.0));          // opaque paper
//   layer = km_over(km_layer(k, s, thickness), layer);  // one glaze on top, repeat per glaze
//   let colour = layer.r;                               // linear RGB

struct KmLayer {
    // Reflectance and transmittance per channel.
    r: vec3<f32>,
    t: vec3<f32>,
}

// The layer formed by thickness `x` of a pigment with absorption `k` and scattering `s`.
fn km_layer(k: vec3<f32>, s_in: vec3<f32>, x: f32) -> KmLayer {
    let s = max(s_in, vec3(1e-4));
    let a = 1.0 + k / s;
    let b = sqrt(a * a - 1.0);
    let bsx = min(b * s * x, vec3(20.0));
    let sh = sinh(bsx);
    let ch = cosh(bsx);
    let c = a * sh + b * ch;
    return KmLayer(sh / c, b / c);
}

// Kubelka's compositing: `top` laid over `below`.
fn km_over(top: KmLayer, below: KmLayer) -> KmLayer {
    let d = 1.0 - top.r * below.r;
    return KmLayer(top.r + top.t * top.t * below.r / d, top.t * below.t / d);
}
