//! U167 (census 2026-10-02): Eudora's flying machines, classes 466, 480, 485, 486, 488, 490, 493, 494, 495, 498 and
//! 555 (level04 0x2ca420, 38 placed). Read from the level04 decomp and disassembly. Native `f32`; the `rand` draws
//! (the break pieces) in the game's order.
//!
//! Each flies its path with the shared flyer driver (`FlyerPathDriver` = L01 0x2f5168, here 0x2cb8b8:
//! [`flyer::driver`]), spins its rotors (manipulators on its joint lists), may carry a rider prop (476–478, a moby link
//! at +0x17c) and may stop over landing pads (up to four cuboids). A hit blows it apart (an explosion, break pieces, for
//! some a blast that hurts what is near) and hides it for ten seconds; it comes back once the camera looks away. Ten
//! hits on the level give skill point 6 (0x13d40e).
//!
//! **Pvars** (P; the driver's own up to +0x134, see [`flyer`]): +0x7c the pad turn's velocity, +0x11c (s32 written
//! over the driver's s16 delay and loop-sound handle), +0x138 the cruise speed (the placed +0xfc), +0x140 / +0x144 the
//! rotor angles, +0x148 byte the pad cuboid, +0x149 byte the pad state, +0x14a s16 the pad timer, +0x14c the yaw to
//! return to, +0x150 four pad cuboids (−1 none), +0x160 the place to return to, +0x170 the hidden timer, +0x174 hidden,
//! +0x178 the draw distance, +0x17c the rider, +0x180 / +0x1c0 the rotor manipulators (lists 0 / 1).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | the path's closing point dropped when within 0.5 of the first (`vec_distance`; the driver does it again); draw distance 0x7f, update distance 0xff, P+0x138 = P+0xfc, P+0x178 = 0x7f | [`update`] |
//! | every tick | `FlyerPathDriver` | [`update`] ([`flyer::driver`]) |
//! | not 484 | `FastDecTimer__FRs(P+0x14a)`; the pad state (below) | [`pads`] |
//! | pad 0 | P+0xfc eases up to P+0x138 by 20·dt²; the timer out: the first pad holding its position (`PointInCuboid`) → P+0x160 = position, P+0x148 = it, pad state 1, P+0x7c = 0, P+0x14c = yaw, the moby's state 3, P+0x11c = `ticks(36000)` as a word | [`pads`] |
//! | pad 1 | `SpringTurn2(the cuboid's yaw, dt²·π/3, dt²·π, dt·π, m, P+0x7c)`; within 1 (xy, `vec_distance2`) of its centre and 10° of its yaw → 2, P+0x14a = `ticks(300)`; moves 3·dt toward the centre | [`pads`] |
//! | pad 2 | the timer out → 3, P+0x7c = 0 | [`pads`] |
//! | pad 3 | the turn back to P+0x14c; within 1 (xy) of P+0x160 and 2° → 0, P+0x14a = `ticks(600)`, P+0x7c, P+0xfc, P+0x11c = 0; moves min(3·dt, the distance) back | [`pads`] |
//! | state 0 (as it was before the driver) | `AttachManipulator(m, 0, P+0x180)`, P+0x140 = 0; 488 / 493 also list 1 at P+0x1c0, P+0x144 = 0 | [`update`] (`manip::attach`) |
//! | else | the rotors: angle += 4π·dt, `0x221e38(angle, node + 0x10, axis)` (466 / 480 axis 0, the rest axis 2; 488 / 493 both rotors) | [`update`] (`manip::set_axis`) |
//! | rider (480, 494) | its draw distance 0x7f; at the flyer's position + its rows · (0.5, 0, −5) (480) or (0, 0, −2) (494); its Euler the flyer's (480: y + π/2); `moby_anim_advance`, `moby_build_rotation`, `moby_anim_sphere_lerp`; mode \|= 6 | [`rider`] |
//! | hit (`MobyGetHitMessage(m, 0x210000)`) | the level's hit count gp−0x7e08 + 1; at ten: skill point 6 (`PlayLevelSoundAtMoby(1)`, banner 0x53d6) | [`hit`] (`story::award_skill_point`) |
//! | hit 485 / 486 | a blast: `coll_sphere_mobys(4, pos, 0x10, m, {flags 0x810000, damage 1})`; then as below | [`hit`] |
//! | hit, all but 493 | `SpawnBeamExplosion` [`BEAM`] at pos + (0, 0, 0.75); class sound 0 (498, 555), none (486) or 1; hidden for `ticks(600)` | [`hit`] |
//! | hit 493 | `0x273f50(1, 13, m, joint + (0, 0, 0.75), −1)` at joints 0 and 1; [`BEAM_BIG`]; class sound 1; a blast of 6, damage 2; hidden for `ticks(600)` | [`hit`] |
//! | hit | `BreakFxB(dt²·12, m, piece, pos, Euler, ticks(90), 0, row 0 · 0.075 + (0, 0, 0.08))` per piece of [`PIECES`] | [`hit`] (`fx::break_piece_with`) |
//! | always | +0xa4 = 0xff | [`update`] |
//! | hidden | `FastDecTimer__FRi(P+0x170)` out and the camera's yaw over π/2 from the way to the flyer: collision back, mode &= ~0x41 (and the rider's), P+0x174 = 0, mode \|= 0x1000; else no collision, mode \|= 0x41 (and the rider's), mode &= ~0x1000 | [`update`] |
//!
//! **The game's, kept:** the pad landing writes `ticks(36000)` as a word over the driver's s16 delay (it reads −29536)
//! and its loop-sound handle (0), so the driver's state 3 ends on the next tick and the flyer flies on along its path
//! while the pad code turns it and pulls at it. **Not the game's, noted [L]:** the hit count gp−0x7e08 (0x15edf8, boot
//! data no other code touches) lasts the power-on session in the game; the port's starts at 0 on each level load.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::flyer;
use crate::moby_update::creature::{self as c, fx, turn, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{pf, pv, HitTemplate, World};
use crate::moby_update::story;
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, PI};

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2c_a420;
pub const CLASSES: [i16; 11] = [466, 480, 485, 486, 488, 490, 493, 494, 495, 498, 555];
/// The classes with rotors (their joint lists hold the manipulators' targets).
pub const JOINTS: [i16; 10] = [466, 480, 485, 486, 488, 493, 494, 495, 498, 555];

