//! **Orxon's air curtains, class 1378** (level10 `0x2e9028`, its draw `0x2e91b8`; census U369; 2 placed; the same
//! class on level 17). An invisible doorway (hidden, not drawn as a moby) that keeps the air on one side: Ratchet within
//! 8 with his body point (0x13f420) in its frame within ±5 across and 0..8 up gets no air on its front (local y < 0.5:
//! 0x14161b = 1, the O2 Mask's cue) and air behind it (0); outside that window, behind it (y > 0) the air comes back.
//! Every tick it draws its shimmer (list 1).
//!
//! **The shimmer** (`0x2e91b8`): six layers 0.1 apart (local y from −0.25), each two quads in the curtain's frame: the
//! wall (−5..5 across by pvar 0, 0..5 up by pvar 1) and a strip above it (5..6) fading to black at its top; FX 0x2c;
//! the colours per layer from 0x1da250 (0x0008170e / 0x0002170e on the even layers, 0x00030a02 on the odd ones); the
//! ST of each layer's corners (0.25 / 0.75) wobbling by an eighth with the sine or cosine of the VSync count over the
//! layer's own periods (0x1da270, 0x1da2b0), the strip's 0.125 above the wall's top corners. The even layers are
//! `ALPHA 0x80 << 32 | 0x68` (`Cs·FIX + Cd`: added), the odd ones `0x80 << 32 | 0x86` (`(Cd − Cs)·FIX`: taken off).
//!
//! The FIX 0x80 is drawn as the vertex alpha 0x80 (the strip's black corners too, so only the colour fades), the odd
//! layers through the subtract blend (`FxQuads::subtract`). [L] The VSync count 0x15f3f8 is the draw tick.
//!
//! Read from the level10 decomp; the tables from the overlay (0x1da250 .. 0x1da38c) and gp (−0x4cc0 the layer count 6,
//! −0x4cb8 .. the per-parity ALPHA fields). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e9028` | the curtain | [`update`] |
//! | `0x2e91b8` | the shimmer | [`fx_quad_groups`] (`Callback::UnitQuads`, six groups) |

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature as c;
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 10;
pub const UPDATE_FN: u32 = 0x2e_9028;
pub const DRAW_FN: u32 = 0x2e_91b8;
pub const CLASSES: [i16; 1] = [1378];

/// gp−0x4cc0.
const LAYERS: usize = 6;
/// The ALPHA FIX 0x80, carried as the vertex alpha.
const FIX: u32 = 0x8000_0000;
const COLOURS: [u32; 8] = [0x0008_170e, 0x0003_0a02, 0x0002_170e, 0x0003_0a02, 0x0002_170e, 0x0003_0a02, 0x0002_170e, 0x0003_0a02];
/// 0x1da270 (the sixth layer's second: 3.14).
const PERIODS: [[f32; 2]; 8] = [[2.0, 2.6], [2.8, 2.2], [3.9, 2.52], [1.8, 3.28], [1.45, 2.13], [1.8, f32::from_bits(0x4048_f5c3)], [1.2, 2.312], [1.85, 3.8]];
/// 0: sine, 1: cosine.
const COS: [[u32; 2]; 8] = [[0, 0], [1, 0], [1, 1], [0, 1], [1, 1], [0, 1], [0, 0], [0, 1]];
const ST: [[f32; 2]; 4] = [[0.25, 0.25], [0.25, 0.75], [0.75, 0.25], [0.75, 0.75]];
const WALL: [[f32; 3]; 4] = [[-5.0, -0.25, 0.0], [-5.0, -0.25, 5.0], [5.0, -0.25, 0.0], [5.0, -0.25, 5.0]];
const STRIP: [[f32; 3]; 4] = [[-5.0, -0.25, 5.0], [-5.0, -0.25, 6.0], [5.0, -0.25, 5.0], [5.0, -0.25, 6.0]];

/// Level10 `0x2e9028` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 8);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.visible = 0;
            m.mode |= 1;
        }
        1 => {
            let pos = c::pos(w, id);
            let h = w.hero_point();
            if c::dist3(pos, [h[0], h[1], h[2], pos[3]]) < 8.0 {
                let b = w.hero.body_point.map(|x| f32::from_bits(x.0));
                let d = [b[0] - pos[0], b[1] - pos[1], b[2] - pos[2]];
                let r = w.m(id).rows;
                let l: [f32; 3] = std::array::from_fn(|i| r[i][0] * d[0] + r[i][1] * d[1] + r[i][2] * d[2]);
                let inside = -5.0 < l[0] && l[0] < 5.0 && 0.0 < l[2] && l[2] < 8.0;
                let air = if inside { Some(if l[1] < 0.5 { 1 } else { 0 }) } else if 0.0 < l[1] { Some(0) } else { None };
                if let Some(v) = air { w.hero_fields_mut().airless = Some(v); }
            }
            if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(row), id); }
        }
        _ => {}
    }
}

/// Level10 `0x2e91b8`: the shimmer, one group a layer in draw order (module doc).
pub fn fx_quad_groups(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= 8) else { return Vec::new() };
    let (sx, sz) = (p::ff(&m.pvars, 0), p::ff(&m.pvars, 4));
    let (r, pos) = (m.rows, m.position);
    let to_world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2] + pos[i]) };
    let vsync = svc.draw_callbacks.tick as i64;
    (0..LAYERS).map(|layer| {
        let mut wob = [0.0f32; 2];
        for (j, o) in wob.iter_mut().enumerate() {
            let per = PERIODS[layer][j];
            let k = ((per * 360.0) as i64).max(1);
            let a = ((vsync % k) as f32 / per - 180.0) * 0.017_453_292;
            *o = if COS[layer][j] == 0 { a.sin() } else { a.cos() } * 0.125;
        }
        let y = layer as f32 * 0.1;
        let st: [[f32; 2]; 4] = std::array::from_fn(|k| [ST[k][0] + wob[0], ST[k][1] + wob[1]]);
        let col = COLOURS[layer] | FIX;
        let corner = |c: [f32; 3]| to_world([c[0] * sx, c[1] + y, c[2] * sz]);
        let sst = [st[1], [st[1][0], st[1][1] + 0.125], st[3], [st[3][0], st[3][1] + 0.125]];
        let quads = vec![
            FxQuad { corners: WALL.map(corner), st, rgba: [col; 4] },
            FxQuad { corners: STRIP.map(corner), st: sst, rgba: [col, FIX, col, FIX] },
        ];
        FxQuads { fx: 0x2c, additive: true, subtract: layer % 2 == 1, quads }
    }).collect()
}
