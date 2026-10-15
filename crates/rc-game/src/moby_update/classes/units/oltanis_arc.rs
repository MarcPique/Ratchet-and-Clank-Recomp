//! **Oltanis's rail arcs, class 386** (level14 `0x2dee28`, its draw `0x2df458`; census U496). A crackling arc that
//! hangs over three grind rails (pvars +0x1f4 / +0x1f8 / +0x1fc, grind path indices) and kills Ratchet in its cuboid
//! (+0x200). Any of them unset: deleted.
//!
//! * **Start** (`0x2df080`): the ten points of the core (+0x00, stride 0x10) at x = +0x1f0·i, the twenty of the
//!   glow (+0xa0) at x = 0.39·i, y = z = 0, w = 1; the three phases (+0x1e0..+0x1e8) and the scroll (+0x1ec) 0.
//! * **Every tick**: active while Ratchet grinds (group 0xf) on one of its rails or stands within 16 of it in x and in
//!   y; else its hum is released. Active: the hum (class 211's sound 6, flags 4) kept playing; Ratchet in the cuboid
//!   (`0x2dfc58`): health 0 and the hit (dir (0, 0, 1, 5627.92), self, flags 0x200001, 5 / 4, the class, damage 100,
//!   +0x20 1). Then, within 64 and in view of the sphere 3.125 ahead (yaw) with radius 4.5 (`FastBSphereCheck`):
//!   the points moved (`0x2df138`) and the draw registered.
//! * **The points** (`0x2df138`): phases += 10°, −46°, +3°; core points 1..4: z = 0.1·sin(+0x1e0 + i·72°) +
//!   `randf_sym(0, 0.2)` (point 0 at 0), x = 1.6 + +0x1f0·i; points 5..9 mirror them (z of 9 − i), x likewise;
//!   glow points 0..18: z = 0.75·sin(+0x1e8 + 12°·i)·sin(+0x1e4 + 57°·i) + `randf_sym(0, 0.2)`, y = 0.5·cos(+0x1e4 +
//!   57°·i), x = 0.39·i (the 20th keeps its start).
//! * **The draw** (`0x2df458` / `0x2df510`): the scroll −= 0.2 a frame (wrapped by 8 at −8); each line (core 10
//!   points: alphas 0x80 / 0x40; glow 20: 0x30 / 0x10) through the moby's matrix as camera-facing ribbon segments:
//!   at each point the side `unit(unit(next − p) × (camera − p))` (`FastVecCross(out, a, b)` = b × a) (the last point keeps the one before); FX 0x2d,
//!   half width 0.2, grey (ST s from the scroll), and FX 0x2c, half width 0.4, red 0x7f4040 (ST 1..0), both
//!   additive, no depth write. On the glow line the first segment's start and the 18th's end are clear.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2dee28` | the update | [`update`] |
//! | `0x2df080` / `0x2df138` | start / move the points | [`start`], [`move_points`] |
//! | `0x2dfc58` | the kill | [`update`] |
//! | `0x2df458` / `0x2df510` | the draw | [`frame`], [`fx_quad_groups`] |
//!
//! Read from the level14 decomp. [L] The draw's camera (0x1674c0) is the one the frame part saw, kept in +0x210.
//! Native `f32`.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::creature as c;
use crate::moby_update::services::{euler_rows, pv, pvar as p, HitTemplate, Services, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 14;
pub const UPDATE_FN: u32 = 0x2d_ee28;
pub const DRAW_FN: u32 = 0x2d_f458;
pub const CLASSES: [i16; 1] = [386];

const GLOW: usize = 0xa0;
const PHASE: usize = 0x1e0;
const SCROLL: usize = 0x1ec;
const SEG: usize = 0x1f0;
const RAILS: usize = 0x1f4;
const CUBOID: usize = 0x200;
const VOICE: usize = 0x204;
const CAM: usize = 0x210;
const SIZE: usize = 0x220;
/// Ratchet's grind group.
const GRIND: i32 = 0xf;

fn set_pt(w: &mut World, id: MobyId, o: usize, v: [f32; 4]) { c::set_pv4(w, id, o, v); }

/// `0x2df080` (module doc).
pub fn start(w: &mut World, id: MobyId) {
    let seg = c::pf(w, id, SEG);
    for i in 0..10 { set_pt(w, id, 0x10 * i, [seg * i as f32, 0.0, 0.0, 1.0]); }
    for i in 0..20 { set_pt(w, id, GLOW + 0x10 * i, [i as f32 * 0.39, 0.0, 0.0, 1.0]); }
    for k in 0..4 { c::set_pf(w, id, PHASE + 4 * k, 0.0); }
}

/// `0x2df138` (module doc).
pub fn move_points(w: &mut World, id: MobyId) {
    let a = c::add_rot(c::pf(w, id, PHASE), f32::from_bits(0x3e32_b8c2));
    c::set_pf(w, id, PHASE, a);
    let b = c::sub_rot(c::pf(w, id, PHASE + 4), f32::from_bits(0x3f4d_87ac));
    c::set_pf(w, id, PHASE + 4, b);
    let g = c::add_rot(c::pf(w, id, PHASE + 8), f32::from_bits(0x3d56_7770));
    c::set_pf(w, id, PHASE + 8, g);
    let seg = c::pf(w, id, SEG);
    let ten = 10.0f32;
    for i in 0..5 {
        let (fi, z) = if i == 0 {
            (0.0, 0.0)
        } else {
            let fi = i as f32;
            let s = wrap_frac(a + fi * (360.0 / (ten * 0.5)) * 0.017_453_292).sin() * 0.1;
            (fi, s + w.rng.randf_sym(0.0, f32::from_bits(0x3e4c_cccd)))
        };
        set_pt(w, id, 0x10 * i, [seg * fi + 1.6, 0.0, z, 1.0]);
    }
    for i in 5..10 {
        let z = c::pf(w, id, 0x10 * (9 - i) + 8);
        set_pt(w, id, 0x10 * i, [seg * i as f32 + 1.6, 0.0, z, 1.0]);
    }
    for i in 0..0x13 {
        let fi = i as f32;
        let u = wrap_frac(b + fi * 0.994_837_64);
        let v = wrap_frac(g + fi * 0.20944);
        let s = v.sin() * 0.75 * u.sin();
        let z = 0.0 + s + w.rng.randf_sym(0.0, f32::from_bits(0x3e4c_cccd));
        set_pt(w, id, GLOW + 0x10 * i, [fi * 0.39, u.cos() * 0.5, z, 1.0]);
    }
}

/// Ratchet grinding one of its rails.
fn on_rail(w: &World, id: MobyId) -> bool {
    if w.hero.group != GRIND { return false; }
    let Some(r) = w.hero.boots.rail else { return false };
    (0..3).any(|k| c::pi32(w, id, RAILS + 4 * k) as usize == r)
}

/// Level14 `0x2dee28` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, SIZE);
    let active = match w.m(id).state {
        0 => {
            if (0..3).any(|k| c::pi32(w, id, RAILS + 4 * k) == -1) || c::pi32(w, id, CUBOID) < 0 {
                w.delete_moby(id);
                return;
            }
            start(w, id);
            w.mm(id).state = 1;
            c::set_pi32(w, id, VOICE, -1);
            false
        }
        1 => {
            let (h, pos) = (super::hero_pos(w), c::pos(w, id));
            on_rail(w, id) || ((h[0] - pos[0]).abs() <= 16.0 && (h[1] - pos[1]).abs() <= 16.0)
        }
        _ => false,
    };
    if !active {
        let v = c::pi32(w, id, VOICE);
        if -1 < v {
            w.release_sound(v, id);
            c::set_pi32(w, id, VOICE, -1);
        }
        return;
    }
    if !w.sound_alive(c::pi32(w, id, VOICE), id) {
        let v = w.play_sound_as(6, 4, id, 0xd3);
        c::set_pi32(w, id, VOICE, v);
    }
    let h = super::hero_pos(w);
    if w.in_cuboid([h[0], h[1], h[2]], c::pi32(w, id, CUBOID)) {
        let tmpl = HitTemplate {
            dir: pv([0.0, 0.0, 1.0, f32::from_bits(0x45af_df66)]),
            attacker: Some(id),
            flags: 0x20_0001,
            b18: 5,
            b19: 4,
            h1a: w.m(id).o_class as u16,
            damage: Pf::f(100.0),
            w20: 1,
        };
        story::set_health(w, 0);
        if let Some(hm) = w.hero_moby { w.deliver_hit(hm, &tmpl); }
    }
    let (pos, yaw) = (c::pos(w, id), w.m(id).rotation[2]);
    let sphere = [pos[0] + yaw.cos() * 3.125, pos[1] + yaw.sin() * 3.125, pos[2], 4.5];
    if !w.view.is_some_and(|v| !v.culled(64.0, sphere)) { return; }
    move_points(w, id);
    if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) {
        w.svc.draw_callbacks.register(Callback::UnitFrame(row), id);
        w.svc.draw_callbacks.register(Callback::UnitQuads(row), id);
    }
}

