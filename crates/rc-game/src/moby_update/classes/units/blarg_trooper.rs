//! **Blarg's troopers, class 1048** (level06 `0x2f7d78` with its hits `0x2f91e8`, its group wake `0x2f9698` and its
//! surround angle `0x2f96f8`; 27 placed, 20 made; census U232; the name is descriptive [L]). Melee walkers in moby
//! groups inside an arena (path +0x1b0: its walls and polygon). They wait (or sleep until Ratchet or a hit wakes the
//! group), stand up and look round, then close in on their target spreading round it with their group (each takes
//! the middle of the widest free arc), jab at keys 1–8 (a 0.5 sphere at joint list 0, damage 1) when Ratchet is not
//! already reeling (0x13f510), back off and come again; alerted, they push along the walls and rear up when one is in
//! the way. Some leap in from a cuboid (+0x1c0 trigger, +0x1bc landing); some (+0x1cc) only guard and jab. They hurt
//! on contact while moving (a 0.7 sphere, damage 0.9). Four health; knocked back by hits (outside the arena when they
//! land: deleted), killed with a piece burst. Read from the level06 decomp; its words gp−0x4d50..−0x4d14. Native
//! `f32`.
//!
//! **Pvars** (0x220; the header: damage +0x20, flash +0x60, knockback +0x70, walker +0xd0): +0x38 the lure, +0x120 the
//! target record (+0x160 its moby, here the runtime index + 1; +0x164 its kind), +0x170 home, +0x190 the turn
//! velocity, +0x194 the approach angle (±+0x198°), +0x1a0 the sight, +0x1a4 the speed, +0x1a8 the timer, +0x1ac the
//! alert, +0x1b0 the arena path, +0x1b4 starts awake, +0x1b8 the longest wait, +0x1bc / +0x1c0 the leap's landing /
//! trigger cuboids, +0x1c4 the wall reach (`randf(0, 1)`), +0x1c8 the glow phase, +0x1cc a guard, +0x1e0 the big head.
//!
//! | state | what | port |
//! |---|---|---|
//! | top | the hits (`0x2f91e8`); the big head (2.3, list 1); drawn within 27 of the camera → the shadow probe, +0x7f 0x15; scale = class scale · 0.8 | [`update`] |
//! | 0 | column 3, +0x29 1, health 4, meter 4; +0x58 10, +0x5a 14; mode \|= 0x5000; home; wall reach; `SeedJumpPattern(J)`, J2 / J3 2, J1 0.8, J0 819, J9 speed·dt; K radius 768, +0x28 0.75, gravity 20·dt², drag 0.0005, flags 9, +0x3d 0; glow phase; a guard → 0xf (seq 2); a leap trigger → 3; asleep → 1 (seq 0), sight 30; else 2, timer `ticks(1)`, seq 0, the group woken | [`init`] |
//! | 1 | a target → 2, timer `ticks(1)`, the group woken (`0x2f9698`: its members in 1 → 2) | |
//! | 2 | in seq 0: the timer (0 → `scale(randf(0, 20))`); out → seq 3 (`ticks(10)`); else at the animation's end: Ratchet 30 away or more → 6, seq 2 from a random frame (`ticks(30)`); nearer → 8, a new angle, seq 1 | |
//! | 3 / 4 / 5 | Ratchet in the trigger → 4 (seq 3); the leap: seq 3 done → seq 6; seq 6 done → 5, home = the landing cuboid, a lob over `ticks(60)` (K flags 5, +0x3d 3, `0x26b368(heading, m, K, 7, 1, 0)`), seq 7 (5 ticks); landed → 6, seq 2 (5 ticks) | |
//! | 6 / 7 | turned to the target (`SpringTurn2(…, 0.05, 0.3, 2π·dt)`); a target → 7, wait `randf(0, +0x1b8)`; else alerted → 9 (seq 1); 7: the wait out → 8, a new angle, seq 1 | |
//! | 8 | the speed `Approach`ed to speed·dt (6·dt²; to 0 by 20·dt² within 2.5); turned to the target; a step toward its point on the surround circle (`0x2f96f8`, at most 2.5 out); pushed 0.5 off the walls; beyond 30 of home or no target → 0xc; within 2.5 and 1 in height and Ratchet not reeling (0x13f510 < `ticks(10)`) → 0xb (seq 4) | [`close_in`] |
//! | 9 | as 8 at full speed; `ClampToPath` toward the target within the wall reach → 10 (seq 5); not alerted → 0xc; within 2.5 → 0xb (seq 4) | [`close_in`] |
//! | 10 | the animation's end → 9 (seq 1, `ticks(20)`) | |
//! | 0xb | turned to the target; keys 1–8: `coll_sphere_mobys(0.5, joint list 0, 0, m, (cos, sin, 0) damage 1 flags 1)`; else the end → 8 (seq 1, `ticks(20)`) | [`jab`] |
//! | 0xc | home at speed, turned and stepping; within 1 → 6 (seq 2); a target → 8 (seq 1) | |
//! | 0xd | backing off at 3·speed (20·dt²) facing the target; the timer out or 5 away → 0xb, speed 5·dt, seq 4 (frame 0, 5 ticks) | |
//! | 0xe | the knockback flight; landed or wrapped: inside the arena → 0xd, anim speed 1, seq 1, timer `ticks(30)`; outside → deleted; pushed off the walls | |
//! | 0xf / 0x10 | the guard: turned to Ratchet (`0x26ac10`: 2π·dt², 4π·dt); within 2 → 0x10 (seq 4); jabbing as 0xb plus a line from the joint to it; the end → 0xf (seq 2) | |
//! | 0x11 | the death flight; landed → `0x26e1f8(1.5, 10, m, pos)` (= `0x2742a8`), the burst (0x6a0 ×2, 0x69f ×1, 5, 2), deleted; below 2 → deleted | |
//! | `0x2f91e8` | the lure → alert `scale(randf(180, 240))`; a hit (0x330000; not by class 0x418) through the resolver (column 4); out5 ≠ 1 outside 0x11 / 3: health − damage, the group woken; ≤ 0 → reaction 1; a guard → 0xb (flash 0xfa); 1 / 2: 0x11, not targetable, K 8·dt / 5·dt, keys 7.5 / 14.5, the flight along the hit (seq 10), flash 0xfa, `SetDeathBits`; 3..10: 0xe, K 2·dt / dt, keys 5 / 8.5, seq 9, flash 0xfa; the flash; in 8 / 9 / 0xc / 0xd on its quarter of the ticks the contact (`0x2686c0(0.7, 0.9, 1, m, pos, 0x10000, 0, 1, 0)`); the glow (180°/s, 0x80808080 → 0x80604040); the sight (alerted 36; asleep: 6 when Ratchet's capsule is on it, else 0; else 30); the target (`0x26eac8`, beyond the sight of home or 3 in height → none; none → Ratchet) | [`hits`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, damage, flash, fx, knock, region, target, turn, walker, V};
use crate::moby_update::manip;
use crate::moby_update::scheduler;
use crate::moby_update::services::{pf, pv as v4, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 6;
pub const UPDATE_FN: u32 = 0x2f_7d78;
pub const CLASSES: [i16; 1] = [1048];
/// gp−0x4d50 (0.8) scale, −0x4d4c (20) gravity, −0x4d3c (2.5) the reach, −0x4d38 (30) the leash, −0x4d30 (30) sight,
/// −0x4d2c (5) the back-off, −0x4d28 (90°) the surround arc, −0x4d1c (180°/s) the glow, −0x4d18 / −0x4d14 its colours.
const SCALE: f32 = f32::from_bits(0x3f4c_cccd);
const GRAVITY: f32 = 20.0;
const REACH: f32 = 2.5;
const LEASH: f32 = 30.0;
const SIGHT: f32 = 30.0;
const BACK_OFF: f32 = 5.0;
const ARC: f32 = 90.0;
const GLOW_RATE: f32 = 180.0;
const GLOW: (u32, u32) = (0x8080_8080, 0x8060_4040);
const DEG: f32 = 0.017_453_292;
/// The attacker class whose hits it ignores.
const IGNORED: i16 = 0x418;

pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const SPEED_V: usize = 0xf4;
    pub const TGT_REC: usize = 0x120;
    pub const TGT: usize = 0x160;
    pub const KIND: usize = 0x164;
    pub const HOME: usize = 0x170;
    pub const TURN_V: usize = 0x190;
    pub const ANGLE: usize = 0x194;
    pub const ANGLE_DEG: usize = 0x198;
    pub const SIGHT: usize = 0x1a0;
    pub const SPEED: usize = 0x1a4;
    pub const TIMER: usize = 0x1a8;
    pub const ALERT: usize = 0x1ac;
    pub const ARENA: usize = 0x1b0;
    pub const AWAKE: usize = 0x1b4;
    pub const WAIT: usize = 0x1b8;
    pub const LANDING: usize = 0x1bc;
    pub const TRIGGER: usize = 0x1c0;
    pub const WALL_REACH: usize = 0x1c4;
    pub const GLOW: usize = 0x1c8;
    pub const GUARD: usize = 0x1cc;
    pub const BIG_HEAD: usize = 0x1e0;
    pub const SIZE: usize = 0x220;
}

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn blend(w: &mut World, id: MobyId, s: u8, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, s, 0, t);
}
fn tgt(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::TGT) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn tgt_pos(w: &World, id: MobyId) -> V { tgt(w, id).map_or([0.0; 4], |m| w.m(m).position) }
fn heading(a: V, b: V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn no_target(w: &World, id: MobyId) -> bool { c::pi32(w, id, pv::KIND) == 2 }
fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, pv::ALERT) != 0 }
fn spring2(w: &mut World, id: MobyId, h: f32) { turn::spring_turn2_pvar(w, id, h, f32::from_bits(0x3d4c_cccd), f32::from_bits(0x3e99_999a), c::DT * std::f32::consts::TAU, pv::TURN_V); }
fn speed_to(w: &mut World, id: MobyId, target: f32, step: f32) {
    let mut v = c::pf(w, id, pv::SPEED_V);
    turn::approach(target, step, &mut v);
    c::set_pf(w, id, pv::SPEED_V, v);
}
/// `0x270d00(0.5, arena, &pos, &pos)`.
fn walls(w: &mut World, id: MobyId) {
    let Some(pts) = usize::try_from(c::pi32(w, id, pv::ARENA)).ok().and_then(|p| w.svc.splines.get(p)).cloned() else { return };
    if let Some(q) = crate::path::push_from_walls(&pts, 0.5, c::pos(w, id)) { c::set_pos(w, id, q); }
}
/// The walker step toward `2·(cos, sin)` of heading `h`, the yaw kept.
fn step_along(w: &mut World, id: MobyId, h: f32) {
    let yaw = c::yaw(w, id);
    c::set_yaw(w, id, h);
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [h.cos() * 2.0, h.sin() * 2.0, 0.0, 0.0], &mut out);
    c::set_yaw(w, id, yaw);
}
fn new_angle(w: &mut World, id: MobyId) {
    let r = c::pf(w, id, pv::ANGLE_DEG) * DEG;
    let a = w.rng.randf(r, -r);
    c::set_pf(w, id, pv::ANGLE, a);
}

