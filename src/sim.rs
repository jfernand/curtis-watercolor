//! One glaze: the shallow-water and pigment-deposition layers of Curtis et al. §4, on a staggered
//! grid. Velocities live on cell faces; pressure and pigment at cell centres.
//!
//! Set up a [`Glaze`] with a wet-area mask, the paper height and a pigment, load pigment into the
//! water (and, optionally, pressure and velocity), then [`Glaze::step`] it a few hundred times.
//! [`Glaze::thickness`] is the dried result, ready for [`crate::render::composite`].
//!
//! Backruns (the capillary layer, §4.6) need a puddle spreading into a damp wash; tile strokes are
//! painted wet-on-dry, so that layer is left out.

use crate::pigment::Pigment;

/// The simulation's constants (Curtis et al. §4). [`Params::default`] is the paper's choice; the
/// `parameters` example paints a strip of swatches for each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// μ: viscosity. Higher values smooth the flow. The step is explicit, so keep it below about
    /// 0.25: much above that the water goes unstable and leaves ring-shaped artefacts.
    pub viscosity: f32,
    /// κ: viscous drag. Higher values stop the water sooner, leaving the pigment where it was
    /// loaded and pooled.
    pub drag: f32,
    /// η: strength of the outward flow that darkens edges. 0 gives no dark rim; the paper uses
    /// 0.01–0.05.
    pub edge_darkening: f32,
    /// K: Gaussian kernel size, in cells, for the distance to the mask edge: how wide the band
    /// that drains toward the rim is.
    pub blur: usize,
    /// How hard the paper's slope pushes the water from its peaks into its valleys.
    pub slope: f32,
    /// N: most relaxation passes per step when making the flow divergence-free.
    pub relax_iterations: usize,
    /// τ: divergence tolerance; relaxation stops early once every correction is below it.
    pub relax_tolerance: f32,
    /// ξ: relaxation rate.
    pub relax_rate: f32,
}

/// Fastest the water may move, in cells per step. The paper's adaptive step keeps the
/// integration stable for moderate speeds; this limit catches the rare runaway (a brush right
/// next to a drain) that would otherwise demand unbounded sub-steps.
const SPEED_LIMIT: f32 = 2.0;

impl Default for Params {
    /// The paper's values (μ = 0.1, κ = 0.01, N = 50, τ = 0.01, ξ = 0.1, K = 10, η in 0.01–0.05).
    fn default() -> Self {
        Self {
            viscosity: 0.1,
            drag: 0.01,
            edge_darkening: 0.03,
            blur: 10,
            slope: 1.0,
            relax_iterations: 50,
            relax_tolerance: 0.01,
            relax_rate: 0.1,
        }
    }
}

/// One wash of one pigment on an `n` × `n` grid: the water's velocity and pressure and the
/// pigment suspended in it and deposited on the paper.
///
/// The wet area, the paper and the parameters are fixed when the glaze is made (the edge
/// weights and the set of wet cells are derived from them). The state, [`u`](Glaze::u),
/// [`v`](Glaze::v), [`p`](Glaze::p), [`g`](Glaze::g) and [`d`](Glaze::d), is public: set it to
/// shape a stroke (an uneven load, a push of water) before or between steps. Keep it zero outside
/// the wet area.
#[derive(Clone, Debug)]
pub struct Glaze {
    n: usize,
    params: Params,
    pigment: Pigment,
    /// Wet-area mask M.
    mask: Vec<bool>,
    /// Paper height h in `[0, 1]`.
    height: Vec<f32>,
    /// x velocity on vertical faces: `u[j * (n + 1) + f]`, face `f` between cells `f − 1` and `f`.
    pub u: Vec<f32>,
    /// y velocity on horizontal faces: `v[f * n + i]`, face `f` between rows `f − 1` and `f`.
    pub v: Vec<f32>,
    /// Water pressure, per cell: raise it to push water outward from a cell.
    pub p: Vec<f32>,
    /// Pigment suspended in the water.
    pub g: Vec<f32>,
    /// Pigment deposited on the paper.
    pub d: Vec<f32>,
    /// (1 − M′)·M, M′ the blurred mask: large just inside the edge of the wet area.
    edge: Vec<f32>,
    /// The wet cells, and the faces between two wet cells: everything the simulation touches.
    /// Water never crosses the other faces, so their velocity stays zero (§4.3.1's boundary
    /// condition) without being enforced each step.
    wet: Vec<usize>,
    open_u: Vec<usize>,
    open_v: Vec<usize>,
    /// Per wet cell: whether its right, left, lower and upper faces are open.
    faces: Vec<[bool; 4]>,
    scratch_u: Vec<f32>,
    scratch_v: Vec<f32>,
    scratch_g: Vec<f32>,
}

