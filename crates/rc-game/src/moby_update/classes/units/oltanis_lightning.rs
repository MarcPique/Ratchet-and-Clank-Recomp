//! **Oltanis's arrival and lightning, class 684** (level14 `0x2ed280`; census U501, one placed). On the first visit
//! it plays the arrival scene (scene 0, flag 0x13d3f0; the ship hidden through it) and then strikes lightning: every
//! `ticks(180)` a bolt from the sky (350 high, within 20 of the point) down to one of the first points of its path
//! (+0x00) whose sphere (10 above it, radius 10) is in view within 255 (up to five candidates, one at random); on
//! later visits it does nothing (the game's flag test). Read from the level14 decomp and disassembly; native `f32`.
//!
//! * **A strike** (`0x2ed530`, level data 0x161cb4.. / 0x1dfd80.. / 0x1e0000..): 40 points from the sky point down,
//!   each wandering sideways `randf_sym(0, 3)` from the last (within ±8; the first from −1, the 40th straight, the
//!   39th between its neighbours), lighting up one after another over `ticks(10)` (their timers start at
//!   `−ticks(10)·i/40` and count up); three branches from points 5..21 of 16 steps 1..10 long, turning up to 40° a
//!   step on their side (within 80°), their timers from their points'; it lasts `ticks(rand_range(56, 90))` (a thunder
//!   sound, `0x294028(0)`: the level def 2 + 0, `World::play_level_def`); after the first `ticks(10)` it flickers (one sine over 20 ticks: the alpha cut
//!   `(255 + 128·sin) & 0xff`) and, while the cut is under 50, sparks fly off the strike point (three pairs of type
//!   53 a tick). The last five points ease straight.
//! * **The draw** (`0x2edc18`): the bolt as camera-facing ribbons (FX 0x2d grey 0x7f7f7f, FX 0x2c red 0x7f4040 at
//!   half the alpha and four times the width), 5 wide narrowing to 0.5 (`2t − t²`); the branches as ribbons sideways
//!   in the bolt's frame, three quarters as wide, ×0.95 a step; all additive.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2ed280` | 684 | [`update`] |
//! | `0x2ed530` / `0x2ee6b0` | the strike, its sparks | [`strike`] |
//! | `0x2edc18` | the draw | [`frame`], [`fx_quad_groups`] |
//!
//! [L] The draw is built in the frame part from the camera it sees.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, SPEED};
use crate::cinematic::EngineRequest;
use crate::moby_update::services::{pv, Services, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2e_d280;
pub const DRAW_FN: u32 = 0x2e_dc18;
pub const CLASSES: [i16; 1] = [684];
const FLAG_ARRIVAL: usize = story::flag_index(0x13_d3f0);

/// Level14 gp words (0x161c50..0x161cb0).
const REST: i32 = 180;
const SKY: f32 = 350.0;
const FADE: i32 = 10;
const COL_A: u32 = 0x7f_7f7f;
const COL_B: u32 = 0x7f_4040;
const W_END: f32 = 0.5;
const W_START: f32 = 5.0;

/// The strike's level data (module doc).
#[derive(Clone, Debug, Default)]
pub struct Lightning {
    /// gp 0x161c50 / 0x161c58: the rest timer, a strike running.
    rest: i32,
    active: bool,
    /// gp 0x161c6c: the flicker's phase so far.
    flicker: f32,
    /// 0x161cb4: the strike timer; 0x161cec its start; 0x161cf0 `1 / (start − ticks(10))`.
    timer: i32,
    start: i32,
    inv: f32,
    /// 0x161cc0: the strike point; 0x161cd0 the sky point.
    ground: [f32; 4],
    sky: [f32; 4],
    /// 0x161ce0 the alpha cut, 0x161ce2 the flicker, 0x161ce4 its sine phase.
    cut: u8,
    wave: i16,
    phase: f32,
    owner: Option<MobyId>,
    sound: bool,
    /// 0x1dfd80: the 40 points (local: x along the bolt, z sideways); 0x1e0000 their timers.
    pts: Vec<[f32; 4]>,
    timers: Vec<i16>,
    /// 0x161cf8: the branches' roots; 0x1e0050 their steps (x, z); 0x1e0350 their timers.
    roots: [i32; 3],
    steps: [[[f32; 2]; 16]; 3],
    btimers: [[i16; 16]; 3],
    /// This frame's quads.
    quads: Vec<FxQuads>,
}

fn st<'a>(w: &'a mut World<'_>) -> &'a mut Lightning { &mut w.svc.units.oltanis_lightning }
/// `FastDecTimer` on a word: whether it is out (it was 0, or reached 0).
fn dec(t: &mut i32) -> bool {
    if *t == 0 { return true; }
    *t = (*t).max(1) - 1;
    *t < 1
}

/// Level14 `0x2ed280` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            if w.svc.level == 14 { w.hero_fields_mut().clank_hidden = Some(1); }
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            st(w).active = false;
            st(w).rest = 0;
        }
        1 => {
            if w.svc.game_mode == 0 && story::flag(w, FLAG_ARRIVAL) == 0 {
                crate::cinematic::start_scene(w, 0, false);
                story::set_flag(w, FLAG_ARRIVAL, 1);
                w.mm(id).state = 2;
                w.svc.cinematic.requests.push(EngineRequest::ShipHidden(true));
            }
        }
        2 => {
            if w.svc.game_mode != 2 {
                w.mm(id).state = 3;
                w.svc.cinematic.requests.push(EngineRequest::ShipHidden(false));
            }
        }
        _ => {
            let mut t = st(w).rest;
            let out = dec(&mut t);
            st(w).rest = t;
            if !out { return; }
            if !st(w).active { pick(w, id); }
            strike(w, id);
            if st(w).timer == 0 {
                let t = w.ticks(REST);
                let s = st(w);
                s.rest = t;
                s.active = false;
            }
        }
    }
}

