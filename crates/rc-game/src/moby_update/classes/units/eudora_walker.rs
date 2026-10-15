//! U170 (census 2026-10-02): class 563, Eudora's leg walkers (level04 0x2d16b8, the only copy: 17 placed, 10 created),
//! and their wood chips, class 1516 (`0x2e4fe0` / `0x2e5178`). The name is descriptive [L]. Read from the level04
//! decomp and disassembly. Native `f32`; the `rand` draws in the game's order.
//!
//! A four-legged creature that walks on the shared leg walker ([`legs`], record at +0x160): it idles at home (or
//! chops wood, throwing chips, when it starts in 3), turns toward Ratchet when he comes within its range and walks at
//! him inside its area (a path polygon, +0x3dc), kicks with its legs and head within 3 when facing him, and walks home
//! when he leaves the area. Hits flinch it, knock it down (then it dashes to a spot in its area away from Ratchet) or
//! push it back; a kill flings it (a knock flight) and it blows up into three pieces.
//!
//! **Pvars** (0x3f0): header (+0x00 D, +0x0c F, +0x10 K); +0x20 D (health 2, meter 2, column 1, +0x30 1.0, +0x38 the
//! lure); +0x58 / +0x5a bytes 8 / 8; +0xb0 F; +0xc0 K; +0x120 the flinch manipulator (joint list 6); +0x160 the walker;
//! +0x3b0 home; +0x3c0 byte starts chopping; +0x3c1 the last state; +0x3c2 s16 the hold; +0x3c4 s16 the hit guard;
//! +0x3c6 s16 the alert timer; +0x3c8 the flinch yaw; +0x3d0 the turn velocity; +0x3d4 / +0x3d8 the dash's heading and
//! speed; +0x3dc the area path; +0x3e0 the fall speed; +0x3e4 the push-back; +0x3e8 the range.
//!
//! | address | what | port |
//! |---|---|---|
//! | top | alerted = lure or alert timer; the target (range ×2 alerted), its heading, the yaw's difference, its xy distance; `0x2563b8(2.5, m, 6, +0x120)` the big-head cheat; the shadow probe within 27; the hit guard | [`update`] |
//! | hits | not in 0 / 0xb: `MobyGetHitMessage(m, 0xa10000)`, the resolver (column 4); K radius 0x15e, +0x28 0.35; a hit with damage, the guard out: K (flags 9, gravity 30·dt², drag 10·dt², speed 5.5·dt); health −= damage (≤ 0 → 1) | [`hits`] |
//! | 1 / 2 | K speed 7·dt, up 13·dt, drag 17·dt², aimed along the push (`0x26fa48`), the flight (seq 8), keys 14 / 24, 0xb, `SetDeathBits`, the flash, push 4/`scale(15)` | [`hits`] |
//! | 3 / 6 | the flinch (−30°); unless kicking (seq 4, keys 16–27): `cmd` \|= 5, seq 5 (`ticks(3)`), hips reset, 9, push 2/`scale(15)` (half for 0x131 / 0xb0 attackers) aimed along the push, the flash, guard `ticks(15)`, alert `ticks(240)` | [`hits`] |
//! | 4 / 5 | seq 6 (`ticks(10)`), hips reset, 1, push 2/`scale(15)`, yaw = the push's heading + π, the flash, guard `ticks(30)`, hold `ticks(180)`, alert | [`hits`] |
//! | 8, seq 4, keys 16–27 | the kick: lines between the joint points of lists 0, 2, 1, 3, 4, 5 (`CollLine_Fix(…, 9, m, T)`), a sphere over points 0–4 and one of 0.5 at list 17 (`coll_sphere_mobys(…, 0x10)`); T = {(Ratchet − pos) at 5·dt, m, 0x10001, damage 1, +0x20 1} | [`kick`] |
//! | 0 | D fields, home, the manipulator, the walker (`0x2d1418`: records 10 but 7 (3), the hip limits, stand seq 0, gravity 9.8·dt², yaw spring π / 2π ·dt², 2π·dt, accel 10·dt², step 0.5, lists 10 / 8 / 14 / 12 and 11 / 9 / 15 / 13, body 7, then [`legs::setup`]); +0x3c0 → seq 11, 3 | [`init`] |
//! | 1 | wrapped: in range → seq 0, else seq 0 or 1 (`randi(6)`); hold out: within 3 and facing (90°) → seq 4, 8; in range: first time → turn (seq 16, 10), else in the area → 5; then away from home (2) and Ratchet out of range or of the area → 6 | [`idle`] |
//! | 2 | seq 2 to its end → 5 (Ratchet outside the area, not alerted: seq 0, 1) | [`update`] (no setter on level 04) |
//! | 3 | chopping: wrapped → seq 12, anim speed `randf(0.8, 1.2)`; key 36 passed → 20 chips; hold out: as 1 (no return home) | [`chop`] |
//! | 4 | not alerted → 1; within 3 facing → seq 4, 8; Ratchet outside the area: facing away → turn (10); else 5 | [`update`] |
//! | 5 | the walker toward Ratchet (4·dt, accel 20·dt²); within 3 facing → seq 4, 8; 2 ahead outside the area: Ratchet inside (not alerted) → turn (10); alerted and outside → 7; Ratchet outside, not alerted → 6 | [`update`] |
//! | 6 | the walker home; within 3 facing → kick; ahead outside → turn home (10, `cmd` \|= 0x10); home or Ratchet in the area → 7 | [`update`] |
//! | 7 | the walker stopping; within 3 facing → kick; stood → 4 (alerted) or 1 | [`update`] |
//! | 8 | seq 4 to its end → hold 0, seq 0, 1 | [`update`] |
//! | 9 | knocked: seq 5 to its end; `cmd` & 4: a dash spot 6 away from Ratchet in the area (headings ±30°, ±45°, ±15°, ±60°, 0 around the way from him; the sign of each pair from `randi(100)`): seq 16, stay; none: kick or seq 0, 1; else the dash: speed up to 1/9 at 20·dt² (seq 16 before key 28), then down by 200·dt², turning to Ratchet; stopped → 1 | [`knocked`] |
//! | 10 | turning (seq 16): wrapped → 5 (6 coming home or Ratchet outside), seq 0, `cmd` \|= 8; else turn to Ratchet (0.03, 0.3, 6.46·dt) or home (π·dt², 2π·dt², 4.19·dt) | [`update`] |
//! | 0xb | the flight; landed / wrapped or z < 1 → `SpawnBeamExplosion` at joint 12, `BreakFxB` 0x6cb / 0x6cc / 0x6cd, `DeleteMoby` | [`update`] |
//! | tail | the flash; the push-back from Ratchet (`0x26d270(0.25, 1, 0.75, 50°, …, 1)`); the flinch yaw ×0.96 into list 6's node; not in 5 and `cmd` ≠ 6: the fall (9.8·dt²) onto `GroundHeight(0.5)`, below z 2 → `SetDeathBits`, `DeleteMoby` | [`update`] |
//!
//! **Chips (1516)**: spawned with the owner, the velocity, position, Euler (three `rand_angle` drawn and dropped), no
//! bounces: scale ·`randf(0.35, 0.7)`, radius a quarter of it, spins `randf_sym(0, 360)`° ·dt about y and z, a
//! `ticks(20)` grace; each tick gravity 20·dt², the move, the spin; 3 below the start → fading (scale ·0.9, alpha −4,
//! deleted under 4); a bounce (with bounces left, after the grace, `coll_sphere` flags 2, moving into the hit) off the
//! hit at 0.75. [`chip_update`].
//!
//! **The game's, noted:** the gravity is skipped when `cmd` (not the state) is 6; the area test of 10 reads path
//! 0x1b0630[−1] when the creature has no area (the port: inside).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, legs, target, turn, walker, DT, DT2};
use crate::moby_update::manip;
use crate::moby_update::services::{self as sv, pf, pv, HitTemplate, World};
use crate::ps2v::Pf;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4, FRAC_PI_6, PI};

