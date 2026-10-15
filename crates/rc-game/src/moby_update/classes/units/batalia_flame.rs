//! **Batalia's burning wrecks' flames, class 671** (level08 `0x2f70a0`, its draw `0x2f5e58`; census U286; 3 placed:
//! #795–#797). A sheet of fire drawn as a grid of quads (+8 columns × +9 rows), its corners swaying with sines of the
//! VSync count and eight random phases, with embers (type 43), sparks (type 22) and, with +0xa bit 4, a point light
//! wandering over the sheet.
//!
//! **Update** (`0x2f70a0`): 0: the light slot (+0x13b) −1, the ember timer = +0xf3, the light's target (+0x140) the
//! position, +0x70 / +0x74 0 → 1, the phases (+0x90 / +0xb0, 8 each) `randf(+0x88, 1)`, → 1. Every tick: the scale
//! `class · max(+0, +4) / 4`; the fade +0x154 (1 within the draw distance less +0x150 of the camera, 0 past it, linear
//! between); out of view (not drawn and `FastBSphereCheck(255, (pos, +0x137))` fails) or past the draw distance: the
//! light freed; else the draw callback and the light (`WritePointLight_B(+0x137, +0x138 / 100, fade·+0x134..+0x136 /
//! 100, +0x140)`, or its colour rewritten); +0xa |= 2.
//!
//! **Draw** (`0x2f5e58`): the phases walk toward their targets by +0x88·+0x8c (a new target `randf(1 − +0x88, 1)`
//! on arrival); the corners (−w/2, 0, +0x80 + 0.01), (w/2, 0, +0x84 + 0.01), (−w/2, 0, 0), (w/2, 0, 0) turned by the
//! moby's rotation; each grid point the row's left-to-right step plus three sways (along the row, down the
//! columns, out of the sheet) and the rise `(h − base)·√sin(πx)·(1 − y)` (module code); its alpha divided by
//! `trunc(1 / (((1 − a) + a·√sin(πx)³)·((1 − b) + b·y)))` (a, b: +0x158, +0x15c: the tip and the sides fade out);
//! ST scrolling by the fraction of sines of the count; FX +0xf (+0x28 without +0xa bit 1; +0xc + count / (+0xe + 1)
//! mod +0xd with +0xd); ALPHA from +0x3c..+0x3f, colour +0x10..+0x13. With +0xa bit 2 (once a tick): every +0xf3
//! ticks +0xf2 embers, every +0x121 ticks +0x120 sparks, from random points along the base; the light drifts toward
//! its target at +0x13c a tick (a new random target on the sheet once there); +0xa ^= 2.
//!
//! Read from the level08 decomp and disassembly (the corners). [L] The VSync count 0x15f3f8 is the draw tick here
//! (`DrawCallbacks::tick`). The ember's second float (`0x27b450`'s +0x30, never read) is 0. Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2f70a0` | the update (module doc) | [`update`] |
//! | `0x2f5e58` | the phases, embers, sparks and the light | [`frame`] (`Callback::UnitFrame`) |
//! | `0x2f5e58` | the grid's quads | [`fx_quads`] (`Callback::UnitQuads`) |
//! | `0x2f5d98(x, p)` | the phases' lerp: lanes `trunc(8x) mod 8` and the next, by the fraction | `phase` |

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, V};
use crate::moby_update::services::{euler_rows, pvar as p, pv, Services, World};
use crate::point_lights::PointLight;
use std::f32::consts::PI;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x2f_70a0;
pub const DRAW_FN: u32 = 0x2f_5e58;
pub const CLASSES: [i16; 1] = [671];

const LEN: usize = 0x160;
/// 0x15ed60 (the speed, 1.0).
const SPEED: f32 = 1.0;

mod o {
    pub const W: usize = 0x00;
    pub const H: usize = 0x04;
    pub const COLS: usize = 0x08;
    pub const ROWS: usize = 0x09;
    pub const FLAGS: usize = 0x0a;
    pub const FRAME0: usize = 0x0c;
    pub const FRAMES: usize = 0x0d;
    pub const FRAME_DIV: usize = 0x0e;
    pub const ALPHA_REG: usize = 0x3c;
    pub const PHASE_STEP: usize = 0x88;
    pub const PHASE_RATE: usize = 0x8c;
    pub const PHASES: usize = 0x90;
    pub const TARGETS: usize = 0xb0;
    pub const EMBER_T: usize = 0xf8;
    pub const SPARK_T: usize = 0x122;
    pub const RADIUS: usize = 0x137;
    pub const SLOT: usize = 0x13b;
    pub const DRIFT: usize = 0x13c;
    pub const TARGET: usize = 0x140;
    pub const FADE_LEN: usize = 0x150;
    pub const FADE: usize = 0x154;
}