/// `0x2f9698(m)`: the group's members in 1 → 2.
fn wake_group(w: &mut World, id: MobyId) {
    let g = w.m(id).group;
    if g < 0 { return; }
    for m in scheduler::group_ids(w, g) {
        if w.m(m).state == 1 { w.mm(m).state = 2; }
    }
}

/// `0x2f96f8(m)` (module doc): the heading from the target to this trooper's point on the circle.
fn surround(w: &World, id: MobyId) -> f32 {
    let (me, t) = (c::pos(w, id), tgt_pos(w, id));
    let a0 = heading(t, me);
    let g = w.m(id).group;
    if g < 0 { return a0; }
    let (mut hi, mut lo) = (ARC * DEG, -(ARC * DEG));
    let dme = c::dist3(t, me);
    for m in scheduler::group_ids(w, g) {
        let q = w.m(m).position;
        if w.m(m).state != 8 && dme <= c::dist3(q, t) { continue; }
        let d = c::sub_rot(heading(t, q), a0);
        if 0.0 < d && d < hi {
            hi = d;
        } else if d < 0.0 && lo < d {
            lo = d;
        }
    }
    c::add_rot(c::add_rot(lo, c::diff_rots(hi, lo) * 0.5), a0)
}

/// States 8 / 9 (module doc).
fn close_in(w: &mut World, id: MobyId, alert: bool) {
    let t = tgt_pos(w, id);
    let d = c::dist2(c::pos(w, id), t);
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    if alert || REACH < d { speed_to(w, id, sp, c::DT2 * 6.0); } else { speed_to(w, id, 0.0, c::DT2 * 20.0); }
    spring2(w, id, heading(c::pos(w, id), t));
    let r = d.min(REACH);
    let a = surround(w, id);
    let goal = [t[0] + a.cos() * r, t[1] + a.sin() * r, t[2], t[3]];
    let h = heading(c::pos(w, id), goal);
    step_along(w, id, h);
    walls(w, id);
    let me = c::pos(w, id);
    if !alert {
        if LEASH < c::dist3(me, c::pv4(w, id, pv::HOME)) || no_target(w, id) {
            set(w, id, 0xc);
            return;
        }
        if REACH <= c::dist2(me, t) || 1.0 <= (t[2] - me[2]).abs() || w.ticks(10) <= w.hero.f510 { return; }
        set(w, id, 0xb);
        blend(w, id, 4, 10);
        return;
    }
    let ap = usize::try_from(c::pi32(w, id, pv::ARENA)).unwrap_or(0);
    let (crossed, at) = region::clamp(w, ap, me, t);
    let wall = crossed && c::dist3(me, at) < c::pf(w, id, pv::WALL_REACH);
    if !alerted(w, id) {
        set(w, id, 0xc);
    } else if wall {
        set(w, id, 10);
        blend(w, id, 5, 10);
    } else if c::dist2(me, t) < REACH {
        set(w, id, 0xb);
        blend(w, id, 4, 10);
    }
}

