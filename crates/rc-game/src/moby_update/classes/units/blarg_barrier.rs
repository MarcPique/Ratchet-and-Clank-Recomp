//! **Blarg's energy barriers, class 1123** (level06 `0x3049f8` with its hurt lines `0x305000` and its draw callback
//! `0x305170`, which feeds the shared strip emitter `0x216b88`; 4 placed; census U245; the name is descriptive [L]).
//! Four stacked crackling beams (0.6, 2.3, 4.0 and 5.7 above it, ±4.75 long before its 1.6 scale) humming while on;
//! two lines across it hurt for 6. Each beam's 30 points re-roll every 45 ticks, one beam at a time, and drift in
//! between; each fades in and out over its 45 ticks. A barrier linked to another 1123 (+0xf10) waits off until that
//! one goes down; a switch moby (+0xf14) with its command byte set turns it off: the hum stops, sound 1, and the beams
//! burst into 50 sparks each. Read from the level06 decomp and disassembly (the strip emitter's VU0 code) and its words
//! gp−0x4a40..−0x49d0. Native `f32`.
//!
//! **Pvars** (0xf1c): beam k's 30 points at +0x1e0·k, its 28 drift velocities (points 1..28) at +0x790 + 0x1e0·k,
//! +0xf00 the four beams' re-roll countdowns (0, 11, 23, 34 at init), +0xf10 the linked barrier (−1 none), +0xf14 the
//! switch moby (−1 none), +0xf18 the hum's voice.
//!
//! | state | what | port |
//! |---|---|---|
//! | 0 | scale = class scale · 2.5; the emitter's counts 45 / 22 (0x162234 / 0x162238); countdowns 0, 11, 23, 34; no voice; linked → 1, no collision; else 2 | [`update`] |
//! | 1 | the linked moby still a 1123 below state 3 → wait; else 2, its class's collision | [`update`] |
//! | 2 | the hum (`PlayClassSound(0, 4)` when not alive); the hurt lines; the scroll (gp−0x4a08) += 2·dt (wrapped); per beam: countdown 0 → 30 points along x from the running x (−4.75, +9.5/30 each; the run is shared by the beams re-rolled in one tick), 30 draws `rand_vec(0, 0.4·dt)`, velocity i = (r_i + r_{i+1} + r_{i+2}) / 3, countdown 45; else points 1..28 += their velocities, countdown − 1; within 48 of the camera the draw; the switch's command byte → no collision, 3, the hum released, sound 1 | [`update`] |
//! | 3 | per beam (its frame: Euler · 1.6 · the beam's Euler 0x1fe890, at 0.6 + 1.7·k up): 50 sparks along its x (`randf_sym(0, 7.125)`), colour `FastTweenColor(randf(0, 1), 0x80802020, 0x80802080)`, size `randf(0.125, 0.333)`, `PartType60Spawn(size, p, rand_vec(1.2·dt, 2.1·dt), colour, ticks(trunc(randf(30, 60))), randi(255), 0)`; → 4 | [`update`] |
//! | `0x305000` | the template `0x268668(6, tmpl, m, 0x10000)` (= `0x26e7d8`); `CollLine_Fix` (flags 0) from rows·(6.5, ±1.75, 0.5) + pos to rows·(−6.5, ±1.75, 0.5) + pos | [`hurt`] (`services::line_hit_in`) |
//! | `0x305170` / `0x216b88` | FX 0xe, ALPHA additive; per beam frame all four strips (16 in all), the record (45, 22, countdowns, 0x80c04070, 0x00ff0000, 30 points, 0.25, the scroll): two vertices per point (p ∓ 0.05 on x, y, z) through the frame, ST (scroll + 0.25·(j + 1), 0 / 1), colour lerped from 0x00ff0000 to 0x80c04070 by the countdown (t ≤ 22: t/22, else 1 − (t − 22)/23) | [`fx_quads`], [`strip_quads`] |

use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::creature::{self as c, V};
use crate::moby_update::services::{pf, pv as v4, pvar as p, HitTemplate, Services, World};
use crate::ps2v::Pf;
use super::{FxQuad, FxQuads};