fn f(m: &[u8], i: usize) -> f32 { p::ff(m, 4 * i) }

/// `0x2f5d98(x, p)`.
fn phase(m: &[u8], x: f32) -> f32 {
    let i = (x * 8.0) as i32;
    let fr = x * 8.0 - i as f32;
    let a = i.rem_euclid(8) as usize;
    let b = (i + 1).rem_euclid(8) as usize;
    f(m, 0x24 + a) * (1.0 - fr) + f(m, 0x24 + b) * fr
}

fn wrap(x: f32) -> f32 { crate::moby_update::classes::flyer::wrap_frac(x) }
fn frac(x: f32) -> f32 { x - x.trunc() }
fn add(a: V, b: V) -> V { c::add(a, b) }
fn sub(a: V, b: V) -> V { c::sub(a, b) }
fn scale(a: V, k: f32) -> V { a.map(|x| x * k) }
fn norm(v: V, l: f32) -> V { c::set_len3(v, l) }
/// `FastVecCross` (w 0).
fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0], 0.0] }

/// The corners turned by the rotation, the steps, and the grid points (local, w = the alpha divisor) at count `t`.
struct Sheet {
    v0: V,
    row_step: V,
    col_last: V,
    grid: Vec<V>,
}

fn sheet(m: &crate::moby_runtime::Moby, t: f32) -> Sheet {
    let q = &m.pvars;
    let (nx, ny) = (q[o::COLS] as usize, q[o::ROWS] as usize);
    let (ix, iy) = (1.0 / nx as f32, 1.0 / ny as f32);
    let rows = euler_rows(pv(m.rotation));
    let r: [[f32; 4]; 4] = rows.map(|row| row.map(|x| f32::from_bits(x.0)));
    let tr = |v: [f32; 3]| -> V { std::array::from_fn(|k| r[0][k] * v[0] + r[1][k] * v[1] + r[2][k] * v[2]) };
    let hw = f(q, 0) * 0.5;
    let v0 = tr([-hw, 0.0, f(q, 0x20) + 0.01]);
    let v1 = tr([hw, 0.0, f(q, 0x21) + 0.01]);
    let v2 = tr([-hw, 0.0, 0.0]);
    let v3 = tr([hw, 0.0, 0.0]);
    let row_step = scale(sub(v2, v0), iy);
    let row_step_r = scale(sub(v3, v1), iy);
    let (c1, c2, c3) = ((f(q, 6) - f(q, 5)) * iy, (f(q, 9) - f(q, 7)) * iy, (f(q, 10) - f(q, 8)) * iy);
    let (q0s, q1s) = ((f(q, 0x1a) - f(q, 0x18)) * ix, (f(q, 0x1b) - f(q, 0x19)) * iy);
    let (ha, hb) = (f(q, 1) - f(q, 0x20), f(q, 1) - f(q, 0x21));
    let n = norm(cross(sub(v1, v0), row_step), 1.0);
    let (mut left, mut right) = (v0, v1);
    let ph0 = f(q, 0x24);
    let (mut a5, mut a7, mut a8, mut q0, mut q1) = (f(q, 5), f(q, 7), f(q, 8), f(q, 0x18), f(q, 0x19));
    let mut grid = vec![[0.0; 4]; (nx + 1) * (ny + 1)];
    let mut col = [0.0; 4];
    for row in 0..=ny {
        let fy = row as f32 / ny as f32;
        let mut cur = left;
        col = scale(sub(right, left), ix);
        let ry = phase(q, fy);
        for k in 0..=nx {
            let fx = k as f32 / nx as f32;
            let s1 = wrap(f(q, 0x17) * t).sin();
            let px = phase(q, fx);
            let base = t * a5 + f(q, 0x16) * s1 + px * f(q, 0x34) + ry * f(q, 0x35) + ph0 * f(q, 0x36);
            let hh = ha * fx + hb * (1.0 - fx);
            let u = wrap(base + q0 + f(q, 0xb) * fx + f(q, 0xd) * fy);
            let v = wrap(base + q1 + f(q, 0xc) * fy + f(q, 0xe) * fx);
            let sq = (fx * PI).sin().abs().sqrt();
            let up = hh * sq * (1.0 - fy);
            let d1 = norm(col, a7 * u.cos() * sq * px);
            let d2 = norm(row_step, a8 * v.sin() * sq * px);
            let w1 = wrap(base + f(q, 0xb) * fx).sin();
            let w2 = wrap(base + f(q, 0xd) * fy).sin();
            let d3 = norm(n, (fy * PI).sin() * (fx * PI).sin() * (f(q, 0x1e) * w1 + f(q, 0x1f) * w2));
            let d4 = norm(row_step, up);
            let mut g = add(cur, sub(add(add(d1, d2), d3), d4));
            g[3] = 1.0 / (((1.0 - f(q, 0x56)) + f(q, 0x56) * sq * sq * sq) * ((1.0 - f(q, 0x57)) + f(q, 0x57) * fy));
            grid[row * (nx + 1) + k] = g;
            cur = add(cur, col);
        }
        left = add(left, row_step);
        right = add(right, row_step_r);
        a5 += c1;
        a7 += c2;
        a8 += c3;
        q0 += q0s;
        q1 += q1s;
    }
    Sheet { v0, row_step, col_last: col, grid }
}

