//! The fleet's sliding laser emitters, class 99 (level 17, 35 created instances): level17 0x2a8da0 (census U541; was
//! U535). An emitter shuttles between its placed point and a target (the centre of cuboid +0x44, or the placed point
//! of the linked moby +0x68), or stands still; while Ratchet is in its cuboid +0x40 (or always, −1) it fires a beam
//! 12 units along its x row: a hit line (damage 1, flags 0x10001, pushing sideways away from the beam), 15 type-79
//! sparks a tick and one camera-facing quad (the draw callback 0x2a95b8) whose width pulses with its animation. Read
//! from the level17 decomp (0x2a8da0, 0x2a95b8, the anim-key reads 0x2495b8 / 0x249618, the spark spawner 0x26fb60)
//! and the disassembly of the spark loops (0x2a93a8..0x2a94b0: the second loop starts from the beam's end). Native
//! `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block**: +0x00 the placed point (v4), +0x10 the beam's start, +0x20 its end, +0x30 the clipped reach (the
//! world-line end − position), +0x40 the hero cuboid (−1: always on), +0x44 the target cuboid (−1: none), +0x48 the
//! speed, +0x4c the beam's half width, +0x50 the shuttle t, +0x54 its step, +0x58 the first key time of sequence 0,
//! +0x5c (last − first)/2π, +0x60 a rider (written 0 by the init; no code writes it otherwise), +0x68 the linked moby
//! (−1: none).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2a8da0 | state 3 → return (no code sets it) | [`update`] |
//! | state 0 | +0x58 = key time of seq 0 frame 0 (0x2495b8: class's sequence header frame 0 +4 ÷ 16), +0x5c = (last frame's − that)/2π (0x249618) | [`update`] ([`key`]) |
//! | | target cuboid −1 and link −1 → state 1; else placed = position, step = speed·dt / `vec_distance`(placed, the link's position or the cuboid's centre +0x30) → state 2 | [`update`] |
//! | | rider +0x60 = 0, width +0x4c = 0.1; to = position + row 0 · 13 (`matrix_mul_vec3`); `CollLine_Fix(pos, to, 2)` hit → to = the hit point; +0x30 = to − position (then the common tail, beam off) | [`update`] ([`reach`]) |
//! | state 1 | beam on when +0x40 = −1 or Ratchet (0x13f3d0) in cuboid +0x40 (`PointInCuboid` 0x25a210 = L01 0x274820); +0x30 kept | [`update`] |
//! | state 2 | t += step; t > 1 → t = 1, t < 0 → t = 0, each with step = −step; position = lerp(placed, target, t) (0x1f9a40; the target: the cuboid's centre, or the linked moby's **pvar +0x00**, its own placed point) | [`update`] |
//! | | +0x40 ≠ −1 and Ratchet not in it → beam off, +0x30 kept; else +0x30 = the clipped reach as in state 0, beam on | [`update`] ([`reach`]) |
//! | tail | v = row 0 · 12; end +0x20 = position + v (rider +0x60 = 0); the rider branch (end pulled back by 0.6 (gp−0x5768 − 0.2), the rider moved by v) is never taken: n/a | [`update`] |
//! | beam on | width = cos(π + (`MobyAnimKeyTime` (0x2494c0 = L01 0x263920) − +0x58)/+0x5c)·0.02 + 0.09; start +0x10 = position − unit(v)·0.2 | [`update`] (`ground::key_time`) |
//! | | `FastBSphereCheck(32, lerp(start, end, ½) with radius \|+0x30\|/2)` culled → nothing more | [`update`] (`fx::in_view`) |
//! | | template (0x254218 = L01 0x26e808): dir = (Ratchet − position) minus its part along unit(row 0), attacker self, flags 0x10001, damage 1, +0x20 = 1 (+0x18..+0x1b stale stack: 0 [L]); `CollLine_Fix(pos, end, 9, self, tmpl)` | [`update`] (`services::line_hit_in`) |
//! | | ×7: u = `randf(0, 0.9)`; p = start + (+0x30)·u (0x2211a0); speed = `randf(10·dt, \|+0x30\|/20)`; `0x26fb60(self, p, unit(+0x30)·speed)` (a type-79 spark riding the emitter) | [`update`] (`World::part79`) |
//! | | ×7: the same from the end: p = end − (+0x30)·u, speed = `randf(10·dt, −\|+0x30\|/20)` (moving back) | [`update`] |
//! | | one free spark at the end: v = −unit(+0x30)·`randf(0.5·dt, dt)` + three `randf(−0.01, 0.01)` jitters; `0x26fb60(0, end, v)` | [`update`] |
//! | | `RegisterDrawCallback(0x2a95b8, self)` (0x1f6448 = L01 0x21afe0) — registered whenever the beam is on, also when culled | [`update`] (`Callback::UnitQuads`) |
//! | 0x2a95b8 | e = start − end; a = unit((start − cam) × e)·width, b = unit((end − cam) × e)·width (`FastVecCross(out, x, y)` = y × x; camera 0x1676c0, the one the update saw [L]); quad start − a, start + a, end − b, end + b; RGBA 0x80243278, FX 0xe, ALPHA 0x48 (additive), ST (0, 0), (0, 1), (1, 0), (1, 1) (0x1e6320) | [`fx_quads`] |
//! | | no sound, light, save flag, bolt | n/a |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, ground, DT};
use crate::moby_update::services::{self as sv, HitTemplate, Services, World};
use crate::ps2v::Pf;
use rc_formats::volumes::ShapeKind;

