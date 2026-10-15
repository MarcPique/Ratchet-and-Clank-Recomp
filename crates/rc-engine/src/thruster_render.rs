//! The Thruster-Pack flames' draw callback (level01 `0x2c9290`, list 2; class 0xa7,
//! `rc_game::moby_update::classes::thruster_flame`), read from the disassembly. For each flame with the pack moby of
//! class 0x260 on the back, in the flame's frame F (Z along the nozzle into the pack, at the nozzle point `p1`: the
//! pack's joint lists 2 / 3, 0 / 1 for side 1, posed as the pack is drawn), three draws through the shared
//! callback-draw machinery (crate::fx_draw: `FastDrawQuadReal` quads and `0x21fda8` strips, FX textures, fog,
//! ALPHA 0x8000000048 = additive `Cs·As + Cd`, Z tested, not written):
//! 1. **two crossed quads**, FX 0xc, RGBA 0x80808080: the tables 0x1dae60 (the y–z plane) and 0x1daea0 (the x–z
//!    plane), (±2, ±2) scaled by the quad size (+0x2c: 0.025 when burning), their two far corners pushed back along
//!    −Z by the callback's `randf(0, 0.1)` jitters, in F moved 0.075 back along −Z; ST (0, 0), (0, 1), (1, 0), (1, 1)
//!    (table 0x1daee0);
//! 2. **the glow**, FX 0xb, RGBA 0x7f2020ff: the first table × 0.1 in the camera-facing basis at `p1 − 0.13·Z`: V =
//!    unit(camera − point), R = unit(V × g) (g = the gravity direction 0x13f5e0), U = V × R;
//! 3. **the flame mesh**, FX 0xa: 196 vertices (table 0x1d98f0, a cone along −Z, 33 units long) scaled by the mesh
//!    size (+0x20: 0.0125) in F, per-vertex RGBA (0x1dab50) and ST (0x1da530) whose t scrolls by −0.025 on every
//!    draw of either flame (wrapping by +7 below −7: the game writes it back into the table), two tri-strips of
//!    vertices 0..148 and 146..196.
//!
//! The tables are the level overlay's, read through `rc_formats::level_overlay::Relocation` from level 01 (the
//! callback is the same code on every level). The flame's frame is computed here from the pack's pose this frame, as
//! the game's callback does at draw time; the jitters are the ones the last `run_frame` drew (one frame behind).
//! Native `f32`.

use crate::fx_draw::{FxGroup, PrimBuf};
use rc_formats::level_overlay::{LevelOverlay, Relocation};
use rc_game::moby_update::classes::thruster_flame::{self as tf, pv};
use rc_game::moby_update::services::pvar;
use std::sync::{Mutex, OnceLock};

/// The reference (level01) table addresses.
const MESH: u32 = 0x1d_98f0;
const MESH_ST: u32 = 0x1d_a530;
const MESH_RGBA: u32 = 0x1d_ab50;
const QUAD_A: u32 = 0x1d_ae60;
const QUAD_B: u32 = 0x1d_aea0;
const QUAD_ST: u32 = 0x1d_aee0;
/// Mesh vertices and its strips `(first, count)` (`0x21fda8(0x94, …)`, `0x21fda8(0x32, … + 146)`).
const MESH_N: usize = 196;
const STRIPS: [(usize, usize); 2] = [(0, 0x94), (146, 0x32)];
/// The FX textures, colours and offsets of the callback.
const QUAD_FX: usize = 0xc;
const GLOW_FX: usize = 0xb;
const MESH_FX: usize = 0xa;
const QUAD_RGBA: u32 = 0x8080_8080;
const GLOW_RGBA: u32 = 0x7f20_20ff;
const QUAD_BACK: f32 = -0.075;
const GLOW_BACK: f32 = -0.13;
const GLOW_SCALE: f32 = 0.1;
const SCROLL: f32 = 0.025;
const WRAP: f32 = 7.0;

/// The callback's tables on the loaded level.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameTables {
    pub mesh: Vec<[f32; 3]>,
    pub st: Vec<[f32; 2]>,
    pub rgba: Vec<u32>,
    pub quads: [[[f32; 3]; 4]; 2],
    pub quad_st: [[f32; 2]; 4],
}

