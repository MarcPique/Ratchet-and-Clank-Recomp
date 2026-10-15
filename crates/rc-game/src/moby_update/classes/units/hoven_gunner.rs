//! **Hoven's gunners, class 294** (level12 `0x2e72c0`, its tick `0x2e81e0`; census U427; 40 placed, in moby groups).
//! Soldiers with a gun (class 499 on joint 1, `eudora_gunner::hold_gun_at`) that wait in ambush, step out (along their
//! path +0x254 when they have one) once their target comes within range or into their trigger cuboid +0x25c, then
//! close in sidestepping (+0x244 degrees off the line, alternating per group through the shared word +0x250) until
//! within 5.5 and facing it, aim (seq 2), fire one gun shot (`gun_shot`, seq 10) and recover (seq 5); a hit of theirs
//! knocks them back, two kill them (a flight and a burst). Their heads and bodies turn toward the target (two look-at
//! records, lists 5 and 6). Some stay hidden until their cuboid +0x270 holds the camera, are deleted when Ratchet
//! enters +0x290, or (+0x268 = 1) smash the crates in front of them.
//!
//! **Pvars** (0x2a0): +0x20 the damage record (health 2, meter 2, +0x2e 1 while alive: 2 = morphed), +0x38 the lure,
//! +0x60 the flash, +0x70 the knockback record, +0xd0 the walker, +0x120 / +0x1a0 the look-at records (+0x188 / +0x208
//! their yaw targets), +0x220 home, +0x230 the range (+0x28c its base), +0x23c the turn velocity, +0x240 s16 the alert,
//! +0x242 stuck (+0x243 its count), +0x244 the sidestep, +0x248 the gun, +0x24c / +0x250 the group's shared words
//! (the mirror toggle, the sidestep), +0x254 the path, +0x258 the path delay, +0x25c the trigger cuboid, +0x260 the
//! node, +0x264 the stop distance, +0x268 crate breaker / hidden at start, +0x269 gone after Ratchet's respawn,
//! +0x26c the area path, +0x270 the camera cuboid, +0x274 the cull, +0x278..+0x284 the groups it counts, +0x288 the
//! cull ticks, +0x290 the leave cuboid.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2e72c0` | the update: hidden in a scene (game mode 2); shown from state 3 on; with +0x269, Ratchet just respawned (state 0 after 0x67) within 30 → gone; the camera outside +0x270 → hidden (and the gun); morphed → gone; the tick; the target `0x274b78` (none: Ratchet's moby, the aim's z and w from his body point) | [`update`] |
//! | `0x2e81e0` | the tick: the lure → alert `ticks(240)`; the range 20 / the base; the cull (+0x274: after `17·ticks(60)` ticks, 9 or more of the four groups' members not in state 2, one tick in 0x27: a hit of 1 from Ratchet on itself); Ratchet outside +0x290: a hit (0x330000; not by its own class or a gun shot 0xb8, not in 9) through the resolver (column 4): the group told 1, K (gravity 30·dt², flags 9, bounce 1, 1, +0x38 1), health − damage; alive: 4.7·dt / 7.7·dt, seq 6, keys 4 / 8, state 4, flash 0xfa, cooldown `ticks(60)`; dead: untargetable, 12·dt / 9·dt, seq 9, keys 10 / 19, state 9, flash 0x78; inside +0x290: gone | [`tick`] |
//! | `0x2e8808` | the move toward a point: stuck → only turn (`0x270cc0`: 15.7·dt², 12.6·dt², 25.1·dt); else turn toward it (+ the sidestep beyond 6.5), the speed toward the given one (0 → 7·dt, or dt when facing away 90° or within 6) at 7·dt², the walker step 100 ahead; with an area: clamped to it, stuck after `ticks(15)` ticks moving less than 0.5·dt | [`step`] |
//! | `0x2e8608` | the aim: the look yaws 0.35 / 0.8 of the turn to the target (±70°) | [`aim_look`] |
//! | `0x2e87b0` | the ground follow: z toward `GroundHeight(0.5, pos)` at 27·dt² | [`ground`] |
//! | `0x2e8720` | the gun (`0x2f1118` creates 499 at joint 1; `0x27b9c0` carries it) | `eudora_gunner::hold_gun_at` |
//! | `0x2e86d0` | gone: +0xb4 = 0, `SetDeathBits`, the gun and itself deleted | [`gone`] |
//! | `0x2e7290` | the burst `0x2782d8(1, 10, m, pos)` | `fx::piece_explosion` |
//!
//! **States.** 0: health 2, meter 2, the walker (radius 0x38d, top speed 5.5·dt, flags 0x18, ledge 1 / 1), home;
//! alone: mirrored half the time; in a group: mirrored when its toggle is 1, the toggle flipped; the cull's start
//! `ticks(rand_range(0, 300))`; a zero sidestep: alone `randf_sym(15, 35)`, in a group the shared word (then 20 more
//! below 1, else negated); on the ground; +0x268 → hidden; → 2.
//! 1: after the delay, along the path (seq 8): the next node within 3, the move at 8·dt; the last node or within the
//! stop distance of the target → 3.
//! 2: beyond 64 of Ratchet: nothing. Seq 0; the target within the range and 3 in z (or in the trigger cuboid, or a
//! command): hidden ones seen by the camera (a clear line from 0.5 up) are deleted, else shown; the group told 1; the
//! path (from its nearest node) → 1, else → 3.
//! 3: crate breakers: a hit of 1 on each crate within 1.5 a unit ahead (within 0.7 in z); the look yaws half the turn
//! (±55°); the move with the sidestep; within 5.5 (or stuck) and facing within 5° → 5; else seq 8.
//! 4: shown; the flight: on the ground: within 6 (or stuck) and facing within 7° → 5, else 3; below z 2 → gone.
//! 5: the base range at least 8; aim, ground; seq 2; the anim over → seq 10, 6.
//! 6: aim, ground; frame 0 passed: from the gun's muzzle (joint 0) toward the aim point (within 13° of its look yaw,
//! else along it) a gun shot at 15·dt (life 18 / (15·dt)), sound 0; the anim over → seq 5, 7.
//! 7: aim, ground; seq 0 when free; one tick in 9 → 5; beyond 10.5 (not stuck) or facing 28° off → 3.
//! 8 (entered by no code): back home, its look yaws from the turn velocity; within 1 → 2.
//! 9: the death flight; landed → `SetDeathBits`, the burst, gone; below z 2 → `SetDeathBits`, gone.
//! The tail (it or its gun drawn): the gun; the look yaws negated when mirrored; the look-ats (0.018, 0.3 on list 5;
//! 0.024, 0.3 on list 6).
//!
//! Read from the level12 decomp. The blends here are `MobyAnimBlendEx(…, 5)` (a forced snapshot, `World::anim_blend_ex`).
//! [L] The game runs the rest of the update on a moby its tick just deleted (inside +0x290): the
//! port stops there. Native `f32`.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::{is_crate, set_death_bits};
use crate::moby_update::classes::units::{eudora_gunner, gun_shot};
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, region, target, turn, walker};
use crate::moby_update::manip;
use crate::moby_update::scheduler::{group_cmd, group_count};
use crate::moby_update::services::{pv, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2e_72c0;
pub const CLASSES: [i16; 1] = [294];

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;
/// The gun shot's class (0xb8): its hits do not count.
const SHOT: i16 = 0xb8;

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const COOLDOWN: usize = 0x26;
    pub const KEEP: usize = 0x2e;
    pub const LURE: usize = 0x38;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const LOOK_A: usize = 0x120;
    pub const YAW_A: usize = 0x188;
    pub const LOOK_B: usize = 0x1a0;
    pub const YAW_B: usize = 0x208;
    pub const HOME: usize = 0x220;
    pub const RANGE: usize = 0x230;
    pub const TURN_V: usize = 0x23c;
    pub const ALERT: usize = 0x240;
    pub const STUCK: usize = 0x242;
    pub const STUCK_N: usize = 0x243;
    pub const SIDE: usize = 0x244;
    pub const GUN: usize = 0x248;
    pub const MIRROR_WORD: usize = 0x24c;
    pub const SIDE_WORD: usize = 0x250;
    pub const PATH: usize = 0x254;
    pub const DELAY: usize = 0x258;
    pub const TRIGGER: usize = 0x25c;
    pub const NODE: usize = 0x260;
    pub const STOP: usize = 0x264;
    pub const HIDDEN: usize = 0x268;
    pub const RESPAWN: usize = 0x269;
    pub const AREA: usize = 0x26c;
    pub const CAMERA: usize = 0x270;
    pub const CULL: usize = 0x274;
    pub const GROUPS: usize = 0x278;
    pub const CULL_T: usize = 0x288;
    pub const RANGE_BASE: usize = 0x28c;
    pub const LEAVE: usize = 0x290;
    pub const SIZE: usize = 0x2a0;
}

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn done(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn heading_to(p: c::V, t: c::V) -> f32 { c::atan(t[0] - p[0], t[1] - p[1]) }
fn gun(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::GUN) - 1).ok().filter(|&g| g < w.table.mobys.len()) }
fn stuck(w: &World, id: MobyId) -> bool { c::pu8(w, id, pv::STUCK) != 0 }
fn path(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o)).ok().filter(|&p| p < w.svc.splines.len()) }
fn node(w: &World, p: usize, i: i32) -> c::V { usize::try_from(i).ok().and_then(|i| w.svc.splines[p].get(i)).map_or([0.0; 4], |q| q.map(f32::from_bits)) }