use super::{FxQuad, FxQuads};

/// The update in the level17 class table.
pub const UPDATE_FN: u32 = 0x2a_8da0;
pub const REFERENCE_LEVEL: u32 = 17;
pub const CLASSES: [i16; 1] = [99];
/// The camera the beam's quad faces (`0x1676c0` on 17), kept by the update (three words; `Services::units`).
pub const CAMERA_WORD: u32 = 0x16_76c0;

pub mod pv {
    pub const HOME: usize = 0x00;
    pub const START: usize = 0x10;
    pub const END: usize = 0x20;
    pub const REACH: usize = 0x30;
    pub const HERO_CUBOID: usize = 0x40;
    pub const CUBOID: usize = 0x44;
    pub const SPEED: usize = 0x48;
    pub const WIDTH: usize = 0x4c;
    pub const T: usize = 0x50;
    pub const STEP: usize = 0x54;
    pub const KEY0: usize = 0x58;
    pub const KEY_SPAN: usize = 0x5c;
    pub const RIDER: usize = 0x60;
    pub const LINK: usize = 0x68;
}

/// The world probe's and the beam's reach along row 0.
pub const PROBE: f32 = 13.0;
pub const BEAM: f32 = 12.0;
/// Sparks per side a tick.
pub const SPARKS: usize = 7;
pub const BEAM_RGBA: u32 = 0x8024_3278;
pub const FX: usize = 0xe;
const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
const TWO_PI: f32 = 6.283_185_5;
const PI: f32 = f32::from_bits(0x4049_0fdb);
const NINE_TENTHS: f32 = f32::from_bits(0x3f66_6666);

type V = [f32; 4];

fn unit(v: V) -> V { let n = c::len3(v); if n == 0.0 { [0.0, 0.0, 0.0, v[3]] } else { [v[0] / n, v[1] / n, v[2] / n, v[3]] } }

/// 0x2495b8 / 0x249618: the time (÷ 16) of sequence `seq`'s first or last frame.
fn key(w: &World, id: MobyId, seq: u8, last: bool) -> f32 {
    let Some(class) = w.classes.anim(w.m(id).o_class) else { return 0.0 };
    let Some(s) = class.sequences.get(seq as usize).and_then(|s| s.as_ref()) else { return 0.0 };
    let f = if last { s.header.frame_count.wrapping_sub(1) } else { 0 };
    class.frame(seq, f).map_or(0.0, |fr| fr.header.time as i32 as f32 * 0.0625)
}

