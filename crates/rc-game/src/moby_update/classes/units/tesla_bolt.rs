//! **The Tesla Claw's bolt as a level effect**: the level copies of the Tesla Claw's chain (`crate::hero::tesla`)
//! that run from a level class rather than the hero's item: the electrified water's bolt (level15 `0x2d79e8` reset,
//! `0x2d7bb0` build, `0x2d8710` draw; [`super::water_shock`]) and the Veldin boss beam's strands (level18 `0x2eacd8`
//! set-up, `0x2eaea0` sim, `0x2eb9f8` draw; [`super::veldin_shots`]). Read from the level15 and level18 decomps and
//! the level18 disassembly of the draw; native `f32`.
//!
//! **Against the claw's chain**: no targets and no hit tests; each step blends 15 % toward a random direction (the
//! claw blends toward its last step) and is always pulled onto the aim over the last points; the colour reads the
//! gold Tesla Claw flag (0x13e533: 0x7f2040, else 0x7f2020). The waves, the second chain, the four arcs and the
//! type-53 sparks are the claw's. Each copy keeps its own globals (one [`Bolt`] per copy, as the game).
//!
//! **The draw** (`0x2eb9f8`; level15 the same calls): the scroll step (−0.3 a tick, wrapped at −8), the strips of
//! the main chain, the second chain and the four arcs (`tesla::strip`: the helper `0x2ebb60` takes the core's width
//! from gp (0.05) for every strip, as the claw's `0x2d0748`), then the glow (`0x2ec130`, level15 `0x2d8e48`): two FX
//! 0xb quads facing the eye 0.3 toward it from the moby, `0.1 + randf_sym(0, 0.025)` and `0.35 + randf_sym(0, 0.05)`
//! across. The draw's two `rand` draws run in the frame part ([`frame`]).

use super::{FxQuad, FxQuads};
use crate::hero::tesla::{self, BeamQuad, Tesla, POINTS};
use crate::moby_update::classes::blaster_shot::rotate;
use crate::moby_update::creature::{self as c, fx, SPEED};
use crate::moby_update::services::{pv, World};
use crate::ps2v::Pf;

const WAVE_AMP: [f32; 2] = [0.03, 0.03];
/// The glow's colours (gp−0x4b60 on 18) and its FX texture.
pub const GLOW_WHITE: u32 = 0x307f_7f7f;
pub const FX_GLOW: usize = 0xb;

/// One copy's globals: the chain record, the camera and gravity the frame part saw, and the glow's jitters.
#[derive(Clone, Debug, Default)]
pub struct Bolt {
    pub t: Tesla,
    pub eye: [f32; 3],
    pub gravity: [f32; 3],
    /// The draw's `randf_sym(0, 0.025)` / `randf_sym(0, 0.05)` of the glow's two sizes.
    pub glow: [f32; 2],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: [f32; 3], k: f32) -> [f32; 3] { a.map(|x| x * k) }
fn len(a: [f32; 3]) -> f32 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }
fn unit(a: [f32; 3], l: f32) -> [f32; 3] { let n = len(a); if n == 0.0 { a } else { scale(a, l / n) } }
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }

/// The set-up (`0x2d79e8` / `0x2eacd8`), once per level load (the first-use flag): the chain straight from `start`
/// toward `end` in 20 steps of 15/20, the offsets and phases cleared, the arcs' timers −1. True when it ran.
pub fn reset(b: &mut Bolt, start: [f32; 3], end: [f32; 3]) -> bool {
    let ch = &mut b.t.chain;
    if !ch.reset { return false; }
    ch.reset = false;
    let step = unit(sub(end, start), 15.0 / 20.0);
    ch.main[0] = start;
    for i in 0..POINTS - 1 { ch.main[i + 1] = add(ch.main[i], step); }
    ch.zoff = [0.0; POINTS + 1];
    ch.side = [[0.0; 3]; POINTS];
    ch.zoff2 = [0.0; 22];
    ch.arc_timer = [-1; 4];
    ch.phase = [0.0; 3];
    true
}

