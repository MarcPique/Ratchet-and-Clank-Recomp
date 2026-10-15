//! Quartu's shaking critters, class 221 (level 15, 24 created instances): level15 0x2c2938 (census U485). An idler
//! that waits a random 2–20 s, then (when the camera is within 42 and no other member of its group is busy) turns its
//! head a little (±12°, the NPC look-at record on joint list 0), plays one of two shake animations, sheds dust from
//! joint 1 while shaking near the camera, and settles back. Any hit with damage kills it: class sound 0, the death
//! explosion, a bolt burst and three pieces. Read from the level15 decomp of 0x2c2938 and the overlay's data (gp−0x5200
//! / −0x51fc / −0x5204 / −0x520c / −0x5208 / −0x5214). Native `f32`; the `rand` draws in the game's order.
//!
//! **Pvar block**: +0x00..+0x7f the look-at record (`manip::look`; its yaw target +0x68), +0x80 s16 the timer, +0x82
//! s16 the shake (0 / 1), +0x84 the head angle, +0x88 its spring velocity, +0x8c its target, +0x90 the dust heading.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2c2938 | state ≠ 6: `MobyGetHitMessage(m, 0x10000, 0)` (0x249d08 = L01 0x26f320) with damage +0x2c > 0 → state 6 | [`update`] (`World::get_hit`) |
//! | | state ≠ 0: record yaw target +0x68 = +0x84; `FUN_002777d8(0.03·k, 0.3·k, m, +0x00, 0)` (0x251fd0 = L01 0x2777d8; k = [0x15ed64] = 1) | [`update`] (`manip::look`) |
//! | state 0 | timer = trunc(`randf(scale(2·60), scale(20·60))`) (gp−0x5200 / −0x51fc, `multiply_global_scale`), +0x88 = +0x84 = 0, +0x82 = 0, → 1, update distance 0xff | [`update`] |
//! | state 1 | `FastDecTimer(s16 +0x80)` out and `vec_distance`(camera 0x1673c0, position) < 42: group ≠ 0xff → count the members of the group list (0x1abe40 on 15) with state ≥ 0, this class and state ≠ 1; none listed → return; any → timer = `ticks(60)`, stay | [`update`] (`scheduler::group_ids`) |
//! | | else +0x8c = `randf(−0.20944, 0.20944)` (±12°), → 2 | [`update`] |
//! | state 2 | +0x82 = `rand_range(0, 1)` (0x2471f8 = L01 0x26c970); `FastDiffRots(+0x84, +0x8c)` < 0.5° → blend seq 2·shake + 1, frame 0, `ticks(6)` (`moby_set_anim_snapshot` 0x212f90), → 3; +0x84 = `SpringTurn(+0x84, +0x8c, dt²·π/2, dt²·π/4, dt·π/4, &+0x88)` (0x247778 = L01 0x26cef0) | [`update`] (`World::anim_blend`, `turn::spring_turn`) |
//! | state 3 | seq B (+0x53) = 2·shake + 1 and anim flags +0x70 & 2 → timer = `ticks(rand_range(15, 45))`, blend seq 2·shake + 2, frame 0, `ticks(3)`, +0x90 = `rand_angle`, → 4 | [`update`] |
//! | state 4 | camera within 20, `randi(2)` = 0 (0x2471b8 = L01 0x26c930) and drawn last frame (+0x31): s1 = `randf(10/5, 10)`·dt (gp−0x5204), s2 = `randf(0, 10/5)`·dt, a = `rand_angle`; v = (cos +0x90, sin +0x90, 0)·s1 + (cos a, sin a, 0)·s2 (0x1f8a08 = L01 0x2211a0); joint 1's point (0x23ee30 = L01 0x2645a8); `PartType21Spawn(40000, point, v, 0x40808080, 0x40808080, rand_range(ticks(60), ticks(120)), 1)` (0x25c2f0 = L01 0x281c10; gp−0x5214, gp−0x520c / −0x5208) | [`update`] (`particles::type21::spawn_rng`) |
//! | | `FastDecTimer(s16 +0x80)` out → blend seq 2·shake + 3, frame 0, `ticks(6)`, → 5 | [`update`] |
//! | state 5 | seq B = 2·shake + 3 and flags & 2 → timer = trunc(`randf(scale(120), scale(1200))`), blend seq 0, frame 0, `ticks(6)`, → 1 | [`update`] |
//! | state 6 | `PlayClassSound(0, 0, m)` (0x22da68); `0x273f50(1.5, 13, m, position, −1)` (0x24e938, the death explosion); `BreakFxA(m)` (0x253268 = L01 0x2787a0: 4..7 bolts); `BreakFxB(0, m, 0x74b / 0x74c / 0x74d, position, rotation, 0, 0, 0, 0)` (0x253570 = L01 0x278ad8); `DeleteMoby(m)` | [`update`] (`fx::death_explosion`, `breakables::bolts`, `fx::break_piece`) |
//! | | no light (the explosion's own), save flag, HUD write | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::breakables;
use crate::moby_update::creature::{self as c, fx, turn::spring_turn, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::scheduler::group_ids;
use crate::moby_update::services::{fast_dec_timer_s16, World};

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2c_2938;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [221];
/// The pieces of its death (`BreakFxB`).
pub const PIECES: [i16; 3] = [0x74b, 0x74c, 0x74d];

pub mod pv {
    pub const REC: usize = 0x00;
    pub const YAW_TARGET: usize = 0x68;
    pub const TIMER: usize = 0x80;
    pub const SHAKE: usize = 0x82;
    pub const HEAD: usize = 0x84;
    pub const HEAD_V: usize = 0x88;
    pub const HEAD_T: usize = 0x8c;
    pub const DUST: usize = 0x90;
}

/// gp−0x5200 / −0x51fc: the wait range (seconds); gp−0x5204: the dust speed (units per second).
pub const WAIT_LO: f32 = 2.0;
pub const WAIT_HI: f32 = 20.0;
pub const DUST_SPEED: f32 = 10.0;
pub const DUST_RGBA: u32 = 0x4080_8080;
/// ±12° (0x3e567750).
pub const HEAD_RANGE: f32 = f32::from_bits(0x3e56_7750);

fn timer(w: &mut World, id: MobyId) -> i32 {
    let mut t = c::pi16(w, id, pv::TIMER);
    let r = fast_dec_timer_s16(&mut t);
    c::set_pi16(w, id, pv::TIMER, t);
    r
}

fn wait(w: &mut World, id: MobyId) {
    let s = w.svc.timing.timer_scale.to_f32();
    let (lo, hi) = (s * (WAIT_LO * 60.0), s * (WAIT_HI * 60.0));
    let t = w.rng.randf(lo, hi) as i32;
    c::set_pi16(w, id, pv::TIMER, t as i16);
}

fn anim_done(w: &World, id: MobyId, seq: i32) -> bool {
    let a = &w.m(id).anim;
    a.seq_b as i32 == seq && a.flags & 2 != 0
}

/// Level15 0x2c2938 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::DUST + 4 { return; }
    if w.m(id).state != 6 && w.get_hit(id, 0x1_0000, false).is_some_and(|h| 0.0 < h.damage.to_f32()) { w.mm(id).state = 6; }
    if w.m(id).state != 0 {
        let a = c::pf(w, id, pv::HEAD);
        c::set_pf(w, id, pv::YAW_TARGET, a);
        manip::look(w, id, id, pv::REC, 0, 0.03, 0.3);
    }
    let shake = c::pi16(w, id, pv::SHAKE) as i32;
    match w.m(id).state {
        0 => {
            wait(w, id);
            c::set_pf(w, id, pv::HEAD_V, 0.0);
            c::set_pf(w, id, pv::HEAD, 0.0);
            c::set_pi16(w, id, pv::SHAKE, 0);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if timer(w, id) == 0 { return; }
            let cam = w.camera_point();
            if 42.0 <= c::dist3([cam[0], cam[1], cam[2], 0.0], w.m(id).position) { return; }
            let g = w.m(id).group;
            if g as u8 != 0xff {
                let ids = group_ids(w, g);
                if ids.is_empty() { return; }
                let me = w.m(id).o_class;
                let busy = ids.iter().filter(|&&m| w.table.mobys.get(m).is_some_and(|o| (o.state as i8) >= 0 && o.o_class == me && o.state != 1)).count();
                if busy != 0 {
                    let t = w.ticks(0x3c);
                    c::set_pi16(w, id, pv::TIMER, t as i16);
                    return;
                }
            }
            let t = w.rng.randf(-HEAD_RANGE, HEAD_RANGE);
            c::set_pf(w, id, pv::HEAD_T, t);
            w.mm(id).state = 2;
        }
        2 => {
            let s = w.rng.rand_range(0, 1);
            c::set_pi16(w, id, pv::SHAKE, s as i16);
            let (cur, tgt) = (c::pf(w, id, pv::HEAD), c::pf(w, id, pv::HEAD_T));
            if c::diff_rots(cur, tgt) < 0.008_726_646 {
                let t = w.ticks(6);
                w.anim_blend(id, (s * 2 + 1) as u8, 0, t);
                w.mm(id).state = 3;
            }
            let mut v = c::pf(w, id, pv::HEAD_V);
            let a = spring_turn(cur, tgt, DT2 * std::f32::consts::FRAC_PI_2, DT2 * std::f32::consts::FRAC_PI_4, DT * std::f32::consts::FRAC_PI_4, &mut v);
            c::set_pf(w, id, pv::HEAD, a);
            c::set_pf(w, id, pv::HEAD_V, v);
        }
        3 => {
            if !anim_done(w, id, shake * 2 + 1) { return; }
            let n = w.rng.rand_range(0xf, 0x2d);
            let t = w.ticks(n);
            c::set_pi16(w, id, pv::TIMER, t as i16);
            let b = w.ticks(3);
            w.anim_blend(id, (shake * 2 + 2) as u8, 0, b);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, pv::DUST, a);
            w.mm(id).state = 4;
        }
        4 => {
            let cam = w.camera_point();
            if c::dist3(w.m(id).position, [cam[0], cam[1], cam[2], 0.0]) < 20.0 && w.rng.randi(2) == 0 && w.m(id).visible != 0 {
                let s1 = w.rng.randf(DUST_SPEED / 5.0, DUST_SPEED) * DT;
                let s2 = w.rng.randf(0.0, DUST_SPEED / 5.0) * DT;
                let a = w.rng.rand_angle();
                let h = c::pf(w, id, pv::DUST);
                let v = [h.cos() * s1 + a.cos() * s2, h.sin() * s1 + a.sin() * s2, 0.0, 0.0];
                let p = w.joint_point(id, 1);
                let (lo, hi) = (w.ticks(0x3c), w.ticks(0x78));
                let life = w.rng.rand_range(lo, hi);
                *w.svc.fx.part_spawns.entry(21).or_default() += 1;
                if let Some(sys) = w.particles.as_deref_mut() {
                    if crate::particles::type21::spawn_rng(sys, w.rng, 40000.0, p, v, DUST_RGBA, DUST_RGBA, life, 1).is_none() { w.svc.fx.part_failed += 1; }
                }
            }
            if timer(w, id) == 0 { return; }
            let t = w.ticks(6);
            w.anim_blend(id, (shake * 2 + 3) as u8, 0, t);
            w.mm(id).state = 5;
        }
        5 => {
            if !anim_done(w, id, shake * 2 + 3) { return; }
            wait(w, id);
            let t = w.ticks(6);
            w.anim_blend(id, 0, 0, t);
            w.mm(id).state = 1;
        }
        6 => {
            w.play_sound(0, 0, id);
            let (pos, rot) = (w.m(id).position, w.m(id).rotation);
            fx::death_explosion(w, 1.5, 13.0, Some(id), pos, -1);
            breakables::bolts(w, id);
            for cl in PIECES { fx::break_piece(w, id, cl, pos, rot, 0, 0); }
            w.delete_moby(id);
        }
        _ => {}
    }
}
