//! **Hoven's burrowers, class 238** (level12 `0x2e1be0`, its tick `0x2e3528`; census U423; 71 placed, 54 created, in
//! moby groups). A small melee creature: most wait burrowed (hidden) until their target comes within their placed
//! range and pop out of the ground in a spray of dust; then they turn to it, strafe toward it (sidestepping either way,
//! swapping sides every 30–90 ticks, kept inside their area polygon), bite (one hit of 1 at frame 13 of sequence 4
//! when within 2 and facing it) and go home when it leaves; idle ones wander about home. Two hits (health 2) kill
//! one; a hit knocks it back; it falls to its death below z 26. Some start in their placed jump-out spot (home w ≠ 0)
//! and leap to their home (a lob) when their group is woken.
//!
//! **Pvars** (0x2f0): +0x20 the damage record (health 2), +0x26 s16 the hit cooldown, +0x38 the lure, +0x60 the suck
//! record (`react::HOVEN_238`), +0x110 the flash, +0x120 the knockback record, +0x180 the walker, +0x1d0 the wander
//! record (`walker::wr`), +0x200 the look-at record (`manip::look`, +0x268 its yaw target), +0x280 home (w: start at
//! the jump-out spot), +0x2a0 the turn velocity, +0x2a4 the sidestep angle (+0x2a8 in degrees, placed 30), +0x2ac the
//! range (placed: the emerge range; awake: 20, 12 once its alert is out), +0x2bc burrowed, +0x2c0 the side timer,
//! +0x2c4 the alert, +0x2c8 s16 the emerge timer, +0x2cc s16 the walk roll, +0x2d0 the area (a level path), +0x2d4
//! stuck ticks, +0x2d8 ticks going home, +0x2dc the awake limit, +0x2e0 its counter's pointer, +0x2e4 the count's
//! tick, +0x2e8 s16 the cull flag, +0x2ea s16 its tick count.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e3528` | the tick: the lure → alert `ticks(240)`; the range (20 / 12) outside states 0, 9, 0xb; the cull (+0x2e8: after `15·ticks(60)` ticks off screen, more than 3 in the group, 1 in 0x45 a tick: a hit of 1 from Ratchet on itself); a hit (0x330000) through the resolver (column 4): the group told 7, damage: K (gravity 0.008, drag 0.002, speed 3.8·dt, up 7.5·dt, flags 9, bounce 1, 1, +0x38 1), health − damage; dead: untargetable, 8·dt / 10·dt, the flight along the hit (seq 6, keys 11 / 18), 99, flash 0x78; else seq 0xe, keys −1, 7, flash 0xfa, cooldown `ticks(60)`; the sparks; the flash | [`tick`] (`damage::resolve`, `knock::*`, `fx::clump_sparks`) |
//! | `0x2e1be0` | the update (below) | [`update`] |
//! | `0x2e18e8` | the run: `randi(2)` → seq 3 / 0xf (0x10 never) over `ticks(8)` | [`run_anim`] |
//! | `0x2e1878` | the ground follow: z approaches `GroundHeight(0.5, pos + 0.5 up)` at 27·dt² | [`steer`] |
//! | `0x2e2eb8` | the steer: the ground follow, `SpringTurn2(h, 0.05, 0.3, 0.2)`, the walker step toward 2·(cos, sin) yaw; with an area: the line from the new point to 0.05 past the old one clamped to the path (outside its polygon: the clamped point), moved less than 0.5·dt for more than `ticks(10)` ticks → 2 | [`steer`] (`walker::step`, `region::clamp`) |
//! | `0x2e19a8` | the emerge dust: 27 type-2 puffs about the point (z −0.25..−0.75, xy ±0.25), out `randf(1, 3)`·dt, up `randf(7, 2)`·dt, falling `randf(−0.1, −1)`·dt, sizes 0.3 / 0.55, colours 0xa0f0e8e0 / 0x20f0e8e0, times `randf(1, 10)`, `randf(15, 25)`, `randf(40, 70)` | [`dust`] |
//! | `0x2e3420` | a type-52 puff within 0.8 (sizes `randf(0, 1.5)`, `randf(1.5, 5.7)`, 0x38002028 / 0x2020, `rand_range(100, 180)`) | [`puff`] |
//! | `0x2e3098` | the death sparks (×1.5 in state 0xf) | `fx::clump_sparks` |
//!
//! **The update.** Target `0x274b78` within the range (none: Ratchet's moby); the look-at scale (the big-head cheat,
//! 2.2). Below z 26: deleted.
//! 0: home = position; burrowed → 9 (seq 0xc, no collision, hidden, frozen); home w ≠ 0 → 0xb (draw distance 0,
//! untargetable, no collision); else 1 (seq 0); health 2, K zoff 0.5 / radius 0x200, the walker (radius 0x38d, top
//! speed 5.5·dt, ledge 0.5 / 0.5, flag 0x10), the wander home, the emerge timer `rand_range(ticks 20, ticks 170)` with
//! the limit, the walk roll `rand_range(ticks 120, ticks 700)`, z on the ground, mirrored half the time.
//! 1: the target within the range of home and 4 in z → 3 (facing 55° off: seq 0x11, else seq 2 at speed 1.55); else a
//! command 7 (or 1, one tick in 19) → the sidestep (3, seq 2); else wander (seq 0xf, step 2.5·dt when the walk roll
//! beats `ticks(20)`, else 0; speed 0.8; `walker::wander(0.35, 0.25)`).
//! 3: turn to it (0.03, 0.3, 2π·dt); the anim over → 4 (run).
//! 4: the look yaw (±40°); steer toward its heading + the sidestep; stuck → 6; the side timer out → swap; within 1.5
//! (xy) → 5 (seq 4); within range + 5.5 of home and 4.4 in z → the group told 1; else (command 1 or one in nine) → 6.
//! 5: Ratchet's anim speed 1.3; turn (0.05, 0.3, 0.2); the bite; the anim over beyond 2 → 4.
//! 6: steer home; within 2 → 1 (seq 0); Ratchet outside the area → stay; the target within range: after
//! `ticks(70)` ticks the group told 1 and 4, else wait; command 1 → 4.
//! 7: the group told 1; the flight: landed → 10 (seq 0xd from frame 8); below z 27 → deleted.
//! 8: held by the Suck Cannon (`react::carried`); landed → 6.
//! 9: burrowed; with the limit: within 2 → wait; the group's awake count (states ≥ 0, not 9) once a tick into the
//! counter, more than 5 → wait. The target within range and 8 in z, one tick in 19: the group told 1 and out; else the
//! emerge timer (command odd) or command 7 → out: 0xd (seq 7), targetable, the dust.
//! 10: the anim key beyond 29 → 4.
//! 0xb: turn home; the target within range of home and 8 in z (or command 2): the group told 2, the lob to home (7·dt,
//! gravity 28·dt², flags 5, seq 8, keys 5 / 10) → 0xc, targetable, collision.
//! 0xc: the flight; landed → 0xe (seq 9 from frame 8); below z 0 → deleted.
//! 0xd: collision, shown and animated; turn; the anim over → the sidestep, 4.
//! 0xe: the anim over → the sidestep, 4.
//! 0xf: a puff, the sparks twice, deleted.
//! 99: the death flight: wrapped, or past key 37 with the blend done → speed 0, `SetDeathBits`, 0xf (mode | 8); below z
//! 10 → `SetDeathBits`, deleted; landed → a puff.
//! The tail (drawn, not in 8 / 9): the look-at (0.018, 0.3); within 28 of the camera: the shadow probe, +0x7f 0x16.
//!
//! Read from the level12 decomp. [L] Ratchet's anim speed (0x1413d0 +0x58 = 1.3 in state 5) is written to his table
//! moby, which the hero code drives; the awake counter's pointer +0x2e0 is never set (the game counts into address 0):
//! a shared unit word. Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, react, region, target, turn, walker};
use crate::moby_update::manip;
use crate::moby_update::scheduler::{group_cmd, group_count, group_walk, GroupWalk};
use crate::moby_update::services::World;
use std::f32::consts::{PI, TAU};

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2e_1be0;
pub const CLASSES: [i16; 1] = [238];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
/// The unit word standing in for address 0 (the awake count).
const COUNT_KEY: u32 = 0;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const LURE: usize = 0x38;
    pub const SUCK: usize = 0x60;
    pub const F: usize = 0x110;
    pub const K: usize = 0x120;
    pub const J: usize = 0x180;
    pub const R: usize = 0x1d0;
    pub const LOOK: usize = 0x200;
    pub const LOOK_YAW: usize = 0x268;
    pub const HOME: usize = 0x280;
    pub const TURN_V: usize = 0x2a0;
    pub const SIDE: usize = 0x2a4;
    pub const SIDE_DEG: usize = 0x2a8;
    pub const RANGE: usize = 0x2ac;
    pub const W2B8: usize = 0x2b8;
    pub const BURROWED: usize = 0x2bc;
    pub const SIDE_T: usize = 0x2c0;
    pub const ALERT: usize = 0x2c4;
    pub const EMERGE_T: usize = 0x2c8;
    pub const WALK: usize = 0x2cc;
    pub const AREA: usize = 0x2d0;
    pub const STUCK: usize = 0x2d4;
    pub const HOME_T: usize = 0x2d8;
    pub const LIMIT: usize = 0x2dc;
    pub const STAMP: usize = 0x2e4;
    pub const CULL: usize = 0x2e8;
    pub const CULL_T: usize = 0x2ea;
    pub const SIZE: usize = 0x2f0;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