pub const REFERENCE_LEVEL: u32 = 4;
pub const UPDATE_FN: u32 = 0x2d_16b8;
pub const CLASSES: [i16; 1] = [563];
pub const CHIP_FN: u32 = 0x2e_5178;
pub const CHIP_CLASSES: [i16; 1] = [1516];
pub const PIECES: [i16; 3] = [0x6cb, 0x6cc, 0x6cd];

mod p {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0xb0;
    pub const K: usize = 0xc0;
    pub const FLINCH: usize = 0x120;
    pub const W: usize = 0x160;
    pub const HOME: usize = 0x3b0;
    pub const CHOPS: usize = 0x3c0;
    pub const LAST: usize = 0x3c1;
    pub const HOLD: usize = 0x3c2;
    pub const GUARD: usize = 0x3c4;
    pub const ALERT: usize = 0x3c6;
    pub const FLINCH_A: usize = 0x3c8;
    pub const TURN_V: usize = 0x3d0;
    pub const DASH_A: usize = 0x3d4;
    pub const DASH_V: usize = 0x3d8;
    pub const AREA: usize = 0x3dc;
    pub const FALL: usize = 0x3e0;
    pub const PUSH: usize = 0x3e4;
    pub const RANGE: usize = 0x3e8;
    pub const SIZE: usize = 0x3f0;
}