/// The jab at keys 1–8 (0xb / 0x10): true while in it.
fn jab(w: &mut World, id: MobyId, line: bool) -> bool {
    let key = c::ground::key_time(w, id);
    if !(1.0 < key && key < 8.0) { return false; }
    let at = w.joint_point(id, 0);
    let yaw = c::yaw(w, id);
    let t = HitTemplate { dir: [pf(yaw.cos()), pf(yaw.sin()), Pf::ZERO, Pf::ZERO], attacker: Some(id), flags: 1, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 1 };
    w.sphere_mobys(pf(0.5), v4(at), 0, Some(id), Some(&t));
    if line {
        let p = c::pos(w, id);
        crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, v4(at), v4(p), 0, Some(id), &t);
    }
    true
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    c::set_pu8(w, id, pv::D + 8, 3);
    c::set_pu8(w, id, pv::D + 9, 1);
    c::set_pf(w, id, pv::D, 4.0);
    c::set_pi16(w, id, pv::D + 4, 4);
    c::set_pu8(w, id, 0x58, 10);
    c::set_pu8(w, id, 0x5a, 14);
    w.mm(id).mode |= 0x5000;
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    let r = w.rng.randf(0.0, 1.0);
    c::set_pf(w, id, pv::WALL_REACH, r);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pf(w, id, pv::J + 8, 2.0);
    c::set_pf(w, id, pv::J + 0xc, 2.0);
    c::set_pf(w, id, pv::J + 4, SCALE);
    c::set_pi32(w, id, pv::J, (SCALE * 1024.0) as i32);
    let sp = c::pf(w, id, pv::SPEED) * c::DT;
    c::set_pf(w, id, pv::SPEED_V, sp);
    c::set_pu8(w, id, pv::K + 0x3d, 0);
    c::set_pi32(w, id, pv::K + knock::k::RADIUS, 768);
    c::set_pf(w, id, pv::K + knock::k::ZOFF, 0.75);
    c::set_pf(w, id, pv::K + knock::k::GRAVITY, GRAVITY * c::DT2);
    c::set_pf(w, id, pv::K + knock::k::DRAG, f32::from_bits(0x3a03_126f));
    c::set_pi32(w, id, pv::K + knock::k::FLAGS, 9);
    let a = w.rng.rand_angle();
    c::set_pf(w, id, pv::GLOW, a);
    if c::pi32(w, id, pv::GUARD) != 0 {
        set(w, id, 0xf);
        c::blend_to(w, id, 2, 0, 0);
        return;
    }
    if 0 <= c::pi32(w, id, pv::TRIGGER) {
        set(w, id, 3);
        return;
    }
    if c::pi32(w, id, pv::AWAKE) == 0 {
        set(w, id, 1);
        c::blend_to(w, id, 0, 0, 0);
        c::set_pf(w, id, pv::SIGHT, SIGHT);
        return;
    }
    set(w, id, 2);
    let t = w.ticks(1);
    c::set_pi32(w, id, pv::TIMER, t);
    c::blend_to(w, id, 0, 0, 0);
    wake_group(w, id);
}

