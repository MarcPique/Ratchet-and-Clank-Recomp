//! **The energy barriers, class 78** (level15 `0x2a2bf0`, its draw `0x2a3138`, its shut-down `0x2a35d8`; census U535,
//! 3 placed on Quartu and 4 on the Fleet, the same code and tables on level 17 `0x2a82c0` / `0x2a8808`). Read from the
//! level15 decomp; native `f32`.
//!
//! * **0**: not drawn as a moby (+0x31 = 0, mode \| 1); already shut down for good (its spawn's collected byte
//!   `0x1bbc84` or its persistent death bit) → 3 (the voice slot 0xff, no collision); else → 1.
//! * **1**: its link (pvar +0x10, a moby index; −1 none) in state 2, or its mission done: shut down (`0x2a35d8`:
//!   no collision, the spawn's killed byte (and collected, as `SetDeathBits` does), the hum released → 2). Else the
//!   hum (sound 0, loop, the slot in +0xbc), 16 motes (type 71, [`crate::particles::type71`]) on its face (across
//!   ±0.25, along ±5, up 0..5, two drifts in its plane of length 1/`ticks(30)`, size 0.125, colour 0x60 alpha with
//!   red 0x60..0x7f, green 0x60..0x7f, blue 0x20..0x2f, fading to 0x60808080, phases `ticks(30)`/4, /2, /4), and
//!   Ratchet's body point (0x13f420) projected onto its plane kept in +0x00 while within ±5 along and 0..5 up
//!   (else 0: the motes drift to it), the shimmer drawn.
//! * **2**: +0x14 counts up; for 61 ticks the shimmer fades out (the touch point 0), then → 3.
//! * **The shimmer** (`0x2a3138`): Orxon's air-curtain draw ([`super::orxon_airlock`]) with this class's tables: six
//!   layers 0.1 apart across it, each the wall (−5..5 along, 0..5 up) and the strip above it (5..6) fading to black,
//!   FX 0x2c, the colours 0x1ce230 (faded by +0x14/60), the even layers added, the odd ones taken off.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2a2bf0` / `0x2a35d8` | 78, the shut-down | [`update`], [`shut_down`] |
//! | `0x2a3138` | the shimmer | [`fx_quad_groups`] |
//!
//! [L] The VSync count 0x15f3f8 is the draw tick (as for the air curtains).

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::story;
use crate::particles::type71;

pub const REFERENCE_LEVEL: u32 = 15;
pub const UPDATE_FN: u32 = 0x2a_2bf0;
pub const DRAW_FN: u32 = 0x2a_3138;
pub const CLASSES: [i16; 1] = [78];

const LEN: usize = 0x18;
const LINK: usize = 0x10;
const FADE: usize = 0x14;
const LAYERS: usize = 6;
/// The ALPHA FIX 0x80, carried as the vertex alpha.
const FIX: u32 = 0x8000_0000;
/// Level15 0x1ce230.
const COLOURS: [u32; 6] = [0x0009_1b1f, 0x0019_1511, 0x0009_1c17, 0x0022_1509, 0x0009_141f, 0x0021_1d19];
/// 0x1ce250.
const PERIODS: [[f32; 2]; 6] = [[2.0, 2.6], [2.8, 2.2], [3.9, 2.52], [1.8, 3.28], [1.45, 2.13], [1.8, f32::from_bits(0x4048_f5c3)]];
/// 0x1ce290 (0: sine, 1: cosine).
const COS: [[u32; 2]; 6] = [[0, 0], [1, 0], [1, 1], [0, 1], [1, 1], [0, 1]];
/// 0x1ce2d0.
const ST: [[f32; 2]; 4] = [[0.25, 0.25], [0.25, 0.75], [0.75, 0.25], [0.75, 0.75]];
/// 0x1ce330 / 0x1ce2f0.
const WALL: [[f32; 3]; 4] = [[-0.25, -5.0, 0.0], [-0.25, -5.0, 5.0], [-0.25, 5.0, 0.0], [-0.25, 5.0, 5.0]];
const STRIP: [[f32; 3]; 4] = [[-0.25, -5.0, 5.0], [-0.25, -5.0, 6.0], [-0.25, 5.0, 5.0], [-0.25, 5.0, 6.0]];

fn voice(w: &World, id: MobyId) -> i32 { let v = w.m(id).cmd; if v == 0xff { -1 } else { v as i32 } }
fn rows(w: &World, id: MobyId) -> [[f32; 4]; 4] { w.m(id).rows }
fn turn(r: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|i| r[0][i] * v[0] + r[1][i] * v[1] + r[2][i] * v[2]) }

/// Level15 `0x2a35d8`: shut down for good (module doc) → 2.
pub fn shut_down(w: &mut World, id: MobyId) {
    w.mm(id).has_collision = false;
    story::kill_record(w, id);
    let v = voice(w, id);
    if v != -1 && w.sound_alive(v, id) { w.release_sound(v, id); }
    let m = w.mm(id);
    m.state = 2;
    m.cmd = 0xff;
}