/// `if (m+0x53 != seq) fun_00212f90(m, seq, frame, t)` (`t` in ticks already).
fn blend(w: &mut World, id: MobyId, seq: u8, frame: i32, t: i32) { c::blend_to(w, id, seq, frame, t); }
fn blend_n(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let t = w.ticks(n);
    blend(w, id, seq, 0, t);
}
fn group(w: &World, id: MobyId) -> i8 { w.m(id).group }
fn tell(w: &mut World, id: MobyId, cmd: u8) {
    let g = group(w, id);
    if g != -1 { group_cmd(w, g, cmd); }
}
fn heading_to(p: c::V, t: c::V) -> f32 { c::atan(t[0] - p[0], t[1] - p[1]) }
fn area(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::AREA)).ok() }

/// The target moby (none: Ratchet's) and its position.
fn target_of(w: &World, t: &target::Target) -> (Option<MobyId>, c::V) {
    let m = t.moby.or(w.hero_moby);
    (m, m.map_or_else(|| super::hero_pos(w), |m| w.m(m).position))
}

/// The sidestep's start: the side flipped one time in two, its angle, `ticks(trunc(randf(30, 90)))`.
fn sidestep(w: &mut World, id: MobyId) {
    if w.rng.randi(0x100) & 1 != 0 {
        let d = -c::pf(w, id, pv::SIDE_DEG);
        c::set_pf(w, id, pv::SIDE_DEG, d);
    }
    let a = c::pf(w, id, pv::SIDE_DEG) * 0.017_453_292;
    c::set_pf(w, id, pv::SIDE, a);
    reroll_side(w, id);
}

