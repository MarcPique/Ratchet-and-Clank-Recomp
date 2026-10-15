//! U287: class 1023, the hopping gunners of Batalia and Gaspar (levels 08: 6, 09: 34 created instances; level08
//! `0x301158`, level09 `0x3009d0`, the same code), and their shot 1292 (`0x307298` / level09 `0x308f08`, created by
//! `0x307190`). An enemy over the shared creature layer: it carries a gun moby (class 1025, no update of its own) on
//! its joint 4, patrols a path (the shared walk [`walker::walk_to`], `0x26de80`; level08 `0x2635a0` is the same
//! code), and once it has a target (Ratchet or a decoy inside its area path, within its range) goes to its two-point
//! hop path, hops between the two points (the arc of `0x301158` case 4), fires a burst of 20 shots between hops (every
//! `ticks(5)`; shots 1292 fly at 20 u/s dropping by `1/n` and hit what they touch for 1), bites / clubs within 3.3
//! (a sphere hit at joint 4 for 1), celebrates when Ratchet dies, and after five hops takes a breather.
//!
//! **Pvars** (0x220; the creature header: damage record +0x20, hit flash +0x60, knockback +0x70, walker +0xd0):
//! +0x120 the target record (`0x274df8`; +0x160 the moby, +0x164 the kind), +0x1c0 the hop's start, +0x1d0 the area
//! path, +0x1d4 the hop path (two points), +0x1d8 s16 the lure timer, +0x1da s16 the hops since the last rest, +0x1dc
//! the search range (+0x1e4, 6 more while lured), +0x1e0 the turn velocity, +0x1e8 the burst's shot count (the shots'
//! drop `1/n`), +0x1ec the shot timer, +0x1f0 the wait before the next hop, +0x1f4 the path node, +0x1f8 the gun,
//! +0x1fc the patrol path, +0x20c s16 the patrol direction, +0x20e s16 the shot index (0..4).
//!
//! **The tick** (`0x3024d8`, when the state is not 0): the scale (class scale × gp−0x4a80 1.0), the walker's top
//! speed `3·dt`; the gun: created once (class 0x401 at joint 4, rows kept) and then carried (the joint's matrix
//! `fun_0020cca8` copied to its rows, its point to its position); the hit (mask 0x330000, the resolver column 4):
//! any result that is not "no reaction" subtracts the damage and reacts by the reaction byte — 3 for the attacker class
//! 0x47, 1 when the health is gone: 1 / 2 the death flight (0xb: `37·dt²` gravity, `15·dt` / `11·dt` out / up by the
//! push, keys 11 / 16, sequence 0xc, untargetable, flash 0xf0, `BoltBurst(2, 3)`); 3–8 a knockback (0xa: `10·dt` /
//! `3·dt`, keys 5 / 10, sequence 8, flash 0x78); 9 / 10 flash 0xfa; the flash; the lure → `randf(180, 240)` ticks; one
//! tick in four the target search (`0x274df8` in the area, the range; farther than the range or 3 in height: no
//! target), else the target's position is followed; no target → Ratchet; in states 0 / 4 / 5 / 7 the height snaps down
//! to the ground (`GroundHeight(0.5)` below).
//!
//! **States**: 0 init; 1 patrol; 2 to the hop path; 3 at a hop point (turn to the target, idle 0 / 1; the wait
//! `+0x1f0` out → hop 4; within 3.3 → 7; five hops → rest 9); 4 the hop (keys 12..23 of the hop sequence 0x10 / 0x12
//! carry it along the arc `2·(1 − (2f − 1)²)` up; landed: line of sight to the target (`CollLine_Fix` from 1 up to the
//! target's 0.75 up) → 5, else 3 with a `randf(45, 90)`-tick wait); 5 the burst (sequence 6); 6 a walk back to the hop
//! path's first point (not entered by this code); 7 the club (keys 16.5..20 of sequence 0xd: a sphere of 0.8 at joint 4,
//! damage 1, flags 1); 8 the celebration (sequence 0xe) and 9 the rest (sequence 10) → 3; 0xa the knockback (landed →
//! 5, or 7 within 3.3; out of the world → deleted); 0xb the death flight (landed: `SetDeathBits`, the death explosion
//! `0x273f50(0.5, 13, …, sound 0)`, the pieces 1692–1694, deleted with its gun).
//!
//! A Morph-o-Ray morph (the damage record's keep byte +0x2e = 2) deletes it and its gun. Coverage: docs/plan/creatures.md
//! §9. Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::{bolt_burst, set_death_bits};
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, knock, target, turn, walker};
use crate::moby_update::services::{pf, pv, HitTemplate, World};
use crate::ps2v::Pf;