const SPLINE: usize = 0x74;
const TURN_V: usize = 0x7c;
const SPEED: usize = 0xfc;
const DELAY: usize = 0x11c;
const CRUISE: usize = 0x138;
const ROTOR0: usize = 0x140;
const ROTOR1: usize = 0x144;
const PAD: usize = 0x148;
const PAD_STATE: usize = 0x149;
const PAD_T: usize = 0x14a;
const BACK_YAW: usize = 0x14c;
const PADS: usize = 0x150;
const BACK_POS: usize = 0x160;
const HIDE_T: usize = 0x170;
const HIDDEN: usize = 0x174;
const DRAW: usize = 0x178;
const RIDER: usize = 0x17c;
const MANIP0: usize = 0x180;
const MANIP1: usize = 0x1c0;
const SIZE: usize = 0x200;

/// The level's hit count (gp−0x7e08).
const HITS: u32 = 0x15_edf8;
/// Skill point 0x13d40e.
const SKILL: u32 = 0x13_d40e;

/// `SpawnBeamExplosion(0, 0, 2, 1, 100000, 3, 15, m, 0, p, 10, 3, 4, −1, 1, 1, −1, 0)` (0x2cad18).
pub const BEAM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 100000.0, scale: 3.0, light: 15.0, streaks: 10, sparks: 3, puffs: 4, debris: 1, sound: -1, shake: true };
/// 493's (0x2caea0): 20 streaks, 5 spark pairs.
pub const BEAM_BIG: fx::Beam = fx::Beam { streaks: 20, sparks: 5, ..BEAM };

/// The break pieces per class (`BreakFxB`, in the game's order).
pub const PIECES: &[(i16, &[i16])] = &[
    (466, &[0x63f, 0x641, 0x642]),
    (480, &[0x642, 0x643, 0x644]),
    (486, &[0x645, 0x646, 0x646, 0x646, 0x646, 0x646, 0x646]),
    (490, &[0x647, 0x648, 0x649]),
    (494, &[0x64d, 0x64e]),
    (495, &[0x64f, 0x650, 0x651, 0x658]),
    (498, &[0x652, 0x653, 0x654]),
    (493, &[0x793, 0x794, 0x795, 0x795, 0x795, 0x796, 0x796, 0x796]),
];

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&m| m < w.table.mobys.len()) }

fn two_rotors(class: i16) -> bool { matches!(class, 488 | 493) }

