//! U97 (census 2026-10-02): class 651, Aridia's anti-grav lifts (level02 0x2dbd38, the only copy: 3 placed), and their
//! glow column (the draw callback 0x2dc2c8). Read from the level02 decomp and disassembly. Native `f32`.
//!
//! A lift rides up and down its cylinder (pvar +0xa0, the volume table 0x1600fc): its two stops are the cylinder's
//! top and bottom (centre z ± the half height, + 0.5). It starts at the top. A lift with +0xa4 = 0 waits until Ratchet
//! stands on it (or owns the Trespasser, item 26) and then works like the others: it moves when he stands on it, and
//! comes for him when he is within 20 of it at its other stop's level. Moving, it springs to the stop, stops short
//! when he is under it, carries its riders, plays its hum (class sound 0, flags 4) and draws its glow column.
//!
//! **Pvars**: +0x60 the platform block (`CarryRiders`), +0xa0 the cylinder, +0xa4 starts working at once, +0xa8 the stop
//! it moves to, +0xb0 its vertical speed, +0xb4 the hum's voice, +0xb8 the glow column's height offset; `cmd` (+0xbc)
//! the stop it is at (1 the top).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2dbd38 | old = −position (`0x210368(−1)`, the scale); the old rotation kept | [`update`] |
//! | 0 | voice −1; +0xa8 = the cylinder's row 2 z + centre z; `cmd` 1; z = that + 0.5 (gp−0x5030); +0xa4 ? 2 : 1 | [`update`] |
//! | 1 | Ratchet on it (ground moby 0x13f64c, air ticks 0x13f65e = 0) or the Trespasser owned (0x13d4da) → the turn | [`update`] ([`turn`]) |
//! | 2 | the hum released; within 20 of Ratchet (xy: `vec_distance2` 0x1f9b80): the other stop (±row 2 z + centre z + 0.5), Ratchet's z within 1 of it → `cmd` flipped, +0xa8 it, speed 0, 3; else Ratchet on it → the turn | [`update`] |
//! | 3 | Ratchet's body point (0x13f420) in the cylinder (0x2748f8), the lift within 1 of 2 above his feet and at or above the stop: brake (target its own z, 4·12·dt²); else to the stop (12·dt², gp−0x5020); max 6·dt (gp−0x5024); the linear spring 0x25df98 (= L01 0x270830) | [`update`] (`turn::spring`) |
//! | | `CarryRiders(+0x60, (0, 0, v), rot, rot)`; v = 0 → the hum released, else started when not alive | [`update`] |
//! | | at the stop: Ratchet off it → +0x70 zeroed, 2; speed ≠ 0 → `RegisterDrawCallback(0x2dc2c8)` | [`update`] |
//! | tail | `CarryRiders(+0x60, position − old, old rotation, rotation)` | [`update`] |
//! | **the turn** | the other stop from `cmd` (top when 0), `cmd` flipped, +0xa8 = it, speed 0, 3 | [`turn`] |
//! | 0x2dc2c8 | the glow column: rings every 2 (gp−0x5018) from the bottom to the top (+ +0xb8), each 24 quads (15°, gp−0x4ff0) of FX 0xe, additive (ALPHA 0, 2, 0, 1), ST (0,0) (0,1) (1,0) (1,1); a quad's corners at radius 1.8 − 0.05 (bottom, z −0.2) / 1.8 (top, z +0.2), turned by 15°·k + (counter % 120)·1.5° (alternating direction per ring) about the lift's position (its rows 2 x / y, z 1); colour `FastTweenColor(\|speed\| / 6dt, 0x00206068, 0x40107020)` | [`fx_quads`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::units::{FxQuad, FxQuads};
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::{pvar as p, Services, World};
use crate::moby_runtime::MobyTable;
use rc_formats::volumes::ShapeKind;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_FN: u32 = 0x2d_bd38;
/// The glow column's draw callback.
pub const DRAW_FN: u32 = 0x2d_c2c8;
pub const CLASSES: [i16; 1] = [651];