pub const UPDATE_FN: u32 = 0x30_1158;
pub const SHOT_FN: u32 = 0x30_7298;
pub const REFERENCE_LEVEL: u32 = 8;
pub const CLASSES: [i16; 1] = [1023];
pub const SHOT_CLASSES: [i16; 1] = [1292];
/// The gun (`CreateMoby(0x401)`; no update in the level tables).
pub const GUN_CLASS: i16 = 0x401;
/// The death's pieces (`BreakFxB` 0x69c..0x69e).
pub const PIECES: [i16; 3] = [0x69c, 0x69d, 0x69e];
/// Joint list 4 (the club and the gun) of 1023, list 0 of the gun (the muzzle).
pub const JOINTS: [i16; 2] = [1023, GUN_CLASS];

pub mod pv {
    pub const D: usize = 0x20;
    pub const MORPH_KEEP: usize = 0x2e;
    pub const LURE: usize = 0x38;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const TGT: usize = 0x120;
    pub const TGT_MOBY: usize = 0x160;
    pub const TGT_KIND: usize = 0x164;
    pub const HOP_FROM: usize = 0x1c0;
    pub const AREA: usize = 0x1d0;
    pub const HOP: usize = 0x1d4;
    pub const ALERT_T: usize = 0x1d8;
    pub const HOPS: usize = 0x1da;
    pub const RANGE: usize = 0x1dc;
    pub const TURN_V: usize = 0x1e0;
    pub const BASE_RANGE: usize = 0x1e4;
    pub const SHOTS: usize = 0x1e8;
    pub const SHOT_T: usize = 0x1ec;
    pub const WAIT_T: usize = 0x1f0;
    pub const NODE: usize = 0x1f4;
    pub const GUN: usize = 0x1f8;
    pub const PATROL: usize = 0x1fc;
    pub const DIR: usize = 0x20c;
    pub const BURST: usize = 0x20e;
    pub const SIZE: usize = 0x210;
}

/// The level08 `.lit` words (gp = 0x166c00, gp−0x4a80..): the scale 1.0, the gravity 37, the knock speeds 10 / 3, the
/// death's 15 / 11, the tick's top speed 3, the move 0.6 / 0.6, the shot interval 5, the shot count step 1, the shot
/// speed 20, the club range 3.3, the hops before a rest 5, the hop height 2.
pub mod k {
    pub const SCALE: f32 = 1.0;
    pub const GRAVITY: f32 = 37.0;
    pub const KNOCK_OUT: f32 = 10.0;
    pub const DEATH_UP: f32 = 11.0;
    pub const DEATH_OUT: f32 = 15.0;
    pub const TOP_SPEED: f32 = 3.0;
    pub const MOVE: f32 = 0.6;
    pub const SHOT_EVERY: i32 = 5;
    pub const SHOT_STEP: f32 = 1.0;
    pub const SHOT_SPEED: f32 = 20.0;
    pub const CLUB_RANGE: f32 = 3.3;
    pub const REST_AFTER: i16 = 5;
    pub const HOP_HEIGHT: f32 = 2.0;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const PATROL: u8 = 1;
    pub const TO_HOP: u8 = 2;
    pub const READY: u8 = 3;
    pub const HOP: u8 = 4;
    pub const FIRE: u8 = 5;
    pub const BACK: u8 = 6;
    pub const CLUB: u8 = 7;
    pub const CHEER: u8 = 8;
    pub const REST: u8 = 9;
    pub const KNOCKED: u8 = 0xa;
    pub const DYING: u8 = 0xb;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `rand() % 4 + base` (the C remainder of a non-negative value).
fn rand4(w: &mut World, base: i32) -> i32 { w.rng.rand() % 4 + base }
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn set_moby_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }
fn gone(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s == crate::moby_runtime::state::DELETED || s == crate::moby_runtime::state::DELETED_STATIC }
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn point(w: &World, p: Option<usize>, i: i32) -> c::V {
    p.and_then(|p| w.svc.splines[p].get(usize::try_from(i).ok()?)).map(|q| q.map(f32::from_bits)).unwrap_or([0.0; 4])
}
fn count(w: &World, p: Option<usize>) -> i32 { p.map_or(0, |p| w.svc.splines[p].len() as i32) }
fn nearest(w: &World, p: Option<usize>, pos: c::V) -> i32 { p.map_or(0, |p| crate::path::nearest_at_distance(&w.svc.splines[p], 0.0, pos)) }
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
fn target_pos(w: &World, t: Option<MobyId>) -> c::V { t.map(|m| c::pos(w, m)).unwrap_or([0.0; 4]) }