/// Level04 0x2ca420 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    let st = w.m(id).state;
    if st == 0 { init(w, id); }
    flyer::driver(w, id);
    let class = w.m(id).o_class;
    if class != 484 { pads(w, id); }
    if st == 0 {
        if JOINTS.contains(&class) {
            manip::attach(w, id, 0, id, MANIP0);
            c::set_pf(w, id, ROTOR0, 0.0);
            if two_rotors(class) {
                manip::attach(w, id, 1, id, MANIP1);
                c::set_pf(w, id, ROTOR1, 0.0);
            }
        }
    } else if JOINTS.contains(&class) {
        let axis = if matches!(class, 466 | 480) { 0 } else { 2 };
        let a = c::add_rot(c::pf(w, id, ROTOR0), DT * 4.0 * PI);
        c::set_pf(w, id, ROTOR0, a);
        manip::set_axis(w, id, id, MANIP0, a, axis);
        if two_rotors(class) {
            let a = c::add_rot(c::pf(w, id, ROTOR1), DT * 4.0 * PI);
            c::set_pf(w, id, ROTOR1, a);
            manip::set_axis(w, id, id, MANIP1, a, 2);
        }
    }
    rider(w, id);
    if w.get_hit(id, 0x21_0000, false).is_some() { hit(w, id); }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, HIDDEN) == 0 { return; }
    let rider = link(w, id, RIDER);
    if c::dec_timer_pvar_i32(w, id, HIDE_T) != 0 {
        let p = w.m(id).position;
        let cam = w.camera.map(|x| x.to_f32());
        let a = c::atan(p[0] - cam[0], p[1] - cam[1]);
        if FRAC_PI_2 < c::diff_rots(w.camera_yaw, a) {
            let coll = super::class_collision(w, class);
            let m = w.mm(id);
            m.has_collision = coll;
            m.mode &= !0x41;
            c::set_pi32(w, id, HIDDEN, 0);
            if let Some(r) = rider { w.mm(r).mode &= !0x41; }
            w.mm(id).mode |= mode::TARGETABLE;
            return;
        }
    }
    let m = w.mm(id);
    m.has_collision = false;
    m.mode |= 0x41;
    if let Some(r) = rider { w.mm(r).mode |= 0x41; }
    w.mm(id).mode &= !mode::TARGETABLE;
}

/// State 0's own part (module table).
fn init(w: &mut World, id: MobyId) {
    let spline = c::pi32(w, id, SPLINE);
    if let Some(s) = usize::try_from(spline).ok().filter(|&s| s < w.svc.splines.len()) {
        let pts = &w.svc.splines[s];
        if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
            if c::dist3(a.map(f32::from_bits), b.map(f32::from_bits)) < 0.5 { w.svc.splines[s].pop(); }
        }
    }
    let m = w.mm(id);
    m.draw_dist = 0x7f;
    m.update_dist = 0xff;
    let v = c::pf(w, id, SPEED);
    c::set_pf(w, id, CRUISE, v);
    c::set_pi32(w, id, DRAW, 0x7f);
}

/// The pad cuboid `k`'s centre and yaw.
fn pad(w: &World, k: u8) -> Option<(c::V, f32)> {
    let s = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, k as i32)?;
    Some((s.matrix[3], s.euler[2]))
}

/// The pad states (module table).
fn pads(w: &mut World, id: MobyId) {
    c::dec_timer_pvar_s16(w, id, PAD_T);
    let acc = DT2 * (PI / 3.0);
    match c::pu8(w, id, PAD_STATE) {
        0 => {
            let (cruise, v) = (c::pf(w, id, CRUISE), c::pf(w, id, SPEED));
            if v < cruise {
                let n = v + DT2 * 20.0;
                c::set_pf(w, id, SPEED, if cruise < n { cruise } else { n });
            }
            for k in 0..4 {
                let cub = c::pi32(w, id, PADS + 4 * k);
                if c::pi16(w, id, PAD_T) != 0 || cub == -1 { continue; }
                let p = w.m(id).position;
                if !w.in_cuboid([p[0], p[1], p[2]], cub) { continue; }
                c::set_pv4(w, id, BACK_POS, p);
                c::set_pu8(w, id, PAD_STATE, 1);
                c::set_pu8(w, id, PAD, cub as u8);
                c::set_pf(w, id, TURN_V, 0.0);
                let yaw = w.m(id).rotation[2];
                c::set_pf(w, id, BACK_YAW, yaw);
                w.mm(id).state = 3;
                let t = w.ticks(36000);
                c::set_pi32(w, id, DELAY, t);
                break;
            }
        }
        1 => {
            let Some((centre, yaw)) = pad(w, c::pu8(w, id, PAD)) else { return };
            turn::spring_turn2_pvar(w, id, yaw, acc, DT2 * PI, DT * PI, TURN_V);
            let p = w.m(id).position;
            if c::dist2(p, centre) < 1.0 && c::diff_rots(w.m(id).rotation[2], yaw) < 0.174_532_92 {
                c::set_pu8(w, id, PAD_STATE, 2);
                let t = w.ticks(300);
                c::set_pi16(w, id, PAD_T, t as i16);
            }
            let step = c::set_len3(c::sub(centre, p), DT * 3.0);
            c::set_pos(w, id, c::add(p, step));
        }
        2 => {
            if c::pi16(w, id, PAD_T) == 0 {
                c::set_pf(w, id, TURN_V, 0.0);
                c::set_pu8(w, id, PAD_STATE, 3);
            }
        }
        3 => {
            let yaw = c::pf(w, id, BACK_YAW);
            turn::spring_turn2_pvar(w, id, yaw, acc, DT2 * PI, DT * PI, TURN_V);
            let p = w.m(id).position;
            let back = c::pv4(w, id, BACK_POS);
            if c::dist2(p, back) < 1.0 && c::diff_rots(w.m(id).rotation[2], yaw) < 0.034_906_585 {
                c::set_pu8(w, id, PAD_STATE, 0);
                let t = w.ticks(600);
                c::set_pi16(w, id, PAD_T, t as i16);
                c::set_pf(w, id, TURN_V, 0.0);
                c::set_pf(w, id, SPEED, 0.0);
                c::set_pi32(w, id, DELAY, 0);
            }
            let d = c::sub(back, p);
            let step = c::set_len3(d, c::len3(d).min(DT * 3.0));
            c::set_pos(w, id, c::add(p, step));
        }
        _ => {}
    }
}

