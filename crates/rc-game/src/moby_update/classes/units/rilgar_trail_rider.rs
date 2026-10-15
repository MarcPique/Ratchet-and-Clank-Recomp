//! **Rilgar's trail riders, class 79** (level05 `0x2d7140` with `0x2d7890`, `0x2d7020` and the draws `0x2d7920` /
//! `0x2d6f48`; 30 placed in group 9; census U184; the name is descriptive [L]). Small craft that fly their spline loop
//! (pvar +0x00) at 30 a second, banking into its turns, humming, with three ribbon trails behind them and three
//! pulsing glows on their joints; a shot (a hit with damage) bursts one into three pieces (0x72e). Nothing runs while
//! the camera is below z 55. Read from the level05 disassembly (the update is no Ghidra function: decompiled after
//! creating it) and decomp, its words gp−0x5830..−0x57f4 and the table 0x1cdd70. Native `f32`.
//!
//! **The spline's banking** (state 0, once per spline: its point 0's w still −1): each point's w = the turn there
//! (`sub_rot` of the heading out of it and the heading into it; the loop is closed), all scaled so that the sharpest
//! turn of points 1..n−2 is 60°; the path pose reads w as the roll.
//!
//! **Pvars** (0x370): +0x00 the spline, +0x04 the start (a fraction of the loop), +0x08 t, +0x0c the first segment's
//! length, +0x1c the pulse phase, +0x20 / +0x120 / +0x220 the trails' 16-point rings, +0x320 s16 the trails' length,
//! +0x322 s16 the ring head, +0x328 the hum's voice, +0x330 the three joint points, +0x360 the joint glows' colour,
//! +0x368 their phase.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | the camera (0x1671c0) below z 55 → nothing; `MobyGetHitMessage(m, 0x10000, 0)` | [`update`] |
//! | state 0 | update / draw distance 0xff; scale = class scale · 0.5; phase and the glows' phase `random_angle_radians`; voice −1; the banking; +0x0c = `VecDistance(p0, p1)`; t = n · +0x04; → 1 | [`update`] ([`bank`]) |
//! | state 1 | phase += 4π·dt; glow +0x90 = `FastTweenColor((sin + 1)/2, 0x80000080, 0x80202020)`; `0x2d7890`: t += 30·dt / +0x0c, wrapped by n; the closed path's pose at t (`0x28d350` = L01 `0x277d40`), x rotation = the pose's w (−roll) | [`update`] (`path::pose`) |
//! | | the hum (`SoundIsAlive`, else `PlayClassSound(1, 4)`); every 4th tick (counter & 3 = 0) the head advances (& 15) and the length grows below the maximum (gp−0x5820: 5, 16 with the contrails cheat 0x15edb2); the rings' heads = position + (−1.2, 0.4, 0.45) / (−1.5, 0, −0.3) / (−1.2, −0.4, 0.45) through the rows, w 1 | [`update`] |
//! | | once a tick for the group (a shared stamp): `RegisterDrawCallback(0x2d7920)`; collision only with x ≥ 8 and y ≥ 8; a hit with damage (+0x2c) > 0 → 2 | [`update`] (`Callback::UnitQuads`) |
//! | state 2 | `PlayClassSound(0, 0)`; three `BreakFxB(12·dt², m, 0x72e, position, Euler, ticks(90), 0, row 0 · 0.15 with z + 0.08, 0)`; `DeleteMoby` | [`update`] (`fx::break_piece_with`) |
//! | every tick (states 0 / 1) | `0x2d7020` (gp−0x581c = 1): with a group and drawn, the joint points of lists 0..2 into +0x330; +0x368 += 110°·dt; c = `FastTweenColor(0.5·sin + 0.5, 0x1e1eb4, 0x1eb41e)`, glow = c, +0x360 = c with alpha 0x30; once a tick for the group `RegisterDrawCallback(0x2d6f48)` | [`update`] (`Callback::UnitGlow`) |
//! | `0x2d7920` | FX 0x13, ALPHA 0x48; per rider of the group (class 0x4f) in state 1 (`FastBSphereCheck(512, (position, 20))`: [L] the renderer's culling instead): for each of its length's segments i and each trail, a quad from ring points `(head − i + 15) & 15` and the next, ±0.15 in z, ST (0, 0.5) (1, 0.5) (0, 0.5) (1, 0.5), colours `FastTweenColor(clamp((i + 1 − k/2) / (max + ((tick + 3) & 3) / (4·max)), 0, 1), 0x40806060, 0x00802020)` (k the corner) | [`fx_quads`] |
//! | `0x2d6f48` | each drawn rider of the group: three glow quads (size 0.45, pull 0.4) at its joint points, colour +0x360 | [`glow_quads`] |