/// `0x270cc0(atan(target − pos), 5π/18·dt² (8.73), same, 4π·dt, yaw, +0x1e0)`.
fn face(w: &mut World, id: MobyId, t: c::V) {
    let p = c::pos(w, id);
    let h = c::atan(t[0] - p[0], t[1] - p[1]);
    turn::turn_toward_pvar(w, id, h, c::DT2 * 8.726_646, c::DT2 * 8.726_646, c::DT * 12.566_371, pv::TURN_V);
}

/// The hop's sequence: 0x12 when the next point lies behind the side row (`moby+0xd0 · (point − pos) < 0`), else 0x10.
fn start_hop(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOP_FROM, p);
    let n = (c::pi32(w, id, pv::NODE) + 1) & 1;
    c::set_pi32(w, id, pv::NODE, n);
    set_state(w, id, st::HOP);
    let q = point(w, path(w, id, pv::HOP), n);
    let d = c::sub(q, p);
    let s = if c::dot3(w.m(id).rows[1], d) < 0.0 { 0x12 } else { 0x10 };
    if seq(w, id) != s {
        let t = w.ticks(5);
        w.anim_blend(id, s, 0, t);
    }
}

fn to_club(w: &mut World, id: MobyId) {
    set_state(w, id, st::CLUB);
    if seq(w, id) != 0xd {
        let t = rand4(w, 8);
        w.anim_blend(id, 0xd, 0, t);
    }
}

fn to_fire(w: &mut World, id: MobyId, extra: bool) {
    c::set_pf(w, id, pv::SHOTS, 1.0);
    set_state(w, id, st::FIRE);
    c::set_pi16(w, id, pv::BURST, 0);
    if seq(w, id) != 6 {
        let mut t = w.ticks(10);
        if extra {
            let t7 = w.ticks(7);
            t += w.rng.rand_range(0, t7);
        }
        w.anim_blend(id, 6, 0, t);
    }
}

/// Five hops done: the rest (sequence 10 from frame 2 over `t(w)` ticks, drawn only when it blends).
fn rest(w: &mut World, id: MobyId, t: fn(&mut World) -> i32) {
    c::set_pi16(w, id, pv::HOPS, 0);
    if seq(w, id) != 10 {
        let n = t(w);
        w.anim_blend(id, 10, 2, n);
    }
    set_state(w, id, st::REST);
}

fn delete_with_gun(w: &mut World, id: MobyId) {
    w.delete_moby(id);
    if let Some(g) = moby_ref(w, id, pv::GUN) { w.delete_moby(g); }
}