/// The rider of 480 / 494 (module table).
fn rider(w: &mut World, id: MobyId) {
    let Some(r) = link(w, id, RIDER) else { return };
    w.mm(r).draw_dist = 0x7f;
    let class = w.m(id).o_class;
    let off = match class { 480 => [0.5, 0.0, -5.0], 494 => [0.0, 0.0, -2.0], _ => return };
    let (p, rows, e) = { let m = w.m(id); (m.position, m.rows, m.rotation) };
    let d: c::V = std::array::from_fn(|k| rows[0][k] * off[0] + rows[1][k] * off[1] + rows[2][k] * off[2]);
    {
        let m = w.mm(r);
        m.position = c::add(p, d);
        m.rotation = e;
        if class == 480 { m.rotation[1] = c::add_rot(e[1], FRAC_PI_2); }
    }
    crate::moby_update::anim_sound::advance(w, r);
    w.build_matrix(r);
    w.mm(r).mode |= 6;
}

/// `coll_sphere_mobys(r, pos, 0x10, m, {0, m, 0x810000, damage})`: the blast of 485 / 486 / 493.
fn blast(w: &mut World, id: MobyId, r: f32, damage: f32) {
    let t = HitTemplate { dir: [Pf::ZERO; 4], attacker: Some(id), flags: 0x81_0000, b18: 0, b19: 0, h1a: 0, damage: pf(damage), w20: 0 };
    let p = w.m(id).position;
    w.sphere_mobys(pf(r), pv(p), 0x10, Some(id), Some(&t));
}

/// The hit (module table).
fn hit(w: &mut World, id: MobyId) {
    let n = w.svc.units.word(HITS).wrapping_add(1);
    w.svc.units.set_word(HITS, n);
    if 10 <= n as i32 { story::award_skill_point(w, story::skill_index(SKILL)); }
    let class = w.m(id).o_class;
    let up: c::V = [0.0, 0.0, 0.75, 0.0];
    if class == 493 {
        for list in 0..2 {
            let j = c::add(w.joint_point(id, list), up);
            fx::death_explosion(w, 1.0, 13.0, Some(id), j, -1);
        }
        let p = c::add(up, w.m(id).position);
        fx::beam_explosion(w, &BEAM_BIG, Some(id), p);
        w.play_sound(1, 0, id);
        blast(w, id, 6.0, 2.0);
    } else {
        if matches!(class, 485 | 486) { blast(w, id, 4.0, 1.0); }
        let p = c::add(up, w.m(id).position);
        fx::beam_explosion(w, &BEAM, Some(id), p);
        match class {
            498 | 555 => { w.play_sound(0, 0, id); }
            486 => {}
            _ => { w.play_sound(1, 0, id); }
        }
    }
    let t = w.ticks(600);
    c::set_pi32(w, id, HIDE_T, t);
    c::set_pi32(w, id, HIDDEN, 1);
    let Some(&(_, pieces)) = PIECES.iter().find(|e| e.0 == class) else { return };
    let (p, e, r0) = { let m = w.m(id); (m.position, m.rotation, m.rows[0]) };
    let mut v = c::scale([r0[0], r0[1], r0[2], 0.0], 0.075);
    v[2] += 0.08;
    for &k in pieces {
        let t = w.ticks(90);
        fx::break_piece_with(w, id, k, p, e, t, 0, v, [0.0; 4], [0.0; 4]);
    }
}