/// `SpawnBeamExplosion(0, 0, 2, 1, 100000, 1, 15, m, 0, joint 12, 10, 3, 4, −1, 1, 1, −1, 0)` (`0x2d1608`).
pub const BEAM: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 100000.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 4, debris: 1, sound: -1, shake: true };
/// The kick's joint lists (`0x1dc450`).
const KICK: [usize; 6] = [0, 2, 1, 3, 4, 5];
/// The dash spot headings (`0x1dc468`).
const DASH: [f32; 5] = [FRAC_PI_6, FRAC_PI_4, 0.261_799_4, FRAC_PI_3, 0.0];

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) {
    let t = w.ticks(t);
    w.anim_blend(id, seq, 0, t);
}
fn set16(w: &mut World, id: MobyId, o: usize, t: i32) {
    let t = w.ticks(t);
    c::set_pi16(w, id, o, t as i16);
}
fn gscale(w: &World, x: f32) -> f32 { sv::fl(w.svc.timing.scale(Pf::f(x))) }
fn heading_to(a: c::V, b: c::V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, p::LURE) != 0 || c::pi16(w, id, p::ALERT) != 0 }
fn range(w: &World, id: MobyId) -> f32 {
    let r = c::pf(w, id, p::RANGE);
    if alerted(w, id) { r + r } else { r }
}
fn area(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, p::AREA)).ok().filter(|&a| a < w.svc.splines.len()) }
/// `PointInPathPolygon(q, area)`; without an area, inside.
fn in_area(w: &World, id: MobyId, q: c::V) -> bool { area(w, id).is_none_or(|a| c::region::point_in_polygon(w, a, q)) }
fn has_area(w: &World, id: MobyId) -> bool { c::pi32(w, id, p::AREA) != -1 }

struct Tick {
    t: target::Target,
    heading: f32,
    dyaw: f32,
    d: f32,
    kt: f32,
}

fn facing_close(k: &Tick) -> bool { k.d < 3.0 && k.dyaw < FRAC_PI_2 }

/// Seq 4 (`ticks(10)`), the hips reset, 8.
fn to_kick(w: &mut World, id: MobyId) {
    blend(w, id, 4, 10);
    legs::reset_hips(w, id, p::W);
    set_st(w, id, 8);
}

/// Seq 16 (`ticks(10)`), the hips reset, 10 (turn velocity 0).
fn to_turn(w: &mut World, id: MobyId) {
    c::set_pf(w, id, p::TURN_V, 0.0);
    blend(w, id, 0x10, 10);
    legs::reset_hips(w, id, p::W);
    set_st(w, id, 10);
}

/// 5 or 6 with the walker reset next tick (`cmd` |= 8).
fn to_walk(w: &mut World, id: MobyId, s: u8) {
    set_st(w, id, s);
    w.mm(id).cmd |= 8;
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId, k: &Tick) {
    let s = st(w, id);
    if s == 0xb || s == 0 { return; }
    let hit = w.get_hit(id, 0xa1_0000, false);
    let res = damage::resolve(w, id, hit, p::D, 0, 4);
    let kr = p::K;
    c::set_pi32(w, id, kr + knock::k::RADIUS, 0x15e);
    c::set_pf(w, id, kr + knock::k::ZOFF, f32::from_bits(0x3eb3_3333));
    let Some(h) = hit else { return };
    if res.damage == 0.0 || c::pi16(w, id, p::GUARD) != 0 { return; }
    // The heading from the attacker is replaced by `0x26fa48` on every path that reads it.
    let mut up = 1.0;
    c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
    c::set_pf(w, id, kr + knock::k::GRAVITY, DT2 * 30.0);
    c::set_pu8(w, id, kr + 0x3d, 0);
    c::set_pf(w, id, kr + knock::k::DRAG, DT2 * 10.0);
    c::set_pf(w, id, kr + knock::k::SPEED, DT * 5.5);
    let hp = c::pf(w, id, p::D) - res.damage;
    c::set_pf(w, id, p::D, hp);
    let reaction = if hp <= 0.0 { 1 } else { res.reaction };
    let dir = h.dir.map(|x| f32::from_bits(x.0));
    let class = h.attacker.filter(|&a| a < w.table.mobys.len()).map(|a| w.m(a).o_class);
    let k15 = 2.0 / gscale(w, 15.0);
    match reaction {
        1 | 2 => {
            c::set_pf(w, id, kr + knock::k::SPEED, DT * 7.0);
            c::set_pf(w, id, kr + knock::k::UP, DT * 13.0);
            c::set_pf(w, id, kr + knock::k::DRAG, DT2 * 17.0);
            let (mut sp, mut u) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, 8, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 24.0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 14.0);
            set_st(w, id, 0xb);
            set_death_bits(w, id, 0, -1);
            c::set_pu8(w, id, p::F + 7, 0x78);
            flash::start(w, id, p::F);
            c::set_pf(w, id, p::PUSH, k15 + k15);
        }
        3 | 6 => {
            c::set_pf(w, id, p::FLINCH_A, f32::from_bits(0xbf06_0a92));
            if w.m(id).anim.seq_a == 4 && 16.0 <= k.kt && k.kt <= 27.0 { return; }
            w.mm(id).cmd |= 5;
            blend(w, id, 5, 3);
            legs::reset_hips(w, id, p::W);
            set_st(w, id, 9);
            let push = if matches!(class, Some(0x131) | Some(0xb0)) { k15 * 0.5 } else { k15 };
            c::set_pf(w, id, p::PUSH, push);
            let mut sp = push;
            knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, p::PUSH, sp);
            c::set_pu8(w, id, p::F + 7, 0x78);
            flash::start(w, id, p::F);
            set16(w, id, p::GUARD, 15);
            set16(w, id, p::ALERT, 240);
        }
        4 | 5 => {
            blend(w, id, 6, 10);
            legs::reset_hips(w, id, p::W);
            set_st(w, id, 1);
            let mut sp = k15;
            let a = knock::aim(dir, &mut sp, &mut up);
            c::set_pf(w, id, p::PUSH, sp);
            w.mm(id).rotation[2] = c::add_rot(PI, a);
            c::set_pu8(w, id, p::F + 7, 0x78);
            flash::start(w, id, p::F);
            set16(w, id, p::GUARD, 30);
            set16(w, id, p::HOLD, 180);
            set16(w, id, p::ALERT, 240);
        }
        _ => {}
    }
}