fn reroll_side(w: &mut World, id: MobyId) {
    let f = w.rng.randf(30.0, 90.0);
    let t = w.ticks(f as i32);
    c::set_pi32(w, id, pv::SIDE_T, t);
}

/// `0x2e18e8`: the run sequence.
fn run_anim(w: &mut World, id: MobyId) {
    let s = match w.rng.randi(2) {
        0 => 3,
        1 => 0xf,
        _ => 0x10,
    };
    blend_n(w, id, s, 8);
}

/// State 4 with the run (`LAB_002e2ab0`).
fn to_chase(w: &mut World, id: MobyId) {
    set_state(w, id, 4);
    c::set_pi32(w, id, pv::STUCK, 0);
    run_anim(w, id);
}

/// `0x2e2eb8(h, m)`: the steer (module doc); the walker's result, or 2 when stuck.
fn steer(w: &mut World, id: MobyId, h: f32) -> u32 {
    let old = c::pos(w, id);
    let mut p = old;
    p[2] += 0.5;
    let g = c::ground::ground(w, p, 0.5, 0).z;
    let mut z = old[2];
    turn::approach(g, DT2 * 27.0, &mut z);
    w.mm(id).position[2] = z;
    turn::spring_turn2_pvar(w, id, h, 0.05, f32::from_bits(0x3e99_999a), f32::from_bits(0x3e4c_cccd), pv::TURN_V);
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    let mut r = walker::step(w, id, pv::J, 1.0, [cy + cy, sy + sy, 0.0, 0.0], &mut out);
    let Some(path) = area(w, id) else { return r };
    let now = c::pos(w, id);
    let back = c::set_len3(c::sub(old, now), 0.05);
    let q = c::add(old, back);
    let (crossed, at) = region::clamp(w, path, now, q);
    if crossed {
        if !region::point_in_polygon(w, path, now) { c::set_pos(w, id, at); }
        if c::dist3(c::pos(w, id), old) < DT * 0.5 {
            let n = c::pi32(w, id, pv::STUCK) + 1;
            c::set_pi32(w, id, pv::STUCK, n);
            if w.ticks(10) < n { r = 2; }
        } else {
            c::set_pi32(w, id, pv::STUCK, 0);
        }
    }
    r
}