/// `MobyAnimBlendEx(m, seq, 0, ticks(n), 5)` when not already on it and no blend is running (module doc [L]).
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let a = w.m(id).anim;
    if a.seq_b != seq && a.seq_a == a.seq_b {
        let t = w.ticks(n);
        w.anim_blend_ex(id, seq, 0, t, 5);
    }
}

/// Hidden (mode | 0x41) or shown, with the gun.
fn show(w: &mut World, id: MobyId, on: bool) {
    for m in std::iter::once(id).chain(gun(w, id)) {
        let mm = w.mm(m);
        if on { mm.mode &= !0x41; } else { mm.mode |= 0x41; }
    }
}

/// `0x2e86d0`: gone (module doc).
fn gone(w: &mut World, id: MobyId) {
    w.mm(id).b4 = 0;
    set_death_bits(w, id, 0, -1);
    if let Some(g) = gun(w, id) { w.delete_moby(g); }
    w.delete_moby(id);
}

/// `0x2e81e0`: the tick (module doc). False: deleted.
fn tick(w: &mut World, id: MobyId) -> bool {
    if c::pi32(w, id, pv::LURE) != 0 {
        c::set_pi32(w, id, pv::LURE, 0);
        let t = w.ticks(0xf0);
        c::set_pi16(w, id, pv::ALERT, t as i16);
    }
    if state(w, id) != 0 {
        let r = c::dec_timer_pvar_s16(w, id, pv::ALERT);
        let range = if r != 0 { c::pf(w, id, pv::RANGE_BASE) } else { 20.0 };
        c::set_pf(w, id, pv::RANGE, range);
    }
    if c::pi32(w, id, pv::CULL) != 0 && state(w, id) != 2 {
        let n = c::pi32(w, id, pv::CULL_T) + 1;
        c::set_pi32(w, id, pv::CULL_T, n);
        if w.ticks(0x3c) * 0x11 < n {
            let count: i32 = (0..4).map(|k| c::pi32(w, id, pv::GROUPS + 4 * k)).filter(|&g| g != -1).map(|g| group_count(w, g, 2)).sum();
            if 9 <= count && w.rng.randi(0x27) == 0 {
                if let Some(hero) = w.hero_moby {
                    let a = c::add_rot(c::yaw(w, id), std::f32::consts::PI);
                    let p = c::pos(w, id);
                    c::attack::hit_moby(w, id, hero, 1.0, 0x1_0000, p, [a.cos(), a.sin(), 0.0, 0.0]);
                }
            }
        }
    }
    let leave = c::pi32(w, id, pv::LEAVE);
    let hero = super::hero_pos(w);
    if leave != -1 && w.in_cuboid([hero[0], hero[1], hero[2]], leave) {
        gone(w, id);
        return false;
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if let Some(h) = res.hit.filter(|_| hit.is_some()) {
        let by = h.attacker.map_or(-1, |a| w.m(a).o_class);
        if by != w.m(id).o_class && by != SHOT && state(w, id) != 9 && res.damage != 0.0 {
            let g = w.m(id).group;
            if g != -1 { group_cmd(w, g, 1); }
            let kr = pv::K;
            let hp = c::pf(w, id, pv::D) - res.damage;
            c::set_pu8(w, id, kr + 0x3d, 0);
            c::set_pf(w, id, kr + 0x38, 1.0);
            c::set_pf(w, id, pv::D, hp);
            c::set_pf(w, id, kr + knock::k::GRAVITY, DT2 * 30.0);
            c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
            c::set_pf(w, id, kr + knock::k::BOUNCE, 1.0);
            c::set_pf(w, id, kr + 0x34, 1.0);
            let dir = h.dir.map(|x| f32::from_bits(x.0));
            let (speed, up, seq, keys, next, fl) = if 0.0 < hp { (DT * 4.7, DT * 7.7, 6, (4.0, 8.0), 4, 0xfa) } else { (DT * 12.0, DT * 9.0, 9, (10.0, 19.0), 9, 0x78) };
            if hp <= 0.0 { w.mm(id).mode &= !mode::TARGETABLE; }
            let (mut sp, mut u) = (speed, up);
            let a = knock::aim(dir, &mut sp, &mut u);
            c::set_pf(w, id, kr + knock::k::SPEED, sp);
            c::set_pf(w, id, kr + knock::k::UP, u);
            knock::start(w, id, kr, a, seq, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, keys.0);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, keys.1);
            set_state(w, id, next);
            c::set_pu8(w, id, pv::F + 7, fl);
            if 0.0 < hp {
                let t = w.ticks(0x3c);
                c::set_pi16(w, id, pv::COOLDOWN, t as i16);
            }
            flash::start(w, id, pv::F);
        }
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::F);
    true
}