use super::{FxQuad, FxQuads, GlowQuad};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, fx, DT};
use crate::moby_update::services::{pvar as p, Services, World};

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x2d_7140;
pub const TRAIL_FN: u32 = 0x2d_7920;
pub const GLOW_FN: u32 = 0x2d_6f48;
pub const CLASS: i16 = 0x4f;
pub const CLASSES: [i16; 1] = [CLASS];
pub const PIECE: i16 = 0x72e;
/// The camera height below which nothing runs.
pub const MIN_CAM_Z: f32 = 55.0;
/// gp−0x5830: the speed along the loop (units a second over the first segment's length).
const SPEED: f32 = 30.0;
const SCALE: f32 = 0.5;
const GLOW: (u32, u32) = (0x8000_0080, 0x8020_2020);
const JOINT_GLOW: (u32, u32) = (0x001e_1eb4, 0x001e_b41e);
const TRAIL_COLOURS: (u32, u32) = (0x4080_6060, 0x0080_2020);
const TRAIL_ST: [[f32; 2]; 4] = [[0.0, 0.5], [1.0, 0.5], [0.0, 0.5], [1.0, 0.5]];
const TRAIL_OFFSETS: [[f32; 3]; 3] = [[-1.2, 0.4, 0.45], [-1.5, 0.0, -0.3], [-1.2, -0.4, 0.45]];
const TRAIL_FX: usize = 0x13;
/// The shared draw stamps (the pvars' shared words in the game), by group.
const TRAIL_STAMP: u32 = 0x2d79_2000;
const GLOW_STAMP: u32 = 0x2d6f_4800;

/// Pvar offsets (module doc).
pub mod pv {
    pub const SPLINE: usize = 0x00;
    pub const START: usize = 0x04;
    pub const T: usize = 0x08;
    pub const SEG: usize = 0x0c;
    pub const PHASE: usize = 0x1c;
    pub const RINGS: usize = 0x20;
    pub const LEN: usize = 0x320;
    pub const HEAD: usize = 0x322;
    pub const VOICE: usize = 0x328;
    pub const JOINTS: usize = 0x330;
    pub const JOINT_RGBA: usize = 0x360;
    pub const JOINT_PHASE: usize = 0x368;
    pub const SIZE: usize = 0x370;
}

/// The trails' maximum length (the contrails cheat: 16).
fn max_len(cheat: bool) -> f32 { if cheat { 16.0 } else { 5.0 } }

/// State 0's banking of the spline (module doc), once: point 0's w still −1.
pub fn bank(pts: &mut [[u32; 4]]) {
    let n = pts.len();
    if n < 3 || f32::from_bits(pts[0][3]) != -1.0 { return; }
    let f = |q: [u32; 4]| q.map(f32::from_bits);
    let heading = |a: [f32; 4], b: [f32; 4]| c::atan(b[0] - a[0], b[1] - a[1]);
    let w0 = c::sub_rot(heading(f(pts[0]), f(pts[1])), heading(f(pts[n - 1]), f(pts[0])));
    pts[0][3] = w0.to_bits();
    let mut max = 0.0f32;
    for k in 1..n - 1 {
        let wk = c::sub_rot(heading(f(pts[k]), f(pts[k + 1])), heading(f(pts[k - 1]), f(pts[k])));
        pts[k][3] = wk.to_bits();
        if max < wk.abs() { max = wk.abs(); }
    }
    let wl = c::sub_rot(heading(f(pts[n - 1]), f(pts[0])), heading(f(pts[n - 2]), f(pts[n - 1])));
    pts[n - 1][3] = wl.to_bits();
    let k = f32::from_bits(0x3f86_0a92) / max;
    for q in pts.iter_mut() { q[3] = (f32::from_bits(q[3]) * k).to_bits(); }
}