impl Glaze {
    /// A still, empty wash over the cells where `mask` is true, on paper of the given `height`
    /// (both `n` × `n`, row-major). Load it with [`Glaze::load_pigment`] or by writing
    /// [`Glaze::g`].
    ///
    /// # Panics
    ///
    /// If `mask` or `height` does not hold `n * n` values.
    pub fn new(n: usize, params: Params, pigment: Pigment, mask: Vec<bool>, height: Vec<f32>) -> Self {
        assert_eq!(mask.len(), n * n, "mask must be n × n");
        assert_eq!(height.len(), n * n, "height must be n × n");
        let edge = edge_weight(n, &mask, params.blur);
        let wet_at = |i: isize, j: isize| {
            i >= 0 && j >= 0 && i < n as isize && j < n as isize && mask[j as usize * n + i as usize]
        };
        let wet: Vec<usize> = (0..n * n).filter(|&c| mask[c]).collect();
        // u face f of row j lies between cells f − 1 and f; v face f of column i between rows f − 1 and f.
        let open_u = (0..n)
            .flat_map(|j| (1..n).map(move |f| (f, j)))
            .filter(|&(f, j)| wet_at(f as isize - 1, j as isize) && wet_at(f as isize, j as isize))
            .map(|(f, j)| j * (n + 1) + f)
            .collect();
        let open_v = (1..n)
            .flat_map(|f| (0..n).map(move |i| (i, f)))
            .filter(|&(i, f)| wet_at(i as isize, f as isize - 1) && wet_at(i as isize, f as isize))
            .map(|(i, f)| f * n + i)
            .collect();
        let faces = wet
            .iter()
            .map(|&c| {
                let (i, j) = ((c % n) as isize, (c / n) as isize);
                [wet_at(i + 1, j), wet_at(i - 1, j), wet_at(i, j + 1), wet_at(i, j - 1)]
            })
            .collect();
        Self {
            n,
            params,
            pigment,
            u: vec![0.0; (n + 1) * n],
            v: vec![0.0; (n + 1) * n],
            p: vec![0.0; n * n],
            g: vec![0.0; n * n],
            d: vec![0.0; n * n],
            mask,
            height,
            edge,
            wet,
            open_u,
            open_v,
            faces,
            scratch_u: vec![0.0; (n + 1) * n],
            scratch_v: vec![0.0; (n + 1) * n],
            scratch_g: vec![0.0; n * n],
        }
    }

    /// Cells per side.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The parameters the glaze was made with.
    pub fn params(&self) -> &Params {
        &self.params
    }

    /// The pigment being washed.
    pub fn pigment(&self) -> &Pigment {
        &self.pigment
    }

    /// The wet-area mask M: true where the paper is wet.
    pub fn mask(&self) -> &[bool] {
        &self.mask
    }

    /// The paper height under each cell, in `[0, 1]`.
    pub fn height(&self) -> &[f32] {
        &self.height
    }

    /// The wet cells, in index order.
    pub fn wet_cells(&self) -> &[usize] {
        &self.wet
    }

    /// Sets the suspended pigment of every wet cell to `amount` (a freshly loaded brushstroke).
    /// For uneven loads, write [`Glaze::g`] directly.
    pub fn load_pigment(&mut self, amount: f32) {
        for &c in &self.wet {
            self.g[c] = amount;
        }
    }

    /// Total pigment, suspended and deposited.
    pub fn pigment_total(&self) -> f64 {
        self.g.iter().chain(&self.d).map(|&x| x as f64).sum()
    }

    /// The final pigment thickness once the water has dried: everything still suspended settles.
    pub fn thickness(&self) -> Vec<f32> {
        self.g.iter().zip(&self.d).map(|(g, d)| g + d).collect()
    }

