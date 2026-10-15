//! **Kalebo's lifts, class 1826** (level16 `0x2e7f80` with its glow `0x2e84e8`, 2 placed; census U568; the name is
//! descriptive [L]): a platform that rides up and down the height of its cylinder (pvar +0xa0, the volume table 0x1600fc)
//! and carries Ratchet. It
//! starts at the top; standing on it sends it to the other end, and so does waiting within 20 (xy) at the height of
//! the other end (within 1). While it moves it hums (a looping class sound 0) and a column of glowing rings turns up
//! the cylinder, brighter the faster it goes. Coming down onto Ratchet standing under it (within 1.5 of the cylinder's
//! centre, about 2 below it) it brakes hard and waits. Read from the level16 decomp and its words gp−0x4d54..−0x4d18.
//!
//! **Pvars** (0xc0, mode 0x20): +0x00 the creature record's offset (0x20), +0x08 the platform block's (0x60), +0xa0 the
//! cylinder, +0xa8 the target z, +0xb0 the velocity, +0xb4 the hum's voice, +0xb8 the rings' first height above the
//! cylinder's bottom, +0xbc their spacing.
//!
//! The ends: e(s) = s·(cylinder row 2).z + cylinder centre z + 0.5 (the up axis at its half height; s = +1 top, −1 bottom).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0xb4 = −1; +0xa8 = e(+1) − 0.5; +0xbc = 1 (at the top); → 2; z = e(+1) | [`update`] |
//! | state 2 | the hum released (`SoundIsAlive` 0x286708, `release_voice_slot`); `vec_distance2(position, Ratchet)` < 20 and \|Ratchet's z − e(the other end)\| < 1, or Ratchet on it (0x13f64c) and grounded → +0xbc ^= 1, +0xa8 = e(the other end), +0xb0 = 0, → 3 | [`update`] |
//! | state 3 | Ratchet more than 1.5 (`vec_distance2`, 2.25) from the cylinder's centre, or \|z − his z − 2\| > 1, or z < +0xa8: `0x257268(+0xa8, 12·dt², 12·dt², 6·dt, &z, &+0xb0)` (L01 0x270830); else the same toward z itself with 48·dt² (the brake) | [`update`] (`turn::spring`) |
//! | | the step 0 → the hum released; moving and no hum → +0xb4 = `PlayClassSound(0, 4)` | [`update`] |
//! | | z = +0xa8 and Ratchet not on it (or airborne) → +0x70 zeroed (`0x1fdac8`), → 2; else +0xb0 ≠ 0 → `RegisterDrawCallback(0x2e84e8, m, position)` | [`update`] (`Callback::UnitQuads`) |
//! | every tick | `CarryRiders(+0x60, position − old position, Euler, Euler)` (0x25c030 = L01 0x2755f8; state 3's own call with the step is overwritten by it) | [`update`] (`triggers::carry_riders`) |
//! | `0x2e84e8` | FX 0xe, ALPHA 0x48 (gp−0x4d34..−0x4d28: 0, 2, 0, 1), TEX1 bilinear; colour `FastTweenColor(\|v\|/(6·dt), 0x00408000, 0x40c08030)`; rings from the cylinder's bottom + +0xb8 to its top + +0xb8, +0xbc apart, turning `(tick mod 120)·1.5°` alternately each way: 24 quads of 15° at radius 2.9 (2.85 at the bottom edge), ±0.5 high, ST (0, 0) (0, 1) (1, 0) (1, 1) | [`fx_quads`] |

use super::{FxQuad, FxQuads};
use crate::moby_runtime::{MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_update::triggers;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_7f80;
/// The glow (draw only).
pub const GLOW_FN: u32 = 0x2e_84e8;
pub const CLASSES: [i16; 1] = [1826];

/// Pvar offsets (module doc).
pub mod pv {
    pub const BLOCK: usize = 0x60;
    pub const STOP: usize = 0x70;
    pub const CYLINDER: usize = 0xa0;
    pub const TARGET: usize = 0xa8;
    pub const VEL: usize = 0xb0;
    pub const VOICE: usize = 0xb4;
    pub const RING_BASE: usize = 0xb8;
    pub const RING_STEP: usize = 0xbc;
    pub const SIZE: usize = 0xc0;
}

/// gp−0x4d54 0.5 (the stand above an end), −0x4d48 6 (top speed per dt), −0x4d44 12 (acceleration per dt²).
pub const STAND: f32 = 0.5;
pub const VMAX: f32 = 6.0;
pub const ACCEL: f32 = 12.0;
/// The call reach (`vec_distance2` < 20) and the brake's box (2.25, 2, 1).
pub const CALL: f32 = 20.0;
/// The glow (gp−0x4d3c 2.9, −0x4d38 0.5, −0x4d24 / −0x4d20 the colours, −0x4d18 15°).
const RING_R: f32 = 2.9;
const RING_H: f32 = 0.5;
const COLOURS: (u32, u32) = (0x0040_8000, 0x40c0_8030);
const SEGMENT: f32 = 15.0;
const GLOW_FX: usize = 0xe;
const ST: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

/// The cylinder's row 2 z (its half height) and centre (the table 0x1600fc the update indexes; on Kalebo cuboids 0 / 1
/// are 1-unit boxes elsewhere, the cylinders 0 / 1 the two shafts).
fn cylinder(svc: &Services, i: i32) -> Option<(f32, [f32; 4])> {
    let s = svc.volumes.shape(rc_formats::volumes::ShapeKind::Cylinder, i)?;
    Some((s.matrix[2][2], s.matrix[3]))
}

fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, pv::VOICE);
    if w.sound_alive(v, id) {
        w.release_sound(v, id);
        c::set_pi32(w, id, pv::VOICE, -1);
    }
}