/// The strike point: up to five of the path's points in view (module doc).
fn pick(w: &mut World, id: MobyId) {
    let Some(pts) = usize::try_from(c::pi32(w, id, 0)).ok().and_then(|i| w.svc.splines.get(i)).map(|s| s.iter().map(|q| q.map(f32::from_bits)).collect::<Vec<_>>()) else { return };
    let mut seen = Vec::new();
    for (i, q) in pts.iter().enumerate() {
        if 5 <= seen.len() { break; }
        if w.view.is_some_and(|v| !v.culled(255.0, [q[0], q[1], q[2] + 10.0, 10.0])) { seen.push(i); }
    }
    if seen.is_empty() { return; }
    let k = w.rng.randi(seen.len() as i32) as usize;
    let q = pts[seen[k]];
    let a = w.rng.randf_sym(0.0, 20.0);
    let b = w.rng.randf_sym(0.0, 20.0);
    let s = st(w);
    s.ground = [q[0], q[1], q[2], 1.0];
    s.sky = [q[0] + a, q[1] + b, SKY, 1.0];
    w.mm(id).position = [q[0], q[1], q[2], 1.0];
}

/// `0x2ed530` (module doc).
pub fn strike(w: &mut World, id: MobyId) {
    let fade = w.ticks(FADE);
    if !st(w).active {
        let (g, s) = (st(w).ground, st(w).sky);
        let len = c::len3(c::sub(s, g));
        let seg = len / 40.0;
        let mut pts = vec![[0.0f32; 4]; 40];
        let mut timers = vec![0i16; 40];
        let mut prev = -1.0f32;
        for (i, p) in pts.iter_mut().enumerate() {
            *p = [seg * i as f32, 0.0, 0.0, 1.0];
            if i < 0x27 {
                let z = (prev + w.rng.randf_sym(0.0, 3.0)).clamp(-8.0, 8.0);
                p[2] = z;
            }
            prev = p[2];
            timers[i] = (-(fade as f32) * (i as f32 / 40.0) + 0.0) as i16;
        }
        pts[38] = std::array::from_fn(|k| pts[37][k] + (pts[39][k] - pts[37][k]) * 0.5);
        let n = w.rng.rand_range(0x38, 0x5a);
        let t = w.ticks(n);
        let inv = 1.0 / (t - fade) as f32;
        let mut sign = if w.rng.randi(2) != 0 { 1.0f32 } else { -1.0 };
        let mut roots = [0i32; 3];
        let mut steps = [[[0.0f32; 2]; 16]; 3];
        let mut btimers = [[0i16; 16]; 3];
        for k in 0..3 {
            let r = w.rng.rand_range(5, 0x16);
            roots[k] = r;
            btimers[k][0] = timers[r as usize];
            let l = w.rng.randf(1.0, 10.0);
            let mut a = w.rng.randf(f32::from_bits(0x3f06_0a92), f32::from_bits(0x3fb2_b8c2)) * sign;
            steps[k][0] = [l * a.cos(), l * a.sin()];
            sign = -sign;
            for j in 1..16 {
                let l = w.rng.randf(1.0, 10.0);
                let d = w.rng.randf_sym(0.0, 40.0) * 0.017_453_292;
                let mut b = c::add_rot(a, d);
                if 1.396_263_4 < b.abs() || (0.0 < a && b < 0.0) || (a < 0.0 && 0.0 < b) { b = c::add_rot(a, -d); }
                steps[k][j] = [l * b.cos(), l * b.sin()];
                btimers[k][j] = timers[((r + j as i32) as usize).min(39)];
                a = b;
            }
        }
        let s = st(w);
        s.sound = true;
        s.active = true;
        s.flicker = 0.0;
        s.owner = Some(id);
        s.cut = 0;
        s.pts = pts;
        s.timers = timers;
        s.timer = t;
        s.start = t;
        s.inv = inv;
        s.roots = roots;
        s.steps = steps;
        s.btimers = btimers;
    }
    if st(w).owner != Some(id) { return; }
    {
        let s = st(w);
        let mut t = s.timer;
        dec(&mut t);
        s.timer = t;
        for i in 0..s.timers.len() {
            s.timers[i] = (s.timers[i] + 1).min(fade as i16);
            if 0x22 < i {
                let f = (i - 0x23) as f32 / 5.0;
                s.pts[i][2] += (0.0 - s.pts[i][2]) * f;
            }
        }
        for b in s.btimers.iter_mut() {
            for t in b.iter_mut() { *t = (*t + 1).min(fade as i16); }
        }
    }
    let (timer, start) = (st(w).timer, st(w).start);
    if timer < start - fade {
        if w.ticks(0x3c) < start {
            let s = st(w);
            if s.flicker < std::f32::consts::TAU {
                s.wave = (s.phase.sin() * 128.0) as i16;
                s.phase = c::add_rot(s.phase, f32::from_bits(0x3ea0_d97c));
                s.flicker += f32::from_bits(0x3ea0_d97c) * SPEED;
            } else {
                s.wave = 0;
            }
        } else {
            st(w).wave = 0;
        }
        let s = st(w);
        let base = (0.0 * s.inv * -255.0 + 255.0) as i32;
        s.cut = (base + s.wave as i32) as u8;
        if s.sound {
            s.sound = false;
            w.play_level_def(0, 0, id);
        }
        if st(w).cut < 0x32 { sparks(w); }
    } else {
        st(w).cut = 0;
    }
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

/// `0x2ee6b0(strike point, 3)`: three pairs of type-53 sparks off it (the point jittered 0.15 each time).
fn sparks(w: &mut World) {
    for _ in 0..3 {
        let size = w.rng.randf(5.0, 20.0);
        let r = w.rng.randi(2);
        let spin = if r != 0 { r } else { r - 1 };
        let a = w.rng.rand_angle();
        let pitch = w.rng.randf(f32::from_bits(0x3eb2_b8c2), f32::from_bits(0x3fc4_9809));
        let speed = w.rng.randf(0.05, 0.5);
        let v = crate::targeting::polar(speed, a, pitch);
        for k in 0..3 {
            let j = w.rng.randf_sym(0.0, f32::from_bits(0x3e19_999a));
            st(w).ground[k] += j;
        }
        let p = st(w).ground;
        let vel = pv([v[0], v[1], v[2], 0.0]);
        let life = w.ticks(0x3c);
        w.part53(Pf::f(size * 0.1), Pf::f(size), Pf::f(f32::from_bits(0x3c13_74bc)), pv(p), life, 0x7f7f_2020, 0, spin as i8, vel);
        let life = w.ticks(0x3c);
        w.part53(Pf::f(size * 0.07), Pf::f(size * 0.7), Pf::f(f32::from_bits(0x3c13_74bc)), pv(p), life, 0x7f7f_7f7f, 1, -spin as i8, vel);
    }
}

/// `FastVecCross(out, a, b)` = b × a.
fn fast_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [b[1] * a[2] - b[2] * a[1], b[2] * a[0] - b[0] * a[2], b[0] * a[1] - b[1] * a[0]] }
fn unit(v: [f32; 3]) -> [f32; 3] { let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); if n == 0.0 { v } else { v.map(|x| x / n) } }
fn alpha(t: i16, fade: i32, cut: u8) -> u32 { ((((t as f32 / fade as f32) * 255.0) as i32) - cut as i32).clamp(0, 255) as u32 }
/// `0x26cc00(−1, 0, 1, 0, t)` = `2t − t²`.
fn ease(t: f32) -> f32 { 2.0 * t - t * t }

