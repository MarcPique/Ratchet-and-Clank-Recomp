//! **Hoven's arc posts, class 1259** (level12 `0x302f30`, the arc `0x302c58`, its draw `0x3028c8`; census U436; 18
//! placed, in pairs linked by +0x2314). Each post slides between the two points of its path (+0x2310) on a folded
//! sine (its phase +0x2318 at +0x2328 degrees a second) and faces its partner (+0x2324 the distance). The pair's
//! master (+0x2320 = 1) keeps four crackling bolts between them and hurts what crosses the line post to post (a hit
//! of 1, flags 0x10001, `CollLine_Fix(pos, partner, 0, none, …)` every tick). Nothing runs beyond 64 (xy) of Ratchet,
//! when neither post is drawn and their middle (radius 16) is out of view, or while the camera is inside +0x2330.
//!
//! **The bolts** (`0x302c58`): four polylines of +0x232c points (pvar `i·0x460`, velocities at `0x1190 + i·0x460`)
//! along the post's x from 0 to the distance. Each bolt's timer (+0x2300 + 4i: 0, 11, 23, 34 at the start) counts
//! down from 45 (0x161e04): running, the inner points drift by their velocities; out, the bolt is rebuilt straight
//! with fresh velocities (each the mean of three `rand_vec(0, 0.4·dt)`, a third of their sum). A shared scroll
//! (0x161df8) runs at 2 a second per master. Drawn within 48 of the camera.
//!
//! **The draw** (`0x3028c8`): each segment a ribbon 0.1 tall (z ±0.05 about its points), FX 0xe, additive, the ST s
//! stepping 0.25 a segment (wrapped) plus the scroll over 0..0.25, t 0..1; the bolt's colour `FastTweenColor` of its
//! timer (above 22: (t − 22) / 23, else 1 − t / 22) from 0x80c04070 to 0x00ff0000; drawn twice, from the partner (the
//! post's frame turned 180°) and from the post.
//!
//! Read from the level12 decomp and disassembly (the frames, the line's ends); the tables 0x1fb510 / 0x1fb530 /
//! 0x1fb5b0 and gp−0x4e34..−0x4e00. [L] The two frames are the post's yaw (+ 180°) without its x / y turns (the class
//! sets only the yaw). Native `f32`.

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature as c;
use crate::moby_update::services::{euler_rows, line_hit_in, pv, pvar as p, HitTemplate, Services, World};
use crate::moby_update::story;
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, PI};

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_2f30;
pub const DRAW_FN: u32 = 0x30_28c8;
pub const CLASSES: [i16; 1] = [1259];

const DT: f32 = c::DT;
/// 0x161e04 / 0x161e08 (stored by every post's start).
const LIFE: i32 = 0x2d;
const MID: i32 = 0x16;
/// The shared scroll (0x161df8, f32 bits in a unit word).
const SCROLL: u32 = 0x16_1df8;

mod pv_ {
    pub const VEL: usize = 0x1190;
    pub const BOLT: usize = 0x460;
    pub const TIMERS: usize = 0x2300;
    pub const PATH: usize = 0x2310;
    pub const LINK: usize = 0x2314;
    pub const PHASE: usize = 0x2318;
    pub const W231C: usize = 0x231c;
    pub const MASTER: usize = 0x2320;
    pub const LEN: usize = 0x2324;
    pub const SPEED: usize = 0x2328;
    pub const COUNT: usize = 0x232c;
    pub const CAMERA: usize = 0x2330;
    pub const SIZE: usize = 0x2340;
}
use pv_ as o;

fn link(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o::LINK)).ok().filter(|&m| m < w.table.mobys.len()) }
fn count(pv: &[u8]) -> usize { p::i32(pv, o::COUNT).clamp(0, 70) as usize }

/// `0x302c58(m)`: the bolts (module doc).
fn bolts(w: &mut World, id: MobyId) {
    let s = f32::from_bits(w.svc.units.word(SCROLL)) + 2.0 * DT;
    w.svc.units.set_word(SCROLL, if 1.0 < s { s - 1.0 } else { s }.to_bits());
    let n = count(&w.m(id).pvars);
    for i in 0..4 {
        let base = i * o::BOLT;
        if c::dec_timer_pvar_i32(w, id, o::TIMERS + 4 * i) == 0 {
            for k in 1..n.saturating_sub(1) {
                let q = c::add(c::pv4(w, id, base + 0x10 * k), c::pv4(w, id, o::VEL + base + 0x10 * k));
                c::set_pv4(w, id, base + 0x10 * k, q);
            }
            continue;
        }
        let len = c::pf(w, id, o::LEN);
        let mut r = Vec::with_capacity(n);
        for k in 0..n {
            c::set_pv4(w, id, base + 0x10 * k, [k as f32 * (len / n as f32), 0.0, 0.0, 1.0]);
            let v = w.rng.rand_vec(0.0, 0.4 * DT);
            r.push([v[0], v[1], v[2], 0.0]);
        }
        for k in 1..n.saturating_sub(1) {
            let v = c::scale(c::add(c::add(r[k - 1], r[k]), r[k + 1]), f32::from_bits(0x3eaa_7efa));
            c::set_pv4(w, id, o::VEL + base + 0x10 * k, v);
        }
        c::set_pi32(w, id, o::TIMERS + 4 * i, LIFE);
    }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if c::dist3(c::pos(w, id), cam) < 48.0 {
        if let Some(row) = super::row(REFERENCE_LEVEL, DRAW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(row), id); }
    }
}