/// The kick (module doc).
fn kick(w: &mut World, id: MobyId) {
    let me = c::pos(w, id);
    let d = c::set_len3(c::sub(super::hero_pos(w), me), DT * 5.0);
    let tmpl = HitTemplate { dir: pv(d), attacker: Some(id), flags: 0x1_0001, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 1 };
    let pts = KICK.map(|l| w.joint_point(id, l));
    for i in 0..5 {
        let coll = w.coll;
        sv::line_hit_in(w.table, w.svc, w.classes, coll, pv(pts[i]), pv(pts[i + 1]), 9, Some(id), &tmpl);
    }
    let centre = c::scale(c::add(pts[0], pts[4]), 0.5);
    let r = c::dist3(pts[0], pts[4]) * 0.5;
    w.sphere_mobys(pf(r), pv(centre), 0x10, Some(id), Some(&tmpl));
    let j = w.joint_point(id, 0x11);
    w.sphere_mobys(pf(0.5), pv(j), 0x10, Some(id), Some(&tmpl));
}

/// `0x2d1418`: the walker record (module table).
fn walker_init(w: &mut World, id: MobyId) {
    let wr = p::W;
    for k in 0..13 { c::set_pi32(w, id, wr + legs::w::TABLES + 4 * k, if k == 7 { 3 } else { 10 }); }
    legs::hip_limits(w, id, wr);
    c::set_pu8(w, id, wr + legs::w::STAND, 0);
    c::set_pu8(w, id, wr + legs::w::STAND + 1, 0);
    c::set_pf(w, id, wr + legs::w::STEP, 0.5);
    c::set_pf(w, id, wr + legs::w::GRAVITY, DT2 * 9.8);
    c::set_pf(w, id, wr + legs::w::YAW_ACC, DT2 * PI);
    c::set_pf(w, id, wr + legs::w::YAW_DAMP, DT2 * 2.0 * PI);
    c::set_pf(w, id, wr + legs::w::YAW_MAX, DT * 2.0 * PI);
    c::set_pf(w, id, wr + legs::w::ACCEL, DT2 * 10.0);
    c::set_pu8(w, id, wr + 0xb2, 0xff);
    c::set_pu8(w, id, wr + 0xb3, 0x10);
    c::set_pu8(w, id, wr + legs::w::BODY_B, 7);
    for (o, v) in [(0xf0, 10u8), (0xf1, 8), (0xf3, 0xc), (0xf2, 0xe), (0x1a0, 0xb), (0x1a1, 9), (0x1a3, 0xd), (0x1a2, 0xf)] { c::set_pu8(w, id, wr + o, v); }
    c::set_pu8(w, id, wr + legs::w::BODY_A, 7);
    legs::setup(w, id, wr);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let d = p::D;
    c::set_pf(w, id, d, 2.0);
    c::set_pi16(w, id, d + 4, 2);
    c::set_pf(w, id, d + 0x10, 1.0);
    c::set_pu8(w, id, d + 8, 1);
    c::set_pu8(w, id, d + 9, 0);
    w.mm(id).cmd = 0;
    c::set_pu8(w, id, 0x5a, 8);
    c::set_pf(w, id, p::FALL, 0.0);
    c::set_pi16(w, id, p::ALERT, 0);
    c::set_pu8(w, id, 0x58, 8);
    let pos = c::pos(w, id);
    c::set_pv4(w, id, p::HOME, pos);
    if !manip::attached(w, id, p::FLINCH) { manip::attach(w, id, 6, id, p::FLINCH); }
    walker_init(w, id);
    if c::pu8(w, id, p::CHOPS) == 0 {
        set_st(w, id, 1);
    } else {
        set_st(w, id, 3);
        blend(w, id, 0xb, 10);
    }
}