/// `0x2e19a8(p)`: the emerge dust (module doc).
fn dust(w: &mut World, at: c::V) {
    for _ in 0..27 {
        let a = w.rng.rand_angle();
        let mut p = at;
        p[2] += w.rng.randf(-0.25, -0.75);
        p[0] += w.rng.randf(-0.25, 0.25);
        p[1] += w.rng.randf(-0.25, 0.25);
        let mut v1 = [0.0f32; 4];
        let mut v2 = [0.0f32; 4];
        v2[0] = a.cos() * w.rng.randf(1.0, 3.0) * DT;
        v2[1] = a.sin() * w.rng.randf(1.0, 3.0) * DT;
        v1[2] = w.rng.randf(7.0, 2.0) * DT;
        v2[2] = w.rng.randf(-0.1, -1.0) * DT;
        v1[3] = f32::from_bits(0x3e99_999a);
        v2[3] = f32::from_bits(0x3f0c_cccd);
        let t1 = w.rng.randf(1.0, 10.0) as i32;
        let t2 = w.rng.randf(15.0, 25.0) as i32;
        let t3 = w.rng.randf(40.0, 70.0) as i32;
        let s = crate::particles::type02::Spawn { pos: p, v1, v2, c1: 0xa0f0_e8e0, c2: 0x20f0_e8e0, t: [t1, t2, t3], def: -1 };
        fx::part02(w, &s);
    }
}

/// `0x2e3420(m)`: a type-52 puff within 0.8 (module doc).
fn puff(w: &mut World, id: MobyId) {
    let a = w.rng.rand_angle();
    let r = w.rng.randf(0.0, 0.8);
    let s1 = w.rng.randf(0.0, 1.5);
    let s2 = w.rng.randf(1.5, 5.7);
    let life = w.rng.rand_range(100, 180);
    let p = c::pos(w, id);
    let q = [a.cos() * r + p[0], a.sin() * r + p[1], p[2] + 0.025, p[3]];
    fx::part52(w, s1, s2, q, 0x3800_2028, 0x2020, life);
}

/// `0x2e3098(m)`: the death sparks.
fn sparks(w: &mut World, id: MobyId) {
    let dying = state(w, id) == 0xf;
    let n = if dying { (10.0f32 * 1.5) as i32 } else { 10 };
    fx::clump_sparks(w, id, pv::K, n, dying);
}