/// `0x2e8608(m, at)`: the aim (module doc).
fn aim_look(w: &mut World, id: MobyId, at: c::V) {
    let d = c::sub_rot(heading_to(c::pos(w, id), at), c::yaw(w, id)).clamp(-1.221_730_5, 1.221_730_5);
    c::set_pf(w, id, pv::YAW_A, d * 0.35);
    c::set_pf(w, id, pv::YAW_B, d * 0.8);
}

/// `0x2e87b0(m)`: the ground follow (module doc).
fn ground(w: &mut World, id: MobyId) {
    let g = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    let mut z = c::pos(w, id)[2];
    turn::approach(g, DT2 * 27.0, &mut z);
    w.mm(id).position[2] = z;
}

/// `0x270cc0` on the yaw with this class's rates.
fn turn_to(w: &mut World, id: MobyId, h: f32) {
    turn::turn_toward_pvar(w, id, h, DT2 * 15.707_963, DT2 * 12.566_371, DT * 25.132_742, pv::TURN_V);
}

/// `0x2e8808(side, speed, m, at)`: the move (module doc).
fn step(w: &mut World, id: MobyId, side: f32, speed: f32, at: c::V) {
    let p = c::pos(w, id);
    if stuck(w, id) {
        turn_to(w, id, heading_to(p, at));
        return;
    }
    let d = c::dist2(p, at);
    let h = heading_to(p, at);
    turn_to(w, id, if d <= 6.5 || side == 0.0 { h } else { c::add_rot(h, side) });
    let mut sp = speed;
    if speed == 0.0 {
        let off = c::diff_rots(c::yaw(w, id), heading_to(c::pos(w, id), at));
        if std::f32::consts::FRAC_PI_2 <= off || d <= 6.0 { sp = DT; } else { sp = DT * 7.0; }
    }
    let mut j9 = c::pf(w, id, pv::J + 0x24);
    turn::approach(sp, DT2 * 7.0, &mut j9);
    c::set_pf(w, id, pv::J + 0x24, j9);
    let old = c::pos(w, id);
    let (cy, sy) = c::cs(c::yaw(w, id));
    let mut out = [0.0; 4];
    walker::step(w, id, pv::J, 1.0, [old[0] + cy * 100.0, old[1] + sy * 100.0, old[2], old[3]], &mut out);
    let Some(area) = path(w, id, pv::AREA) else { return };
    let now = c::pos(w, id);
    let q = c::add(old, c::set_len3(c::sub(old, now), 0.05));
    let (crossed, at) = region::clamp(w, area, now, q);
    if !crossed { return; }
    if !region::point_in_polygon(w, area, now) { c::set_pos(w, id, at); }
    if c::dist3(c::pos(w, id), old) < DT * 0.5 {
        let n = c::pu8(w, id, pv::STUCK_N).wrapping_add(1);
        c::set_pu8(w, id, pv::STUCK_N, n);
        if w.ticks(0xf) < n as i32 { c::set_pu8(w, id, pv::STUCK, 1); }
    } else {
        c::set_pu8(w, id, pv::STUCK_N, 0);
    }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    c::set_pf(w, id, pv::D, 2.0);
    c::set_pi16(w, id, pv::D + 4, 2);
    c::set_pu8(w, id, 0x5a, 8);
    let r = c::pf(w, id, pv::RANGE);
    c::set_pf(w, id, pv::RANGE_BASE, r);
    c::set_pi32(w, id, pv::K + knock::k::RADIUS, 0x200);
    c::set_pf(w, id, pv::K + knock::k::ZOFF, 0.5);
    c::set_pu8(w, id, 0x58, 8);
    c::set_pu8(w, id, pv::KEEP, 1);
    c::set_pu8(w, id, pv::D + 9, 1);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pi32(w, id, pv::J, 0x38d);
    c::set_pf(w, id, pv::J + 8, 1.0);
    c::set_pi32(w, id, pv::J + 0x38, 0x18);
    c::set_pf(w, id, pv::J + 0x24, DT * 5.5);
    c::set_pf(w, id, pv::J + 0xc, 1.0);
    let p = c::pos(w, id);
    c::set_pv4(w, id, pv::HOME, p);
    let g = w.m(id).group;
    if g == -1 {
        if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
    } else {
        let at = c::pi32(w, id, pv::MIRROR_WORD);
        let v = w.svc.shared_i32(at);
        if v == 1 { w.mm(id).mode |= mode::MIRROR; }
        w.svc.set_shared_i32(at, (v == 0) as i32);
    }
    if c::pi32(w, id, pv::CULL) != 0 {
        let k = w.rng.rand_range(0, 300);
        let t = w.ticks(k);
        c::set_pi32(w, id, pv::CULL_T, t);
    }
    w.mm(id).rotation[0] = 0.0;
    w.mm(id).rotation[1] = 0.0;
    let mut side = c::pf(w, id, pv::SIDE);
    if side == 0.0 {
        if g == -1 {
            side = w.rng.randf_sym(15.0, 35.0);
        } else {
            let at = c::pi32(w, id, pv::SIDE_WORD);
            let v = w.svc.shared_i32(at);
            side = v as f32;
            w.svc.set_shared_i32(at, if v < 1 { v + 0x14 } else { -v });
        }
    }
    c::set_pf(w, id, pv::SIDE, side * 0.017_453_292);
    let z = c::ground::ground(w, c::pos(w, id), 0.5, 0).z;
    w.mm(id).position[2] = z;
    if c::pu8(w, id, pv::HIDDEN) != 0 { show(w, id, false); }
    set_state(w, id, 2);
}

