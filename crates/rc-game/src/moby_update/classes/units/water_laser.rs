//! Drek's fleet's underwater laser spinners, class 669: level17 0x2d77f0 (census U542 of the 2026-10-01 run, U540
//! of the 2026-09-30 run; 15 created instances), with its draw callback 0x2d7cf0. Three arms 120° apart spin about
//! the moby's x row (rate = +0x00·1.35°·dt a tick); each arm is a fan of five hit lines 5 long (95° + arm·120°,
//! −25°..+25° in 12.5° steps) in its y-z plane. Nothing runs unless Ratchet is in the water. Read from the level17
//! decomp and disassembly (0x2d77f0, 0x2d7cf0, the gp words 0x161b78..0x161bbc). Native `f32`.
//!
//! **Pvar block**: +0x00 the spin speed, +0x04 the spin step a tick (radians).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2d7830 | `0x20a4c0` (= L01 `0x22dea8`): Ratchet not in the water (groups 0x11 / 0x12, states 0x6a / 0x82 / 0x76 / 0x75) → nothing | [`update`] (`help::hero_in_water`) |
//! | state 0 | update distance 0xff, → 1, +0x04 = +0x00·1.35 (gp−0x5088)·π/180·dt | [`update`] |
//! | state 1 | rot.x = `fast_add_rotations(rot.x, +0x04)`; `RegisterDrawCallback(0x2d7cf0, m)` (list 1) | [`update`] (`Callback::UnitQuads`) |
//! | (every state) | Ratchet − pos, its dot with row 0, `FastVecNormalize(±1, row 0)`: written to a stack vector nothing reads | n/a (dead) |
//! | | the hit template: dir (stack, 1, 5627.92) (the two stack words are never written: 0 here [L]), attacker m, flags 0x10001, bytes 1 / 1, the class, damage 1, +0x20 1 | [`update`] |
//! | | R = `EulerToMatrix(rot)` (`0x1fcaf8`), columns normalised (`0x256a20`: a no-op on a rotation, up to rounding), row 3 (0, 0, 0, 1) | [`update`] (`moby_light::rotation_rows`) |
//! | | 12·row 2 (`0x1fc778`, m+0xe0) into the particle vector before the loop: overwritten before any read | n/a (dead) |
//! | arm k = 0..2, line j = 0..4 | b = add(add(95° (gp−0x5084), normalize(k·2.0943952)), normalize(25° (gp−0x5080)·j·0.5) − 25°); end = pos + R·`0x25ce50(5 (gp−0x507c), π/2, b)` | [`update`] |
//! | | \|pos − Ratchet\| < 30 → `CollLine_Fix(pos, end, 9, m, template)` | [`update`] (`services::line_hit_in`) |
//! | | `FastBSphereCheck(20, (pos, 5))` seen and j = 0 or 4: 8 (gp−0x5078) particles: s = ±1 by `rand() & 1`; life = max(1, trunc(scale(randf(2, 4)))) (gp−0x5070 · 0.5, 4); v = (end − pos)·s, L = \|v\|; start = s < 0 ? end : pos; start += v·`randf(0, 0.9)`; v = unit(v)·`randf(10·dt, L / (4·life))`; `PartType60Spawn(0.75 (gp−0x5074), start, v, 0xff004fff (gp−0x506c), life & 0xff, randi(255), 0)` | [`update`] (`World::part60`) |
//! | 0x2d7cf0 | scroll f = fmod(tick·0.125 (gp−0x5044)·dt, 1); FX 0x28, ALPHA 0x48 (gp−0x5068..−0x505c = 0, 2, 0, 1: additive), TEX1 0xff9000000260; the frame = `EulerToMatrix(rot)` at pos | [`fx_quads`] + `rc-engine` fx_draw |
//! | | two passes p = 0, 1, three arms each: corners `0x25ce50(0 / 5 / 5 / 5, π/2, normalize(95° + arm·2.0943952 + (0, −25°, +25°, 0)[i]))`, w 1; ST = table 0x1d3f70 ((0, 0) (0, 1) (0.25, 0) (0.25, 1)) + f·((1, 0.628) (0x161bc8), (1, −0.628) (0x161bc0))[p]; RGBA 0x2f7fff00 (gp−0x5048) | [`fx_quads`] |
//! | | no sound, light, save flag, bolt; other mobys only through the hit lines | n/a |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, add_rot, fx, pf, set_pf, DT};
use crate::moby_update::services::{self as sv, HitTemplate, Services, World};
use crate::ps2v::Pf;

use super::{FxQuad, FxQuads};

/// The update in the level17 class table.
pub const UPDATE_FN: u32 = 0x2d_77f0;
pub const REFERENCE_LEVEL: u32 = 17;
pub const CLASSES: [i16; 1] = [669];