pub const REFERENCE_LEVEL: u32 = 6;
pub const UPDATE_FN: u32 = 0x30_49f8;
pub const DRAW_FN: u32 = 0x30_5170;
pub const CLASSES: [i16; 1] = [1123];
/// The emitter's record counts (0x162234 / 0x162238): the re-roll period and its midpoint.
pub const PERIOD: i32 = 0x2d;
pub const MID: i32 = 0x16;
pub const POINTS: usize = 30;
/// gp−0x4a08 (0x1621f8): the strips' scroll, shared by the barriers.
const SCROLL_KEY: u32 = 0x16_21f8;
/// gp−0x4a04 (2): its speed; −0x4a00 (0.4): the drift; −0x49fc (9.5): the beams' length; −0x49f8 (1.6): the frame's
/// scale; −0x49f0 (2.5): the class scale's factor; −0x4a0c (0.25): the strip's S step.
const SCROLL_SPEED: f32 = 2.0;
const DRIFT: f32 = f32::from_bits(0x3ecc_cccd);
const LENGTH: f32 = 9.5;
const FRAME_SCALE: f32 = f32::from_bits(0x3fcc_cccd);
const S_STEP: f32 = 0.25;
/// gp−0x4a20 / −0x4a1c: the strips' colours; −0x4a18 / −0x4a14 / −0x4a10: the hurt lines' corner.
const COLOURS: (u32, u32) = (0x80c0_4070, 0x00ff_0000);
const LINE: [f32; 3] = [6.5, 1.75, 0.5];
/// 0x1fe890: the beams' Euler angles (yaw π, 0, π, 0).
const BEAM_YAW: [f32; 4] = [std::f32::consts::PI, 0.0, std::f32::consts::PI, 0.0];
pub const FX: usize = 0xe;

/// Pvar offsets (module doc).
pub mod pv {
    pub const POINTS: usize = 0x000;
    pub const BEAM: usize = 0x1e0;
    pub const VEL: usize = 0x790;
    pub const COUNT: usize = 0xf00;
    pub const LINK: usize = 0xf10;
    pub const SWITCH: usize = 0xf14;
    pub const VOICE: usize = 0xf18;
    pub const SIZE: usize = 0xf1c;
}

/// Four beams of 30 points with their drift velocities and re-roll countdowns (1123 keeps them in its pvars, the
/// gates 1035 in level globals 0x1db0f0 / 0x1db870 / 0x161e40).
#[derive(Clone, Debug, PartialEq)]
pub struct Beams {
    pub pts: [[V; POINTS]; 4],
    pub vel: [[V; 28]; 4],
    pub count: [i32; 4],
}

impl Default for Beams {
    fn default() -> Self { Beams { pts: [[[0.0; 4]; POINTS]; 4], vel: [[[0.0; 4]; 28]; 4], count: [0, 0xb, 0x17, 0x22] } }
}

impl Beams {
    /// One tick (1123's state 2, the gates' `0x2f7000`): a beam at 0 re-rolls along `axis` from the running value (−length/2,
    /// plus length/30 a point; the run continues into the next beam re-rolled this tick), its 30 `rand_vec(0, drift)`
    /// draws smoothed into 28 velocities, countdown `period`; else points 1..28 drift, countdown − 1. `axis` is the
    /// component the run is written to: 0 (x) for 1123's beams, 1 (y) for the gates' (`0x2f7000` stores it at +4,
    /// across their frames' x rotations: the nine frames make the gate's star).
    pub fn step(&mut self, w: &mut World, length: f32, drift: f32, period: i32, axis: usize) {
        let step = length / 30.0;
        let mut x = -(length * 0.5);
        for k in 0..4 {
            if self.count[k] == 0 {
                let mut r = [[0.0f32; 4]; POINTS];
                for (i, ri) in r.iter_mut().enumerate() {
                    // `0x217300` stores vf0, then the run's component.
                    self.pts[k][i] = [0.0, 0.0, 0.0, 1.0];
                    self.pts[k][i][axis] = x;
                    x += step;
                    let q = w.rng.rand_vec(0.0, drift);
                    *ri = [q[0], q[1], q[2], 0.0];
                }
                for i in 0..28 {
                    self.vel[k][i] = c::scale(c::add(c::add(r[i], r[i + 1]), r[i + 2]), f32::from_bits(0x3eaa_7efa));
                }
                self.count[k] = period;
            } else {
                for i in 0..28 { self.pts[k][i + 1] = c::add(self.pts[k][i + 1], self.vel[k][i]); }
                self.count[k] -= 1;
            }
        }
    }

    fn from_pvars(pv: &[u8]) -> Self {
        let mut b = Beams::default();
        for k in 0..4 {
            for i in 0..POINTS { b.pts[k][i] = p::v4f(pv, self::pv::POINTS + self::pv::BEAM * k + 0x10 * i); }
            for i in 0..28 { b.vel[k][i] = p::v4f(pv, self::pv::VEL + self::pv::BEAM * k + 0x10 * i); }
            b.count[k] = p::i32(pv, self::pv::COUNT + 4 * k);
        }
        b
    }