/// `0x3024d8`: the tick before the states (module doc).
fn pre(w: &mut World, id: MobyId) {
    let o = w.m(id).o_class;
    w.mm(id).scale = crate::moby_update::classes::units::class_scale(w, o) * k::SCALE;
    c::set_pf(w, id, pv::J + 0x24, k::TOP_SPEED * c::DT);
    match moby_ref(w, id, pv::GUN) {
        None => {
            if let Some(g) = w.create_moby(GUN_CLASS) {
                set_moby_ref(w, id, pv::GUN, Some(g));
                let jp = w.joint_point(id, 4);
                let m = w.mm(g);
                m.draw_dist = 0x40;
                m.update_dist = 0x40;
                m.visible = 1;
                m.mode |= mode::KEEP_ROWS;
                m.position = jp;
            }
            w.mm(id).mode |= mode::TARGETABLE;
        }
        Some(g) => {
            let jp = w.joint_point(id, 4);
            let mtx = joint_matrix(w, id, 4);
            let m = w.mm(g);
            m.rows[..3].copy_from_slice(&mtx);
            m.position = jp;
            w.build_matrix(g);
        }
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let attacker47 = res.hit.and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == 0x47);
        let mut r = if attacker47 { 3 } else { res.reaction as i32 };
        if hp <= 0.0 { r = 1; }
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::RADIUS, 0x200);
        c::set_pf(w, id, kr + knock::k::ZOFF, 0.5);
        c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
        c::set_pf(w, id, kr + knock::k::GRAVITY, k::GRAVITY * c::DT2);
        c::set_pu8(w, id, kr + 0x3d, 0);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        let push = |w: &mut World, out: f32, up: f32, apex: f32, land: f32, seq: u8| {
            c::set_pf(w, id, kr + knock::k::KEY_APEX, apex);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, land);
            let (mut sp, mut u) = (out, up);
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, seq, 1, 0);
        };
        match r {
            1 | 2 => {
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pf(w, id, kr + knock::k::ZOFF, 0.25);
                push(w, k::DEATH_OUT * c::DT, k::DEATH_UP * c::DT, 11.0, 16.0, 0xc);
                set_state(w, id, st::DYING);
                c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
                bolt_burst(w, id, 2, 3, 0, -1);
            }
            3..=8 => {
                push(w, k::KNOCK_OUT * c::DT, 3.0 * c::DT, 5.0, 10.0, 8);
                set_state(w, id, st::KNOCKED);
                c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            }
            9 | 10 => c::set_pu8(w, id, pv::FLASH + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    if c::pi32(w, id, pv::LURE) != 0 {
        let f = w.rng.randf(180.0, 240.0);
        let t = w.ticks(f as i32);
        c::set_pi16(w, id, pv::ALERT_T, t as i16);
        c::set_pi32(w, id, pv::LURE, 0);
    }
    c::dec_timer_pvar_s16(w, id, pv::ALERT_T);
    let mut range = c::pf(w, id, pv::BASE_RANGE);
    if c::pi16(w, id, pv::ALERT_T) != 0 { range += 6.0; }
    c::set_pf(w, id, pv::RANGE, range);
    if w.rng.randi(4) == 0 {
        // A path index of −1 would make the game read the word before its path table; every 1023 has an area [L].
        let t = target::acquire_in(w, id, range, path(w, id, pv::AREA));
        c::set_pv4(w, id, pv::TGT, t.pos);
        c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
        c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
        c::set_pv4(w, id, pv::TGT + 0x30, t.body);
        set_moby_ref(w, id, pv::TGT_MOBY, t.moby);
        c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
        if t.kind != 2 {
            let p = c::pos(w, id);
            if range < c::dist2(p, t.pos) || 3.0 < (p[2] - t.pos[2]).abs() { c::set_pi32(w, id, pv::TGT_KIND, 2); }
        }
    } else {
        match moby_ref(w, id, pv::TGT_MOBY).filter(|&m| !gone(w, m)) {
            Some(m) => {
                let p = c::pos(w, m);
                c::set_pv4(w, id, pv::TGT, p);
            }
            None => {
                c::set_pi32(w, id, pv::TGT_MOBY, 0);
                c::set_pi32(w, id, pv::TGT_KIND, 2);
            }
        }
    }
    if moby_ref(w, id, pv::TGT_MOBY).is_none() {
        let h = w.hero_moby;
        set_moby_ref(w, id, pv::TGT_MOBY, h);
    }
    if matches!(state(w, id), st::INIT | st::FIRE | st::CLUB | st::HOP) {
        let mut p = c::pos(w, id);
        let g = ground::ground(w, p, 0.5, 0).z;
        if g < p[2] {
            p[2] = g;
            c::set_pos(w, id, p);
        }
    }
}

/// `fun_0020cca8(moby, list, out)`: the world rows of joint list `list`'s last joint (its pose rows turned by the moby's
/// rows; `rc_formats::moby_anim::attach_matrix`, the same value).
fn joint_matrix(w: &World, id: MobyId, list: usize) -> [[f32; 4]; 3] {
    let m = w.m(id);
    let chain = w.svc.joint_lists.get(&m.o_class).and_then(|l| l.get(list)).filter(|c| !c.is_empty());
    let Some((class, chain)) = w.classes.anim(m.o_class).zip(chain) else { return [m.rows[0], m.rows[1], m.rows[2]] };
    let snap = w.svc.snapshots.get(id).and_then(|s| s.as_ref());
    let p = rc_formats::moby_anim::evaluate_chain(class, &m.anim, snap, chain);
    let rows = [m.rows[0], m.rows[1], m.rows[2]].map(|r| r.map(f32::to_bits));
    let r = rc_formats::moby_anim::attach_matrix(&p, &rows, [m.position[0], m.position[1], m.position[2]], m.scale);
    [r[0], r[1], r[2]]
}

/// State 0.
fn init(w: &mut World, id: MobyId) {
    c::set_pu8(w, id, pv::D + 9, 0);
    c::set_pu8(w, id, pv::MORPH_KEEP, 1);
    c::set_pu8(w, id, pv::D + 8, 1);
    c::set_pf(w, id, pv::D, 2.0);
    c::set_pi16(w, id, pv::D + 4, 2);
    c::set_pu8(w, id, 0x58, 15);
    c::set_pu8(w, id, 0x5a, 12);
    let hop = path(w, id, pv::HOP);
    if c::pi32(w, id, pv::AREA) == -1 || hop.is_none() {
        // `printf` of the missing path, then DeleteMoby.
        w.delete_moby(id);
        return;
    }
    let j = pv::J;
    walker::seed(&mut w.mm(id).pvars, j);
    c::set_pf(w, id, j + 8, 0.5);
    c::set_pf(w, id, j + 0xc, 0.5);
    c::set_pi32(w, id, j, 512);
    c::set_pf(w, id, j + 0x40, c::DT2 * 6.981_317);
    c::set_pf(w, id, j + 0x44, c::DT * 8.726_646);
    c::set_pf(w, id, j + 0x28, f32::from_bits(0x3eb2_b8c2));
    c::set_pf(w, id, j + 0x2c, 0.2);
    c::set_pf(w, id, j + 0x3c, c::DT2 * 6.981_317);
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    let p0 = point(w, hop, 0);
    c::set_pos(w, id, p0);
    let mut s = st::READY;
    let patrol = path(w, id, pv::PATROL);
    if patrol.is_some() {
        let d = (w.rng.rand() & 1) as i16 * 2 - 1;
        c::set_pi16(w, id, pv::DIR, d);
        let i = nearest(w, patrol, p0) + d as i32;
        c::set_pi32(w, id, pv::NODE, i);
        if !(i < count(w, patrol) && -1 < i) {
            c::set_pi16(w, id, pv::DIR, -d);
            c::set_pi32(w, id, pv::NODE, i - 2 * d as i32);
        }
        if seq(w, id) != 2 {
            let t = rand4(w, 7);
            w.anim_blend(id, 2, 0, t);
        }
        s = st::PATROL;
    }
    set_state(w, id, s);
    c::set_pi16(w, id, pv::HOPS, 0);
}

/// The morph and the out-of-range skip, then the tick and the states (`0x301158`).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible == 0 && 40.0 < c::dist3(c::pos(w, id), cam) { return; }
    // 0x26dae0(2.5, moby, 6, +0x170): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 6, id, 0x170);
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 29.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x17;
    }
    if w.svc.game_mode == 2 { return; }
    if state(w, id) != st::INIT { pre(w, id); }
    let tgt = moby_ref(w, id, pv::TGT_MOBY);
    let patrol = path(w, id, pv::PATROL);
    let hop = path(w, id, pv::HOP);
    // Ratchet dead (state 0x3d) for less than ticks(30): a one-in-49 celebration each tick within 25.
    if w.hero.state == 0x3d && w.hero.timer < w.ticks(30) && !matches!(state(w, id), 8 | 10 | 4 | 11) {
        let hp = crate::moby_update::classes::units::hero_pos(w);
        if c::dist2(c::pos(w, id), hp) < 25.0 && w.rng.randi(0x31) == 0 {
            if seq(w, id) != 0xe {
                let t = w.ticks(0xb);
                w.anim_blend(id, 0xe, 0, t);
            }
            set_state(w, id, st::CHEER);
        }
    }
    if state(w, id) != st::INIT && c::pu8(w, id, pv::MORPH_KEEP) == 2 {
        delete_with_gun(w, id);
        return;
    }
    match state(w, id) {
        st::INIT => init(w, id),
        st::PATROL => {
            let mut i = c::pi32(w, id, pv::NODE);
            if c::dist2(c::pos(w, id), point(w, patrol, i)) < c::pf(w, id, pv::J + 0x10) {
                let d = c::pi16(w, id, pv::DIR);
                i += d as i32;
                if !(i < count(w, patrol) && -1 < i) {
                    c::set_pf(w, id, pv::J + 0x10, 0.0);
                    c::set_pi16(w, id, pv::DIR, -d);
                    i -= 2 * d as i32;
                }
                c::set_pi32(w, id, pv::NODE, i);
            }
            let mut out = [0.0; 4];
            let t = point(w, patrol, i);
            walker::walk_to(w, id, pv::J, t, &mut out);
            if tgt.is_some() && kind(w, id) != 2 && c::dec_timer_pvar_i32(w, id, pv::WAIT_T) != 0 {
                let n = nearest(w, hop, c::pos(w, id));
                c::set_pi32(w, id, pv::NODE, n);
                set_state(w, id, st::TO_HOP);
            }
        }
        st::TO_HOP => {
            let n = nearest(w, hop, c::pos(w, id));
            c::set_pi32(w, id, pv::NODE, n);
            let q = point(w, hop, n);
            let d = c::dist2(c::pos(w, id), q);
            if 1.0 <= d && c::pf(w, id, pv::J + 0x10) <= d {
                let mut out = [0.0; 4];
                walker::walk_to(w, id, pv::J, q, &mut out);
                if seq(w, id) != 2 {
                    let t = rand4(w, 7);
                    w.anim_blend(id, 2, 0, t);
                }
                return;
            }
            c::set_pf(w, id, pv::J + 0x10, 0.0);
            set_state(w, id, st::READY);
        }
        st::READY => ready(w, id, tgt),
        st::HOP => hop_state(w, id, tgt, hop),
        st::FIRE => fire(w, id, tgt, hop),
        st::BACK => {
            let t = target_pos(w, tgt);
            face(w, id, t);
            let kt = ground::key_time(w, id);
            if 12.0 < kt && kt < 23.0 && w.m(id).anim.t == 0.0 {
                let d = c::sub(point(w, hop, 0), c::pos(w, id));
                let mut mv = c::clamp_len3(d, c::DT * 24.0);
                walker::move_collide(w, id, k::MOVE, k::MOVE, 0.0, &mut mv, 0);
                return;
            }
            if !wrapped(w, id) { return; }
            let f = w.rng.randf(45.0, 90.0);
            let t = w.ticks(f as i32);
            c::set_pi32(w, id, pv::WAIT_T, t);
            set_state(w, id, st::READY);
            if seq(w, id) != 0 {
                let t = w.ticks(10);
                w.anim_blend(id, 0, 0, t);
            }
        }
        st::CLUB => {
            let t = target_pos(w, tgt);
            face(w, id, t);
            let kt = ground::key_time(w, id);
            if 16.5 < kt && kt < 20.0 {
                let mut jp = w.joint_point(id, 4);
                let p = c::pos(w, id);
                let mut d = c::sub(jp, p);
                d[2] = 0.0;
                let mut d = c::set_len3(d, 1.0);
                jp[2] -= 0.25;
                d[2] = 1.0;
                // `0x26e808(1, tmpl, moby, 1, dir)` then `coll_sphere_mobys(0.8, jp, 0, moby, tmpl)`; the dir's w is
                // the normalised difference's (no exact-push marker) [L].
                let tmpl = HitTemplate { dir: [pf(d[0]), pf(d[1]), Pf::ONE, pf(d[3])], attacker: Some(id), flags: 1, damage: Pf::ONE, w20: 1, ..Default::default() };
                w.sphere_mobys(pf(0.8), pv(jp), 0, Some(id), Some(&tmpl));
            }
            if wrapped(w, id) { set_state(w, id, st::TO_HOP); }
        }
        st::CHEER | st::REST => {
            if !wrapped(w, id) { return; }
            set_state(w, id, st::READY);
            if seq(w, id) != 0 {
                let t = w.ticks(10);
                w.anim_blend(id, 0, 0, t);
            }
        }
        st::KNOCKED => {
            let r = knock::update(w, id, pv::K);
            if r & 0x40 != 0 {
                if c::dist2(c::pos(w, id), target_pos(w, tgt)) < k::CLUB_RANGE {
                    to_club(w, id);
                } else {
                    to_fire(w, id, true);
                }
            } else if r & 0x120 != 0 {
                w.delete_moby(id);
            }
        }
        st::DYING if knock::update(w, id, pv::K) & 0x160 != 0 => {
            let p = c::pos(w, id);
            let rot = w.m(id).rotation;
            set_death_bits(w, id, 0, -1);
            fx::death_explosion(w, 0.5, 13.0, Some(id), p, 0);
            for cl in PIECES { fx::break_piece(w, id, cl, p, rot, 0, 0); }
            delete_with_gun(w, id);
        }
        _ => {}
    }
}