/// Level08 `0x2f70a0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < LEN { return; }
    if w.m(id).state == 0 {
        w.mm(id).pvars[o::SLOT] = 0xff;
        let t = w.m(id).pvars[0xf3] as i32;
        c::set_pi32(w, id, o::EMBER_T, t);
        let pos = c::pos(w, id);
        c::set_pv4(w, id, o::TARGET, pos);
        for i in [0x1c, 0x1d] {
            if c::pf(w, id, 4 * i) == 0.0 { c::set_pf(w, id, 4 * i, 1.0); }
        }
        let lo = c::pf(w, id, o::PHASE_STEP);
        for k in 0..8 {
            let a = w.rng.randf(lo, 1.0);
            c::set_pf(w, id, o::PHASES + 4 * k, a);
            let b = w.rng.randf(lo, 1.0);
            c::set_pf(w, id, o::TARGETS + 4 * k, b);
        }
        w.mm(id).state = 1;
    }
    let big = c::pf(w, id, o::W).max(c::pf(w, id, o::H));
    let s = super::class_scale(w, 671) * big * 0.25;
    w.mm(id).scale = s;
    let dd = w.m(id).draw_dist as f32;
    let pos = c::pos(w, id);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    let d = c::dist3(pos, cam);
    let fade_len = c::pf(w, id, o::FADE_LEN);
    let near = (dd - fade_len).max(0.0);
    let far = dd < d;
    let fade = if far { 0.0 } else if near < d { 1.0 - (d - near) / fade_len } else { 1.0 };
    c::set_pf(w, id, o::FADE, fade);
    let radius = w.m(id).pvars[o::RADIUS] as f32;
    let seen = w.m(id).visible != 0 || crate::moby_update::creature::fx::in_view(w, 255.0, pos, radius);
    if !seen || far {
        let slot = w.m(id).pvars[o::SLOT] as i8;
        if slot != -1 {
            w.svc.point_lights.free(slot as u8 as usize);
            w.mm(id).pvars[o::SLOT] = 0xff;
        }
    } else {
        if let Some(r) = super::row(REFERENCE_LEVEL, DRAW_FN) {
            w.svc.draw_callbacks.register(Callback::UnitFrame(r), id);
            w.svc.draw_callbacks.register(Callback::UnitQuads(r), id);
        }
        if p::u16(&w.m(id).pvars, o::FLAGS) & 4 != 0 {
            let q = &w.m(id).pvars;
            let colour = [q[0x134], q[0x135], q[0x136]].map(|b| fade * b as f32 / 100.0);
            let slot = q[o::SLOT] as i8;
            if slot == -1 {
                let l = PointLight { color: colour, intensity: q[0x138] as f32 / 100.0, pos: std::array::from_fn(|k| f(q, 0x50 + k)), radius };
                let load = f32::from_bits(w.svc.frame_load[1].0);
                let got = w.svc.point_lights.alloc(l, load).map_or(0xff, |i| i as u8);
                w.mm(id).pvars[o::SLOT] = got;
            } else if let Some(Some(l)) = w.svc.point_lights.slots.get_mut(slot as u8 as usize) {
                l.color = colour;
            }
        }
    }
    let fl = p::u16(&w.m(id).pvars, o::FLAGS) | 2;
    p::set_i16(&mut w.mm(id).pvars, o::FLAGS, fl as i16);
}

