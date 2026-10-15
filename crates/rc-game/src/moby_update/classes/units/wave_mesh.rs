//! **The rippling water meshes of Quartu and the Fleet** (census U552): Quartu's 1408 / 1409 / 1565 / 1567 (level15
//! `0x2eb0a0` / `0x2eb458` / `0x2edf58` / `0x2ee338`, draws `0x2eac90` / `0x2eb108` / `0x2edbe0` / `0x2edfc0`) and the
//! Fleet's 1405 / 1406 / 1407 / 1426 (level17 `0x2f08e0` / `0x2f0ca0` / `0x2f1060` / `0x2f1418`, draws `0x2f04d0` /
//! `0x2f0948` / `0x2f0d08` / `0x2f10c8`); one placed each, 1426 twice (the two pools by the start). One code with its
//! own tables per class (1426's constants gp−0x488c.. equal 1407's gp−0x48ec..). Read from the level15 / level17 decomp and
//! data; native `f32`.
//!
//! * **The update**: at first the ambient (0x40, 0x40, 0x40) (`0x23f958`) → 1; then hidden (mode \| 1), draw
//!   distance 0, the draw registered.
//! * **The draw**: the wave's angle +0x04 moves on by 360°/`scale(period)`·dt a tick (+0x00 keeps the last one).
//!   Each strip point (the class's strips, `scale`d about the moby) rises by `amp·sin(angle + 2π·(|xy| mod L)/L)`
//!   (L the wavelength 8); its colour the moby's ambient with alpha 0x40. The strips are drawn twice (FX 0x2d, alpha
//!   blended), their ST moved by `(t·scroll) mod 1` with t = counter·speed·dt, the scroll (1, −0.628) the first time
//!   and (1, 0.628) the second (`DrawEnvOverlayMesh`, each strip a triangle strip).
//!
//! | class | strips (points) | scale | period | speed |
//! |---|---|---|---|---|
//! | 1408 | 148, 148, 31 | 1 | 1.14 | 0.1 |
//! | 1409 | 68 | 1 | 1.14 | 0.025 |
//! | 1565 | 46 | 0.5 | 2 | 0.1 |
//! | 1567 | 88 | 1.1 | 2 | 0.1 |
//! | 1405 | 148, 11 | 1 | 1.14 | 0.1 |
//! | 1406 | 89 | 1 | 1.14 | 0.025 |
//! | 1407 | 45 | 1 | 1.14 | 0.025 |
//! | 1426 | 45 | 1 | 1.14 | 0.025 |

use std::sync::Arc;

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{normalize_angle, pvar as p, Services, World};
use crate::ps2v::Pf;

/// One class's draw: its level, update and draw, the strips' (points, positions, ST) tables, the scale, the period
/// and the ST speed.
pub struct Def {
    pub level: u32,
    pub class: i16,
    pub update: u32,
    pub draw: u32,
    pub strips: &'static [(usize, u32, u32)],
    pub scale: f32,
    pub period: f32,
    pub speed: f32,
}

pub const DEFS: [Def; 8] = [
    Def { level: 15, class: 1408, update: 0x2e_b0a0, draw: 0x2e_ac90, strips: &[(148, 0x1d_9e40, 0x1d_ad98), (148, 0x1d_a530, 0x1d_b238), (31, 0x1d_ac20, 0x1d_b6d8)], scale: 1.0, period: 1.14, speed: 0.1 },
    Def { level: 15, class: 1409, update: 0x2e_b458, draw: 0x2e_b108, strips: &[(68, 0x1d_dba0, 0x1d_ded0)], scale: 1.0, period: 1.14, speed: 0.025 },
    Def { level: 15, class: 1565, update: 0x2e_df58, draw: 0x2e_dbe0, strips: &[(46, 0x1d_e860, 0x1d_ea88)], scale: 0.5, period: 2.0, speed: 0.1 },
    Def { level: 15, class: 1567, update: 0x2e_e338, draw: 0x2e_dfc0, strips: &[(88, 0x1d_f100, 0x1d_f520)], scale: 1.1, period: 2.0, speed: 0.1 },
    Def { level: 17, class: 1405, update: 0x2f_08e0, draw: 0x2f_04d0, strips: &[(148, 0x1d_a040, 0x1d_a7b8), (11, 0x1d_a730, 0x1d_ac58)], scale: 1.0, period: 1.14, speed: 0.1 },
    Def { level: 17, class: 1406, update: 0x2f_0ca0, draw: 0x2f_0948, strips: &[(89, 0x1d_be20, 0x1d_c250)], scale: 1.0, period: 1.14, speed: 0.025 },
    Def { level: 17, class: 1407, update: 0x2f_1060, draw: 0x2f_0d08, strips: &[(45, 0x1d_cee0, 0x1d_d100)], scale: 1.0, period: 1.14, speed: 0.025 },
    Def { level: 17, class: 1426, update: 0x2f_1418, draw: 0x2f_10c8, strips: &[(45, 0x1d_d760, 0x1d_d980)], scale: 1.0, period: 1.14, speed: 0.025 },
];
pub const CLASSES: [[i16; 1]; 8] = [[1408], [1409], [1565], [1567], [1405], [1406], [1407], [1426]];

const FX: usize = 0x2d;
const ALPHA: u32 = 0x40;
const WAVELENGTH: f32 = 8.0;
const AMP: f32 = 0.1;
const SCROLL: [f32; 4] = [1.0, f32::from_bits(0xbf20_d97c), 1.0, f32::from_bits(0x3f20_d97c)];
/// The pvars: +0x00 the last angle, +0x04 the angle; the port's +0x08 / +0x0c the draw's time (counter·speed·dt).
const LEN: usize = 0x10;