    /// One time step of the main loop (§4.2).
    pub fn step(&mut self) {
        self.update_velocities();
        self.relax_divergence();
        self.flow_outward();
        self.move_pigment();
        self.transfer_pigment();
    }

    /// Adds a velocity field (laid out like [`Glaze::u`] and [`Glaze::v`]) to the faces between wet
    /// cells; the rest is ignored, since water cannot cross into dry paper.
    pub fn add_velocity(&mut self, du: &[f32], dv: &[f32]) {
        for &f in &self.open_u {
            self.u[f] += du[f];
        }
        for &f in &self.open_v {
            self.v[f] += dv[f];
        }
    }

    fn max_speed(&self) -> f32 {
        let u = self.open_u.iter().map(|&f| self.u[f].abs());
        let v = self.open_v.iter().map(|&f| self.v[f].abs());
        u.chain(v).fold(0.0f32, f32::max)
    }

    /// §4.3.1: the shallow-water equations, damped and pushed downhill by the paper's slope.
    fn update_velocities(&mut self) {
        let n = self.n;
        let (mu, kappa, slope) = (self.params.viscosity, self.params.drag, self.params.slope);
        // Paper slope: water runs off the peaks into the valleys.
        for &face in &self.open_u {
            let (j, f) = (face / (n + 1), face % (n + 1));
            self.u[face] -= slope * (self.height[j * n + f] - self.height[j * n + f - 1]);
        }
        for &face in &self.open_v {
            let (f, i) = (face / n, face % n);
            self.v[face] -= slope * (self.height[f * n + i] - self.height[(f - 1) * n + i]);
        }

        let steps = self.max_speed().ceil().max(1.0) as usize;
        let dt = 1.0 / steps as f32;
        for _ in 0..steps {
            let (u, v, p) = (&self.u, &self.v, &self.p);
            let ni = n as isize;
            let uf = |f: isize, j: isize| {
                if f < 0 || f > ni || j < 0 || j >= ni { 0.0 } else { u[(j * (ni + 1) + f) as usize] }
            };
            let vf = |i: isize, f: isize| {
                if i < 0 || i >= ni || f < 0 || f > ni { 0.0 } else { v[(f * ni + i) as usize] }
            };
            let pc = |i: isize, j: isize| p[(j * ni + i) as usize];
            // Cell-centred velocities, and u·v at the cell corner (i + .5, j + .5).
            let uc = |i: isize, j: isize| (uf(i, j) + uf(i + 1, j)) / 2.0;
            let vc = |i: isize, j: isize| (vf(i, j) + vf(i, j + 1)) / 2.0;
            let uv = |i: isize, j: isize| {
                let uu = (uf(i + 1, j) + uf(i + 1, j + 1)) / 2.0;
                let vv = (vf(i, j + 1) + vf(i + 1, j + 1)) / 2.0;
                uu * vv
            };
            for &face in &self.open_u {
                // Face (i + .5, j) is u[f = i + 1].
                let (j, i) = ((face / (n + 1)) as isize, (face % (n + 1)) as isize - 1);
                let here = u[face];
                let a = uc(i, j).powi(2) - uc(i + 1, j).powi(2) + uv(i, j - 1) - uv(i, j);
                let b = uf(i + 2, j) + uf(i, j) + uf(i + 1, j + 1) + uf(i + 1, j - 1) - 4.0 * here;
                self.scratch_u[face] = here + dt * (a + mu * b + pc(i, j) - pc(i + 1, j) - kappa * here);
            }
            for &face in &self.open_v {
                // Face (i, j + .5) is v[f = j + 1].
                let (j, i) = ((face / n) as isize - 1, (face % n) as isize);
                let here = v[face];
                let a = vc(i, j).powi(2) - vc(i, j + 1).powi(2) + uv(i - 1, j) - uv(i, j);
                let b = vf(i + 1, j + 1) + vf(i - 1, j + 1) + vf(i, j + 2) + vf(i, j) - 4.0 * here;
                self.scratch_v[face] = here + dt * (a + mu * b + pc(i, j) - pc(i, j + 1) - kappa * here);
            }
            // Closed faces are zero in both buffers, so swapping keeps them zero.
            std::mem::swap(&mut self.u, &mut self.scratch_u);
            std::mem::swap(&mut self.v, &mut self.scratch_v);
            self.limit_speed();
        }
    }