/// The draw's frame part (module doc): every quad, from the camera.
pub fn frame(w: &mut World, _id: MobyId) {
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let fade = w.ticks(FADE);
    let s = &w.svc.units.oltanis_lightning;
    if s.pts.len() < 40 { return; }
    let (g, sk) = ([s.ground[0], s.ground[1], s.ground[2]], [s.sky[0], s.sky[1], s.sky[2]]);
    let r0 = unit([g[0] - sk[0], g[1] - sk[1], g[2] - sk[2]]);
    let r1 = unit(fast_cross(r0, [1.0, 0.0, 0.0]));
    let r2 = fast_cross(r1, r0);
    let world = |q: [f32; 4]| -> [f32; 3] { std::array::from_fn(|k| r0[k] * q[0] + r1[k] * q[1] + r2[k] * q[2] + sk[k]) };
    let off = |p: [f32; 3], d: [f32; 3], k: f32| -> [f32; 3] { std::array::from_fn(|i| p[i] + d[i] * k) };
    let facing = |p: [f32; 3]| -> [f32; 3] { fast_cross(unit([cam[0] - p[0], cam[1] - p[1], cam[2] - p[2]]), r0) };
    let st4 = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];
    let (mut qa, mut qb) = (Vec::new(), Vec::new());
    // The bolt (from point 1; the last point repeats its predecessor, unlit).
    let mut w0 = W_START;
    let mut p = world(s.pts[1]);
    let mut side = facing(p);
    let mut a_prev = alpha(s.timers[0], fade, s.cut);
    let (mut v0, mut v1, mut u0, mut u1) = (off(p, side, w0), off(p, side, -w0), off(p, side, w0 * 4.0), off(p, side, -w0 * 4.0));
    for i in 1..0x28 {
        if i != 0x27 { p = world(s.pts[i]); }
        side = facing(p);
        let (n0, n1, m0, m1) = (off(p, side, w0), off(p, side, -w0), off(p, side, w0 * 4.0), off(p, side, -w0 * 4.0));
        let a = if i != 0x27 { alpha(s.timers[i], fade, s.cut) } else { 0 };
        let (ca, cb) = (|x: u32| COL_A | x << 24, |x: u32| COL_B | (x >> 1) << 24);
        qa.push(FxQuad { corners: [v0, v1, n0, n1], st: st4, rgba: [ca(a_prev), ca(a_prev), ca(a), ca(a)] });
        qb.push(FxQuad { corners: [u0, u1, m0, m1], st: st4, rgba: [cb(a_prev), cb(a_prev), cb(a), cb(a)] });
        (v0, v1, u0, u1, a_prev) = (n0, n1, m0, m1, a);
        w0 = W_START + (W_END - W_START) * ease(i as f32 / 40.0);
    }
    // The branches, sideways in the bolt's frame.
    for k in 0..3 {
        let root = s.roots[k].clamp(0, 39) as usize;
        let mut wb = (W_START + (W_END - W_START) * ease(root as f32 / 40.0)) * 0.75;
        let mut a_prev = alpha(s.btimers[k][0], fade, s.cut);
        let mut q = s.pts[root];
        let mut cur = world(q);
        q[0] += s.steps[k][0][0];
        q[2] += s.steps[k][0][1];
        let mut next = world(q);
        let (mut v0, mut v1, mut u0, mut u1) = (off(cur, r2, wb), off(cur, r2, -wb), off(cur, r2, wb * 4.0), off(cur, r2, -wb * 4.0));
        for j in 1..16 {
            cur = next;
            q[0] += s.steps[k][j][0];
            q[2] += s.steps[k][j][1];
            next = world(q);
            let (n0, n1, m0, m1) = (off(cur, r2, wb), off(cur, r2, -wb), off(cur, r2, wb * 4.0), off(cur, r2, -wb * 4.0));
            let a = alpha(s.btimers[k][j], fade, s.cut);
            let (ca, cb) = (|x: u32| COL_A | x << 24, |x: u32| COL_B | (x >> 1) << 24);
            qa.push(FxQuad { corners: [v0, v1, n0, n1], st: st4, rgba: [ca(a_prev), ca(a_prev), ca(a), ca(a)] });
            qb.push(FxQuad { corners: [u0, u1, m0, m1], st: st4, rgba: [cb(a_prev), cb(a_prev), cb(a), cb(a)] });
            (v0, v1, u0, u1, a_prev) = (n0, n1, m0, m1, a);
            wb *= 0.95;
        }
    }
    let quads = vec![FxQuads { fx: 0x2d, additive: true, subtract: false, quads: qa }, FxQuads { fx: 0x2c, additive: true, subtract: false, quads: qb }];
    w.svc.units.oltanis_lightning.quads = quads;
}

/// Level14 `0x2edc18` (module doc).
pub fn fx_quad_groups(_table: &MobyTable, svc: &Services, _id: MobyId) -> Vec<FxQuads> { svc.units.oltanis_lightning.quads.clone() }