/// `0x2f91e8` (module doc).
fn hits(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::LURE) != 0 {
        let f = w.rng.randf(180.0, 240.0);
        let t = w.svc.timing.scale(Pf::f(f)).to_f32() as i32;
        c::set_pi32(w, id, pv::ALERT, t);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    c::dec_timer_pvar_i32(w, id, pv::ALERT);
    let hit = w.get_hit(id, 0x33_0000, false).filter(|h| h.attacker.is_none_or(|a| w.m(a).o_class != IGNORED));
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    let s = st(w, id);
    if res.out5 != 1 && s != 0x11 && s != 3 {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        wake_group(w, id);
        let mut reaction = res.reaction;
        if hp <= 0.0 {
            reaction = 1;
        } else if c::pi32(w, id, pv::GUARD) != 0 {
            reaction = 0xb;
            c::set_pu8(w, id, pv::F + 7, 0xfa);
        }
        let dir = res.hit.map_or([0.0; 4], |h| h.dir.map(|x| f32::from_bits(x.0)));
        let a = c::atan(dir[0], dir[1]);
        match reaction {
            1 | 2 => {
                set(w, id, 0x11);
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pf(w, id, pv::K + knock::k::UP, c::DT * 5.0);
                c::set_pf(w, id, pv::K + knock::k::KEY_APEX, 7.5);
                c::set_pf(w, id, pv::K + knock::k::KEY_LAND, 14.5);
                c::set_pf(w, id, pv::K + knock::k::SPEED, c::DT * 8.0);
                knock::start(w, id, pv::K, a, 10, 1, 0);
                c::set_pu8(w, id, pv::F + 7, 0xfa);
                set_death_bits(w, id, 0, -1);
            }
            3..=10 => {
                set(w, id, 0xe);
                c::set_pf(w, id, pv::K + knock::k::KEY_APEX, 5.0);
                c::set_pf(w, id, pv::K + knock::k::KEY_LAND, 8.5);
                c::set_pf(w, id, pv::K + knock::k::UP, c::DT);
                c::set_pf(w, id, pv::K + knock::k::SPEED, c::DT + c::DT);
                knock::start(w, id, pv::K, a, 9, 1, 0);
                c::set_pu8(w, id, pv::F + 7, 0xfa);
            }
            _ => {}
        }
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::F);
    let s = st(w, id);
    if matches!(s, 8 | 9 | 0xc | 0xd) && (id as u64 & 3) == (w.counter & 3) {
        let p = c::pos(w, id);
        attack::sphere_hit(w, f32::from_bits(0x3f33_3333), f32::from_bits(0x3f66_6666), 1.0, id, p, 0x1_0000, 0, 1, 0);
    }
    let g = c::add_rot(c::pf(w, id, pv::GLOW), GLOW_RATE * DEG * c::DT);
    c::set_pf(w, id, pv::GLOW, g);
    w.mm(id).glow = crate::particles::tween_color(((g.sin() + 1.0) * 0.5).to_bits(), GLOW.0, GLOW.1);
    let sight = if alerted(w, id) {
        SIGHT + 6.0
    } else if s == 1 {
        if w.hero.cap_moby == Some(id) || w.hero.wall_moby == Some(id) { 6.0 } else { 0.0 }
    } else {
        SIGHT
    };
    c::set_pf(w, id, pv::SIGHT, sight);
    let reg = usize::try_from(c::pi32(w, id, pv::ARENA)).ok();
    let t = target::acquire_in(w, id, sight, reg);
    c::set_pv4(w, id, pv::TGT_REC, t.pos);
    c::set_pi32(w, id, pv::TGT, t.moby.map_or(0, |m| m as i32 + 1));
    let mut kind = t.kind as i32;
    if kind != 2 && (sight < c::dist2(c::pv4(w, id, pv::HOME), t.pos) || 3.0 < (c::pos(w, id)[2] - t.pos[2]).abs()) { kind = 2; }
    c::set_pi32(w, id, pv::KIND, kind);
    if c::pi32(w, id, pv::TGT) == 0 {
        let h = w.hero_moby.map_or(0, |m| m as i32 + 1);
        c::set_pi32(w, id, pv::TGT, h);
    }
}