/// State 3.
fn ready(w: &mut World, id: MobyId, tgt: Option<MobyId>) {
    let t = target_pos(w, tgt);
    face(w, id, t);
    if wrapped(w, id) {
        let s = seq(w, id);
        let r = w.rng.rand() & 1;
        if s as i32 != r {
            let n = w.rng.rand() & 1;
            let t7 = w.ticks(7);
            let m = w.rng.rand() % 7;
            w.anim_blend(id, n as u8, 0, t7 + m);
        }
    }
    if k::REST_AFTER < c::pi16(w, id, pv::HOPS) {
        return rest(w, id, |w| {
            let t7 = w.ticks(7);
            let t5 = w.ticks(5);
            t7 + w.rng.rand_range(0, t5)
        });
    }
    if tgt.is_none() { return; }
    if kind(w, id) != 2 && c::dec_timer_pvar_i32(w, id, pv::WAIT_T) != 0 {
        start_hop(w, id);
        return;
    }
    if c::dist2(c::pos(w, id), t) < k::CLUB_RANGE {
        set_state(w, id, st::CLUB);
        if seq(w, id) != 0xd {
            let t = w.ticks(10);
            w.anim_blend(id, 0xd, 0, t);
        }
    }
}

/// State 4: the hop.
fn hop_state(w: &mut World, id: MobyId, tgt: Option<MobyId>, hop: Option<usize>) {
    let t = target_pos(w, tgt);
    face(w, id, t);
    let kt = ground::key_time(w, id);
    if 12.0 < kt && kt < 23.0 && w.m(id).anim.seq_a == w.m(id).anim.seq_b {
        let f = (1.0 - (23.0 - kt) / 11.0).clamp(0.0, 1.0);
        let a = c::pv4(w, id, pv::HOP_FROM);
        let b = point(w, hop, c::pi32(w, id, pv::NODE));
        let q: c::V = std::array::from_fn(|l| a[l] + (b[l] - a[l]) * f);
        let g = (f - 0.5) + (f - 0.5);
        c::set_pos(w, id, [q[0], q[1], q[2] + k::HOP_HEIGHT * (1.0 - g * g), q[3]]);
        return;
    }
    if !wrapped(w, id) { return; }
    // The line of sight: from 1 above the feet to the target's 0.75 above its feet; the moby it hits must be the target.
    let p = c::pos(w, id);
    let a = [p[0], p[1], p[2] + 1.0, p[3]];
    let b = [t[0], t[1], t[2] + 0.75, t[3]];
    let seen = w.coll_line(pv(a), pv(b), 0, Some(id)).and_then(|o| o.moby);
    if k::REST_AFTER < c::pi16(w, id, pv::HOPS) { return rest(w, id, |w| rand4(w, 5)); }
    if c::dist2(p, t) < k::CLUB_RANGE { return to_club(w, id); }
    if seen != tgt || tgt.is_none() {
        set_state(w, id, st::READY);
        if seq(w, id) != 0 {
            let t = w.ticks(5);
            w.anim_blend(id, 0, 0, t);
        }
        let f = w.rng.randf(45.0, 90.0);
        let t = w.ticks(f as i32);
        c::set_pi32(w, id, pv::WAIT_T, t);
        return;
    }
    to_fire(w, id, false);
}

