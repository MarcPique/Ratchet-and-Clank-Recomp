//! **The weather emitter** (class 1400; level08 `0x307cf0` (census U317), level12 `0x308be0` and level14 `0x306390`
//! (U433): one source built per level, the same code but for addresses): Batalia's and Oltanis' rain, Hoven's snow
//! (the code's own test: level 12 → snow). It keeps the weather globals the drops read (`Particles::weather`: the
//! camera's move this tick 0x160250, the floor 0x160260) and spawns the drops around the camera every tick: rain
//! streaks (type 0, `0x272cb8`) and their splashes (`SpawnImpactSparks`, type 1), or snow flakes (type 73,
//! `0x27f5f0`). Read from the level08 decomp; the level data words (L08 0x162340.., L12 0x162000.., L14 0x162210..)
//! hold the same values on the three levels and no code writes them: constants here.
//!
//! **The data words** (L08): +0x00 0.5 the slowest far drop, +0x04 10 the drops' start above the camera, +0x08 18 the
//! main ring's radius, +0x0c / +0x10 / +0x14 −1 (overrides of the count range and the slowest drop: off), +0x18 1 the
//! far drops' kind, +0x1c 5 the floor below the camera; written: +0x20 / +0x24 the wind (0x162360), +0x30 the wind's
//! timer (seconds), +0x34 the drop count.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x307b90(snow)` | the wind: rain (5, 2, 8, 4) / snow (2.5, 20, 3, 2) as (s, t0, n1, n0); x, y = `randf(−s·dt, s·dt)`; the timer `randf(scale(t0), scale(10))`; the count = `trunc(|wind.xy| / |(s·dt, s·dt)|·(n1 − n0) + n0)` | [`wind`] |
//! | `0x307cf0` state 0 | +0x30 = 0xff, → 2, the wind | [`update`] |
//! | state 2 | the timer (`0x211b08`: −dt, out → the wind again); 0x160250 = camera − pvar +0x30; the main ring (count drops 18 around the point 16 ahead of the camera, 10 above it; within 8 of the camera fast and kind 0, else slower with distance (−0.15 / −0.2 a unit past 8, at least 0.5) and kind 1; the floor 5 below the camera or the grid's height above it; only below the start height); the near ring (1.4·count, 8 around the camera, kind 0); rain: the splashes (1.4·count 30 around the point 30 ahead, half that 8 around the point 4 ahead); pvar +0x30 = the camera | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;
use crate::particles::{type00, type01, type73};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x30_7cf0;
pub const CLASSES: [i16; 1] = [1400];
pub const HOVEN_LEVEL: u32 = 12;
pub const HOVEN_FN: u32 = 0x30_8be0;
pub const OLTANIS_LEVEL: u32 = 14;
pub const OLTANIS_FN: u32 = 0x30_6390;

/// The data words (L08 addresses; their values on every level).
const SLOWEST: f32 = 0.5;
const ABOVE: f32 = 10.0;
const RADIUS: f32 = 18.0;
const FAR_KIND: i32 = 1;
const BELOW: f32 = 5.0;
/// The written words, kept by their L08 address.
pub const WIND_X: u32 = 0x16_2360;
pub const WIND_Y: u32 = 0x16_2364;
pub const TIMER: u32 = 0x16_2370;
const COUNT: u32 = 0x16_2374;
/// dt (0x15ed6c / 0x15ed7c, NTSC).
const DT: f32 = 1.0 / 60.0;
const TAU: f32 = f32::from_bits(0x40c9_0fdb);

fn word_f(w: &World, a: u32) -> f32 { f32::from_bits(w.svc.units.word(a)) }
fn set_word_f(w: &mut World, a: u32, v: f32) { w.svc.units.set_word(a, v.to_bits()); }
fn scale(w: &World, x: f32) -> f32 { w.svc.timing.scale(Pf::f(x)).to_f32() }

/// `0x307b90(snow)` (module doc).
fn wind(w: &mut World, snow: bool) {
    let (s, t0, n1, n0) = if snow { (2.5f32, 20.0f32, 3.0f32, 2.0f32) } else { (5.0, 2.0, 8.0, 4.0) };
    let x = w.rng.randf(-(s * DT), s * DT);
    set_word_f(w, WIND_X, x);
    let y = w.rng.randf(-(s * DT), s * DT);
    set_word_f(w, WIND_Y, y);
    let (a, b) = (scale(w, t0), scale(w, 10.0));
    let t = w.rng.randf(a, b);
    set_word_f(w, TIMER, t);
    let m = (s * DT * (s * DT) * 2.0).sqrt();
    let n = ((x * x + y * y).sqrt() / m * (n1 - n0) + n0) as i32;
    w.svc.units.set_word(COUNT, n as u32);
}