    fn limit_speed(&mut self) {
        let clamp = |x: &mut f32| *x = if x.is_finite() { x.clamp(-SPEED_LIMIT, SPEED_LIMIT) } else { 0.0 };
        for &f in &self.open_u {
            clamp(&mut self.u[f]);
        }
        for &f in &self.open_v {
            clamp(&mut self.v[f]);
        }
    }

    /// §4.3.2: redistribute water until the flow is (nearly) divergence-free. Outflow lowers a
    /// cell's pressure and pulls water back in. (Relaxed in place, which converges faster than
    /// the paper's two-buffer version.)
    fn relax_divergence(&mut self) {
        let n = self.n;
        let xi = self.params.relax_rate;
        for _ in 0..self.params.relax_iterations {
            let mut worst = 0.0f32;
            for (&c, open) in self.wet.iter().zip(&self.faces) {
                let (i, j) = (c % n, c / n);
                let (right, left, down, up) = (j * (n + 1) + i + 1, j * (n + 1) + i, (j + 1) * n + i, j * n + i);
                let div = self.u[right] - self.u[left] + self.v[down] - self.v[up];
                let delta = -xi * div;
                self.p[c] += delta;
                if open[0] {
                    self.u[right] += delta;
                }
                if open[1] {
                    self.u[left] -= delta;
                }
                if open[2] {
                    self.v[down] += delta;
                }
                if open[3] {
                    self.v[up] -= delta;
                }
                worst = worst.max(delta.abs());
            }
            if worst < self.params.relax_tolerance {
                break;
            }
        }
        self.limit_speed();
    }

    /// §4.3.3: drain pressure near the edge of the wet area, so water (and pigment) flows outward
    /// as it does in a drying stroke whose rim is pinned.
    fn flow_outward(&mut self) {
        let eta = self.params.edge_darkening;
        for &c in &self.wet {
            self.p[c] -= eta * self.edge[c];
        }
    }

    /// §4.4: carry suspended pigment along with the water, conserving it.
    fn move_pigment(&mut self) {
        let n = self.n;
        let steps = self.max_speed().ceil().max(1.0) as usize;
        let dt = 1.0 / steps as f32;
        for _ in 0..steps {
            for &c in &self.wet {
                self.scratch_g[c] = self.g[c];
            }
            for &c in &self.wet {
                let g = self.g[c];
                if g <= 0.0 {
                    continue;
                }
                let (i, j) = (c % n, c / n);
                // Closed faces have zero velocity, so pigment only moves between wet cells.
                let right = dt * self.u[j * (n + 1) + i + 1].max(0.0);
                let left = dt * (-self.u[j * (n + 1) + i]).max(0.0);
                let down = dt * self.v[(j + 1) * n + i].max(0.0);
                let up = dt * (-self.v[j * n + i]).max(0.0);
                let total = right + left + down + up;
                if total <= 0.0 {
                    continue;
                }
                // Never move more pigment than the cell holds.
                let scale = if total > 1.0 { g / total } else { g };
                for (frac, target) in
                    [(right, c + 1), (left, c.wrapping_sub(1)), (down, c + n), (up, c.wrapping_sub(n))]
                {
                    if frac > 0.0 {
                        self.scratch_g[target] += frac * scale;
                    }
                }
                self.scratch_g[c] -= total * scale;
            }
            for &c in &self.wet {
                self.g[c] = self.scratch_g[c];
            }
        }
    }

    /// §4.5: adsorption onto and desorption from the paper. Valleys collect pigment and peaks
    /// shed it, in proportion to the pigment's granulation.
    fn transfer_pigment(&mut self) {
        let Pigment { density: rho, staining: omega, granulation: gamma, .. } = self.pigment;
        for &c in &self.wet {
            let h = self.height[c];
            let (g, d) = (self.g[c], self.d[c]);
            let mut down = g * (1.0 - h * gamma) * rho;
            let mut up = d * (1.0 + (h - 1.0) * gamma) * rho / omega;
            if d + down > 1.0 {
                down = (1.0 - d).max(0.0);
            }
            if g + up > 1.0 {
                up = (1.0 - g).max(0.0);
            }
            self.d[c] = d + down - up;
            self.g[c] = g + up - down;
        }
    }
}