fn spline(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::SPLINE)).ok().filter(|&i| i < w.svc.splines.len()) }

fn once_per_group(w: &mut World, id: MobyId, stamp: u32) -> bool {
    let key = stamp + w.m(id).group as u8 as u32;
    let tick = w.counter as u32;
    if w.svc.units.word(key) == tick { return false; }
    w.svc.units.set_word(key, tick);
    true
}

/// `0x2d7020` (module doc).
fn joint_glows(w: &mut World, id: MobyId) {
    if w.m(id).group == -1 || w.m(id).visible == 0 { return; }
    for k in 0..3 {
        let j = w.joint_point(id, k);
        c::set_pv4(w, id, pv::JOINTS + 0x10 * k, j);
    }
    let ph = c::add_rot(c::pf(w, id, pv::JOINT_PHASE), DT * 1.919_862_2);
    c::set_pf(w, id, pv::JOINT_PHASE, ph);
    let col = crate::particles::tween_color((ph.sin() * 0.5 + 0.5).to_bits(), JOINT_GLOW.0, JOINT_GLOW.1);
    w.mm(id).glow = col;
    c::set_pi32(w, id, pv::JOINT_RGBA, ((col & 0x00ff_ffff) | 0x3000_0000) as i32);
    if w.counter != 0 && once_per_group(w, id, GLOW_STAMP) {
        if let Some(r) = super::row(REFERENCE_LEVEL, GLOW_FN) { w.svc.draw_callbacks.register(Callback::UnitGlow(r), id); }
    }
}

/// Level05 `0x2d7140` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if fv_z(w) < MIN_CAM_Z { return; }
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    let hit = w.get_hit(id, 0x1_0000, false);
    match w.m(id).state {
        0 => {
            let s = super::class_scale(w, w.m(id).o_class) * SCALE;
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.draw_dist = 0xff;
            m.scale = s;
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::PHASE, a);
            c::set_pi32(w, id, pv::VOICE, -1);
            let b = w.rng.rand_angle();
            c::set_pf(w, id, pv::JOINT_PHASE, b);
            if let Some(i) = spline(w, id) {
                bank(&mut w.svc.splines[i]);
                let pts = &w.svc.splines[i];
                let (p0, p1) = (pts[0].map(f32::from_bits), pts.get(1).copied().unwrap_or(pts[0]).map(f32::from_bits));
                let d = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2) + (p1[2] - p0[2]).powi(2)).sqrt();
                let n = pts.len() as f32;
                c::set_pf(w, id, pv::SEG, d);
                let t = n * c::pf(w, id, pv::START);
                c::set_pf(w, id, pv::T, t);
            }
            w.mm(id).state = 1;
        }
        1 => {
            let ph = c::add_rot(c::pf(w, id, pv::PHASE), DT * 12.566_371);
            c::set_pf(w, id, pv::PHASE, ph);
            w.mm(id).glow = crate::particles::tween_color(((ph.sin() + 1.0) * 0.5).to_bits(), GLOW.0, GLOW.1);
            if let Some(i) = spline(w, id) {
                let pts = w.svc.splines[i].clone();
                let n = pts.len() as f32;
                let mut t = c::pf(w, id, pv::T) + (SPEED * DT) / c::pf(w, id, pv::SEG);
                if n < t { t -= n; }
                c::set_pf(w, id, pv::T, t);
                let (pos, rot) = crate::path::pose(&pts, true, t, true);
                let m = w.mm(id);
                m.position = pos;
                m.rotation = rot;
                m.rotation[0] = rot[3];
            }
            if !w.sound_alive(c::pi32(w, id, pv::VOICE), id) {
                let s = w.play_sound(1, 4, id);
                c::set_pi32(w, id, pv::VOICE, s);
            }
            if w.counter & 3 == 0 {
                let head = (c::pi16(w, id, pv::HEAD) + 1) & 0xf;
                c::set_pi16(w, id, pv::HEAD, head);
                let len = c::pi16(w, id, pv::LEN);
                if (len as f32) < max_len(w.svc.cheats.on(crate::cheats::slot::CONTRAILS)) { c::set_pi16(w, id, pv::LEN, len + 1); }
            }
            let (rows, pos, head) = (w.m(id).rows, w.m(id).position, c::pi16(w, id, pv::HEAD) as usize);
            for (k, o) in TRAIL_OFFSETS.iter().enumerate() {
                let q: c::V = std::array::from_fn(|j| if j == 3 { 1.0 } else { o[0] * rows[0][j] + o[1] * rows[1][j] + o[2] * rows[2][j] + pos[j] });
                c::set_pv4(w, id, pv::RINGS + 0x100 * k + 0x10 * head, q);
            }
            if once_per_group(w, id, TRAIL_STAMP) {
                if let Some(r) = super::row(REFERENCE_LEVEL, TRAIL_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(r), id); }
            }
            let coll = 8.0 <= pos[0] && 8.0 <= pos[1] && super::class_collision(w, CLASS);
            w.mm(id).has_collision = coll;
            if hit.is_some_and(|h| 0.0 < h.damage.to_f32()) { w.mm(id).state = 2; }
        }
        2 => {
            w.play_sound(0, 0, id);
            let (pos, rot, r0) = (w.m(id).position, w.m(id).rotation, w.m(id).rows[0]);
            let mut v = c::scale(r0, 0.15);
            v[2] += 0.08;
            for _ in 0..3 {
                let t = w.ticks(90);
                fx::break_piece_with(w, id, PIECE, pos, rot, t, 0, v, [0.0; 4], [0.0; 4]);
            }
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    joint_glows(w, id);
}