/// Level15 `0x2a2bf0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, LEN);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.visible = 0;
            m.mode |= 1;
            let (lvl, sid) = (w.svc.level, w.m(id).spawn_id);
            let gone = w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(lvl, sid));
            let m = w.mm(id);
            if gone {
                m.state = 3;
                m.cmd = 0xff;
                m.has_collision = false;
            } else {
                m.state = 1;
            }
        }
        1 => {
            let link = c::pi32(w, id, LINK);
            let linked_off = usize::try_from(link).ok().filter(|&m| m < w.table.mobys.len()).is_some_and(|m| w.m(m).state == 2);
            if linked_off || story::mission_done(w, w.m(id).mission as i32) {
                shut_down(w, id);
                return;
            }
            if !w.sound_alive(voice(w, id), id) {
                let v = w.play_sound(0, 4, id);
                w.mm(id).cmd = v as u8;
            }
            motes(w, id);
            let pos = c::pos(w, id);
            let r = rows(w, id);
            let b = w.hero.body_point.map(|x| f32::from_bits(x.0));
            let d = [b[0] - pos[0], b[1] - pos[1], b[2] - pos[2]];
            let l: [f32; 3] = std::array::from_fn(|i| r[i][0] * d[0] + r[i][1] * d[1] + r[i][2] * d[2]);
            let local = [0.0, l[1], l[2]];
            let touch = if l[1].abs() < 5.0 && 0.0 < l[2] && l[2] < 5.0 {
                let t = turn(&r, local);
                [t[0] + pos[0], t[1] + pos[1], t[2] + pos[2], 0.0]
            } else {
                [0.0; 4]
            };
            c::set_pv4(w, id, 0, touch);
            register(w, id);
        }
        2 => {
            let n = c::pi32(w, id, FADE);
            c::set_pi32(w, id, FADE, n + 1);
            if n < 0x3d {
                c::set_pv4(w, id, 0, [0.0; 4]);
                register(w, id);
            } else {
                w.mm(id).state = 3;
            }
        }
        _ => {}
    }
}

fn register(w: &mut World, id: MobyId) {
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(row), id); }
}

/// The 16 motes of a tick (module doc).
fn motes(w: &mut World, id: MobyId) {
    let pos = c::pos(w, id);
    let r = rows(w, id);
    let t30 = w.ticks(0x1e);
    let quarter = (t30 / 4) as f32 as i32;
    let half = (t30 / 2) as f32 as i32;
    for _ in 0..16 {
        let px = w.rng.randf(-0.25, 0.25);
        let py = w.rng.randf(-5.0, 5.0);
        let pz = w.rng.randf(0.0, 5.0);
        let v1 = [0.0, w.rng.randf(-5.0, 5.0), w.rng.randf(-5.0, 5.0)];
        let v2 = [0.0, w.rng.randf(-5.0, 5.0), w.rng.randf(-5.0, 5.0)];
        let p = turn(&r, [px, py, pz]);
        let p = [p[0] + pos[0], p[1] + pos[1], p[2] + pos[2], pos[3]];
        let unit = |v: [f32; 3]| -> [f32; 3] { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); let k = 1.0 / t30 as f32; if n == 0.0 { v } else { v.map(|x| x * k / n) } };
        let (v1, v2) = (unit(turn(&r, v1)), unit(turn(&r, v2)));
        let b = w.rng.randi(0x10) as u32;
        let g = w.rng.randi(0x20) as u32;
        let rr = w.rng.randi(0x20) as u32;
        let c1 = (b + 0x20) << 16 | (g + 0x60) << 8 | 0x6000_0000 | (rr + 0x60);
        let s = type71::Spawn { pos: p, v1: [v1[0], v1[1], v1[2], 0.125], v2, c1, c2: 0x6080_8080, t: [quarter as i16, half as i16, quarter as i16], moby: id as u32 + 1 };
        fx::part71(w, &s);
    }
}

/// Level15 `0x2a3138`: the shimmer, one group a layer in draw order (module doc).
pub fn fx_quad_groups(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= LEN) else { return Vec::new() };
    let fade = p::i32(&m.pvars, FADE);
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
        let x = layer as f32 * 0.1;
        let st: [[f32; 2]; 4] = std::array::from_fn(|k| [ST[k][0] + wob[0], ST[k][1] + wob[1]]);
        let base = if fade == 0 { COLOURS[layer] } else { crate::particles::tween_color((fade as f32 / 60.0).to_bits(), COLOURS[layer], 0) };
        let col = (base & 0xff_ffff) | FIX;
        let corner = |c: [f32; 3]| to_world([c[0] + x, c[1], c[2]]);
        let sst = [st[1], [st[1][0], st[1][1] + 0.125], st[3], [st[3][0], st[3][1] + 0.125]];
        let quads = vec![
            FxQuad { corners: WALL.map(corner), st, rgba: [col; 4] },
            FxQuad { corners: STRIP.map(corner), st: sst, rgba: [col, FIX, col, FIX] },
        ];
        FxQuads { fx: 0x2c, additive: true, subtract: layer % 2 == 1, quads }
    }).collect()
}