/// Level06 `0x2f7d78` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    hits(w, id);
    manip::big_head(w, f32::from_bits(0x4013_3333), id, 1, id, pv::BIG_HEAD);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    let o = w.m(id).o_class;
    w.mm(id).scale = super::class_scale(w, o) * SCALE;
    match st(w, id) {
        0 => init(w, id),
        1 => {
            if no_target(w, id) { return; }
            set(w, id, 2);
            let t = w.ticks(1);
            c::set_pi32(w, id, pv::TIMER, t);
            wake_group(w, id);
        }
        2 => {
            if seq(w, id) == 0 {
                if c::pi32(w, id, pv::TIMER) == 0 {
                    let f = w.rng.randf(0.0, 20.0);
                    let t = w.svc.timing.scale(Pf::f(f)).to_f32() as i32;
                    c::set_pi32(w, id, pv::TIMER, t);
                    return;
                }
                if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
                blend(w, id, 3, 10);
                return;
            }
            if !done(w, id) { return; }
            if SIGHT <= c::dist2(c::pos(w, id), super::hero_pos(w)) {
                set(w, id, 6);
                if seq(w, id) == 2 { return; }
                let o = w.m(id).o_class;
                let n = w.classes.anim(o).and_then(|a| a.sequence(2)).map_or(1, |q| q.header.frame_count as i32);
                let f = w.rng.randf(0.0, n as f32 - 1.0) as i32;
                let t = w.ticks(30);
                w.anim_blend(id, 2, f, t);
                return;
            }
            set(w, id, 8);
            new_angle(w, id);
            blend(w, id, 1, 10);
        }
        3 => {
            let c0 = c::pi32(w, id, pv::TRIGGER);
            if c0 < 0 || !w.in_cuboid(w.hero_point(), c0) { return; }
            set(w, id, 4);
            blend(w, id, 3, 5);
        }
        4 => {
            if !done(w, id) { return; }
            match seq(w, id) {
                3 => {
                    let t = w.ticks(5);
                    w.anim_blend(id, 6, 0, t);
                }
                6 => {
                    set(w, id, 5);
                    if let Some((p, _)) = crate::moby_update::story::cuboid(w, c::pi32(w, id, pv::LANDING)) {
                        c::set_pv4(w, id, pv::HOME, [p[0], p[1], p[2], 1.0]);
                    }
                    let (me, home) = (c::pos(w, id), c::pv4(w, id, pv::HOME));
                    let sp = c::dist2(me, home) / w.ticks(0x3c) as f32;
                    c::set_pf(w, id, pv::K + knock::k::SPEED, sp);
                    let mut time = 0.0;
                    let up = knock::lob_up(sp, -c::pf(w, id, pv::K + knock::k::GRAVITY), me, home, &mut time);
                    c::set_pf(w, id, pv::K + knock::k::UP, up);
                    c::set_pi32(w, id, pv::K + knock::k::FLAGS, 5);
                    c::set_pu8(w, id, pv::K + 0x3d, 3);
                    knock::start(w, id, pv::K, heading(me, home), 7, 1, 0);
                    c::blend_to(w, id, 7, 0, 5);
                }
                _ => {}
            }
        }
        5 => {
            if knock::update(w, id, pv::K) & 1 != 0 {
                set(w, id, 6);
                c::blend_to(w, id, 2, 0, 5);
            }
        }
        6 => {
            let t = c::pv4(w, id, pv::TGT_REC);
            spring2(w, id, heading(c::pos(w, id), t));
            if !no_target(w, id) {
                set(w, id, 7);
                let n = c::pi32(w, id, pv::WAIT) as f32;
                let f = w.rng.randf(0.0, n);
                c::set_pi32(w, id, pv::TIMER, f as i32);
                return;
            }
            if !alerted(w, id) { return; }
            set(w, id, 9);
            blend(w, id, 1, 10);
        }
        7 => {
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
            set(w, id, 8);
            new_angle(w, id);
            blend(w, id, 1, 10);
        }
        8 => close_in(w, id, false),
        9 => close_in(w, id, true),
        10 => {
            if done(w, id) {
                set(w, id, 9);
                blend(w, id, 1, 20);
            }
        }
        0xb => {
            spring2(w, id, heading(c::pos(w, id), tgt_pos(w, id)));
            if jab(w, id, false) { return; }
            if done(w, id) {
                set(w, id, 8);
                blend(w, id, 1, 20);
            }
        }
        0xc => {
            let sp = c::pf(w, id, pv::SPEED) * c::DT;
            speed_to(w, id, sp, c::DT2 * 6.0);
            let home = c::pv4(w, id, pv::HOME);
            spring2(w, id, heading(c::pos(w, id), home));
            let y = c::yaw(w, id);
            step_along(w, id, y);
            walls(w, id);
            if c::dist3(c::pos(w, id), home) < 1.0 {
                set(w, id, 6);
                blend(w, id, 2, 20);
            } else if !no_target(w, id) {
                set(w, id, 8);
                blend(w, id, 1, 20);
            }
        }
        0xd => {
            let sp = c::pf(w, id, pv::SPEED) * 3.0 * c::DT;
            speed_to(w, id, sp, c::DT2 * 20.0);
            let t = tgt_pos(w, id);
            spring2(w, id, heading(c::pos(w, id), t));
            let away = c::add_rot(heading(c::pos(w, id), t), f32::from_bits(0x4049_0fd0));
            step_along(w, id, away);
            walls(w, id);
            if c::dec_timer_pvar_i32(w, id, pv::TIMER) == 0 && c::dist3(c::pos(w, id), t) <= BACK_OFF { return; }
            set(w, id, 0xb);
            c::set_pf(w, id, pv::SPEED_V, c::DT * 5.0);
            // gp−0x4d24 = 1: the second form (frame gp−0x4d20 = 0, 5 ticks).
            if seq(w, id) != 4 {
                let t5 = w.ticks(5);
                w.anim_blend(id, 4, 0, t5);
            }
        }
        0xe => {
            let r = knock::update(w, id, pv::K);
            if r & 0x41 != 0 {
                let p = c::pos(w, id);
                let inside = usize::try_from(c::pi32(w, id, pv::ARENA)).is_ok_and(|a| region::point_in_polygon(w, a, p));
                if !inside {
                    w.delete_moby(id);
                    return;
                }
                set(w, id, 0xd);
                w.mm(id).anim.speed = 1.0;
                blend(w, id, 1, 20);
                let t = w.ticks(30);
                c::set_pi32(w, id, pv::TIMER, t);
            }
            walls(w, id);
        }
        s @ (0xf | 0x10) => {
            let h = heading(c::pos(w, id), super::hero_pos(w));
            turn::turn_toward_pvar(w, id, h, c::DT2 * std::f32::consts::TAU, c::DT2 * std::f32::consts::TAU, c::DT * 12.566_371, pv::TURN_V);
            if s == 0xf {
                if c::dist2(c::pos(w, id), super::hero_pos(w)) < 4.0 {
                    set(w, id, 0x10);
                    blend(w, id, 4, 20);
                }
                return;
            }
            if jab(w, id, true) { return; }
            if done(w, id) {
                set(w, id, 0xf);
                blend(w, id, 2, 20);
            }
        }
        0x11 => {
            if knock::update(w, id, pv::K) & 0x40 == 0 {
                if c::pos(w, id)[2] < 2.0 { w.delete_moby(id); }
                return;
            }
            let p = c::pos(w, id);
            fx::piece_explosion(w, 1.5, 10.0, Some(id), p);
            crate::moby_update::classes::breakables::burst_pieces(w, id, 0x6a0, 2, 0x69f, 1, 5, 2);
            w.delete_moby(id);
        }
        _ => {}
    }
}