/// State 2 (module doc). False: deleted.
fn ambush(w: &mut World, id: MobyId, tpos: c::V) -> bool {
    let p = c::pos(w, id);
    blend(w, id, 0, 8);
    let range = c::pf(w, id, pv::RANGE);
    let seen = (tpos[2] - p[2]).abs() <= 3.0 && c::dist3(tpos, p) <= range;
    let trig = c::pi32(w, id, pv::TRIGGER);
    let go = seen || (trig != -1 && w.in_cuboid([tpos[0], tpos[1], tpos[2]], trig)) || w.m(id).cmd != 0;
    if !go { return true; }
    if w.m(id).mode & 1 != 0 {
        let a = [p[0], p[1], p[2] + 0.5, p[3]];
        let cam = w.camera;
        if w.coll_line(pv(a), cam, 2, Some(id)).is_none() {
            if let Some(g) = gun(w, id) { w.delete_moby(g); }
            w.delete_moby(id);
            return false;
        }
        show(w, id, true);
    }
    let g = w.m(id).group;
    if g != -1 { group_cmd(w, g, 1); }
    match path(w, id, pv::PATH) {
        Some(pp) => {
            let n = crate::path::nearest_at_distance(&w.svc.splines[pp], 0.0, c::pos(w, id));
            c::set_pi32(w, id, pv::NODE, n);
            set_state(w, id, 1);
        }
        None => set_state(w, id, 3),
    }
    true
}