/// `0x2e3528`: the tick (module doc).
fn tick(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::LURE) != 0 {
        c::set_pi32(w, id, pv::LURE, 0);
        let t = w.ticks(0xf0);
        c::set_pi32(w, id, pv::ALERT, t);
    }
    let s = state(w, id);
    if s != 9 && s != 0xb && s != 0 {
        let r = c::dec_timer_pvar_i32(w, id, pv::ALERT);
        c::set_pf(w, id, pv::RANGE, if r == 0 { 20.0 } else { 12.0 });
    }
    if c::pi16(w, id, pv::CULL) != 0 && state(w, id) != 9 {
        let n = c::pi16(w, id, pv::CULL_T).wrapping_add(1);
        c::set_pi16(w, id, pv::CULL_T, n);
        let g = group(w, id);
        if w.ticks(0x3c) * 15 < n as i32 && w.m(id).visible == 0 && g != -1 && 3 < group_count(w, g as i32, -1) && w.rng.randi(0x45) == 0 {
            if let Some(hero) = w.hero_moby {
                let a = c::add_rot(c::yaw(w, id), PI);
                let p = c::pos(w, id);
                c::attack::hit_moby(w, id, hero, 1.0, 0x1_0000, p, [a.cos(), a.sin(), 0.0, 0.0]);
            }
        }
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if let Some(h) = res.hit.filter(|_| hit.is_some() && state(w, id) != 99 && state(w, id) != 8) {
        tell(w, id, 7);
        if res.damage != 0.0 {
            let kr = pv::K;
            let hp = c::pf(w, id, pv::D) - res.damage;
            c::set_pf(w, id, kr + knock::k::GRAVITY, f32::from_bits(0x3c03_126f));
            c::set_pf(w, id, kr + knock::k::DRAG, f32::from_bits(0x3a03_126f));
            c::set_pf(w, id, kr + knock::k::SPEED, 3.8 * DT);
            c::set_pf(w, id, kr + knock::k::UP, 7.5 * DT);
            c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
            c::set_pf(w, id, kr + 0x38, 1.0);
            c::set_pf(w, id, pv::D, hp);
            c::set_pu8(w, id, kr + 0x3d, 0);
            c::set_pf(w, id, kr + knock::k::BOUNCE, 1.0);
            c::set_pf(w, id, kr + 0x34, 1.0);
            let dir = h.dir.map(|x| f32::from_bits(x.0));
            if hp <= 0.0 {
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pf(w, id, kr + knock::k::SPEED, DT * 8.0);
                c::set_pf(w, id, kr + knock::k::UP, DT * 10.0);
                let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, kr + knock::k::SPEED, sp);
                c::set_pf(w, id, kr + knock::k::UP, up);
                knock::start(w, id, kr, a, 6, 1, 0);
                c::set_pf(w, id, kr + knock::k::KEY_APEX, 11.0);
                c::set_pf(w, id, kr + knock::k::KEY_LAND, 18.0);
                set_state(w, id, 99);
                c::set_pu8(w, id, pv::F + 7, 0x78);
            } else {
                let (mut sp, mut up) = (c::pf(w, id, kr + knock::k::SPEED), c::pf(w, id, kr + knock::k::UP));
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, kr + knock::k::SPEED, sp);
                c::set_pf(w, id, kr + knock::k::UP, up);
                knock::start(w, id, kr, a, 0xe, 1, 0);
                c::set_pf(w, id, kr + knock::k::KEY_LAND, -1.0);
                c::set_pf(w, id, kr + knock::k::KEY_APEX, -1.0);
                set_state(w, id, 7);
                c::set_pu8(w, id, pv::F + 7, 0xfa);
                let t = w.ticks(0x3c);
                c::set_pi16(w, id, pv::COOLDOWN, t as i16);
            }
            flash::start(w, id, pv::F);
            sparks(w, id);
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::F);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let p = c::pos(w, id);
    if c::pi32(w, id, pv::BURROWED) == 0 {
        if c::pf(w, id, pv::HOME + 0xc) == 0.0 {
            c::set_pv4(w, id, pv::HOME, p);
            set_state(w, id, 1);
            blend(w, id, 0, 0, 3);
        } else {
            set_state(w, id, 0xb);
            let m = w.mm(id);
            m.draw_dist = 0;
            m.mode &= !mode::TARGETABLE;
            m.has_collision = false;
        }
    } else {
        c::set_pv4(w, id, pv::HOME, p);
        set_state(w, id, 9);
        blend(w, id, 0xc, 0, 3);
        let m = w.mm(id);
        m.has_collision = false;
        m.mode = (m.mode & !mode::TARGETABLE) | 0x41;
    }
    c::set_pi32(w, id, pv::ALERT, 0);
    c::set_pi32(w, id, pv::W2B8, 0);
    c::set_pf(w, id, pv::K + knock::k::ZOFF, 0.5);
    c::set_pf(w, id, pv::D, 2.0);
    c::set_pu8(w, id, 0x5a, 5);
    c::set_pi32(w, id, pv::K + knock::k::RADIUS, 0x200);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, pv::D + 9, 1);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pi32(w, id, pv::J, 0x38d);
    let fl = c::pi32(w, id, pv::J + 0x38) | 0x10;
    c::set_pi32(w, id, pv::J + 0x38, fl);
    c::set_pf(w, id, pv::J + 0x24, DT * 5.5);
    c::set_pf(w, id, pv::J + 0xc, 0.5);
    c::set_pf(w, id, pv::J + 8, 0.5);
    if c::pi32(w, id, pv::LIMIT) != 0 {
        let (a, b) = (w.ticks(0x14), w.ticks(0xaa));
        let t = w.rng.rand_range(a, b);
        c::set_pi16(w, id, pv::EMERGE_T, t as i16);
    }
    c::set_pv4(w, id, pv::R, p);
    let (a, b) = (w.ticks(0x78), w.ticks(700));
    let t = w.rng.rand_range(a, b);
    c::set_pi16(w, id, pv::WALK, t as i16);
    let mut q = c::pos(w, id);
    q[2] += 0.8;
    let z = c::ground::ground(w, q, 0.5, 0).z;
    w.mm(id).position[2] = z;
    if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
}

/// The emerge (`LAB_002e22c0`).
fn emerge(w: &mut World, id: MobyId) {
    set_state(w, id, 0xd);
    blend(w, id, 7, 0, 1);
    w.mm(id).mode |= mode::TARGETABLE;
    let p = c::pos(w, id);
    dust(w, p);
}