fn fv_z(w: &World) -> f32 { f32::from_bits(w.camera[2].0) }

fn group_riders<'a>(table: &'a MobyTable, svc: &'a Services, id: MobyId) -> impl Iterator<Item = &'a crate::moby_runtime::Moby> {
    let list = table.mobys.get(id).and_then(|m| usize::try_from(m.group).ok()).and_then(|g| svc.groups.lists.get(g)).and_then(|l| l.as_ref());
    list.into_iter().flatten().filter_map(|&e| table.mobys.get((e & 0x7fff) as usize)).filter(|m| m.o_class == CLASS && m.pvars.len() >= pv::SIZE)
}

/// `0x2d7920`: the group's ribbons (module doc; draw only).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let max = max_len(svc.cheats.on(crate::cheats::slot::CONTRAILS));
    let frac = (svc.draw_callbacks.tick.wrapping_add(3) & 3) as f32;
    let colours: Vec<[u32; 4]> = (0..16)
        .map(|i| {
            std::array::from_fn(|k| {
                let f = ((i as f32 + 1.0 - (k / 2) as f32) / (max + frac / (max * 4.0))).clamp(0.0, 1.0);
                crate::particles::tween_color(f.to_bits(), TRAIL_COLOURS.0, TRAIL_COLOURS.1)
            })
        })
        .collect();
    let mut quads = Vec::new();
    for m in group_riders(table, svc, id).filter(|m| m.state == 1) {
        let (len, head) = (p::i16(&m.pvars, pv::LEN).max(0) as usize, p::i16(&m.pvars, pv::HEAD) as usize);
        for (i, rgba) in colours.iter().enumerate().take(len.min(16)) {
            for t in 0..3 {
                let base = (head + 16 - i + 15) & 15;
                let corners = std::array::from_fn(|k| {
                    let q = p::v4f(&m.pvars, pv::RINGS + 0x100 * t + 0x10 * ((base + k / 2) & 15));
                    [q[0], q[1], q[2] + if k & 1 == 0 { 0.15 } else { -0.15 }]
                });
                quads.push(FxQuad { corners, st: TRAIL_ST, rgba: *rgba });
            }
        }
    }
    Some(FxQuads { fx: TRAIL_FX, additive: true, subtract: false, quads })
}

/// `0x2d6f48`: the group's joint glows (module doc; draw only).
pub fn glow_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Vec<GlowQuad> {
    let mut out = Vec::new();
    for m in group_riders(table, svc, id).filter(|m| m.visible != 0) {
        let rgba = p::u32(&m.pvars, pv::JOINT_RGBA);
        for k in 0..3 {
            let q = p::v4f(&m.pvars, pv::JOINTS + 0x10 * k);
            out.push(GlowQuad { size: f32::from_bits(0x3ee6_6666), pull: f32::from_bits(0x3ecc_cccd), point: [q[0], q[1], q[2]], rgba });
        }
    }
    out
}