/// State 1 (module doc).
fn patrol(w: &mut World, id: MobyId, tpos: c::V) {
    if c::dec_timer_pvar_i32(w, id, pv::DELAY) == 0 { return; }
    blend(w, id, 8, 0xb);
    let Some(pp) = path(w, id, pv::PATH) else { return };
    let n = w.svc.splines[pp].len() as i32;
    let mut i = c::pi32(w, id, pv::NODE);
    if c::dist2(c::pos(w, id), node(w, pp, i)) < 3.0 { i += 1; }
    if n - 1 < i { i = n - 1; }
    c::set_pi32(w, id, pv::NODE, i);
    let q = node(w, pp, i);
    step(w, id, 0.0, DT * 8.0, q);
    if i == n - 1 || c::dist2(c::pos(w, id), tpos) < c::pf(w, id, pv::STOP) { set_state(w, id, 3); }
}

/// State 3 (module doc).
fn close_in(w: &mut World, id: MobyId, tpos: c::V) {
    let p = c::pos(w, id);
    if c::pu8(w, id, pv::HIDDEN) == 1 {
        let (cy, sy) = c::cs(c::yaw(w, id));
        let at = [p[0] + cy, p[1] + sy, p[2], p[3]];
        let hero = w.hero_moby;
        for m in w.sphere_mobys_list(Pf::f(1.5), at.map(Pf::f), 0, Some(id), None) {
            if is_crate(w, Some(m)) && (p[2] - w.m(m).position[2]).abs() < 0.7 {
                if let Some(h) = hero { c::attack::hit_moby(w, m, h, 1.0, 0x1_0000, p, [0.0; 4]); }
            }
        }
    }
    let d = c::sub_rot(heading_to(p, tpos), c::yaw(w, id)).clamp(-0.959_931_1, 0.959_931_1);
    c::set_pf(w, id, pv::YAW_B, d * 0.5);
    c::set_pf(w, id, pv::YAW_A, d * 0.5);
    let side = c::pf(w, id, pv::SIDE);
    step(w, id, side, 0.0, tpos);
    let p = c::pos(w, id);
    if (c::dist2(p, tpos) < 5.5 || stuck(w, id)) && c::diff_rots(c::yaw(w, id), heading_to(p, tpos)) < 0.087_266_46 {
        set_state(w, id, 5);
        return;
    }
    blend(w, id, 8, 0xb);
}