/// State 9 (module doc). True: stay burrowed this tick.
fn burrowed(w: &mut World, id: MobyId, tpos: c::V) {
    if c::pi32(w, id, pv::LIMIT) != 0 {
        if c::dist2(c::pos(w, id), tpos) < 2.0 { return; }
        let now = w.counter as i32;
        if c::pi32(w, id, pv::STAMP) < now {
            c::set_pi32(w, id, pv::STAMP, now);
            let g = group(w, id) as u8 as i32;
            let n = group_walk(w, g, GroupWalk::of(false, false)).into_iter().filter(|&m| (w.m(m).state as i8) >= 0 && w.m(m).state != 9).count();
            w.svc.units.set_word(COUNT_KEY, n as u32);
        }
        if 5 < w.svc.units.word(COUNT_KEY) as i32 { return; }
    }
    let p = c::pos(w, id);
    let near = c::len3(c::sub(tpos, p)) < c::pf(w, id, pv::RANGE);
    if near && (p[2] - tpos[2]).abs() < 8.0 && w.rng.randi(0x13) == 0 {
        tell(w, id, 1);
        emerge(w, id);
        return;
    }
    let cmd = w.m(id).cmd;
    if cmd & 1 != 0 && c::dec_timer_pvar_s16(w, id, pv::EMERGE_T) != 0 {
        emerge(w, id);
        return;
    }
    if cmd == 7 { emerge(w, id); }
}

/// State 1 (module doc).
fn idle(w: &mut World, id: MobyId, tpos: c::V) {
    let home = c::pv4(w, id, pv::HOME);
    let p = c::pos(w, id);
    let d = c::dist3(tpos, home);
    let off = c::diff_rots(c::yaw(w, id), heading_to(p, tpos));
    if d < c::pf(w, id, pv::RANGE) && (p[2] - tpos[2]).abs() < 4.0 {
        set_state(w, id, 3);
        if 0.959_931_1 <= off {
            blend_n(w, id, 0x11, 5);
        } else {
            blend_n(w, id, 2, 5);
            w.mm(id).anim.speed = f32::from_bits(0x3fc6_6666);
        }
        return;
    }
    let cmd = w.m(id).cmd;
    let start = if cmd & 1 == 0 { cmd == 7 } else { w.rng.randi(0x13) == 0 || cmd == 7 };
    if !start {
        blend_n(w, id, 0xf, 0xe);
        let walk = w.ticks(0x14) < c::pi16(w, id, pv::WALK) as i32;
        let mut s = c::pf(w, id, pv::R + walker::wr::STEP);
        turn::approach(if walk { c::DT * 2.5 } else { 0.0 }, DT2 * 7.0, &mut s);
        c::set_pf(w, id, pv::R + walker::wr::STEP, s);
        w.mm(id).anim.speed = 0.8;
        walker::wander(w, id, f32::from_bits(0x3eb3_3333), 0.25, pv::R);
        return;
    }
    sidestep(w, id);
    set_state(w, id, 3);
    blend_n(w, id, 2, 7);
    w.mm(id).cmd = 0;
}

/// State 4 (module doc).
fn chase(w: &mut World, id: MobyId, tpos: c::V) {
    let p = c::pos(w, id);
    let a = heading_to(p, tpos);
    let d = c::sub_rot(a, c::yaw(w, id)).clamp(-0.698_131_7, 0.698_131_7);
    c::set_pf(w, id, pv::LOOK_YAW, d);
    let h = c::add_rot(a, c::pf(w, id, pv::SIDE));
    if steer(w, id, h) == 2 {
        go_home(w, id);
        return;
    }
    if c::dec_timer_pvar_i32(w, id, pv::SIDE_T) != 0 {
        reroll_side(w, id);
        let s = -c::pf(w, id, pv::SIDE);
        c::set_pf(w, id, pv::SIDE, s);
    }
    let v = c::sub(tpos, c::pv4(w, id, pv::HOME));
    let p = c::pos(w, id);
    if c::dist2(p, tpos) < 1.5 {
        set_state(w, id, 5);
        if w.m(id).anim.seq_b != 4 {
            let t = w.ticks(5);
            w.anim_blend(id, 4, 0, t);
        }
        w.mm(id).cmd = 0;
        return;
    }
    if c::len3(v) < c::pf(w, id, pv::RANGE) + 5.5 && (p[2] - tpos[2]).abs() < 4.4 {
        tell(w, id, 1);
        w.mm(id).cmd = 0;
        return;
    }
    if w.m(id).cmd != 1 && w.rng.randi(9) == 0 {
        go_home(w, id);
        return;
    }
    w.mm(id).cmd = 0;
}