/// The sim (`0x2d7bb0` / `0x2eaea0`): the chain from `start` along `end`'s direction turned 10°, onto `aim`; the
/// arcs at `start` (0, 1) and `end` (2, 3); the sparks at `start` and `aim` (module doc).
#[allow(clippy::too_many_lines)]
pub fn build(w: &mut World, b: &mut Bolt, start: [f32; 3], end: [f32; 3], aim: [f32; 3]) {
    let gold = w.hero.weapons.gold[3] != 0;
    let grav = crate::hero::physics::to_f32x3(w.hero.gravity_dir);
    let cam = crate::hero::physics::to_f32x3(w.camera);
    let mut seg = len(sub(start, aim)) * 0.05;
    let ch = &mut b.t.chain;
    ch.phase[0] = c::add_rot(ch.phase[0], f32::from_bits(0x3eb2_b8c2));
    ch.phase[1] = c::sub_rot(ch.phase[1], f32::from_bits(0x3f4d_87ac));
    ch.phase[2] = c::add_rot(ch.phase[2], f32::from_bits(0x3d56_7770));
    ch.color = if gold { 0x7f_2040 } else { 0x7f_2020 };
    let mut dirn = unit(sub(end, start), 1.0);
    let side = cross(grav, dirn);
    dirn = rotate(dirn, f32::from_bits(0x3e32_b8c2), side);
    let mut step = scale(dirn, seg);
    ch.main[0] = start;
    ch.second[0] = start;
    for i in 1..POINTS {
        let fi = i as f32;
        let amp = WAVE_AMP[0] + (WAVE_AMP[1] - WAVE_AMP[0]) * fi * 0.05;
        let wave = amp * c::add_rot(ch.phase[0] + fi * f32::from_bits(0x3c8e_fa35), 0.0).sin() + w.rng.randf_sym(0.0, 0.1);
        let old = ch.zoff[i];
        ch.zoff[i] = wave;
        ch.main[i][2] -= old;
        let (a, bb) = (w.rng.rand_angle(), w.rng.rand_angle());
        let mut r = fx::polar(seg, a, bb);
        r[2] *= 0.5;
        let d: [f32; 3] = std::array::from_fn(|k| step[k] + (r[k] - step[k]) * f32::from_bits(0x3e19_999a));
        let l = len(d);
        if l == 0.0 {
            ch.main[i] = ch.main[i - 1];
        } else {
            let d = scale(d, seg / l);
            ch.main[i] = add(ch.main[i - 1], d);
            step = d;
        }
        ch.main[i][2] += wave;
        let rel = sub(ch.main[i], ch.main[i - 1]);
        let to = sub(aim, ch.main[i]);
        let l = len(to);
        if l != 0.0 {
            let k = if i == 19 { seg = l; 0.5 } else if i < 16 { 0.1 } else { seg = l / (19 - i) as f32; 0.5 };
            let to = scale(to, seg / l);
            step = add(step, scale(sub(to, step), k));
        }
        if i < 10 {
            let p2 = c::add_rot(ch.phase[1] + fi * f32::from_bits(0x3f7e_adae), 0.0);
            let p3 = c::add_rot(ch.phase[2] + fi * f32::from_bits(0x3e56_7750), 0.0);
            let s3 = 0.8 * p3.sin();
            ch.second[i] = ch.main[i];
            let z2 = s3 * p2.sin() + w.rng.randf_sym(0.0, 0.1);
            ch.zoff2[i] = z2;
            ch.second[i][2] += z2;
            let ang = c::add_rot(c::atan(rel[0], rel[1]), std::f32::consts::FRAC_PI_2);
            let rr = 0.5 * p2.cos();
            ch.side[i - 1] = [ang.cos() * rr, ang.sin() * rr, 0.0];
            ch.second[i] = add(ch.second[i], ch.side[i - 1]);
        } else {
            ch.second[i] = ch.main[i];
            ch.second[i][2] += ch.zoff2[21 - i];
            ch.second[i] = add(ch.second[i], ch.side[20 - i]);
        }
    }
    let side_n = unit(cross(grav, dirn), 1.0);
    let up = unit(grav, 1.0);
    for k in 0..4 {
        let (base, al) = if k < 2 { (start, 0.4) } else { (end, 0.5) };
        let mut t = ch.arc_timer[k];
        let fired = crate::hero::guns::dec16(&mut t);
        ch.arc_timer[k] = t;
        if !fired {
            let mut d = [[0.0f32; 3]; 4];
            for (j, dj) in d.iter_mut().enumerate() {
                let r = sub(ch.arcs[k][j + 1], ch.arcs[k][j]);
                *dj = [r[0] + w.rng.randf_sym(0.0, 0.1), r[1] + w.rng.randf_sym(0.0, 0.1), r[2] + w.rng.randf_sym(0.0, 0.1)];
            }
            ch.arcs[k][0] = base;
            for (j, dj) in d.iter().enumerate() { ch.arcs[k][j + 1] = add(ch.arcs[k][j], *dj); }
            let tv = ch.arc_timer[k] as i32;
            let dd = (tv - [4, 2, 4, 2][k]).abs() as f32;
            ch.arc_alpha[k] = ((1.0 - dd / w.ticks(15) as f32) * 32.0) as i32 as i16;
        } else {
            ch.arc_timer[k] = w.ticks([8, 4, 8, 4][k]) as i16;
            ch.arc_alpha[k] = 0;
            let a = w.rng.randf_sym(f32::from_bits(0x3f06_0a92), f32::from_bits(0x3f86_0a92));
            let bb = w.rng.randf(-f32::from_bits(0x3f86_0a92), f32::from_bits(0x3e32_b8c2));
            let mut v = scale(dirn, al);
            v = rotate(v, bb, side_n);
            v = rotate(v, a, up);
            ch.arcs[k][0] = base;
            ch.arcs[k][1] = add(base, v);
            let mut sign = 1.0f32;
            for j in 0..3 {
                let cc = sub(ch.arcs[k][j + 1], cam);
                let ang = w.rng.randf(f32::from_bits(0x3e32_b8c2), f32::from_bits(0x3f75_be0b)) * sign;
                sign = -sign;
                v = rotate(v, ang, cc);
                ch.arcs[k][j + 2] = add(ch.arcs[k][j + 1], v);
            }
        }
    }
    let col = ch.color | 0x7f00_0000;
    let a = w.rng.randf_sym(f32::from_bits(0x3f06_0a92), f32::from_bits(0x3f86_0a92));
    let bb = w.rng.randf(-f32::from_bits(0x3f06_0a92), f32::from_bits(0x3f86_0a92));
    let mut v = scale(dirn, 0.09 * SPEED);
    v = rotate(v, bb, side_n);
    v = rotate(v, a, up);
    let mut spin = w.rng.rand_range(1, 8);
    if w.rng.randi(2) != 0 { spin = -spin; }
    let f = w.rng.randf(0.2, 0.6);
    let life = w.ticks(15);
    let vel = pv([v[0], v[1], v[2], 0.0]);
    let sp = pv([start[0], start[1], start[2], 0.0]);
    w.part53(Pf::f(f * 0.3), Pf::f(f), Pf::ZERO, sp, life, col, 0, spin as i8, vel);
    w.part53(Pf::f(f * 0.15), Pf::f(f * 0.5), Pf::ZERO, sp, life, 0x307f_7f7f, 0, -spin as i8, vel);
    let mut spin = w.rng.rand_range(1, 8);
    if w.rng.randi(2) != 0 { spin = -spin; }
    let f = w.rng.randf(0.15, 3.0);
    let life = w.ticks(12);
    w.part53(Pf::f(f * 0.1), Pf::f(f), Pf::ZERO, pv([aim[0], aim[1], aim[2], 0.0]), life, col, 0, spin as i8, pv([0.0; 4]));
}