impl FlameTables {
    /// Reads the tables of `target` at the addresses `rel` gives for the level-01 ones.
    pub fn parse(target: &LevelOverlay, rel: &Relocation) -> Option<FlameTables> {
        let at = |a: u32| rel.data(a);
        let f = |a: u32| target.u32(a).map(f32::from_bits);
        let v3 = |a: u32| Some([f(a)?, f(a + 4)?, f(a + 8)?]);
        let (m, s, c) = (at(MESH)?, at(MESH_ST)?, at(MESH_RGBA)?);
        let mesh = (0..MESH_N as u32).map(|i| v3(m + 16 * i)).collect::<Option<Vec<_>>>()?;
        let st = (0..MESH_N as u32).map(|i| Some([f(s + 8 * i)?, f(s + 8 * i + 4)?])).collect::<Option<Vec<_>>>()?;
        let rgba = (0..MESH_N as u32).map(|i| target.u32(c + 4 * i)).collect::<Option<Vec<_>>>()?;
        let quad = |a: u32| -> Option<[[f32; 3]; 4]> { Some([v3(a)?, v3(a + 16)?, v3(a + 32)?, v3(a + 48)?]) };
        let q = at(QUAD_ST)?;
        let quad_st = [0u32, 1, 2, 3].map(|i| [f(q + 8 * i).unwrap_or(0.0), f(q + 8 * i + 4).unwrap_or(0.0)]);
        Some(FlameTables { mesh, st, rgba, quads: [quad(at(QUAD_A)?)?, quad(at(QUAD_B)?)?], quad_st })
    }
}

/// The tables and the mesh ST the draws scroll (the game writes it back into its table).
type Loaded = (FlameTables, Mutex<Vec<[f32; 2]>>);

/// The loaded level's tables (once per process).
fn tables() -> Option<&'static Loaded> {
    static T: crate::level_load::PerLevel<Option<Loaded>> = [const { OnceLock::new() }; 20];
    crate::level_load::per_level(&T).get_or_init(|| {
        let (root, index) = (crate::level_load::extracted_root(), crate::level_load::level_index());
        let ov = |l: u32| crate::disc_source::level_file(&root, l, "overlay.bin").ok().and_then(|b| LevelOverlay::parse(&b).ok());
        let (Some(target), Some(reference)) = (ov(index), ov(1)) else {
            eprintln!("thruster flames: no level overlay: the flames draw only their particles");
            return None;
        };
        let t = FlameTables::parse(&target, &Relocation::new(&reference, &target));
        if t.is_none() { eprintln!("thruster flames: the callback's tables were not found on level {index:02}"); }
        t.map(|t| {
            let st = Mutex::new(t.st.clone());
            (t, st)
        })
    })
    .as_ref()
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn scale3(a: [f32; 3], s: f32) -> [f32; 3] { a.map(|x| x * s) }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn unit3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { v } else { scale3(v, 1.0 / l) }
}
/// `x·r0 + y·r1 + z·r2 + t`.
fn xform(r: &[[f32; 3]; 3], t: [f32; 3], v: [f32; 3]) -> [f32; 3] { add3(add3(add3(scale3(r[0], v[0]), scale3(r[1], v[1])), scale3(r[2], v[2])), t) }

/// The flame's draw inputs: its frame (rows X, Y, Z and the nozzle point), mesh and quad sizes, the four jitters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlameDraw {
    pub rows: [[f32; 3]; 3],
    pub at: [f32; 3],
    pub mesh_size: f32,
    pub quad_size: f32,
    pub jitter: [f32; 4],
}

/// The world geometry of one flame's draws: the two quads' corners (strip order), the glow's corners, the mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct FlameGeom {
    pub quads: [[[f32; 3]; 4]; 2],
    pub glow: [[f32; 3]; 4],
    pub mesh: Vec<[f32; 3]>,
}