/// State 5: the burst.
fn fire(w: &mut World, id: MobyId, tgt: Option<MobyId>, _hop: Option<usize>) {
    let t = target_pos(w, tgt);
    if c::dist2(c::pos(w, id), t) < k::CLUB_RANGE {
        c::set_pi32(w, id, pv::SHOT_T, 0);
        return to_club(w, id);
    }
    if c::dec_timer_pvar_i32(w, id, pv::SHOT_T) == 0 { return; }
    let (l0, l1) = fx::frame_load(w);
    let t = w.ticks(k::SHOT_EVERY + (l1 * 1.3 + l0) as i32);
    c::set_pi32(w, id, pv::SHOT_T, t);
    let n = c::pf(w, id, pv::SHOTS) + k::SHOT_STEP;
    c::set_pf(w, id, pv::SHOTS, n);
    let burst = c::pi16(w, id, pv::BURST);
    if !((0.8 <= l1 || 0.8 <= l0) && burst & 1 != 0) {
        if let Some(g) = moby_ref(w, id, pv::GUN) {
            let muzzle = w.joint_point(g, 0);
            let (cy, sy) = c::cs(c::yaw(w, id));
            let v = [cy * k::SHOT_SPEED * c::DT, sy * k::SHOT_SPEED * c::DT, (-1.0 / n) * k::SHOT_SPEED * c::DT, 0.0];
            if let Some(s) = spawn_shot(w, v, muzzle, id, tgt, 180, burst as i32) { w.mm(s).scale *= 1.7; }
        }
    }
    let b = burst + 1;
    c::set_pi16(w, id, pv::BURST, if 4 < b { 0 } else { b });
    if n <= 20.0 { return; }
    let h = c::pi16(w, id, pv::HOPS) + 1;
    c::set_pi16(w, id, pv::HOPS, h);
    start_hop(w, id);
}