/// `0x2f5e58`'s game state and draws (module doc).
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < LEN { return; }
    let (step, rate) = (c::pf(w, id, o::PHASE_STEP), c::pf(w, id, o::PHASE_RATE));
    for k in 0..8 {
        let (mut cur, tgt) = (c::pf(w, id, o::PHASES + 4 * k), c::pf(w, id, o::TARGETS + 4 * k));
        let reached = if cur < tgt {
            cur += step * rate;
            tgt <= cur
        } else {
            cur -= step * rate;
            cur <= tgt
        };
        c::set_pf(w, id, o::PHASES + 4 * k, cur);
        if reached {
            let t = w.rng.randf(1.0 - step, 1.0);
            c::set_pf(w, id, o::TARGETS + 4 * k, t);
        }
    }
    if p::u16(&w.m(id).pvars, o::FLAGS) & 2 == 0 { return; }
    let t = w.svc.draw_callbacks.tick as f32;
    let sh = sheet(w.m(id), t);
    let pos = c::pos(w, id);
    let width = c::pf(w, id, o::W);
    if c::dec_timer_pvar_i32(w, id, o::EMBER_T) != 0 {
        let q = w.m(id).pvars.clone();
        let reload = w.ticks(q[0xf3] as i32);
        c::set_pi32(w, id, o::EMBER_T, reload);
        for _ in 0..q[0xf2] {
            let r = w.rng.randf(-1.0, 1.0);
            let k = w.rng.randf(0.0, r * f(&q, 0x3d));
            let cs = (k * PI).cos().abs().sqrt();
            let a = norm(sh.row_step, -(f(&q, 0x3f) + cs * f(&q, 0x40)));
            let b = norm(sh.col_last, k * width * 0.5);
            let at = add(add(b, pos), a);
            let s = f(&q, 0x37);
            let vx = w.rng.randf(-s, s) * SPEED;
            let vy = w.rng.randf(-s, s) * SPEED;
            let vz = w.rng.randf(0.0, f(&q, 0x38)) * SPEED;
            let size = w.rng.randf(f(&q, 0x39), f(&q, 0x3a));
            let life = w.ticks(p::u16(&q, 0xf0) as i32);
            let alpha = (q[0xef] as f32 * k) as i32 as u8;
            let s = crate::particles::type43::Spawn { size, g: 0.0, pos: at, vel: [vx, vy, vz, 0.0], life, rgb: [q[0xec], q[0xed], q[0xee]], alpha };
            part43(w, s);
        }
    }
    if c::dec_timer_pvar_s16(w, id, o::SPARK_T) != 0 {
        let q = w.m(id).pvars.clone();
        let reload = w.ticks(q[0x121] as i32);
        c::set_pi16(w, id, o::SPARK_T, reload as i16);
        let c1 = p::u32(&q, 0x114);
        let c2 = p::u32(&q, 0x118);
        for _ in 0..q[0x120] {
            let half = width * 0.5;
            let size = w.rng.randf(f(&q, 0x43), f(&q, 0x44));
            let rr = w.rng.rand_range(p::u16(&q, 0x11c) as i32, p::u16(&q, 0x11e) as i32);
            let life = w.ticks(rr);
            let r = w.rng.randf(-1.0, 1.0);
            let k = w.rng.randf(-(r * f(&q, 0x49)), r * f(&q, 0x49));
            let cs = (k * PI).cos().abs().sqrt();
            let a = norm(sh.row_step, -(f(&q, 0x4b) + cs * f(&q, 0x4c)));
            let b = norm(sh.col_last, k * half);
            let at = add(add(b, pos), a);
            let s = f(&q, 0x41);
            let vx = w.rng.randf(-s, s) * SPEED;
            let vy = w.rng.randf(-s, s) * SPEED;
            let vz = w.rng.randf(0.0, f(&q, 0x42)) * SPEED;
            let g = (1.0 - f(&q, 0x4a)) + cs * f(&q, 0x4a);
            let vel = [vx * g, vy * g, vz * g, 0.0];
            crate::moby_update::creature::projectile::part22(w, &crate::particles::type22::Spawn { size, pos: at, vel, c1, c2, life });
        }
    }
    let slot = w.m(id).pvars[o::SLOT] as i8;
    if slot != -1 {
        let i = slot as u8 as usize;
        let target = c::pv4(w, id, o::TARGET);
        let drift = c::pf(w, id, o::DRIFT) * SPEED;
        if let Some(Some(l)) = w.svc.point_lights.slots.get(i).copied() {
            let lp = [l.pos[0], l.pos[1], l.pos[2], 0.0];
            if c::dist3(target, lp) < drift {
                let (b1, b2) = (w.m(id).pvars[0x139] as f32, w.m(id).pvars[0x13a] as f32);
                let r1 = w.rng.randf(-b1, b1);
                let along = (r1 + width) * 0.5;
                let r2 = w.rng.randf(-b2, b2);
                let h = c::pf(w, id, o::H);
                let a = norm(sh.col_last, along);
                let b = norm(sh.row_step, -((r2 + h) * 0.5));
                let nt = add(add(add(a, b), sh.v0), pos);
                c::set_pv4(w, id, o::TARGET, nt);
            } else {
                let np = add(lp, norm(sub(target, lp), drift));
                let radius = w.m(id).pvars[o::RADIUS] as f32;
                if let Some(Some(l)) = w.svc.point_lights.slots.get_mut(i) {
                    l.pos = [np[0], np[1], np[2]];
                    l.radius = radius;
                }
            }
        }
    }
    let fl = p::u16(&w.m(id).pvars, o::FLAGS) ^ 2;
    p::set_i16(&mut w.mm(id).pvars, o::FLAGS, fl as i16);
}