const DEG: f32 = 0.017_453_292;
/// gp−0x5088: the spin factor.
pub const SPIN: f32 = 1.35;
/// gp−0x5084 / −0x5080: the arm's base angle and the fan's half-width (degrees).
pub const BASE_DEG: f32 = 95.0;
pub const FAN_DEG: f32 = 25.0;
/// gp−0x507c: the beam length.
pub const LEN: f32 = 5.0;
/// Hit lines only within this of Ratchet.
pub const HIT_RANGE: f32 = 30.0;
/// gp−0x5078..−0x506c: particles per edge line, their size, the life range's top, their colour.
pub const PARTS: i32 = 8;
pub const PART_SIZE: f32 = 0.75;
pub const PART_LIFE: f32 = 4.0;
pub const PART_RGBA: u32 = 0xff00_4fff;
/// 2π/3 as loaded (0x40060a92).
const THIRD: f32 = 2.094_395_2;
const HALF_PI: f32 = 1.570_796_4;
/// The draw callback's FX texture, colour and scroll rate (gp−0x5048 / −0x5044).
pub const FX: usize = 0x28;
pub const QUAD_RGBA: u32 = 0x2f7f_ff00;
pub const SCROLL: f32 = 0.125;
/// The frame counter `0x15f5cc`, kept for the draw callback (`Services::units`).
pub const COUNTER_WORD: u32 = 0x15_f5cc;

fn rows(rot: [f32; 4]) -> [[f32; 4]; 3] {
    rc_formats::moby_light::rotation_rows([rot[0], rot[1], rot[2]]).map(|r| r.map(f32::from_bits))
}

fn apply(r: &[[f32; 4]; 3], v: [f32; 4], p: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|k| if k == 3 { 1.0 } else { v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k] + p[k] })
}

fn norm(a: f32) -> f32 { sv::normalize_angle(Pf::f(a)).to_f32() }

/// Level17 0x2d77f0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 8 { return; }
    if !crate::help::hero_in_water(w.hero.state, w.hero.group) { return; }
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            w.mm(id).state = 1;
            let r = pf(w, id, 0) * SPIN * DEG * DT;
            set_pf(w, id, 4, r);
        }
        1 => {
            let r = add_rot(w.m(id).rotation[0], pf(w, id, 4));
            w.mm(id).rotation[0] = r;
            let tick = w.counter as u32;
            w.svc.units.set_word(COUNTER_WORD, tick);
            if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
        }
        _ => {}
    }
    let t = HitTemplate { dir: [Pf::ZERO, Pf::ZERO, Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x1_0001, b18: 1, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 };
    let (pos, rot) = (w.m(id).position, w.m(id).rotation);
    let r = rows(rot);
    for k in 0..3 {
        for j in 0..5 {
            let a = norm(FAN_DEG * DEG * j as f32 * 0.5) - FAN_DEG * DEG;
            let b = add_rot(add_rot(BASE_DEG * DEG, norm(k as f32 * THIRD)), a);
            let end = apply(&r, fx::polar(LEN, HALF_PI, b), pos);
            let h = super::hero_pos(w);
            if c::dist3(pos, h) < HIT_RANGE {
                let (p1, p2) = (pos.map(Pf::f), end.map(Pf::f));
                sv::line_hit_in(w.table, w.svc, w.classes, w.coll, p1, p2, 9, Some(id), &t);
            }
            let seen = fx::in_view(w, 20.0, pos, LEN);
            if seen && (j == 0 || j == 4) { particles(w, pos, end); }
        }
    }
}

fn particles(w: &mut World, pos: [f32; 4], end: [f32; 4]) {
    for _ in 0..PARTS {
        let s = if w.rng.rand() & 1 != 0 { 1.0 } else { -1.0 };
        let r = w.rng.randf(PART_LIFE * 0.5, PART_LIFE);
        let life = (sv::fl(w.svc.timing.scale(Pf::f(r))) as i32).max(1);
        let mut v = c::sub(end, pos).map(|x| x * s);
        let len = c::len3(v);
        let mut start = if s < 0.0 { end } else { pos };
        let k = w.rng.randf(0.0, f32::from_bits(0x3f66_6666));
        v = v.map(|x| x * k);
        for i in 0..3 { start[i] += v[i]; }
        let sp = w.rng.randf(DT * 10.0, len / (life * 4) as f32);
        let v = c::set_len3(v, sp);
        let rot = w.rng.randi(0xff) as u8;
        w.part60(PART_SIZE, start, v, PART_RGBA, (life & 0xff) as u16, rot, 0);
    }
}

/// The spinner's quads (0x2d7cf0) for moby `id`.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    let tick = svc.units.word(COUNTER_WORD) as i32;
    let f = (tick as f32 * (SCROLL * DT)) % 1.0;
    let r = rows(m.rotation);
    const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [0.25, 0.0], [0.25, 1.0]];
    const S: [f32; 2] = [1.0, 0.628_318_55];
    const T: [f32; 2] = [1.0, -0.628_318_55];
    let off = [0.0, -FAN_DEG * DEG, FAN_DEG * DEG, 0.0];
    let mut quads = Vec::with_capacity(6);
    for p in 0..2 {
        let st = ST.map(|q| [q[0] + f * S[p], q[1] + f * T[p]]);
        for arm in 0..3 {
            let corners = std::array::from_fn(|i| {
                let len = if i == 0 { 0.0 } else { LEN };
                let a = norm(BASE_DEG * DEG + arm as f32 * THIRD + off[i]);
                let q = apply(&r, fx::polar(len, HALF_PI, a), m.position);
                [q[0], q[1], q[2]]
            });
            quads.push(FxQuad { corners, st, rgba: [QUAD_RGBA; 4] });
        }
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}