/// In range: the first time a turn (10, `cmd` |= 1); after it, Ratchet in the area (or alerted) → 5.
fn engage(w: &mut World, id: MobyId, k: &Tick) -> bool {
    if w.m(id).cmd & 1 == 0 {
        to_turn(w, id);
        w.mm(id).cmd |= 1;
        return true;
    }
    if !alerted(w, id) && has_area(w, id) && !in_area(w, id, k.t.pos) { return false; }
    to_walk(w, id, 5);
    true
}

/// State 1 (module doc).
fn idle(w: &mut World, id: MobyId, k: &Tick) {
    if wrapped(w, id) {
        if k.d <= range(w, id) {
            blend(w, id, 0, 20);
        } else {
            let mut table = [0u8; 6];
            table[5] = 1;
            let i = w.rng.randi(6) as usize;
            blend(w, id, table[i], 20);
        }
    }
    if c::pi16(w, id, p::HOLD) != 0 { return; }
    if 3.0 <= k.d || FRAC_PI_2 <= k.dyaw {
        if k.d < range(w, id) { engage(w, id, k); }
    } else {
        blend(w, id, 4, 10);
        set_st(w, id, 8);
    }
    if alerted(w, id) || c::dist2(c::pos(w, id), c::pv4(w, id, p::HOME)) <= 2.0 { return; }
    if k.d <= c::pf(w, id, p::RANGE) && in_area(w, id, k.t.pos) { return; }
    to_walk(w, id, 6);
}

/// State 3 (module doc).
fn chop(w: &mut World, id: MobyId, k: &Tick) {
    if w.m(id).anim.seq_a != 0 && wrapped(w, id) {
        blend(w, id, 0xc, 10);
        let s = w.rng.randf(0.8, 1.2);
        w.mm(id).anim.speed = s;
    }
    if c::ground::passed_frame(w, id, 36.0) {
        for _ in 0..20 {
            let r = [w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), w.rng.randf(-1.0, 1.0), 0.0];
            let at = w.joint_point(id, 0);
            let y = w.m(id).rotation[2];
            let vx = y.cos() * -(w.rng.randf(5.0, 10.0) * DT);
            let vy = y.sin() * -(w.rng.randf(5.0, 10.0) * DT);
            let mut r = c::set_len3(r, DT * 3.0);
            r[2] += DT * 3.0;
            let vel = c::add([vx, vy, 0.0, 0.0], r);
            let rot = [w.rng.rand_angle(), w.rng.rand_angle(), w.rng.rand_angle(), 0.0];
            spawn_chip(w, id, vel, at, rot, 0);
        }
    }
    if c::pi16(w, id, p::HOLD) != 0 { return; }
    if facing_close(k) {
        blend(w, id, 4, 10);
        set_st(w, id, 8);
    } else if k.d < range(w, id) {
        engage(w, id, k);
    }
}

/// State 9 (module doc).
fn knocked(w: &mut World, id: MobyId, k: &Tick) {
    let a = &w.m(id).anim;
    if a.seq_b == 5 && (a.seq_a != 5 || !wrapped(w, id)) { return; }
    if w.m(id).cmd & 4 != 0 {
        w.mm(id).cmd &= !4;
        if has_area(w, id) {
            let me = c::pos(w, id);
            let h = super::hero_pos(w);
            let away = c::atan(me[0] - h[0], me[1] - h[1]);
            let mut sign = 1.0f32;
            for i in 0..9 {
                if i & 1 == 0 {
                    sign = if w.rng.randi(100) & 1 == 0 { 1.0 } else { -1.0 };
                } else {
                    sign = -sign;
                }
                let a = c::add_rot(sign * DASH[i >> 1], away);
                let q = c::add(fx::polar(6.0, a, w.m(id).rotation[1]), me);
                if in_area(w, id, q) {
                    blend(w, id, 0x10, 10);
                    c::set_pf(w, id, p::DASH_A, a);
                    c::set_pf(w, id, p::DASH_V, 0.0);
                    return;
                }
            }
            if w.m(id).anim.seq_a == 5 {
                if facing_close(k) {
                    blend(w, id, 4, 10);
                    set_st(w, id, 8);
                } else {
                    blend(w, id, 0, 10);
                    set_st(w, id, 1);
                }
                return;
            }
        }
        blend(w, id, 0, 10);
    } else {
        let dashing = k.kt < 28.0 && w.m(id).anim.seq_a == 0x10;
        let v = c::pf(w, id, p::DASH_V);
        let v = if dashing {
            let cap = 1.0 / 9.0;
            if v < cap { v + DT2 * 20.0 } else { cap }
        } else if 0.0 < v {
            v - DT2 * 200.0
        } else {
            set_st(w, id, 1);
            c::set_pi16(w, id, p::HOLD, 0);
            return;
        };
        c::set_pf(w, id, p::DASH_V, v);
        let mv = fx::polar(v, c::pf(w, id, p::DASH_A), w.m(id).rotation[1]);
        let pos = c::add(c::pos(w, id), mv);
        c::set_pos(w, id, pos);
        let mut tv = c::pf(w, id, p::TURN_V);
        let h = heading_to(c::pos(w, id), k.t.pos);
        let y = turn::spring_turn(w.m(id).rotation[2], h, DT2 * PI, DT2 * 2.0 * PI, DT * 4.188_790_3, &mut tv);
        c::set_pf(w, id, p::TURN_V, tv);
        w.mm(id).rotation[2] = y;
        return;
    }
    set_st(w, id, 1);
    c::set_pi16(w, id, p::HOLD, 0);
}