/// State 6 (module doc).
fn fire(w: &mut World, id: MobyId, aim: c::V) {
    let Some(g) = gun(w, id) else { return };
    if !c::ground::passed_frame(w, id, 0.0) { return; }
    let p = w.joint_point(g, 0);
    let v = c::sub(aim, p);
    let mut a = c::atan(v[0], v[1]);
    let look = c::add_rot(c::yaw(w, id), c::pf(w, id, pv::YAW_B));
    if 0.226_892_8 < c::diff_rots(a, look) { a = look; }
    let e = c::atan(c::len2(v), v[2]);
    let vel = fx::polar(DT * 15.0, a, e);
    w.play_sound(0, 0, id);
    let life = (18.0 / (DT * 15.0)) as i32;
    gun_shot::spawn(w, id, p, vel, life);
}

/// Level12 `0x2e72c0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    manip::big_head_scale(w, 3.0, id, pv::LOOK_A);
    if w.svc.game_mode == 2 {
        show(w, id, false);
        return;
    }
    if 3 <= state(w, id) { show(w, id, true); }
    if c::pu8(w, id, pv::RESPAWN) != 0 && w.hero.state == 0 && w.hero.prev_state == 0x67 && c::dist2(c::pos(w, id), super::hero_pos(w)) < 30.0 {
        gone(w, id);
        return;
    }
    let cub = c::pi32(w, id, pv::CAMERA);
    if cub != -1 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if !w.in_cuboid([cam[0], cam[1], cam[2]], cub) {
            show(w, id, false);
            return;
        }
        show(w, id, true);
    }
    if c::pu8(w, id, pv::KEEP) == 2 {
        gone(w, id);
        return;
    }
    if !tick(w, id) { return; }
    let t = target::acquire(w, id, c::pf(w, id, pv::RANGE));
    let tm = t.moby.or(w.hero_moby);
    let tpos = tm.map_or_else(|| super::hero_pos(w), |m| w.m(m).position);
    let mut aim = t.aim;
    if t.moby.is_none() {
        let b = w.hero.body_point.map(|x| f32::from_bits(x.0));
        aim[2] = b[2];
        aim[3] = b[3];
    }
    match state(w, id) {
        0 => init(w, id),
        1 => patrol(w, id, tpos),
        2 => {
            if 64.0 < c::dist3(c::pos(w, id), super::hero_pos(w)) { return; }
            if !ambush(w, id, tpos) { return; }
        }
        3 => close_in(w, id, tpos),
        4 => {
            w.mm(id).mode &= !0x41;
            let r = knock::update(w, id, pv::K);
            if r & 0x60 == 0 {
                if c::pos(w, id)[2] < 2.0 {
                    gone(w, id);
                    return;
                }
            } else {
                let p = c::pos(w, id);
                let near = c::dist2(p, tpos) < 6.0 || stuck(w, id);
                let s = if near && c::diff_rots(c::yaw(w, id), heading_to(p, tpos)) < 0.122_173_05 { 5 } else { 3 };
                set_state(w, id, s);
            }
        }
        5 => {
            if c::pf(w, id, pv::RANGE_BASE) < 8.0 { c::set_pf(w, id, pv::RANGE_BASE, 8.0); }
            aim_look(w, id, tpos);
            ground(w, id);
            blend(w, id, 2, 8);
            if done(w, id) {
                blend(w, id, 10, 5);
                set_state(w, id, 6);
            }
        }
        6 => {
            aim_look(w, id, tpos);
            ground(w, id);
            fire(w, id, aim);
            if done(w, id) {
                blend(w, id, 5, 8);
                set_state(w, id, 7);
            }
        }
        7 => {
            aim_look(w, id, tpos);
            ground(w, id);
            let a = w.m(id).anim;
            if (a.seq_b != 5 || done(w, id)) && a.seq_b != 0 && a.seq_a == a.seq_b {
                let t = w.ticks(8);
                w.anim_blend(id, 0, 0, t);
            }
            if w.rng.randi(9) == 0 {
                set_state(w, id, 5);
            } else {
                let p = c::pos(w, id);
                if (10.5 < c::dist2(p, tpos) && !stuck(w, id)) || 0.488_692_2 < c::diff_rots(c::yaw(w, id), heading_to(p, tpos)) {
                    set_state(w, id, 3);
                }
            }
        }
        8 => {
            let home = c::pv4(w, id, pv::HOME);
            step(w, id, 0.0, 0.0, home);
            let f = (c::pf(w, id, pv::TURN_V) * 30.0).clamp(-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4);
            c::set_pf(w, id, pv::YAW_A, f * 0.7);
            c::set_pf(w, id, pv::YAW_B, f * 0.4);
            if c::dist2(c::pos(w, id), home) < 1.0 { set_state(w, id, 2); }
        }
        9 => {
            let r = knock::update(w, id, pv::K);
            if r & 0x60 != 0 {
                set_death_bits(w, id, 0, -1);
                let p = c::pos(w, id);
                fx::piece_explosion(w, 1.0, 10.0, Some(id), p);
                gone(w, id);
                return;
            }
            if c::pos(w, id)[2] < 2.0 {
                set_death_bits(w, id, 0, -1);
                gone(w, id);
                return;
            }
        }
        _ => {}
    }
    let gun_drawn = gun(w, id).is_some_and(|g| w.m(g).visible != 0);
    if w.m(id).visible == 0 && !gun_drawn { return; }
    eudora_gunner::hold_gun_at(w, id, pv::GUN);
    if w.m(id).mode & mode::MIRROR != 0 {
        let (a, b) = (c::pf(w, id, pv::YAW_A), c::pf(w, id, pv::YAW_B));
        c::set_pf(w, id, pv::YAW_A, -a);
        c::set_pf(w, id, pv::YAW_B, -b);
    }
    manip::look(w, id, id, pv::LOOK_A, 5, 0.018, 0.3);
    manip::look(w, id, id, pv::LOOK_B, 6, 0.024, 0.3);
}