/// Shot pvars: +0x00 velocity, +0x10 the shooter, +0x14 the target, +0x18 the life timer, +0x28 0, +0x2c the shot
/// index of the burst (`0x307190`).
pub mod shot {
    pub const VEL: usize = 0x00;
    pub const SHOOTER: usize = 0x10;
    pub const TARGET: usize = 0x14;
    pub const LIFE: usize = 0x18;
    pub const INDEX: usize = 0x2c;
}

/// `0x307190(vel, pos, shooter, target, life, index)`: a shot 1292 (update distance 0xff, draw distance 0x7e, state 1,
/// drawn), yaw and pitch along the velocity, its matrix built.
pub fn spawn_shot(w: &mut World, v: c::V, pos: c::V, shooter: MobyId, target: Option<MobyId>, life: i32, index: i32) -> Option<MobyId> {
    let s = w.create_moby(SHOT_CLASSES[0])?;
    let t = w.ticks(life);
    {
        let m = w.mm(s);
        m.update_dist = 0xff;
        m.draw_dist = 0x7e;
        m.state = 1;
        m.visible = 1;
        m.position = pos;
    }
    c::set_pv4(w, s, shot::VEL, v);
    set_moby_ref(w, s, shot::SHOOTER, Some(shooter));
    set_moby_ref(w, s, shot::TARGET, target);
    c::set_pi32(w, s, shot::INDEX, index);
    c::set_pi32(w, s, 0x28, 0);
    c::set_pi32(w, s, shot::LIFE, t);
    let yaw = c::atan(v[0], v[1]);
    let pitch = c::atan(c::len2(v), v[2]);
    let m = w.mm(s);
    m.rotation[2] = yaw;
    m.rotation[1] = -pitch;
    w.build_matrix(s);
    Some(s)
}