    fn to_pvars(&self, pv: &mut [u8]) {
        for k in 0..4 {
            for i in 0..POINTS { p::set_v4f(pv, self::pv::POINTS + self::pv::BEAM * k + 0x10 * i, self.pts[k][i]); }
            for i in 0..28 { p::set_v4f(pv, self::pv::VEL + self::pv::BEAM * k + 0x10 * i, self.vel[k][i]); }
            p::set_i32(pv, self::pv::COUNT + 4 * k, self.count[k]);
        }
    }

    /// `0x216b88` through one frame: the four strips (each with its own countdown's colour).
    pub fn quads(&self, rows: [[f32; 3]; 3], at: [f32; 3], colours: (u32, u32), s_step: f32, s_off: f32, out: &mut Vec<FxQuad>) {
        for k in (0..4).rev() {
            let s = Strip { period: PERIOD, mid: MID, count: self.count[k], colours, s_step, s_off };
            strip_quads(&self.pts[k], rows, at, &s, out);
        }
    }
}

/// One strip record of the emitter `0x216b88`: its 45 / 22 counts, the countdown, the two colours, the S step and
/// offset.
#[derive(Clone, Copy, Debug)]
pub struct Strip {
    pub period: i32,
    pub mid: i32,
    pub count: i32,
    pub colours: (u32, u32),
    pub s_step: f32,
    pub s_off: f32,
}

/// `0x216b88`: the strip of `pts` through `frame` (`rows` and the translation), two vertices per point (p ∓ 0.05).
pub fn strip_quads(pts: &[V], rows: [[f32; 3]; 3], at: [f32; 3], s: &Strip, out: &mut Vec<FxQuad>) {
    let k = if s.count - s.mid < 1 { s.count as f32 / s.mid as f32 } else { 1.0 - (s.count - s.mid) as f32 / (s.period - s.mid) as f32 };
    let lerp = |a: u32, b: u32| -> u32 {
        let ch = |x: u32, sh: u32| ((x >> sh) & 0xff) as f32;
        (0..4).fold(0u32, |acc, i| {
            let sh = 8 * i;
            let v = (ch(b, sh) + (ch(a, sh) - ch(b, sh)) * k) as u32 & 0xff;
            acc | (v << sh)
        })
    };
    let rgba = lerp(s.colours.0, s.colours.1);
    let xf = |q: [f32; 3]| -> [f32; 3] { std::array::from_fn(|i| rows[0][i] * q[0] + rows[1][i] * q[1] + rows[2][i] * q[2] + at[i]) };
    let vert = |q: V, d: f32| xf([q[0] + d, q[1] + d, q[2] + d]);
    for j in 0..pts.len().saturating_sub(1) {
        let (a, b) = (pts[j], pts[j + 1]);
        let (s0, s1) = (s.s_off + s.s_step * (j + 1) as f32, s.s_off + s.s_step * (j + 2) as f32);
        out.push(FxQuad {
            corners: [vert(a, -0.05), vert(a, 0.05), vert(b, -0.05), vert(b, 0.05)],
            st: [[s0, 0.0], [s0, 1.0], [s1, 0.0], [s1, 1.0]],
            rgba: [rgba; 4],
        });
    }
}

/// `R_moby · s · R(e)` as rows (the frames of `0x305170` / state 3).
pub fn frame(rot: V, s: f32, e: [f32; 3]) -> [[f32; 3]; 3] {
    let m = rc_formats::moby_light::rotation_rows([rot[0], rot[1], rot[2]]).map(|r| r.map(f32::from_bits));
    let b = rc_formats::moby_light::rotation_rows(e).map(|r| r.map(f32::from_bits));
    std::array::from_fn(|i| std::array::from_fn(|j| s * (b[i][0] * m[0][j] + b[i][1] * m[1][j] + b[i][2] * m[2][j])))
}

fn beam_frame(rot: V, pos: V, k: usize) -> ([[f32; 3]; 3], [f32; 3]) {
    (frame(rot, FRAME_SCALE, [0.0, 0.0, BEAM_YAW[k]]), [pos[0], pos[1], pos[2] + k as f32 * 1.7 + 0.6])
}