/// The draw's frame part: the camera and gravity kept, the scroll step, the glow's two `rand` draws (`jitter`: the
/// copies that draw the glow).
pub fn frame(w: &mut World, b: &mut Bolt, jitter: bool) {
    b.eye = crate::hero::physics::to_f32x3(w.camera);
    b.gravity = crate::hero::physics::to_f32x3(w.hero.gravity_dir);
    b.t.count = POINTS as i16;
    let s = &mut b.t.chain.scroll;
    *s -= 0.3 * SPEED;
    if *s <= -8.0 { *s += 8.0; }
    if jitter { b.glow = [w.rng.randf_sym(0.0, 0.025), w.rng.randf_sym(0.0, 0.05)]; }
}

/// The draw's strips (module doc): the main chain, the second, the four arcs.
pub fn strips(b: &Bolt) -> Vec<BeamQuad> {
    let ch = &b.t.chain;
    let mut out = Vec::new();
    tesla::strip(&mut out, &ch.main, POINTS, tesla::CORE_W, 0.4, 0.9, 0x80, 0x40, ch.color, ch.scroll, b.eye);
    tesla::strip(&mut out, &ch.second, POINTS, tesla::CORE_W, 0.3, 0.1, 0x10, 0x20, ch.color, ch.scroll, b.eye);
    for k in 0..4 {
        let a = ch.arc_alpha[k].max(0) as u32;
        tesla::strip(&mut out, &ch.arcs[k], 5, tesla::CORE_W, 0.3, 0.2, a, a, ch.color, ch.scroll, b.eye);
    }
    out
}

/// The glow (`0x2ec130`) at `at`: two FX 0xb quads facing the eye 0.3 toward it, `sizes` across, `rgba` each.
pub fn glow(b: &Bolt, at: [f32; 3], sizes: [f32; 2], rgba: [u32; 2]) -> Vec<BeamQuad> {
    let toward = add(at, unit(sub(b.eye, at), 0.3));
    let f = unit(sub(b.eye, toward), 1.0);
    let r = unit(cross(f, b.gravity), -1.0);
    let u = cross(r, f);
    (0..2).map(|k| {
        let q = |y: f32, z: f32| add(toward, add(scale(r, y * sizes[k]), scale(u, z * sizes[k])));
        BeamQuad { fx: FX_GLOW, corners: [q(-1.0, 1.0), q(-1.0, -1.0), q(1.0, 1.0), q(1.0, -1.0)], st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]], rgba: [rgba[k]; 4] }
    }).collect()
}

/// Beam quads as additive effect groups, one per FX texture in first-use order.
pub fn groups(quads: Vec<BeamQuad>) -> Vec<FxQuads> {
    let mut groups: Vec<FxQuads> = Vec::new();
    for q in quads {
        let quad = FxQuad { corners: q.corners, st: q.st, rgba: q.rgba };
        match groups.iter_mut().find(|g| g.fx == q.fx) {
            Some(g) => g.quads.push(quad),
            None => groups.push(FxQuads { fx: q.fx, additive: true, subtract: false, quads: vec![quad] }),
        }
    }
    groups
}