fn on_it(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 }

fn go(w: &mut World, id: MobyId, target: f32) {
    let m = w.mm(id);
    m.cmd ^= 1;
    c::set_pf(w, id, pv::TARGET, target);
    c::set_pf(w, id, pv::VEL, 0.0);
    w.mm(id).state = 3;
}

/// Level16 `0x2e7f80` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let old = w.m(id).position;
    let Some((half, centre)) = cylinder(w.svc, c::pi32(w, id, pv::CYLINDER)) else { return };
    let end = |s: f32| s * half + centre[2] + STAND;
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, pv::VOICE, -1);
            c::set_pf(w, id, pv::TARGET, half + centre[2]);
            let m = w.mm(id);
            m.cmd = 1;
            m.state = 2;
            m.position[2] = end(1.0);
        }
        2 => {
            release(w, id);
            let s = if w.m(id).cmd == 0 { 1.0 } else { -1.0 };
            let h = super::hero_pos(w);
            if (c::dist2(w.m(id).position, h) < CALL && (h[2] - end(s)).abs() < 1.0) || on_it(w, id) { go(w, id, end(s)); }
        }
        3 => {
            let h = super::hero_pos(w);
            let (z, target) = (w.m(id).position[2], c::pf(w, id, pv::TARGET));
            let clear = 2.25 < c::dist2(h, centre) || 1.0 < ((z - h[2]) - 2.0).abs() || z < target;
            let (to, accel) = if clear { (target, ACCEL * DT2) } else { (z, ACCEL * 4.0 * DT2) };
            let (mut x, mut v) = (z, c::pf(w, id, pv::VEL));
            let step = turn::spring(to, accel, accel, VMAX * DT, &mut x, &mut v);
            w.mm(id).position[2] = x;
            c::set_pf(w, id, pv::VEL, v);
            if step == 0.0 {
                release(w, id);
            } else if !w.sound_alive(c::pi32(w, id, pv::VOICE), id) {
                let s = w.play_sound(0, 4, id);
                c::set_pi32(w, id, pv::VOICE, s);
            }
            if x == target && !on_it(w, id) {
                for k in 0..4 { c::set_pf(w, id, pv::STOP + 4 * k, 0.0); }
                w.mm(id).state = 2;
            } else if v != 0.0 {
                if let Some(r) = super::row(REFERENCE_LEVEL, GLOW_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(r), id); }
            }
        }
        _ => {}
    }
    let m = w.mm(id);
    let disp = [m.position[0] - old[0], m.position[1] - old[1], m.position[2] - old[2], m.position[3] - old[3]];
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, pv::BLOCK, disp, rot, rot);
}

/// Level16 `0x2e84e8`: the rings (module doc; draw only).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id).filter(|m| m.pvars.len() >= pv::SIZE)?;
    let (half, centre) = cylinder(svc, p::i32(&m.pvars, pv::CYLINDER))?;
    let (base, step) = (p::ff(&m.pvars, pv::RING_BASE), p::ff(&m.pvars, pv::RING_STEP));
    let top = centre[2] + half + base;
    let v = p::ff(&m.pvars, pv::VEL).abs();
    let rgba = crate::particles::tween_color((v / (VMAX * DT)).to_bits(), COLOURS.0, COLOURS.1);
    let corner = |k: usize| -> [f32; 3] {
        let a = (k >> 1) as f32 * SEGMENT * 0.017_453_292;
        let r = if k & 1 == 0 { RING_R - 0.05 } else { RING_R };
        [a.cos() * r, a.sin() * r, if k & 1 == 0 { -RING_H } else { RING_H }]
    };
    let mut quads = Vec::new();
    let (mut z, mut dir) = ((centre[2] - half) + base, 1i32);
    if step <= 0.0 { return None; }
    while z < top {
        dir = -dir;
        let phase = (svc.draw_callbacks.tick % 120) as f32 * 0.026_179_917 * dir as f32;
        let n = 360.0 / SEGMENT;
        let mut i = 0.0f32;
        while i < n {
            let a = SEGMENT * 0.017_453_292 * i + phase;
            let (r0, r1) = ([a.cos(), a.sin()], [(a + 1.570_796_4).cos(), (a + 1.570_796_4).sin()]);
            let corners = std::array::from_fn(|k| {
                let l = corner(k);
                [r0[0] * l[0] + r1[0] * l[1] + m.rows[2][0] * l[2] + m.position[0], r0[1] * l[0] + r1[1] * l[1] + m.rows[2][1] * l[2] + m.position[1], l[2] + z]
            });
            quads.push(FxQuad { corners, st: ST, rgba: [rgba; 4] });
            i += 1.0;
        }
        z += step;
    }
    Some(FxQuads { fx: GLOW_FX, additive: true, subtract: false, quads })
}