/// The geometry of one flame (module doc).
pub fn flame_geom(t: &FlameTables, d: &FlameDraw, cam: [f32; 3], gravity: [f32; 3]) -> FlameGeom {
    let z = d.rows[2];
    // 1. The quads, in F moved 0.075 back; each quad's corners 1 and 3 pushed back by its two jitters.
    let q_at = add3(d.at, scale3(z, QUAD_BACK));
    let quads: [[[f32; 3]; 4]; 2] = std::array::from_fn(|k| {
        std::array::from_fn(|i| {
            let mut v = scale3(t.quads[k][i], d.quad_size);
            if i & 1 == 1 { v[2] -= d.jitter[2 * k + i / 2]; }
            xform(&d.rows, q_at, v)
        })
    });
    // 2. The glow at p1 − 0.13·Z, facing the camera.
    let g_at = add3(d.at, scale3(z, GLOW_BACK));
    let v = unit3(sub3(cam, g_at));
    let r = unit3(cross(v, gravity));
    let u = cross(v, r);
    let glow = std::array::from_fn(|i| xform(&[v, r, u], g_at, scale3(t.quads[0][i], GLOW_SCALE)));
    // 3. The mesh.
    let mesh = t.mesh.iter().map(|&p| xform(&d.rows, d.at, scale3(p, d.mesh_size))).collect();
    FlameGeom { quads, glow, mesh }
}

/// The three draws of one flame (module doc) with the mesh ST `st` (already scrolled for this draw).
pub fn flame_prims(t: &FlameTables, st: &[[f32; 2]], d: &FlameDraw, cam: [f32; 3], gravity: [f32; 3]) -> Vec<FxGroup> {
    let g = flame_geom(t, d, cam, gravity);
    let mut quads = PrimBuf::default();
    for q in g.quads { quads.quad(q, t.quad_st, [QUAD_RGBA; 4]); }
    let mut glow = PrimBuf::default();
    glow.quad(g.glow, t.quad_st, [GLOW_RGBA; 4]);
    let mut mesh = PrimBuf::default();
    for (a, n) in STRIPS {
        mesh.strip((a..(a + n).min(g.mesh.len())).map(|i| (g.mesh[i], st[i], t.rgba[i])));
    }
    vec![
        FxGroup { fx: QUAD_FX, additive: true, subtract: false, prims: quads },
        FxGroup { fx: GLOW_FX, additive: true, subtract: false, prims: glow },
        FxGroup { fx: MESH_FX, additive: true, subtract: false, prims: mesh },
    ]
}

/// One scroll step of the mesh ST (the callback's loop: t − 0.025, + 7 below −7).
pub fn scroll(st: &mut [[f32; 2]]) {
    for s in st.iter_mut() {
        let v = s[1] - SCROLL;
        s[1] = if v < -WRAP { v + WRAP } else { v };
    }
}