/// The draw's frame part (`0x2df458`'s scroll; the camera kept).
pub fn frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    let s = c::pf(w, id, SCROLL) - c::SPEED * 0.2;
    c::set_pf(w, id, SCROLL, if s <= -8.0 { s + 8.0 } else { s });
    let cam = crate::hero::physics::to_f32x3(w.camera);
    c::set_pv4(w, id, CAM, [cam[0], cam[1], cam[2], 1.0]);
}

/// `0x2df510(m, line, a, b, n)`: one line's two ribbons (module doc).
fn line(m: &crate::moby_runtime::Moby, o: usize, a: u32, b: u32, n: usize) -> [Vec<FxQuad>; 2] {
    let r = euler_rows(pv(m.rotation)).map(|row| row.map(|x| f32::from_bits(x.0)));
    let pos = m.position;
    let at = |i: usize| -> [f32; 3] {
        let q = [p::ff(&m.pvars, o + 0x10 * i), p::ff(&m.pvars, o + 0x10 * i + 4), p::ff(&m.pvars, o + 0x10 * i + 8)];
        std::array::from_fn(|k| r[0][k] * q[0] + r[1][k] * q[1] + r[2][k] * q[2] + pos[k])
    };
    let cam = [p::ff(&m.pvars, CAM), p::ff(&m.pvars, CAM + 4), p::ff(&m.pvars, CAM + 8)];
    let scroll = p::ff(&m.pvars, SCROLL);
    let side = |p0: [f32; 3], p1: [f32; 3]| -> [f32; 3] {
        let v = |a: [f32; 3]| -> c::V { [a[0], a[1], a[2], 0.0] };
        let d = c::set_len3(c::sub(v(p1), v(p0)), 1.0);
        let t = c::sub(v(cam), v(p0));
        // FastVecCross(out, t, d) = d × t.
        let x = c::set_len3([d[1] * t[2] - d[2] * t[1], d[2] * t[0] - d[0] * t[2], d[0] * t[1] - d[1] * t[0], 0.0], 1.0);
        [x[0], x[1], x[2]]
    };
    let off = |p0: [f32; 3], s: [f32; 3], k: f32| -> [f32; 3] { std::array::from_fn(|i| p0[i] + s[i] * k) };
    let (ca, cb) = (a << 24 | 0x7f_7f7f, b << 24 | 0x7f_4040);
    let (sta, stb) = ([[scroll, 0.0], [scroll, 1.0], [scroll + 1.0, 0.0], [scroll + 1.0, 1.0]], [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]]);
    let (mut qa, mut qb) = (Vec::new(), Vec::new());
    let mut prev = at(0);
    let mut sp = side(prev, at(1));
    for i in 1..n {
        let pi = at(i);
        let s = if i != n - 1 { side(pi, at(i + 1)) } else { sp };
        let (mut cola, mut colb) = ([ca; 4], [cb; 4]);
        if b == 0x10 && i == 1 {
            cola[0] = 0xff_ffff;
            cola[1] = 0xff_ffff;
            colb[0] = 0x7f_4040;
            colb[1] = 0x7f_4040;
        } else if b == 0x10 && i == n - 2 {
            cola[2] = 0xff_ffff;
            cola[3] = 0xff_ffff;
            colb[2] = 0x7f_4040;
            colb[3] = 0x7f_4040;
        }
        qa.push(FxQuad { corners: [off(prev, sp, 0.2), off(prev, sp, -0.2), off(pi, s, 0.2), off(pi, s, -0.2)], st: sta, rgba: cola });
        qb.push(FxQuad { corners: [off(prev, sp, 0.4), off(prev, sp, -0.4), off(pi, s, 0.4), off(pi, s, -0.4)], st: stb, rgba: colb });
        prev = pi;
        sp = s;
    }
    [qa, qb]
}

/// Level14 `0x2df458`: the arc's ribbons (module doc).
pub fn fx_quad_groups(table: &MobyTable, _svc: &Services, id: MobyId) -> Vec<FxQuads> {
    let Some(m) = table.mobys.get(id).filter(|m| m.pvars.len() >= SIZE) else { return Vec::new() };
    let [a0, b0] = line(m, 0, 0x80, 0x40, 10);
    let [a1, b1] = line(m, GLOW, 0x30, 0x10, 20);
    [(0x2d, a0), (0x2c, b0), (0x2d, a1), (0x2c, b1)].into_iter().map(|(fx, quads)| FxQuads { fx, additive: true, subtract: false, quads }).collect()
}