/// `0x27b450`: one type-43 ember (one raw `rand()` with a record).
fn part43(w: &mut World, s: crate::particles::type43::Spawn) {
    *w.svc.fx.part_spawns.entry(43).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type43::spawn(sys, w.rng, s).is_none() { w.svc.fx.part_failed += 1; }
}

/// `0x2f5e58`'s quads (module doc).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= LEN)?;
    let q = &m.pvars;
    let (nx, ny) = (q[o::COLS] as usize, q[o::ROWS] as usize);
    if nx == 0 || ny == 0 { return None; }
    let tick = svc.draw_callbacks.tick;
    let t = tick as f32;
    let sh = sheet(m, t);
    let alpha = ((f(q, 0x55) * q[0x13] as f32 * m.alpha as f32 * 0.007_812_5) as i32 & 0xff) as u32;
    let frame = if q[o::FRAMES] != 0 { q[o::FRAME0].wrapping_add(((tick as i32 / (q[o::FRAME_DIV] as i32 + 1)) % q[o::FRAMES] as i32) as u8) } else { q[o::FRAME0] };
    let fx = if p::u16(q, o::FLAGS) & 1 == 0 { frame as usize + 0x28 } else { frame as usize };
    let a = &q[o::ALPHA_REG..o::ALPHA_REG + 4];
    let additive = (a[0], a[1], a[2], a[3]) == (0, 2, 0, 1);
    let (ix, iy) = (1.0 / nx as f32, 1.0 / ny as f32);
    let s0 = frac(t * f(q, 0x10) + f(q, 0x11) * wrap(t * f(q, 0x12)).sin());
    let s: Vec<f32> = (0..=nx).map(|k| s0 + k as f32 * (ix / f(q, 0x1c))).collect();
    let t0 = frac(t * f(q, 0x13) + f(q, 0x14) * wrap(t * f(q, 0x15)).sin());
    let tt: Vec<f32> = (0..=ny).map(|k| t0 + k as f32 * (iy / f(q, 0x1d))).collect();
    let rgb = (q[0x11] as u32) << 16 | (q[0x12] as u32) << 8 | q[0x10] as u32;
    let colour = |g: V| -> u32 {
        let d = g[3] as i32;
        let a = if d == 0 { 0 } else { alpha as i32 / d };
        (a as u32) << 24 | rgb
    };
    let pos = m.position;
    let mut quads = Vec::with_capacity(nx * ny);
    for row in 0..ny {
        for col in 0..nx {
            let g = [sh.grid[row * (nx + 1) + col], sh.grid[row * (nx + 1) + col + 1], sh.grid[(row + 1) * (nx + 1) + col], sh.grid[(row + 1) * (nx + 1) + col + 1]];
            let corners = g.map(|v| [v[0] + pos[0], v[1] + pos[1], v[2] + pos[2]]);
            let st = [[s[col], tt[row]], [s[col + 1], tt[row]], [s[col], tt[row + 1]], [s[col + 1], tt[row + 1]]];
            quads.push(FxQuad { corners, st, rgba: g.map(colour) });
        }
    }
    Some(FxQuads { fx, additive, subtract: false, quads })
}