/// The walker toward `at` (4·dt, accel 20·dt²).
fn walk(w: &mut World, id: MobyId, at: c::V) {
    let h = heading_to(c::pos(w, id), at);
    legs::step(w, id, p::W, DT * 4.0, h, DT2 * 20.0);
}

/// 2 ahead along the yaw.
fn ahead_in_area(w: &World, id: MobyId) -> bool {
    let me = c::pos(w, id);
    let y = w.m(id).rotation[2];
    in_area(w, id, [me[0] + y.cos() * 2.0, me[1] + y.sin() * 2.0, me[2], me[3]])
}

/// `cmd` bit 8: the walker reset.
fn walker_reset(w: &mut World, id: MobyId) {
    if w.m(id).cmd & 8 != 0 {
        w.mm(id).cmd &= !8;
        legs::reset(w, id, p::W);
    }
}

/// Level04 0x2d16b8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < p::SIZE { return; }
    let t = target::acquire(w, id, range(w, id));
    manip::big_head(w, 2.5, id, 6, id, p::FLINCH);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| x.to_f32());
        if c::dist3(c::pos(w, id), cam) < 27.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x15;
        }
    }
    let me = c::pos(w, id);
    let heading = heading_to(me, t.pos);
    let k = Tick { dyaw: c::diff_rots(w.m(id).rotation[2], heading), d: c::dist2(me, t.pos), kt: c::ground::key_time(w, id), heading, t };
    c::dec_timer_pvar_s16(w, id, p::GUARD);
    hits(w, id, &k);
    if w.m(id).state >= 0xfd { return; }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, p::LURE) != 0 { set16(w, id, p::ALERT, 240); }
    c::set_pi32(w, id, p::LURE, 0);
    c::dec_timer_pvar_s16(w, id, p::ALERT);
    if st(w, id) == 8 && w.m(id).anim.seq_a == 4 && 16.0 < k.kt && k.kt < 27.0 { kick(w, id); }
    c::dec_timer_pvar_s16(w, id, p::HOLD);
    match st(w, id) {
        0 => init(w, id),
        1 => idle(w, id, &k),
        2 => {
            if w.m(id).anim.seq_a == 2 && wrapped(w, id) {
                if !alerted(w, id) && has_area(w, id) && !in_area(w, id, k.t.pos) {
                    blend(w, id, 0, 10);
                    set_st(w, id, 1);
                } else {
                    to_walk(w, id, 5);
                }
            }
        }
        3 => chop(w, id, &k),
        4 => {
            if !alerted(w, id) {
                set_st(w, id, 1);
            } else if facing_close(&k) {
                to_kick(w, id);
            } else if has_area(w, id) && !in_area(w, id, k.t.pos) {
                if FRAC_PI_2 < k.dyaw { to_turn(w, id); }
            } else {
                to_walk(w, id, 5);
            }
        }
        5 => {
            walker_reset(w, id);
            walk(w, id, k.t.pos);
            if facing_close(&k) { return tail(w, id, to_kick); }
            if has_area(w, id) {
                let alert = alerted(w, id);
                if !ahead_in_area(w, id) {
                    let t_in = in_area(w, id, k.t.pos);
                    if t_in {
                        to_turn(w, id);
                    } else if alert {
                        set_st(w, id, 7);
                    }
                }
                if !alert && !in_area(w, id, k.t.pos) { set_st(w, id, 6); }
            }
        }
        6 => {
            walker_reset(w, id);
            if facing_close(&k) { return tail(w, id, to_kick); }
            let home = c::pv4(w, id, p::HOME);
            if 2.0 <= c::dist2(c::pos(w, id), home) {
                walk(w, id, home);
                if has_area(w, id) {
                    if !ahead_in_area(w, id) {
                        to_turn(w, id);
                        w.mm(id).cmd |= 0x10;
                    } else if in_area(w, id, k.t.pos) {
                        set_st(w, id, 7);
                    }
                }
            } else {
                set_st(w, id, 7);
            }
        }
        7 => {
            if facing_close(&k) { return tail(w, id, to_kick); }
            let y = w.m(id).rotation[2];
            legs::step(w, id, p::W, DT * 0.0, y, DT2 * 20.0);
            if c::pu8(w, id, p::W + legs::w::GAIT) == 0 { set_st(w, id, if alerted(w, id) { 4 } else { 1 }); }
        }
        8 => {
            if w.m(id).anim.seq_a == 4 && wrapped(w, id) {
                c::set_pi16(w, id, p::HOLD, 0);
                blend(w, id, 0, 10);
                set_st(w, id, 1);
            }
        }
        9 => knocked(w, id, &k),
        10 => {
            let a = &w.m(id).anim;
            if a.seq_a == 0x10 && wrapped(w, id) {
                let home_bound = w.m(id).cmd & 0x10 != 0;
                set_st(w, id, if !home_bound && in_area(w, id, k.t.pos) { 5 } else { 6 });
                blend(w, id, 0, 10);
                let m = w.mm(id);
                m.cmd = (m.cmd & !0x10) | 8;
            } else {
                let mut tv = c::pf(w, id, p::TURN_V);
                let y = w.m(id).rotation[2];
                let y = if w.m(id).cmd & 0x10 == 0 {
                    turn::spring_turn(y, k.heading, 0.03, 0.3, DT * 6.457_718_4, &mut tv)
                } else {
                    let home = c::pv4(w, id, p::HOME);
                    turn::spring_turn(y, heading_to(c::pos(w, id), home), DT2 * PI, DT2 * 2.0 * PI, DT * 4.188_790_3, &mut tv)
                };
                c::set_pf(w, id, p::TURN_V, tv);
                w.mm(id).rotation[2] = y;
            }
        }
        0xb if knock::update(w, id, p::K) & 0x40 != 0 || w.m(id).position[2] < 1.0 => {
            let j = w.joint_point(id, 0xc);
            fx::beam_explosion(w, &BEAM, Some(id), j);
            let (pos, rot) = (c::pos(w, id), w.m(id).rotation);
            for k in PIECES { fx::break_piece(w, id, k, pos, rot, 0, 0); }
            return w.delete_moby(id);
        }
        _ => {}
    }
    tail(w, id, |_, _| {});
}