const BLOCK: usize = 0x60;
const CYL: usize = 0xa0;
const AUTO: usize = 0xa4;
const STOP: usize = 0xa8;
const SPEED: usize = 0xb0;
const VOICE: usize = 0xb4;
const GLOW_Z: usize = 0xb8;
const SIZE: usize = 0xbc;
/// gp−0x5030 / −0x5020 / −0x5024 (level02): the stop's height over the cylinder end, the spring's rate (·dt²) and the
/// top speed (·dt).
const LIFT: f32 = 0.5;
const ACCEL: f32 = 12.0;
const MAX: f32 = 6.0;
/// The item whose ownership starts the waiting lift (0x13d4c0 + 26).
const TRESPASSER: usize = 26;
/// The glow column (gp−0x5018 / −0x5014 / −0x5010 / −0x4ffc / −0x4ff8 / −0x4ff0).
const RING_STEP: f32 = 2.0;
const RADIUS: f32 = 1.8;
const HALF: f32 = 0.2;
const COLOUR_SLOW: u32 = 0x0020_6068;
const COLOUR_FAST: u32 = 0x4010_7020;
const ANGLE: f32 = 15.0;
const FX: usize = 0xe;

/// The cylinder's (row 2 z, centre z).
fn cylinder(svc: &Services, idx: i32) -> Option<(f32, f32)> {
    let s = svc.volumes.shape(ShapeKind::Cylinder, idx)?;
    Some((s.matrix[2][2], s.matrix[3][2]))
}

fn on_it(w: &World, id: MobyId) -> bool { w.hero.ground_moby == Some(id) && w.hero.air_ticks == 0 }

/// The other stop: `(0, 0, ±1)` through the cylinder's rows (z: ± row 2 z) + its centre z + 0.5, top when `cmd` is 0.
fn other_stop(w: &World, id: MobyId) -> f32 {
    let s = if w.m(id).cmd == 0 { 1.0 } else { -1.0 };
    let (r2, cz) = cylinder(w.svc, c::pi32(w, id, CYL)).unwrap_or((0.0, 0.0));
    s * r2 + cz + LIFT
}

/// The turn (module table): to the other stop.
fn turn(w: &mut World, id: MobyId, stop: f32) {
    let m = w.mm(id);
    m.cmd = (m.cmd.wrapping_add(1)) & 1;
    p::set_ff(&mut m.pvars, STOP, stop);
    p::set_ff(&mut m.pvars, SPEED, 0.0);
    m.state = 3;
}

fn release(w: &mut World, id: MobyId) {
    let v = c::pi32(w, id, VOICE);
    if w.sound_alive(v, id) {
        w.release_sound(v, id);
        c::set_pi32(w, id, VOICE, -1);
    }
}

/// Level02 0x2dbd38 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    let (old, old_rot) = (w.m(id).position, w.m(id).rotation);
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, VOICE, -1);
            let (r2, cz) = cylinder(w.svc, c::pi32(w, id, CYL)).unwrap_or((0.0, 0.0));
            c::set_pf(w, id, STOP, r2 + cz);
            let auto = c::pi32(w, id, AUTO) != 0;
            let m = w.mm(id);
            m.cmd = 1;
            m.position[2] = r2 + cz + LIFT;
            m.state = if auto { 2 } else { 1 };
        }
        1 => {
            if on_it(w, id) || w.hero.owned.has(TRESPASSER) {
                let s = other_stop(w, id);
                turn(w, id, s);
            }
        }
        2 => {
            release(w, id);
            // `vec_distance2` 0x1f9b80: x and y only (Ratchet at the other stop is a shaft's height away).
            let near = c::dist2(w.m(id).position, crate::moby_update::classes::units::hero_pos(w)) < 20.0;
            let s = other_stop(w, id);
            let hz = crate::moby_update::classes::units::hero_pos(w)[2];
            if (near && (hz - s).abs() < 1.0) || on_it(w, id) { turn(w, id, s); }
        }
        3 => moving(w, id),
        _ => {}
    }
    let pos = w.m(id).position;
    let rot = w.m(id).rotation;
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3] - old[3]];
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, delta, old_rot, rot);
}