/// The callback for flame moby `id` of the running game (crate::fx_draw's list 2): nothing without the tables, the
/// Thruster-Pack on the back or its pose.
pub fn flame_groups(game: &rc_game::tick::Game, id: rc_game::moby_runtime::MobyId, cam: [f32; 3]) -> Vec<FxGroup> {
    let Some((t, st)) = tables() else { return Vec::new() };
    let Some(m) = game.mobys.mobys.get(id) else { return Vec::new() };
    let h = &game.hero;
    if pvar::i32(&m.pvars, pv::PACK) == 0 || h.back.as_ref().is_none_or(|b| b.state == 0 || b.pack_o_class != tf::PACK_CLASS) { return Vec::new(); }
    let side = (pvar::i32(&m.pvars, pv::SIDE) != 0) as usize;
    let [la, lb] = tf::LISTS[side];
    let (Some(p0), Some(p1)) = (rc_game::hero::fx::pack_point(h, la), rc_game::hero::fx::pack_point(h, lb)) else { return Vec::new() };
    let f = tf::frame([p0[0], p0[1], p0[2]], [p1[0], p1[1], p1[2]]);
    let d = FlameDraw {
        rows: [0, 1, 2].map(|i| [f[i][0], f[i][1], f[i][2]]),
        at: [f[3][0], f[3][1], f[3][2]],
        mesh_size: pvar::ff(&m.pvars, pv::SIZES),
        quad_size: pvar::ff(&m.pvars, pv::SIZES + 12),
        jitter: std::array::from_fn(|k| pvar::ff(&m.pvars, pv::JITTER + 4 * k)),
    };
    let g = h.gravity_dir.map(|x| x.to_f32());
    let mut st = st.lock().unwrap_or_else(|e| e.into_inner());
    scroll(&mut st);
    flame_prims(t, &st, &d, cam, [g[0], g[1], g[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The callback's tables are found through the relocation on all 19 levels and are the same data everywhere: the
    /// quads (0, ±2, ±2) / (±2, 0, ±2), the 196-vertex cone along −z (about 33 units long).
    #[test]
    fn tables_on_every_level() {
        let root = crate::level_load::extracted_root();
        let ov = |l: u32| std::fs::read(root.join(format!("levels/{l:02}/overlay.bin"))).ok().and_then(|b| LevelOverlay::parse(&b).ok());
        let Some(reference) = ov(1) else { eprintln!("skipped: no extracted/"); return };
        let base = FlameTables::parse(&reference, &Relocation::new(&reference, &reference)).expect("level 01 tables");
        assert_eq!(base.quads[0], [[0.0, -2.0, 2.0], [0.0, -2.0, -2.0], [0.0, 2.0, 2.0], [0.0, 2.0, -2.0]]);
        assert_eq!(base.quad_st, [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]]);
        let zmin = base.mesh.iter().map(|p| p[2]).fold(0.0f32, f32::min);
        assert!((-34.0..-32.0).contains(&zmin), "{zmin}");
        for l in 0..19 {
            let Some(t) = ov(l) else { continue };
            assert_eq!(FlameTables::parse(&t, &Relocation::new(&reference, &t)).as_ref(), Some(&base), "level {l:02}");
        }
    }

    /// The ST scroll: −0.025 a draw, wrapped by +7 once below −7.
    #[test]
    fn scroll_wraps() {
        let mut st = vec![[0.5, 0.0], [0.5, -6.99]];
        scroll(&mut st);
        assert_eq!(st[0][1], -0.025);
        assert!((st[1][1] - (-7.015 + 7.0)).abs() < 1e-5, "{st:?}");
    }

    /// The draws of a burning flame along −x: the quads 0.075 back and their far corners jittered, the glow 0.13
    /// back facing the camera, the mesh in the frame.
    #[test]
    fn flame_draws_in_its_frame() {
        let quad = [[0.0, -2.0, 2.0], [0.0, -2.0, -2.0], [0.0, 2.0, 2.0], [0.0, 2.0, -2.0]];
        let t = FlameTables {
            mesh: vec![[0.0, 0.0, -33.0]; MESH_N],
            st: vec![[0.5, 0.0]; MESH_N],
            rgba: vec![0x8080_8080; MESH_N],
            quads: [quad, [[-2.0, 0.0, 2.0], [-2.0, 0.0, -2.0], [2.0, 0.0, 2.0], [2.0, 0.0, -2.0]]],
            quad_st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]],
        };
        // Z = +x (into the pack), so the exhaust points along −x.
        let f = tf::frame([11.0, 0.0, 5.0], [10.0, 0.0, 5.0]);
        let d = FlameDraw {
            rows: [0, 1, 2].map(|i| [f[i][0], f[i][1], f[i][2]]),
            at: [10.0, 0.0, 5.0],
            mesh_size: 0.0125,
            quad_size: 0.025,
            jitter: [0.1, 0.0, 0.0, 0.0],
        };
        let groups = flame_prims(&t, &t.st, &d, [0.0, -20.0, 5.0], [0.0, 0.0, -1.0]);
        assert_eq!(groups.iter().map(|g| (g.fx, g.additive)).collect::<Vec<_>>(), vec![(0xc, true), (0xb, true), (0xa, true)]);
        let g = flame_geom(&t, &d, [0.0, -20.0, 5.0], [0.0, 0.0, -1.0]);
        // The mesh tip: 33·0.0125 = 0.4125 along −x from the nozzle.
        let tip = g.mesh[0];
        assert!((tip[0] - (10.0 - 0.4125)).abs() < 1e-4 && (tip[2] - 5.0).abs() < 1e-4, "{tip:?}");
        // Quad 1's corner 1: z −(0.05 + 0.1), then 0.075 back: x = 10 − 0.225; quad 2's corners are not jittered.
        assert!((g.quads[0][1][0] - (10.0 - 0.225)).abs() < 1e-5, "{:?}", g.quads[0]);
        assert!((g.quads[1][1][0] - (10.0 - 0.125)).abs() < 1e-5, "{:?}", g.quads[1]);
        // The glow's centre 0.13 back, 0.2 across along the camera-facing axes.
        let c: [f32; 3] = std::array::from_fn(|k| (0..4).map(|i| g.glow[i][k]).sum::<f32>() / 4.0);
        assert!((c[0] - 9.87).abs() < 1e-5 && c[1].abs() < 1e-5 && (c[2] - 5.0).abs() < 1e-5, "{c:?}");
        assert!((g.glow[0][2] - 5.2).abs() < 1e-5, "{:?}", g.glow);
    }
}