/// To 6 (`LAB_002e28dc`).
fn go_home(w: &mut World, id: MobyId) {
    set_state(w, id, 6);
    c::set_pi32(w, id, pv::HOME_T, 0);
    w.mm(id).cmd = 0;
}

/// State 5 (module doc).
fn bite(w: &mut World, id: MobyId, tm: Option<MobyId>, tpos: c::V) {
    if let Some(h) = w.hero_moby { w.mm(h).anim.speed = f32::from_bits(0x3fa6_6666); }
    let p = c::pos(w, id);
    turn::spring_turn2_pvar(w, id, heading_to(p, tpos), 0.05, f32::from_bits(0x3e99_999a), f32::from_bits(0x3e4c_cccd), pv::TURN_V);
    let m = w.m(id);
    if m.anim.seq_a == m.anim.seq_b && c::ground::passed_frame(w, id, 13.0) {
        let p = c::pos(w, id);
        if (p[2] - tpos[2]).abs() < 0.5 && c::dist2(p, tpos) < 2.0 && c::diff_rots(heading_to(p, tpos), c::yaw(w, id)) < 0.261_799_4 {
            if let Some(t) = tm {
                let (cy, sy) = c::cs(c::yaw(w, id));
                let at = [tpos[0], tpos[1], tpos[2] + 0.75, tpos[3]];
                c::attack::hit_moby(w, t, id, 1.0, 1, at, [cy * 0.2, sy * 0.2, 0.0, 0.0]);
            }
        }
    }
    if done(w, id) && 2.0 <= c::dist2(c::pos(w, id), tpos) { to_chase(w, id); }
}

/// State 6 (module doc).
fn home(w: &mut World, id: MobyId, tpos: c::V) {
    let home = c::pv4(w, id, pv::HOME);
    steer(w, id, heading_to(c::pos(w, id), home));
    if c::dist2(c::pos(w, id), home) < 2.0 {
        set_state(w, id, 1);
        blend_n(w, id, 0, 7);
    }
    let d = c::dist3(tpos, home);
    let n = c::pi32(w, id, pv::HOME_T) + 1;
    c::set_pi32(w, id, pv::HOME_T, n);
    let hero = super::hero_pos(w);
    if area(w, id).is_some_and(|a| !region::point_in_polygon(w, a, hero)) {
        w.mm(id).cmd = 0;
        return;
    }
    if d < c::pf(w, id, pv::RANGE) {
        if w.ticks(0x46) < n {
            tell(w, id, 1);
            to_chase(w, id);
        }
        w.mm(id).cmd = 0;
        return;
    }
    if w.m(id).cmd == 1 { to_chase(w, id); }
    w.mm(id).cmd = 0;
}

/// State 0xb (module doc).
fn jump_out(w: &mut World, id: MobyId, tpos: c::V) {
    let home = c::pv4(w, id, pv::HOME);
    turn::spring_turn2_pvar(w, id, heading_to(c::pos(w, id), home), 0.05, f32::from_bits(0x3e99_999a), f32::from_bits(0x3e4c_cccd), pv::TURN_V);
    let near = c::len3(c::sub(tpos, home)) < c::pf(w, id, pv::RANGE) && (home[2] - tpos[2]).abs() < 8.0;
    if !near && w.m(id).cmd != 2 { return; }
    tell(w, id, 2);
    let kr = pv::K;
    w.mm(id).draw_dist = 0x40;
    c::set_pf(w, id, kr + knock::k::DRAG, 0.0);
    c::set_pf(w, id, kr + knock::k::GRAVITY, 28.0 * DT2);
    c::set_pf(w, id, kr + knock::k::SPEED, 7.0 * DT);
    let mut t = 0.0;
    let up = knock::lob_up(7.0 * DT, -28.0, c::pos(w, id), home, &mut t);
    c::set_pf(w, id, kr + knock::k::UP, up);
    c::set_pi32(w, id, kr + knock::k::FLAGS, 5);
    c::set_pu8(w, id, kr + 0x3d, 0);
    let yaw = c::yaw(w, id);
    knock::start(w, id, kr, yaw, 8, 1, 0);
    c::set_pf(w, id, kr + knock::k::KEY_APEX, 5.0);
    c::set_pf(w, id, kr + knock::k::KEY_LAND, 10.0);
    set_state(w, id, 0xc);
    let coll = super::class_collision(w, w.m(id).o_class);
    let m = w.mm(id);
    m.mode |= mode::TARGETABLE;
    m.has_collision = coll;
}