/// (1 − M′)·M, with M′ the mask blurred by a Gaussian of `k` cells: how close each wet cell is to
/// the edge of the wet area.
fn edge_weight(n: usize, mask: &[bool], k: usize) -> Vec<f32> {
    let sigma = k as f32 / 3.0;
    let radius = k as isize;
    let kernel: Vec<f32> = (-radius..=radius).map(|x| libm::expf(-(x * x) as f32 / (2.0 * sigma * sigma))).collect();
    let sum: f32 = kernel.iter().sum();
    let m: Vec<f32> = mask.iter().map(|&w| if w { 1.0 } else { 0.0 }).collect();
    let blur = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0; n * n];
        for j in 0..n {
            for i in 0..n {
                let mut acc = 0.0;
                for (o, w) in kernel.iter().enumerate() {
                    let d = o as isize - radius;
                    let (x, y) = if horizontal { (i as isize + d, j as isize) } else { (i as isize, j as isize + d) };
                    if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                        acc += w * src[y as usize * n + x as usize];
                    }
                }
                out[j * n + i] = acc / sum;
            }
        }
        out
    };
    let blurred = blur(&blur(&m, true), false);
    blurred.iter().zip(&m).map(|(b, m)| (1.0 - b) * m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pigment::by_name;

    /// A disc of wet paint on flat or rough paper.
    fn disc(n: usize, pigment: &str, height: impl Fn(usize, usize) -> f32) -> Glaze {
        let c = n as f32 / 2.0;
        let r = n as f32 * 0.35;
        let mask: Vec<bool> =
            (0..n * n).map(|k| ((k % n) as f32 + 0.5 - c).hypot((k / n) as f32 + 0.5 - c) < r).collect();
        let h = (0..n * n).map(|k| height(k % n, k / n)).collect();
        let mut g = Glaze::new(n, Params::default(), by_name(pigment).unwrap(), mask, h);
        for k in 0..n * n {
            if g.mask()[k] {
                g.g[k] = 0.4;
            }
        }
        g
    }

    #[test]
    fn pigment_is_conserved() {
        let mut g = disc(32, "Burnt Umber", |i, j| ((i * 7 + j * 13) % 10) as f32 / 10.0);
        let before = g.pigment_total();
        for _ in 0..30 {
            g.step();
        }
        let after = g.pigment_total();
        assert!((after - before).abs() / before < 1e-3, "{before} → {after}");
    }

    #[test]
    fn edges_darken() {
        let n = 48;
        let mut g = disc(n, "Quinacridone Rose", |_, _| 0.5);
        for _ in 0..120 {
            g.step();
        }
        let x = g.thickness();
        let c = n as f32 / 2.0;
        let r = n as f32 * 0.35;
        let (mut rim, mut rim_n, mut centre, mut centre_n) = (0.0, 0, 0.0, 0);
        for (k, &xk) in x.iter().enumerate() {
            let d = ((k % n) as f32 + 0.5 - c).hypot((k / n) as f32 + 0.5 - c);
            if d < r && d > r - 3.0 {
                rim += xk;
                rim_n += 1;
            } else if d < r * 0.5 {
                centre += xk;
                centre_n += 1;
            }
        }
        let (rim, centre) = (rim / rim_n as f32, centre / centre_n as f32);
        assert!(rim > centre * 1.1, "rim {rim} should be darker than centre {centre}");
    }

    #[test]
    fn granulating_pigment_settles_in_the_valleys() {
        let n = 40;
        let height = |i: usize, j: usize| {
            if (i / 2 + j / 2).is_multiple_of(2) { 0.9 } else { 0.1 }
        };
        let mut g = disc(n, "French Ultramarine", height);
        for _ in 0..60 {
            g.step();
        }
        let (mut valley, mut peak) = (0.0, 0.0);
        for k in 0..n * n {
            if g.mask()[k] {
                if g.height()[k] < 0.5 { valley += g.d[k] } else { peak += g.d[k] }
            }
        }
        assert!(valley > peak * 1.2, "valleys {valley} vs peaks {peak}");
    }
}