/// The probe of states 0 / 2: `position + row 0 · 13` clipped by a world line (flags 2); +0x30 = that − position.
fn reach(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let r0 = w.m(id).rows[0];
    let to = [pos[0] + r0[0] * PROBE, pos[1] + r0[1] * PROBE, pos[2] + r0[2] * PROBE, pos[3] + r0[3] * PROBE];
    let to = match w.line(sv::pv(pos), sv::pv(to), 2, None) {
        None => to,
        Some(h) => sv::fv(h.point),
    };
    c::set_pv4(w, id, pv::REACH, c::sub(to, pos));
}

/// The target of the shuttle: cuboid +0x44's centre, or the linked moby's position (`for_step`) / pvar +0x00.
fn target(w: &World, id: MobyId, for_step: bool) -> Option<V> {
    let cub = c::pi32(w, id, pv::CUBOID);
    if cub != -1 {
        let s = w.svc.volumes.shape(ShapeKind::Cuboid, cub)?;
        let m = s.matrix[3];
        return Some(m);
    }
    let link = usize::try_from(c::pi32(w, id, pv::LINK)).ok()?;
    let m = w.table.mobys.get(link)?;
    if for_step { return Some(m.position); }
    (m.pvars.len() >= 0x10).then(|| sv::pvar::v4f(&m.pvars, 0))
}

fn hero_in(w: &World, id: MobyId) -> bool {
    let cub = c::pi32(w, id, pv::HERO_CUBOID);
    cub == -1 || w.in_cuboid(w.hero_point(), cub)
}

/// Level17 0x2a8da0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::LINK + 4 { return; }
    let on = match w.m(id).state {
        3 => return,
        0 => {
            let k0 = key(w, id, 0, false);
            c::set_pf(w, id, pv::KEY0, k0);
            let k1 = key(w, id, 0, true);
            c::set_pf(w, id, pv::KEY_SPAN, (k1 - k0) / TWO_PI);
            if c::pi32(w, id, pv::CUBOID) == -1 && c::pi32(w, id, pv::LINK) == -1 {
                w.mm(id).state = 1;
            } else {
                let pos = w.m(id).position;
                c::set_pv4(w, id, pv::HOME, pos);
                // A missing cuboid / moby: the game reads whatever is there [L]; the port keeps the step at 0.
                let d = target(w, id, true).map_or(0.0, |t| c::dist3(pos, t));
                let step = if d == 0.0 { 0.0 } else { (c::pf(w, id, pv::SPEED) * DT) / d };
                c::set_pf(w, id, pv::STEP, step);
                w.mm(id).state = 2;
            }
            c::set_pi32(w, id, pv::RIDER, 0);
            c::set_pf(w, id, pv::WIDTH, 0.1);
            reach(w, id);
            false
        }
        1 => hero_in(w, id),
        2 => {
            let mut t = c::pf(w, id, pv::T) + c::pf(w, id, pv::STEP);
            if 1.0 < t || t < 0.0 {
                t = if 1.0 < t { 1.0 } else { 0.0 };
                let s = -c::pf(w, id, pv::STEP);
                c::set_pf(w, id, pv::STEP, s);
            }
            c::set_pf(w, id, pv::T, t);
            if let Some(b) = target(w, id, false) {
                let a = c::pv4(w, id, pv::HOME);
                w.mm(id).position = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
            }
            if !hero_in(w, id) {
                false
            } else {
                reach(w, id);
                true
            }
        }
        _ => false,
    };
    let pos = w.m(id).position;
    let r0 = w.m(id).rows[0];
    let v = [r0[0] * BEAM, r0[1] * BEAM, r0[2] * BEAM, r0[3] * BEAM];
    let end = c::add(pos, v);
    c::set_pv4(w, id, pv::END, end);
    if !on { return; }
    let k = ground::key_time(w, id);
    let a = c::add_rot(PI, (k - c::pf(w, id, pv::KEY0)) / c::pf(w, id, pv::KEY_SPAN));
    c::set_pf(w, id, pv::WIDTH, a.cos() * 0.02 + 0.09);
    let start = c::sub(pos, c::scale(unit(v), 0.2));
    c::set_pv4(w, id, pv::START, start);
    let reach = c::pv4(w, id, pv::REACH);
    let l = c::len3(reach);
    let mid = std::array::from_fn(|k| start[k] + (end[k] - start[k]) * 0.5);
    if fx::in_view(w, 32.0, mid, l * 0.5) {
        let h = w.hero.pos.map(|x| f32::from_bits(x.0));
        let d = c::sub(h, pos);
        let along = c::dot3(r0, d);
        let d = c::sub(d, c::scale(unit(r0), along));
        let tmpl = HitTemplate { dir: sv::pv(d), attacker: Some(id), flags: 0x1_0001, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 1 };
        sv::line_hit_in(w.table, w.svc, w.classes, w.coll, sv::pv(pos), sv::pv(end), 9, Some(id), &tmpl);
        let dir = unit(reach);
        for _ in 0..SPARKS {
            let u = w.rng.randf(0.0, NINE_TENTHS);
            let p = c::add(c::scale(reach, u), start);
            let sp = w.rng.randf(DT * 10.0, l / 20.0);
            let vel = c::scale(dir, sp);
            w.part79(Some(id), p, [vel[0], vel[1], vel[2]]);
        }
        for _ in 0..SPARKS {
            let u = w.rng.randf(0.0, NINE_TENTHS);
            let p = c::sub(end, c::scale(reach, u));
            let sp = w.rng.randf(DT * 10.0, -l / 20.0);
            let vel = c::scale(dir, sp);
            w.part79(Some(id), p, [vel[0], vel[1], vel[2]]);
        }
        let s = w.rng.randf(DT * 0.5, DT);
        let mut vel = c::scale(dir, -s);
        for x in vel.iter_mut().take(3) { *x += w.rng.randf(-0.01, 0.01); }
        w.part79(None, end, [vel[0], vel[1], vel[2]]);
    }
    let cam = w.camera_point();
    for (k, x) in cam.iter().enumerate() { w.svc.units.set_word(CAMERA_WORD + 4 * k as u32, x.to_bits()); }
    if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
}