/// `0x307298`: the shot (state 1 flying, 2 bursting).
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { return; }
    match state(w, id) {
        1 => {
            let old = c::pos(w, id);
            if moby_ref(w, id, shot::TARGET).is_none_or(|t| gone(w, t)) { set_state(w, id, 2); }
            let (l0, l1) = fx::frame_load(w);
            let v = c::pv4(w, id, shot::VEL);
            if l1 < 0.9 && l0 < 0.9 {
                // The trail: a type-4 puff 0.1 behind along the velocity, drifting at 0.95..0.985 of it.
                let s = w.rng.randf(0.95, 0.985);
                let pv4 = c::scale(v, s);
                let at = c::add(c::set_len3(v, -0.1), old);
                let n = w.rng.rand_range(15, 20);
                let life = w.ticks(n);
                let g = w.rng.rand_range(7, 10);
                let growth = w.ticks(g) as i16;
                let sp = crate::particles::type04::Spawn { pos: at, vel: pv4, c1: 0x6f00_afff, c2: 0xff, life, base: 0x28, growth, additive: true };
                fx::part04(w, &sp);
            }
            let p = c::add(old, v);
            c::set_pos(w, id, p);
            let shooter = moby_ref(w, id, shot::SHOOTER);
            if let Some(o) = w.coll_line(pv(p), pv(old), 0x10, shooter) {
                if o.moby != shooter {
                    if let Some(m) = o.moby {
                        let at = [o.point[0], o.point[1], o.point[2], 0.0];
                        crate::moby_update::creature::attack::hit_moby(w, m, id, 1.0, 0x1_0001, at, v);
                    }
                    set_state(w, id, 2);
                }
            }
            if c::dec_timer_pvar_i32(w, id, shot::LIFE) != 0 { set_state(w, id, 2); }
        }
        2 => {
            let (l0, l1) = fx::frame_load(w);
            let light = if (l1 < 0.85 && l0 < 0.85) || c::pi32(w, id, shot::INDEX) == 1 { 13.0 } else { 0.0 };
            let p = c::pos(w, id);
            fx::death_explosion(w, 0.25, light, Some(id), p, -1);
            w.delete_moby(id);
        }
        _ => {}
    }
}