/// The tail (`LAB_002e2df8`).
fn tail(w: &mut World, id: MobyId) {
    let s = state(w, id);
    if w.m(id).visible == 0 || s == 9 || s == 8 { return; }
    manip::look(w, id, id, pv::LOOK, 0, 0.018, 0.3);
    if w.m(id).visible == 0 { return; }
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if c::dist3(c::pos(w, id), cam) < 28.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x16;
    }
}

/// Level12 `0x2e1be0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    tick(w, id);
    manip::big_head_scale(w, 2.2, id, pv::LOOK);
    let range = c::pf(w, id, pv::RANGE);
    let t = target::acquire(w, id, range);
    let (tm, tpos) = target_of(w, &t);
    if c::pos(w, id)[2] < 26.0 {
        w.delete_moby(id);
        return;
    }
    match state(w, id) {
        0 => init(w, id),
        1 => idle(w, id, tpos),
        3 => {
            let h = heading_to(c::pos(w, id), tpos);
            turn::spring_turn2_pvar(w, id, h, f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3e99_999a), DT * TAU, pv::TURN_V);
            if done(w, id) { to_chase(w, id); }
        }
        4 => chase(w, id, tpos),
        5 => bite(w, id, tm, tpos),
        6 => home(w, id, tpos),
        7 => {
            tell(w, id, 1);
            if knock::update(w, id, pv::K) & knock::res::LANDED != 0 {
                set_state(w, id, 10);
                if w.m(id).anim.seq_b != 0xd {
                    let t = w.ticks(8);
                    w.anim_blend(id, 0xd, 8, t);
                }
                return;
            }
            if c::pos(w, id)[2] < 27.0 {
                w.delete_moby(id);
                return;
            }
        }
        8 => {
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, 6);
                c::set_pi32(w, id, pv::HOME_T, 0);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        9 => burrowed(w, id, tpos),
        10 => {
            if 29.0 < c::ground::key_time(w, id) {
                to_chase(w, id);
                return;
            }
        }
        0xb => jump_out(w, id, tpos),
        0xc => {
            if knock::update(w, id, pv::K) & knock::res::LANDED != 0 {
                set_state(w, id, 0xe);
                if w.m(id).anim.seq_b != 9 {
                    let t = w.ticks(7);
                    w.anim_blend(id, 9, 8, t);
                }
                return;
            }
            if c::pos(w, id)[2] < 0.0 {
                w.delete_moby(id);
                return;
            }
        }
        0xd => {
            let coll = super::class_collision(w, w.m(id).o_class);
            let m = w.mm(id);
            m.has_collision = coll;
            m.mode &= !0x41;
            let h = heading_to(c::pos(w, id), tpos);
            turn::spring_turn2_pvar(w, id, h, f32::from_bits(0x3cf5_c28f), f32::from_bits(0x3e99_999a), DT * TAU, pv::TURN_V);
            if done(w, id) {
                sidestep(w, id);
                to_chase(w, id);
                w.mm(id).cmd = 0;
            }
        }
        0xe => {
            if done(w, id) {
                set_state(w, id, 4);
                c::set_pi32(w, id, pv::STUCK, 0);
                sidestep(w, id);
                run_anim(w, id);
            }
        }
        0xf => {
            puff(w, id);
            sparks(w, id);
            sparks(w, id);
            w.delete_moby(id);
            return;
        }
        99 => {
            let r = knock::update(w, id, pv::K);
            let wrapped = r & knock::res::ANIM_WRAPPED != 0;
            let dead = if wrapped || (37.0 < c::ground::key_time(w, id) && w.m(id).anim.seq_a == w.m(id).anim.seq_b) {
                true
            } else {
                if c::pos(w, id)[2] < 10.0 {
                    set_death_bits(w, id, 0, -1);
                    w.delete_moby(id);
                    return;
                }
                if r & knock::res::LANDED != 0 { puff(w, id); }
                false
            };
            if dead {
                w.mm(id).anim.speed = 0.0;
                set_death_bits(w, id, 0, -1);
                set_state(w, id, 0xf);
                w.mm(id).mode |= 8;
            }
        }
        _ => {}
    }
    tail(w, id);
}