/// `FastVecCross(out, x, y)` = y × x.
fn cross_yx(x: [f32; 3], y: [f32; 3]) -> [f32; 3] { [y[1] * x[2] - y[2] * x[1], y[2] * x[0] - y[0] * x[2], y[0] * x[1] - y[1] * x[0]] }

/// Level17 0x2a95b8 for the emitter `id` (module doc): one quad along its beam.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < pv::LINK + 4 { return None; }
    let p = |o: usize| sv::pvar::v4f(&m.pvars, o);
    let (s, e) = (p(pv::START), p(pv::END));
    let wdt = sv::pvar::ff(&m.pvars, pv::WIDTH);
    let cam: [f32; 3] = std::array::from_fn(|k| f32::from_bits(svc.units.word(CAMERA_WORD + 4 * k as u32)));
    let d3 = |a: V, b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let along = [s[0] - e[0], s[1] - e[1], s[2] - e[2]];
    let side = |x: [f32; 3]| {
        let v = cross_yx(x, along);
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n == 0.0 { [0.0; 3] } else { v.map(|q| q * wdt / n) }
    };
    let (a, b) = (side(d3(s, cam)), side(d3(e, cam)));
    let corners = [
        [s[0] - a[0], s[1] - a[1], s[2] - a[2]],
        [s[0] + a[0], s[1] + a[1], s[2] + a[2]],
        [e[0] - b[0], e[1] - b[1], e[2] - b[2]],
        [e[0] + b[0], e[1] + b[1], e[2] + b[2]],
    ];
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads: vec![FxQuad { corners, st: ST, rgba: [BEAM_RGBA; 4] }] })
}