/// State 3 (module table).
fn moving(w: &mut World, id: MobyId) {
    let body = w.hero.body_point.map(|x| f32::from_bits(x.0));
    let feet_z = crate::moby_update::classes::units::hero_pos(w)[2];
    let z = w.m(id).position[2];
    let stop = c::pf(w, id, STOP);
    let inside = w.in_cylinder([body[0], body[1], body[2]], c::pi32(w, id, CYL));
    let brake = inside && ((z - feet_z) - 2.0).abs() <= 1.0 && stop <= z;
    let (target, k) = if brake { (z, ACCEL * 4.0 * DT2) } else { (stop, ACCEL * DT2) };
    let mut zz = z;
    let mut v = c::pf(w, id, SPEED);
    let step = turn::spring(target, k, k, MAX * DT, &mut zz, &mut v);
    w.mm(id).position[2] = zz;
    c::set_pf(w, id, SPEED, v);
    let rot = w.m(id).rotation;
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, BLOCK, [0.0, 0.0, step, 0.0], rot, rot);
    if step == 0.0 {
        release(w, id);
    } else {
        let s = c::pi32(w, id, VOICE);
        if !w.sound_alive(s, id) {
            let s = w.play_sound(0, 4, id);
            c::set_pi32(w, id, VOICE, s);
        }
    }
    if w.m(id).position[2] == c::pf(w, id, STOP) && !on_it(w, id) {
        for k in 0..4 { c::set_pf(w, id, BLOCK + 0x10 + 4 * k, 0.0); }
        w.mm(id).state = 2;
        return;
    }
    if c::pf(w, id, SPEED) != 0.0 {
        if let Some(i) = crate::moby_update::classes::units::row(REFERENCE_LEVEL, DRAW_FN) {
            w.svc.draw_callbacks.register(crate::moby_update::classes::draw_callbacks::Callback::UnitQuads(i), id);
        }
    }
}

/// `FastNormalizeAngle` 0x222088.
fn norm(a: f32) -> f32 { crate::moby_update::services::normalize_angle(crate::ps2v::Pf::f(a)).to_f32() }

/// Level02 0x2dc2c8, the glow column (module table).
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < SIZE { return None; }
    let (half, cz) = cylinder(svc, p::i32(&m.pvars, CYL))?;
    let lift = p::ff(&m.pvars, GLOW_Z);
    let top = cz + half + lift;
    let t = p::ff(&m.pvars, SPEED).abs() / (MAX * DT);
    let rgba = crate::hud::tween_color(t, COLOUR_SLOW, COLOUR_FAST);
    let fcos = |a: f32| crate::hero::physics::fast_cos(crate::ps2v::Pf::f(a)).to_f32();
    let fsin = |a: f32| crate::hero::physics::fast_sin(crate::ps2v::Pf::f(a)).to_f32();
    let deg = 0.017_453_292;
    // The quad's local corners: bottom at the narrower radius, then top, at 0 and at one step.
    let local: [[f32; 3]; 4] = std::array::from_fn(|k| {
        let (r, z) = if k & 1 == 0 { (RADIUS - 0.05, -HALF) } else { (RADIUS, HALF) };
        let a = (k >> 1) as f32 * ANGLE * deg;
        [fcos(a) * r, fsin(a) * r, z]
    });
    let row2 = [m.rows[2][0], m.rows[2][1], 1.0];
    let mut quads = Vec::new();
    let mut dir = 1.0f32;
    let mut z = (cz - half) + lift;
    let n = (360.0 / ANGLE) as usize;
    let spin = ((svc.draw_callbacks.tick % 120) as f32) * 0.026_179_917;
    while z < top {
        dir = -dir;
        let base = spin * dir;
        for j in 0..n {
            let a = norm(ANGLE * deg * j as f32 + base);
            let b = norm(ANGLE * deg * j as f32 + std::f32::consts::FRAC_PI_2 + base);
            let r0 = [fcos(a), fsin(a), 0.0];
            let r1 = [fcos(b), fsin(b), 0.0];
            let corners = local.map(|l| std::array::from_fn(|k| l[0] * r0[k] + l[1] * r1[k] + l[2] * row2[k] + if k == 2 { z } else { m.position[k] }));
            quads.push(FxQuad { corners, st: [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]], rgba: [rgba; 4] });
        }
        z += RING_STEP;
    }
    Some(FxQuads { fx: FX, additive: true, subtract: false, quads })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    /// State 2: Ratchet within 20 (xy, `vec_distance2`) at the other stop's height calls the lift there. Lowered after
    /// the Trespasser, Aridia's top lift (#564) must come back up for him: he stands a shaft's height above it.
    #[test]
    fn a_lift_below_comes_up_for_ratchet_standing_above_it() {
        let lift = Moby { o_class: 651, scale: 1.0, state: 2, cmd: 0, position: [100.0, 100.0, -29.5, 1.0], pvars: vec![0; SIZE], hit_slot: 0xff, ..Moby::default() };
        let mut t = MobyTable::new(vec![lift], 8);
        let mut hero = crate::hero::Hero::new();
        hero.pos = [100.0f32, 103.0, 0.5, 1.0].map(crate::ps2v::Pf::f);
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!((w.m(0).state, w.m(0).cmd), (3, 1), "the lift did not set off for the top");
        assert_eq!(c::pf(&w, 0, STOP), 0.5);
    }
}