/// The shared tail after a state's `then` (module table: the flash, push-back, flinch, last state, fall).
fn tail(w: &mut World, id: MobyId, then: fn(&mut World, MobyId)) {
    then(w, id);
    flash::update(w, id, p::F);
    let push = c::pf(w, id, p::PUSH);
    if 0.0 < push {
        let me = c::pos(w, id);
        let h = super::hero_pos(w);
        let a = c::atan(me[0] - h[0], me[1] - h[1]);
        let mut to = [me[0] + a.cos() * push, me[1] + a.sin() * push, me[2], me[3]];
        let s = gscale(w, 15.0);
        c::set_pf(w, id, p::PUSH, push + -2.0 / (s * s));
        let mut from = me;
        walker::move_ground(w, id, 0.25, 1.0, 0.75, f32::from_bits(0x3f5f_66f3), &mut from, &mut to, 1);
        let m = w.mm(id);
        m.position[0] = from[0];
        m.position[1] = from[1];
    }
    let fa = c::pf(w, id, p::FLINCH_A) * 0.96;
    c::set_pf(w, id, p::FLINCH_A, fa);
    manip::set_axis(w, id, id, p::FLINCH, fa, 1);
    if st(w, id) != 9 {
        let s = st(w, id);
        c::set_pu8(w, id, p::LAST, s);
    }
    if st(w, id) == 5 || w.m(id).cmd == 6 { return; }
    let v = c::pf(w, id, p::FALL) + DT2 * 9.8;
    c::set_pf(w, id, p::FALL, v);
    w.mm(id).position[2] -= v - 2.0;
    let gz = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    let z = w.m(id).position[2] - 2.0;
    w.mm(id).position[2] = z;
    if z < gz {
        w.mm(id).position[2] = gz;
        c::set_pf(w, id, p::FALL, 0.0);
    }
    if w.m(id).position[2] < 2.0 {
        set_death_bits(w, id, 0, -1);
        w.delete_moby(id);
    }
}