/// A random point within `r` of `p` (xy), the game's two angles: x by the cosine of one, y by the sine of the next.
fn around(w: &mut World, p: [f32; 4], r: f32) -> [f32; 4] {
    let a = w.rng.randf(0.0, TAU);
    let x = p[0] + a.cos() * w.rng.randf(0.0, r);
    let b = w.rng.randf(0.0, TAU);
    let y = p[1] + b.sin() * w.rng.randf(0.0, r);
    [x, y, p[2], p[3]]
}

fn drop(w: &mut World, snow: bool, floor: f32, p: [f32; 4], kind: i32, v: [f32; 4]) {
    let ty = if snow { 73 } else { 0 };
    *w.svc.fx.part_spawns.entry(ty).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else { return };
    let r = if snow { type73::spawn(sys, w.rng, floor, p, kind, v) } else { type00::spawn(sys, w.rng, floor, p, kind, v) };
    if r.is_none() { w.svc.fx.part_failed += 1; }
}

fn splash(w: &mut World, p: [f32; 4]) {
    let Some(sys) = w.particles.as_deref_mut() else { return };
    type01::impact_sparks_here(sys, p);
}

/// The grid's height at `p` (`0x278020`; none outside the grid [L]).
fn height(w: &World, p: [f32; 4]) -> Option<f32> { w.particles.as_deref().and_then(|s| s.grid.as_deref()).and_then(|g| g.height(p[0], p[1])) }

/// Level08 `0x307cf0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { w.mm(id).pvars.resize(0x40, 0); }
    let snow = w.svc.level == 12;
    let (spread, fall_k, fast) = if snow { (DT, 0.2f32, 5.0f32) } else { (DT + DT, 0.15, 18.0) };
    let slowest = SLOWEST;
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.state = 2;
            wind(w, snow);
        }
        2 => tick(w, id, snow, spread, fall_k, fast, slowest),
        _ => {}
    }
    let cam = w.camera.map(|x| x.to_f32());
    c::set_pv4(w, id, 0x30, cam);
}

fn tick(w: &mut World, id: MobyId, snow: bool, spread: f32, fall_k: f32, fast: f32, slowest: f32) {
    let t = word_f(w, TIMER);
    if t <= DT {
        set_word_f(w, TIMER, 0.0);
        wind(w, snow);
    } else {
        set_word_f(w, TIMER, t - DT);
    }
    let cam = w.camera.map(|x| x.to_f32());
    let last = c::pv4(w, id, 0x30);
    let step = [cam[0] - last[0], cam[1] - last[1], cam[2] - last[2], cam[3] - last[3]];
    if let Some(sys) = w.particles.as_deref_mut() { sys.weather.wind = step; }
    let fwd = w.camera_rows[0];
    let ahead = |d: f32| [cam[0] + fwd[0] * d, cam[1] + fwd[1] * d, cam[2] + ABOVE, cam[3]];
    let top = cam[2] + ABOVE;
    let floor0 = cam[2] - BELOW;
    let (wx, wy) = (word_f(w, WIND_X), word_f(w, WIND_Y));
    let n = w.svc.units.word(COUNT) as i32;
    // 0x160260: the floor (the game stores it before each drop's height test; the same value every time).
    if let Some(sys) = w.particles.as_deref_mut() { sys.weather.floor = floor0; }
    // The main ring around the point 16 ahead.
    let base = ahead(16.0);
    for _ in 0..n.max(0) {
        let p = around(w, base, RADIUS);
        let d = ((p[0] - cam[0]).powi(2) + (p[1] - cam[1]).powi(2)).sqrt();
        let vx = w.rng.randf(-spread, spread) + wx;
        let vy0 = w.rng.randf(-spread, spread);
        let speed = if d <= 8.0 { fast } else { (fast - (d - 8.0) * fall_k).max(slowest) };
        let v = [vx, vy0 + wy, -(speed * DT), 0.0];
        let Some(h) = height(w, p) else { continue };
        if h < top {
            let floor = floor0.max(h);
            let kind = if d <= 8.0 { 0 } else { FAR_KIND };
            drop(w, snow, floor, p, kind, v);
        }
    }
    // The near ring around the camera.
    let n2 = (n as f32 * 1.4) as i32;
    let base = [cam[0], cam[1], top, cam[3]];
    for _ in 0..n2.max(0) {
        let vx = w.rng.randf(-spread, spread) + wx;
        let vy = w.rng.randf(-spread, spread) + wy;
        let v = [vx, vy, -(fast * DT), 0.0];
        let p = around(w, base, 8.0);
        let Some(h) = height(w, p) else { continue };
        if h < top { drop(w, snow, floor0.max(h), p, 0, v); }
    }
    if snow { return; }
    // The splashes: 30 around the point 30 ahead, then half as many 8 around the point 4 ahead.
    let base = ahead(30.0);
    for _ in 0..n2.max(0) {
        let p = around(w, base, 30.0);
        splash(w, p);
    }
    let base = ahead(4.0);
    for _ in 0..(n2 / 2).max(0) {
        let p = around(w, base, 8.0);
        splash(w, p);
    }
}