/// One strip's points and ST as stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strip {
    pub pts: Vec<[f32; 3]>,
    pub st: Vec<[f32; 2]>,
}

/// The level's strips, by [`DEFS`] index (empty for another level's classes; set by the engine at the level load).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meshes {
    pub defs: Vec<Vec<Strip>>,
}

impl Meshes {
    /// Reads the strips of `level`'s classes from its overlay (None without any, or when a table does not read).
    pub fn parse(ov: &rc_formats::water::Overlay, level: u32) -> Option<Arc<Meshes>> {
        if !DEFS.iter().any(|d| d.level == level) { return None; }
        let mut defs = Vec::new();
        for d in &DEFS {
            if d.level != level {
                defs.push(Vec::new());
                continue;
            }
            let mut strips = Vec::new();
            for &(n, pa, sa) in d.strips {
                let pts = (0..n as u32).map(|k| Some([ov.f32(pa + 12 * k).ok()?, ov.f32(pa + 12 * k + 4).ok()?, ov.f32(pa + 12 * k + 8).ok()?])).collect::<Option<Vec<_>>>()?;
                let st = (0..n as u32).map(|k| Some([ov.f32(sa + 8 * k).ok()?, ov.f32(sa + 8 * k + 4).ok()?])).collect::<Option<Vec<_>>>()?;
                strips.push(Strip { pts, st });
            }
            defs.push(strips);
        }
        Some(Arc::new(Meshes { defs }))
    }
}

fn def_of(level: u32, f: u32) -> Option<usize> { DEFS.iter().position(|d| d.level == level && (d.update == f || d.draw == f)) }

/// The update of [`DEFS`]`[k]` (module doc), with the draw's per-tick part (the angle and the time).
pub fn update(w: &mut World, id: MobyId, k: usize) {
    crate::moby_update::story::pvars(w, id, LEN);
    if w.m(id).state == 0 {
        let m = w.mm(id);
        m.ambient[..3].copy_from_slice(&[0x40; 3]);
        m.state = 1;
        return;
    }
    let d = &DEFS[k];
    if let Some(row) = super::row(d.level, d.draw) {
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
    let m = w.mm(id);
    m.draw_dist = 0;
    m.mode |= 1;
    let a = c::pf(w, id, 4);
    c::set_pf(w, id, 0, a);
    let period = w.svc.timing.scale(Pf::f(d.period)).to_f32();
    c::set_pf(w, id, 4, c::add_rot(a, 360.0 / period * 0.017_453_292 * DT));
    let t = w.counter as f32 * d.speed * DT;
    c::set_pf(w, id, 8, t);
}

macro_rules! updates {
    ($($name:ident = $k:expr),*) => { $(pub fn $name(w: &mut World, id: MobyId) { update(w, id, $k) })* };
}
updates!(update_1408 = 0, update_1409 = 1, update_1565 = 2, update_1567 = 3, update_1405 = 4, update_1406 = 5, update_1407 = 6, update_1426 = 7);
pub const UPDATES: [fn(&mut World, MobyId); 8] = [update_1408, update_1409, update_1565, update_1567, update_1405, update_1406, update_1407, update_1426];

/// The draw `f` of `level` for moby `id` (module doc).
pub fn fx_quads(table: &MobyTable, svc: &Services, level: u32, f: u32, id: MobyId) -> Option<FxQuads> {
    let k = def_of(level, f)?;
    let d = &DEFS[k];
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= LEN)?;
    let strips = svc.units.wave_meshes.as_ref()?.defs.get(k)?;
    let angle = p::ff(&m.pvars, 4);
    let t = p::ff(&m.pvars, 8);
    let rgba = ALPHA << 24 | (m.ambient[2] as u32) << 16 | (m.ambient[1] as u32) << 8 | m.ambient[0] as u32;
    let mut quads = Vec::new();
    for pass in 0..2 {
        let ds = (t * SCROLL[2 * pass]) % 1.0;
        let dt = (t * SCROLL[2 * pass + 1]) % 1.0;
        for s in strips {
            let pts: Vec<[f32; 3]> = s.pts.iter().map(|v| {
                let r = (v[0] * v[0] + v[1] * v[1]).sqrt() % WAVELENGTH;
                let ph = c::add_rot(normalize_angle(Pf::f(r * std::f32::consts::TAU / WAVELENGTH)).to_f32(), angle);
                [d.scale * v[0] + m.position[0], d.scale * v[1] + m.position[1], d.scale * v[2] + m.position[2] + AMP * ph.sin()]
            }).collect();
            let st: Vec<[f32; 2]> = s.st.iter().map(|q| [q[0] + ds, q[1] + dt]).collect();
            strip_quads(&pts, &st, rgba, &mut quads);
        }
    }
    Some(FxQuads { fx: FX, additive: false, subtract: false, quads })
}

/// A triangle strip as quads (triangles 2i and 2i + 1 are quad i's; an odd last triangle a quad with its last
/// corner repeated).
fn strip_quads(pts: &[[f32; 3]], st: &[[f32; 2]], rgba: u32, out: &mut Vec<FxQuad>) {
    let n = pts.len();
    let mut i = 0;
    while i + 2 < n {
        let j = [i, i + 1, i + 2, (i + 3).min(n - 1)];
        out.push(FxQuad { corners: j.map(|k| pts[k]), st: j.map(|k| st[k]), rgba: [rgba; 4] });
        i += 2;
    }
}