/// Level12 `0x302f30` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, o::SIZE);
    if c::pi32(w, id, o::LINK) == -1 { return; }
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, o::W231C, -1);
            for (k, t) in [0, 0xb, 0x17, 0x22].into_iter().enumerate() { c::set_pi32(w, id, o::TIMERS + 4 * k, t); }
            w.mm(id).state = 1;
        }
        1 => {
            let Some(l) = link(w, id) else { return };
            let lp = w.m(l).position;
            let pos = c::pos(w, id);
            w.mm(id).rotation[2] = c::atan(lp[0] - pos[0], lp[1] - pos[1]);
            c::set_pf(w, id, o::LEN, c::dist3(pos, lp));
            let ph = c::add_rot(c::pf(w, id, o::PHASE), c::pf(w, id, o::SPEED) * 0.017_453_292 * DT);
            c::set_pf(w, id, o::PHASE, ph);
            let mut f = ph.sin();
            if FRAC_PI_2 < ph && ph < PI { f = 2.0 - f; }
            if -PI < ph && ph < -FRAC_PI_2 { f = 2.0 - f; }
            let mut t = f * 0.5 + 0.5;
            if 1.0 < t { t -= 2.0; }
            let t = t.abs();
            if let Some(path) = usize::try_from(c::pi32(w, id, o::PATH)).ok().and_then(|i| w.svc.splines.get(i)) {
                let a = path.first().map_or([0.0; 4], |q| q.map(f32::from_bits));
                let b = path.get(1).map_or([0.0; 4], |q| q.map(f32::from_bits));
                w.mm(id).position = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
            }
            let pos = c::pos(w, id);
            if 64.0 < c::dist2(pos, super::hero_pos(w)) { return; }
            if w.m(id).visible == 0 && w.m(l).visible == 0 {
                let mid = c::scale(c::add(pos, lp), 0.5);
                let culled = w.view.is_none_or(|v| v.culled(32.0, [mid[0], mid[1], mid[2], 16.0]));
                if culled { return; }
            }
            let cub = c::pi32(w, id, o::CAMERA);
            if cub != -1 {
                let cam = w.camera.map(|x| f32::from_bits(x.0));
                if w.in_cuboid([cam[0], cam[1], cam[2]], cub) { return; }
            }
            if c::pi32(w, id, o::MASTER) == 0 { return; }
            bolts(w, id);
            let tmpl = HitTemplate { attacker: Some(id), flags: 0x1_0001, damage: Pf::ONE, w20: 1, ..Default::default() };
            let lp = w.m(l).position;
            line_hit_in(w.table, w.svc, w.classes, w.coll, pv(c::pos(w, id)), pv(lp), 0, None, &tmpl);
        }
        _ => {}
    }
}

/// Level12 `0x3028c8`: the bolts' ribbons (module doc).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= o::SIZE)?;
    let q = &m.pvars;
    let l = usize::try_from(p::i32(q, o::LINK)).ok().and_then(|i| table.mobys.get(i));
    let scroll = f32::from_bits(svc.units.word(SCROLL));
    let frame = |yaw: f32, at: [f32; 4]| -> ([[f32; 4]; 4], [f32; 4]) {
        let r = euler_rows(pv([m.rotation[0], m.rotation[1], yaw, 0.0])).map(|row| row.map(|x| f32::from_bits(x.0)));
        (r, at)
    };
    let frames = [frame(c::add_rot(m.rotation[2], PI), l.map_or(m.position, |l| l.position)), frame(m.rotation[2], m.position)];
    let n = count(q);
    let mut quads = Vec::new();
    const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [0.25, 0.0], [0.25, 1.0]];
    const OFS: [f32; 4] = [-0.05, 0.05, -0.05, 0.05];
    for i in 0..4 {
        let t = p::i32(q, o::TIMERS + 4 * i);
        let f = if MID < t { (t - MID) as f32 / (LIFE - MID) as f32 } else { 1.0 - t as f32 / MID as f32 };
        let col = crate::particles::tween_color(f.to_bits(), 0x80c0_4070, 0x00ff_0000);
        let mut acc = 0.0f32;
        for j in 0..n.saturating_sub(1) {
            acc += 0.25;
            if 1.0 < acc { acc -= 1.0; }
            let st = ST.map(|s| [s[0] + acc + scroll, s[1]]);
            let local: [[f32; 3]; 4] = std::array::from_fn(|k| {
                let pt = p::v4f(q, i * o::BOLT + (j + (k >> 1)) * 0x10);
                [pt[0], pt[1], pt[2] + OFS[k]]
            });
            for (r, at) in frames {
                let corners = local.map(|v| std::array::from_fn(|a| r[0][a] * v[0] + r[1][a] * v[1] + r[2][a] * v[2] + at[a]));
                quads.push(FxQuad { corners, st, rgba: [col; 4] });
            }
        }
    }
    Some(FxQuads { fx: 0xe, additive: true, subtract: false, quads })
}