/// `0x305000` (module doc).
fn hurt(w: &mut World, id: MobyId) {
    let t = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(id), flags: 0x1_0000, b18: 0, b19: 0, h1a: 0, damage: pf(6.0), w20: 0 };
    let (rows, pos) = (w.m(id).rows, w.m(id).position);
    let xf = |q: [f32; 3]| -> V { std::array::from_fn(|i| if i == 3 { pos[3] } else { rows[0][i] * q[0] + rows[1][i] * q[1] + rows[2][i] * q[2] + pos[i] }) };
    for y in [LINE[1], -LINE[1]] {
        let a = xf([LINE[0], y, LINE[2]]);
        let b = xf([-LINE[0], y, LINE[2]]);
        crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, v4(a), v4(b), 0, Some(id), &t);
    }
}

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len())
}

/// Level06 `0x3049f8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    match w.m(id).state {
        0 => {
            let o = w.m(id).o_class;
            w.mm(id).scale = super::class_scale(w, o) * 2.5;
            for (k, n) in [0, 0xb, 0x17, 0x22].into_iter().enumerate() { c::set_pi32(w, id, pv::COUNT + 4 * k, n); }
            c::set_pi32(w, id, pv::VOICE, -1);
            if c::pi32(w, id, pv::LINK) != -1 {
                w.mm(id).state = 1;
                w.mm(id).has_collision = false;
            } else {
                w.mm(id).state = 2;
            }
        }
        1 => {
            if link(w, id, pv::LINK).is_some_and(|l| w.m(l).o_class == 0x463 && w.m(l).state < 3) { return; }
            w.mm(id).state = 2;
            w.mm(id).has_collision = w.m(id).has_class;
        }
        2 => {
            let v = c::pi32(w, id, pv::VOICE);
            if !w.sound_alive(v, id) {
                let s = w.play_sound(0, 4, id);
                c::set_pi32(w, id, pv::VOICE, s);
            }
            hurt(w, id);
            let mut sc = f32::from_bits(w.svc.units.word(SCROLL_KEY)) + SCROLL_SPEED * c::DT;
            if 1.0 < sc { sc -= 1.0; }
            w.svc.units.set_word(SCROLL_KEY, sc.to_bits());
            let mut b = Beams::from_pvars(&w.m(id).pvars);
            b.step(w, LENGTH, DRIFT * c::DT, PERIOD, 0);
            b.to_pvars(&mut w.mm(id).pvars);
            let cam = w.camera.map(|x| f32::from_bits(x.0));
            if c::dist3(c::pos(w, id), cam) < 48.0 {
                if let Some(r) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitQuads(r), id); }
            }
            let Some(sw) = link(w, id, pv::SWITCH) else { return };
            if w.m(sw).cmd == 0 { return; }
            w.mm(id).has_collision = false;
            w.mm(id).state = 3;
            let v = c::pi32(w, id, pv::VOICE);
            if w.sound_alive(v, id) { w.release_sound(v, id); }
            c::set_pi32(w, id, pv::VOICE, -1);
            w.play_sound(1, 0, id);
        }
        3 => {
            let (rot, pos) = (w.m(id).rotation, w.m(id).position);
            for k in 0..4 {
                let (rows, at) = beam_frame(rot, pos, k);
                let axis = c::set_len3([rows[0][0], rows[0][1], rows[0][2], 0.0], 1.0);
                for _ in 0..50 {
                    let s = w.rng.randf_sym(0.0, LENGTH * 0.75);
                    let pt = c::add(c::scale(axis, s), [at[0], at[1], at[2], 1.0]);
                    let f = w.rng.randf(0.0, 1.0);
                    let col = crate::particles::tween_color(f.to_bits(), 0x8080_2020, 0x8080_2080);
                    let size = w.rng.randf(0.125, f32::from_bits(0x3eaa_7efa));
                    let life = w.rng.randf(30.0, 60.0) as i32;
                    let v = w.rng.rand_vec(1.2 * c::DT, f32::from_bits(0x4006_6666) * c::DT);
                    let life = w.ticks(life);
                    let seed = w.rng.randi(0xff);
                    w.part60(size, pt, [v[0], v[1], v[2], 0.0], col, life as u16, seed as u8, 0);
                }
            }
            w.mm(id).state = 4;
        }
        _ => {}
    }
}

/// `0x305170` (module doc; draw only).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < pv::SIZE { return None; }
    let s_off = f32::from_bits(svc.units.word(SCROLL_KEY));
    let b = Beams::from_pvars(&m.pvars);
    let mut quads = Vec::new();
    for k in 0..4 {
        let (rows, at) = beam_frame(m.rotation, m.position, k);
        b.quads(rows, at, COLOURS, S_STEP, s_off, &mut quads);
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}