// -------------------------------------------------------------------------------------------------
// The chips, class 1516

mod cp {
    pub const OWNER: usize = 0x00;
    pub const FLOOR: usize = 0x04;
    pub const RADIUS: usize = 0x08;
    pub const BOUNCES: usize = 0x0c;
    pub const GRACE: usize = 0x0e;
    pub const VEL: usize = 0x10;
    pub const SPIN: usize = 0x20;
    pub const SIZE: usize = 0x30;
}

/// `randf_sym(0, 360)`·(π/180)·dt (gp−0x4e88 / −0x4e84; 0.017334 as the game rounds π/180).
fn chip_spin(w: &mut World) -> f32 { w.rng.randf_sym(0.0, 360.0) * f32::from_bits(0x3c8e_fa35) * DT }

/// `0x2e4fe0(0.35, 0.7, owner, vel, pos, euler, 1516, bounces)` (module doc).
fn spawn_chip(w: &mut World, owner: MobyId, vel: c::V, at: c::V, rot: c::V, bounces: i16) -> Option<MobyId> {
    let m = w.create_moby(CHIP_CLASSES[0])?;
    let (light, ambient) = { let o = w.m(owner); (o.light, o.ambient) };
    {
        let mo = w.mm(m);
        if mo.pvars.len() < cp::SIZE { mo.pvars.resize(0x80, 0); }
        mo.draw_dist = 0x7e;
        mo.visible = 1;
        mo.update_dist = 0xff;
        mo.light = light;
        mo.ambient = ambient;
        mo.state = 0;
    }
    c::set_pi32(w, m, cp::OWNER, owner as i32);
    for _ in 0..3 { w.rng.rand_angle(); }
    {
        let mo = w.mm(m);
        mo.position = at;
        mo.rotation = rot;
    }
    c::set_pv4(w, m, cp::VEL, vel);
    let z = w.m(m).position[2];
    c::set_pf(w, m, cp::FLOOR, z);
    let g = w.ticks(20);
    c::set_pi16(w, m, cp::GRACE, g as i16);
    c::set_pf(w, m, cp::SPIN, 0.0);
    let sy = chip_spin(w);
    c::set_pf(w, m, cp::SPIN + 4, sy);
    let sz = chip_spin(w);
    c::set_pf(w, m, cp::SPIN + 8, sz);
    let s = w.rng.randf(0.35, 0.7);
    w.mm(m).scale *= s;
    c::set_pf(w, m, cp::RADIUS, s * 0.25);
    c::set_pi16(w, m, cp::BOUNCES, bounces);
    w.build_matrix(m);
    Some(m)
}

/// `0x2e5178`: the chip's flight (module doc).
pub fn chip_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < cp::SIZE { return; }
    let mut v = c::pv4(w, id, cp::VEL);
    v[2] -= DT2 * 20.0;
    c::set_pv4(w, id, cp::VEL, v);
    let pos = c::add(c::pos(w, id), v);
    c::set_pos(w, id, pos);
    let s = c::pv4(w, id, cp::SPIN);
    {
        let m = w.mm(id);
        for (r, d) in m.rotation.iter_mut().zip(&s[..3]) { *r = c::add_rot(*r, *d); }
    }
    match st(w, id) {
        0 => {
            if w.m(id).position[2] < c::pf(w, id, cp::FLOOR) - 3.0 {
                set_st(w, id, 1);
                return;
            }
            if c::dec_timer_pvar_s16(w, id, cp::GRACE) == 0 || c::pi16(w, id, cp::BOUNCES) == 0 { return; }
            let r = c::pf(w, id, cp::RADIUS);
            let Some(o) = w.coll_sphere(pv(c::pos(w, id)), pf(r), 2, None) else { return };
            let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
            if 0.0 <= c::dot3(v, n) { return; }
            if let Some(pc) = o.pushed_centre {
                let m = w.mm(id);
                m.position[0] = pc[0];
                m.position[1] = pc[1];
                m.position[2] = pc[2];
            }
            let b = c::pi16(w, id, cp::BOUNCES) - 1;
            c::set_pi16(w, id, cp::BOUNCES, b);
            let rv = sv::reflect(pv(v), pv(n)).map(|x| x.to_f32());
            c::set_pv4(w, id, cp::VEL, c::scale(rv, 0.75));
            c::set_pf(w, id, cp::SPIN, 0.0);
            let sy = chip_spin(w);
            c::set_pf(w, id, cp::SPIN + 4, sy);
            let sz = chip_spin(w);
            c::set_pf(w, id, cp::SPIN + 8, sz);
        }
        1 => {
            w.mm(id).scale *= 0.9;
            let a = w.m(id).alpha;
            if a < 4 { w.delete_moby(id); } else { w.mm(id).alpha = a - 4; }
        }
        _ => {}
    }
}
