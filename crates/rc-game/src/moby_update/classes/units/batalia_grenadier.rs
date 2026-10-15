//! **Batalia's grenadiers** (class 333, level08 `0x2dabf0`, census U294, 8 placed) and **their grenades** (class 248,
//! `0x2d23f0`, made only by them: `0x2d22e0`). A grenadier lives among the other members of its moby group (its
//! "spots", whose +0xbc byte marks the one taken): it hides in its spot, pops up when Ratchet is 8..20 away and the
//! camera looks its way (or when the Taunter lures it), steps out, lobs a grenade at him and hides again in another
//! spot; knocked back by a hit, it hops away to a new spot; killed, it is thrown and bursts into smoke. Read from the
//! level08 decomp.
//!
//! **Pvars** (0x170): +0x00 the damage record (+0x20 health 2, +0x29 1), +0x38 the lure, +0x40 the walk's output, +0x58
//! / +0x5a 17 / 10, +0x60 the hit flash, +0x70 the knockback, +0xd0 the walker (J8 / J12 2, J36 6·dt, J60 0.03, J64
//! 0.3, J68 0.25), +0x120 the spot, +0x124 the last grenade, +0x128 the pop-up / throw timer, +0x12c the turn speed,
//! +0x130 / +0x132 the fade (dead code), +0x134 the spot timer, +0x160 the big-head record.
//!
//! **The spot choice** `0x2daa10`: over the group's list, skipping class 333 and taken spots: a spot less than 90°
//! off the line to Ratchet wins over the best so far when it is more off it than that one; a spot at least 90° off wins
//! when it is 16 or more away and the best so far is nearer than it or nearer than 16; one nearer than 16 wins only
//! when it is farther than the best and the best is at least 16 away (the game's comparisons, kept as they are);
//! none → keep the old spot.
//!
//! | address | what | port |
//! |---|---|---|
//! | prelude | the big-head cheat (`0x278720`: 3, joint 10); drawn within 29 of the camera → the shadow probe, +0x7f = 0x17; mode 0x400 off in state 3 | [`update`] |
//! | hits | `MobyGetHitMessage(0x330000)`, `0x26f378(…, +0x20, 0, 4)`; a hit (not 99): health less the damage; alive: under 1 damage the flash 100, else knocked (flags 9, speed 5·dt, up 7·dt, gravity 30·dt², drag 0.0005, away from Ratchet, sequence 6, keys 6 / 10), flash 0x78, → 0x5a; dead: knocked (8·dt, 10·dt, sequence 9), `SetDeathBits`, flash 0xfa, → 99, the spot freed; the flash | [`hits`] |
//! | 0 | the walker, health 2, the first spot (taken), timers, sequence 4, → 1 | [`update`] |
//! | 1 | walk to the spot; there (3-D within 3, facing it within 15°): Ratchet beyond 6 → sequence 5, → 6; else another spot; the spot timer (240) out → another spot | [`update`] |
//! | 6 | walk in; the wrap → the spot's sequence 1, into it (0.1 down), sequence 1, → 3, facing Ratchet, collision off, timer 120 | [`update`] |
//! | 3 | hidden, turning to Ratchet; the pop-up (above) → the spot's sequence 1, sequence 2, → 4, collision on | [`update`] |
//! | 4 | walk 2 out in front of the spot; the wrap: Ratchet 6 or more away and the camera on it → sequence 3, → 5; else another spot, sequence 4, → 1 | [`update`] |
//! | 5 | turning to him; at key 19 the throw (`0x2d22e0`: class 248 from joint 9, the lob landing 2 short of him, 3..10 away, at its distance / 90 a tick, gravity 15·dt²); the wrap → sequence 4, → 1 | [`throw`] |
//! | 0x5a / 0x5c / 0x5d | the flight; the wrap → another spot, sequence 4, → 1; below z 5 → deleted | [`update`] |
//! | 0x5b | the wrap → sequence 7, → 0x5d (no setter) | [`update`] |
//! | 99 | the flight; the wrap → the smoke burst (`0x2da8d8`: 14 segments of the body outline 0x1db100, `0x2da3f0`), deleted (state 100's fade never runs) | [`burst`] |
//! | tail | the flash, the lure cleared | [`update`] |
//!
//! **The grenade 248** (pvars: +0x00 the flash, +0x10 velocity, +0x20 timer, +0x24 bounces, +0x28, +0x2c 1, +0x30 the
//! thrower): a 0.5 sphere test (flags 0x11) each tick: a moby other than the thrower before its first bounce → it goes
//! off. Flying (1): spin by its speed, sparks (type 4) before the first bounce, gravity 15·dt², the line (0.16 below,
//! the hit template damage 1, flags 0x810000): a moby → off; the ground → bounce (sound 1, reflected, a random sideways
//! nudge `randf(±1.5·dt)`, ×0.4, the timer 180) and resting (2) when slower than dt (timer 120, flashing at 60 / 40 /
//! 20); the timer out → off. Off: sound 0, a 1.0 area hit (damage 1), then `SpawnBeamExplosion(0, 0, 3, 1.7, 4, 1, 7;
//! 7 streaks, 10 sparks, 20 puffs, debris −1, shake)` (0x62) or the death explosion `0x273f50(0.5, 13)` (99, the
//! thrower's grenades); deleted. The blob shadow (0.2).

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flash, fx, knock, target, turn, walker, DT, DT2, V};
use crate::moby_update::services::{pf, pv as v4, HitTemplate, World};
use crate::particles::type23;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 8;
pub const UPDATE_FN: u32 = 0x2d_abf0;
pub const CLASSES: [i16; 1] = [333];
pub const SHOT_FN: u32 = 0x2d_23f0;
pub const SHOT_CLASSES: [i16; 1] = [248];

const SELF: i16 = 333;
const SHOT: i16 = 248;

pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const OUT: usize = 0x40;
    pub const F: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const SPOT: usize = 0x120;
    pub const SHOT: usize = 0x124;
    pub const TIMER: usize = 0x128;
    pub const TURN_V: usize = 0x12c;
    pub const SPOT_T: usize = 0x134;
    pub const HEAD: usize = 0x160;
    pub const SIZE: usize = 0x170;
}

/// The body outline 0x1db100 (x, y, z, size): the burst's segment ends, in the moby's frame.
#[rustfmt::skip]
const OUTLINE: [[f32; 4]; 24] = [
    [0.0, 0.0, 0.187, 0.172],
    [-0.545, 0.0, 0.3, 0.2],
    [-0.583, 0.0, 0.599, 0.115],
    [-0.865, 0.0, 0.295, 0.115],
    [0.0, -0.095, 0.176, 0.099],
    [0.207, -0.144, 0.175, 0.117],
    [0.524, -0.211, 0.193, 0.19],
    [0.628, -0.236, 0.412, 0.117],
    [0.0, 0.095, 0.176, 0.099],
    [0.207, 0.122, 0.175, 0.117],
    [0.524, 0.174, 0.193, 0.19],
    [0.588, 0.195, 0.452, 0.117],
    [-0.525, -0.233, 0.308, 0.128],
    [0.0, -0.664, 0.062, 0.21],
    [-0.525, 0.256, 0.308, 0.128],
    [0.078, 0.528, 0.062, 0.21],
    [-0.499, -0.215, 0.088, 0.142],
    [-0.844, -0.183, 0.088, 0.142],
    [-0.869, -0.198, 0.128, 0.066],
    [-1.39, -0.2, 0.087, 0.066],
    [-0.499, 0.155, 0.088, 0.142],
    [-0.786, 0.183, 0.088, 0.142],
    [-0.886, 0.215, 0.128, 0.066],
    [-1.757, 0.266, 0.048, 0.066],
];
/// The burst's segments (`0x2da8d8`): (from, to, puffs).
const SEGMENTS: [(usize, usize, i32); 14] =
    [(0, 1, 10), (2, 3, 10), (4, 5, 10), (5, 6, 10), (6, 7, 8), (8, 9, 10), (9, 10, 10), (10, 11, 8), (12, 13, 20), (14, 15, 20), (16, 17, 8), (20, 21, 8), (18, 19, 20), (22, 23, 20)];

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn blend(w: &mut World, id: MobyId, seq: u8, t: i32) { if w.m(id).anim.seq_b != seq { c::blend_to(w, id, seq, 0, t); } }
fn heading(a: V, b: V) -> f32 { c::atan(b[0] - a[0], b[1] - a[1]) }
fn spot(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, pv::SPOT)).ok().filter(|&m| m < w.table.mobys.len()) }
fn set_spot(w: &mut World, id: MobyId, m: Option<MobyId>) { c::set_pi32(w, id, pv::SPOT, m.map_or(-1, |m| m as i32)); }
fn cam(w: &World) -> V { w.camera.map(|x| x.to_f32()) }
/// The camera looks at it: its yaw within 32° of the line camera → moby.
fn seen(w: &World, id: MobyId) -> bool {
    let p = c::pos(w, id);
    let k = cam(w);
    c::diff_rots(w.camera_yaw, c::atan(p[0] - k[0], p[1] - k[1])) <= 0.558_505_36
}
fn set_collision(w: &mut World, id: MobyId, on: bool) {
    let has = on && super::class_collision(w, w.m(id).o_class);
    w.mm(id).has_collision = has;
}

/// `0x2daa10` (module doc).
fn pick(w: &World, id: MobyId) -> Option<MobyId> {
    let g = usize::try_from(w.m(id).group).ok()?;
    let list = w.svc.groups.lists.get(g)?.as_ref()?;
    let pos = c::pos(w, id);
    let hero = super::hero_pos(w);
    let to_hero = heading(pos, hero);
    let (mut best, mut best_d, mut best_a) = (None, 0.0f32, 0.0f32);
    for &e in list {
        let m = e as usize;
        let Some(mo) = w.table.mobys.get(m) else { continue };
        if mo.o_class == SELF || mo.cmd != 0 { continue; }
        let a = c::diff_rots(heading(pos, mo.position), to_hero);
        let d = c::dist3(pos, mo.position);
        let take = if a < std::f32::consts::FRAC_PI_2 {
            best_a < a
        } else if d < 16.0 || (best_d <= d && 16.0 <= best_d) {
            16.0 > d && best_d < d
        } else {
            true
        };
        if take {
            best = Some(m);
            best_d = d;
            best_a = a;
        }
    }
    best
}

/// Another spot (the old one kept when there is none), the old freed and the new taken.
fn respot(w: &mut World, id: MobyId) {
    let old = spot(w, id);
    let new = pick(w, id).or(old);
    set_spot(w, id, new);
    if let Some(o) = old { w.mm(o).cmd = 0; }
    if let Some(n) = new { w.mm(n).cmd = 1; }
}

fn to_walk(w: &mut World, id: MobyId, blend_t: i32) {
    blend(w, id, 4, blend_t);
    set_st(w, id, 1);
    let t = w.ticks(0xf0);
    c::set_pi32(w, id, pv::SPOT_T, t);
}

fn walk(w: &mut World, id: MobyId, to: V) {
    let mut out = c::pv4(w, id, pv::OUT);
    walker::walk_to(w, id, pv::J, to, &mut out);
    c::set_pv4(w, id, pv::OUT, out);
}

fn knock_away(w: &mut World, id: MobyId, speed: f32, up: f32, seq: u8) {
    let k = pv::K;
    c::set_pi32(w, id, k + knock::k::FLAGS, 9);
    c::set_pf(w, id, k + knock::k::SPEED, speed);
    c::set_pf(w, id, k + knock::k::UP, up);
    c::set_pf(w, id, k + knock::k::GRAVITY, DT2 * 30.0);
    c::set_pf(w, id, k + knock::k::DRAG, f32::from_bits(0x3a03_126f));
    c::set_pu8(w, id, k + 0x3d, 0);
    let p = c::pos(w, id);
    let h = super::hero_pos(w);
    let a = c::atan(p[0] - h[0], p[1] - h[1]);
    knock::start(w, id, k, a, seq, 1, 0);
    c::set_pf(w, id, k + knock::k::KEY_APEX, 6.0);
    c::set_pf(w, id, k + knock::k::KEY_LAND, 10.0);
}

/// The hits (module doc).
fn hits(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.hit.is_some() && res.out5 != 1 && st(w, id) != 99 {
        w.mm(id).mode |= 0x1000;
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        if 0.0 < hp {
            if res.damage < 1.0 {
                c::set_pu8(w, id, pv::F + 7, 100);
            } else {
                knock_away(w, id, DT * 5.0, DT * 7.0, 6);
                c::set_pu8(w, id, pv::F + 7, 0x78);
                set_st(w, id, 0x5a);
            }
        } else {
            knock_away(w, id, 8.0 * DT, 10.0 * DT, 9);
            set_death_bits(w, id, 0, -1);
            c::set_pu8(w, id, pv::F + 7, 0xfa);
            set_st(w, id, 99);
            w.mm(id).mode &= !0x1000;
            if let Some(s) = spot(w, id) { w.mm(s).cmd = 0; }
        }
        flash::start(w, id, pv::F);
    }
    w.mm(id).hit_slot = 0xff;
}

/// Level08 `0x2dabf0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    crate::moby_update::manip::big_head(w, 3.0, id, 10, id, pv::HEAD);
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam(w)) < 29.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x17;
    }
    if st(w, id) == 3 { w.mm(id).mode &= !0x400; } else { w.mm(id).mode |= 0x400; }
    hits(w, id);
    let t = target::acquire(w, id, 16.0);
    let tm = t.moby.or(w.hero_moby);
    let tpos = tm.map_or(super::hero_pos(w), |m| w.m(m).position);
    let pos = c::pos(w, id);
    match st(w, id) {
        0 => {
            w.mm(id).mode |= 0x1000;
            c::set_pu8(w, id, pv::D + 9, 1);
            c::set_pf(w, id, pv::D, 2.0);
            walker::seed(&mut w.mm(id).pvars, pv::J);
            let j = pv::J;
            c::set_pf(w, id, j + 8, 2.0);
            c::set_pf(w, id, j + 0xc, 2.0);
            c::set_pf(w, id, j + 0x3c, f32::from_bits(0x3cf5_c28f));
            c::set_pf(w, id, j + 0x40, f32::from_bits(0x3e99_999a));
            c::set_pf(w, id, j + 0x44, 0.25);
            c::set_pf(w, id, j + 0x24, 6.0 * DT);
            c::set_pu8(w, id, 0x58, 17);
            c::set_pu8(w, id, 0x5a, 10);
            c::set_pv4(w, id, pv::OUT, [0.0; 4]);
            let s = pick(w, id);
            set_spot(w, id, s);
            if let Some(s) = s { w.mm(s).cmd = 1; }
            let t180 = w.ticks(0xb4);
            c::set_pi32(w, id, pv::TIMER, t180);
            to_walk(w, id, 0);
        }
        1 => {
            let Some(s) = spot(w, id) else { return tail(w, id) };
            let sp = w.m(s).position;
            walk(w, id, sp);
            if c::dist3(pos, sp) < 3.0 && c::diff_rots(w.m(id).rotation[2], heading(pos, sp)) < 0.261_799_4 {
                if 6.0 < c::dist2(pos, tpos) {
                    blend(w, id, 5, 10);
                    set_st(w, id, 6);
                } else {
                    let t240 = w.ticks(0xf0);
                    c::set_pi32(w, id, pv::SPOT_T, t240);
                    respot(w, id);
                }
            } else if c::dec_timer_pvar_i32(w, id, pv::SPOT_T) != 0 {
                respot(w, id);
                let t240 = w.ticks(0xf0);
                c::set_pi32(w, id, pv::SPOT_T, t240);
            }
        }
        3 => {
            let d = c::dist2(pos, tpos);
            turn::spring_turn2_pvar(w, id, heading(pos, tpos), 0.02, 0.3, 0.2, pv::TURN_V);
            let lured = c::pi32(w, id, pv::LURE) != 0 && 0.5 < d;
            let pop = if d < 20.0 && 8.0 < d {
                (c::dec_timer_pvar_i32(w, id, pv::TIMER) != 0 && seen(w, id)) || lured
            } else {
                lured
            };
            if pop {
                if let Some(s) = spot(w, id) {
                    let t6 = w.ticks(6);
                    c::blend_to(w, s, 1, 0, t6);
                }
                let t7 = w.ticks(7);
                blend(w, id, 2, t7);
                set_st(w, id, 4);
                w.mm(id).mode |= 0x1000;
                set_collision(w, id, true);
            }
        }
        4 => {
            let Some(s) = spot(w, id) else { return tail(w, id) };
            let yaw = w.m(id).rotation[2];
            let sp = w.m(s).position;
            let to = [sp[0] + yaw.cos() * 2.0, sp[1] + yaw.sin() * 2.0, sp[2], sp[3]];
            if 0.2 < c::dist2(pos, to) { walk(w, id, to); }
            if wrapped(w, id) {
                if 6.0 <= c::dist2(pos, tpos) && seen(w, id) {
                    let t6 = w.ticks(6);
                    blend(w, id, 3, t6);
                    set_st(w, id, 5);
                } else {
                    respot(w, id);
                    let t7 = w.ticks(7);
                    to_walk(w, id, t7);
                }
            }
        }
        5 => {
            turn::spring_turn2_pvar(w, id, heading(pos, tpos), 0.02, 0.3, 0.2, pv::TURN_V);
            let a = &w.m(id).anim;
            if a.seq_a == a.seq_b && c::ground::key_time(w, id) == 19.0 {
                throw(w, id, tpos);
            } else if wrapped(w, id) {
                to_walk(w, id, 3);
            }
        }
        6 => {
            let Some(s) = spot(w, id) else { return tail(w, id) };
            let sp = w.m(s).position;
            walk(w, id, sp);
            if !wrapped(w, id) { return tail(w, id); }
            let t6 = w.ticks(6);
            c::blend_to(w, s, 1, 0, t6);
            c::set_pos(w, id, [sp[0], sp[1], sp[2] - 0.1, sp[3]]);
            blend(w, id, 1, 10);
            set_st(w, id, 3);
            w.mm(id).mode &= !0x1000;
            let p = c::pos(w, id);
            w.mm(id).rotation[2] = heading(p, tpos);
            let t120 = w.ticks(0x78);
            c::set_pi32(w, id, pv::TIMER, t120);
            set_collision(w, id, false);
        }
        0x5a | 0x5c | 0x5d => {
            if knock::update(w, id, pv::K) & knock::res::ANIM_WRAPPED != 0 {
                let old = spot(w, id);
                if let Some(n) = pick(w, id) {
                    set_spot(w, id, Some(n));
                    if let Some(o) = old { w.mm(o).cmd = 0; }
                    w.mm(n).cmd = 1;
                }
                to_walk(w, id, 10);
            }
            if c::pos(w, id)[2] < 5.0 {
                w.delete_moby(id);
                return;
            }
        }
        0x5b => {
            if wrapped(w, id) {
                blend(w, id, 7, 3);
                set_st(w, id, 0x5d);
            }
        }
        99 if knock::update(w, id, pv::K) & knock::res::ANIM_WRAPPED != 0 => {
            w.mm(id).anim.speed = 0.0;
            set_st(w, id, 100);
            set_collision(w, id, false);
            burst(w, id);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    tail(w, id);
}

fn tail(w: &mut World, id: MobyId) {
    flash::update(w, id, pv::F);
    c::set_pi32(w, id, pv::LURE, 0);
}

/// State 5's throw (module doc).
fn throw(w: &mut World, id: MobyId, tpos: V) {
    let pos = c::pos(w, id);
    let mut v = c::sub(tpos, pos);
    v[2] = 0.0;
    let l = c::len3(v);
    let per = l / w.svc.timing.scale(Pf::f(90.0)).to_f32();
    let reach = (l - (0.0 * per + 2.0)).clamp(3.0, 10.0);
    let v = c::clamp_len3(v, reach);
    let land = c::add(v, pos);
    let jp = w.joint_point(id, 9);
    let mut b = c::sub(tpos, jp);
    let lb = (b[0] * b[0] + b[1] * b[1]).sqrt();
    if lb != 0.0 {
        b[0] *= per / lb;
        b[1] *= per / lb;
    }
    let mut time = 0.0;
    b[2] = knock::lob_up(per, -(DT2 * 15.0), jp, land, &mut time);
    let shot = spawn_shot(w, id, jp, b);
    c::set_pi32(w, id, pv::SHOT, shot.map_or(-1, |s| s as i32));
    if let Some(s) = shot {
        let (t180, t10) = (w.ticks(0xb4), w.ticks(10));
        c::set_pi32(w, s, 0x20, t180);
        c::set_pi16(w, s, 0xc, t10 as i16);
        c::set_pi16(w, s, 0xe, t10 as i16);
        w.mm(s).state = 1;
        w.mm(s).cmd |= 3;
        c::set_pi32(w, id, pv::TIMER, t180);
    }
}

/// `0x2d22e0(thrower, p, v)`: `CreateMoby(248)`.
fn spawn_shot(w: &mut World, owner: MobyId, p: V, v: V) -> Option<MobyId> {
    let m = w.create_moby(SHOT)?;
    if w.m(m).pvars.len() < 0x40 { w.mm(m).pvars.resize(0x40, 0); }
    {
        let mo = w.mm(m);
        mo.draw_dist = 0xff;
        mo.update_dist = 0xff;
        mo.state = 1;
        mo.visible = 1;
    }
    c::set_pi32(w, m, 0x30, owner as i32 + 1);
    let t300 = w.ticks(300);
    c::set_pi32(w, m, 0x20, t300);
    c::set_pf(w, m, 0x2c, 1.0);
    c::set_pi32(w, m, 0x24, 0);
    c::set_pos(w, m, p);
    c::set_pv4(w, m, 0x10, v);
    let rx = w.rng.randf(-180.0, 180.0) * 0.017_453_292;
    w.mm(m).rotation[0] = rx;
    w.mm(m).rotation[2] = c::atan(v[0], v[1]) + std::f32::consts::FRAC_PI_2;
    w.build_matrix(m);
    Some(m)
}

/// `0x2da8d8` / `0x2da7c0` / `0x2da3f0`: the smoke burst along the body outline (module doc).
fn burst(w: &mut World, id: MobyId) {
    let rows = w.m(id).rows;
    let pos = c::pos(w, id);
    let place = |q: [f32; 4]| -> V { std::array::from_fn(|k| if k == 3 { 1.0 } else { rows[0][k] * q[0] + rows[1][k] * q[1] + rows[2][k] * q[2] + pos[k] }) };
    for (a, b, n) in SEGMENTS {
        let (pa, pb) = (place(OUTLINE[a]), place(OUTLINE[b]));
        let k = 1.75;
        segment(w, OUTLINE[a][3] * k, OUTLINE[b][3] * k, pa, pb, n);
    }
}

/// `0x2da3f0(s0, s1, m, a, b, n)`: n puffs (type 23) along a → b, then two sparks (type 21) at a and b.
fn segment(w: &mut World, s0: f32, s1: f32, a: V, b: V, n: i32) {
    let step = c::scale(c::sub(b, a), 1.0 / n as f32);
    let ds = (s1 - s0) / n as f32;
    for i in 0..n {
        let p = c::add(c::scale(step, i as f32), a);
        let vx = w.rng.randf_sym(0.0, 0.005);
        let vy = w.rng.randf_sym(0.0, 0.005);
        let vz = w.rng.randf(0.0005, 0.005);
        let al = w.rng.rand_range(0x30, 0x70) as u32;
        let g = w.rng.rand_range(0x30, 0x7f) as u32;
        let hi = w.rng.randf(1.0, f32::from_bits(0x3f82_8f5c));
        let spin = w.rng.rand_range(-2, 2);
        *w.svc.fx.part_spawns.entry(23).or_default() += 1;
        let size = ds + s0 * 210_000.0;
        let rec = w.particles.as_deref_mut().and_then(|sys| type23::spawn(sys, w.rng, 0.1, 1.0, hi, size, p, spin, [vx, vy, vz, 1.0], al << 24 | g | g << 16 | g << 8));
        let Some(i) = rec else {
            w.svc.fx.part_failed += 1;
            continue;
        };
        let blend = w.rng.randi(2) != 0;
        let g2 = if blend { w.rng.rand_range(0x60, 0xe0) as u32 } else { 0 };
        if let Some(sys) = w.particles.as_deref_mut() {
            let r = &mut sys.pool.recs[i];
            if blend {
                r[3] = 0x44;
                crate::particles::rec::set_u32(r, 4, al << 24 | g2 | g2 << 16 | g2 << 8);
            }
            crate::particles::rec::set_i16(r, 10, 0);
            crate::particles::rec::set_u32(r, 0x24, 2);
            r[0x2a] = al as u8;
            r[0x2b] = 0;
        }
    }
    for (size, at) in [(10000.0, a), (20000.0, b)] {
        let ax = w.rng.rand_angle();
        let x = ax.cos() * 0.05;
        let ay = w.rng.rand_angle();
        let y = ay.sin() * 0.05;
        let z = w.rng.randf(0.01, f32::from_bits(0x3cf5_c28f));
        let (t10, t20) = (w.ticks(10), w.ticks(0x14));
        let life = w.rng.rand_range(t10, t20);
        fx::part21(w, size, at, [x, y, z, 0.0], 0x4f00_7fff, 0x1fff_ffff, life, 1);
    }
}

// -------------------------------------------------------------------------------------------------
// The grenade 248

const SPARKS: [u32; 4] = [0x2fff_ffff, 0x2f00_ffff, 0x2f00_7fff, 0x2f00_4fff];
/// gp−0x53a0 (0x161860): the grenade's gravity.
const GRAVITY: f32 = 15.0;

fn thrower(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, 0x30) - 1).ok() }
fn off_state(w: &World, id: MobyId) -> u8 { if w.m(id).cmd & 2 != 0 { 99 } else { 0x62 } }

/// Hit a moby: off unless it is the thrower before the first bounce; placed at the hit, bounced and set to 2·dt.
fn hit_moby(w: &mut World, id: MobyId, o: &crate::collision_query::CollOutput) -> bool {
    if o.moby == thrower(w, id) && c::pi32(w, id, 0x24) == 0 { return false; }
    let s = off_state(w, id);
    w.mm(id).state = s;
    c::set_pos(w, id, [o.point[0], o.point[1], o.point[2], 1.0]);
    let v = reflect(c::pv4(w, id, 0x10), o.normal);
    c::set_pv4(w, id, 0x10, c::set_len3(v, DT + DT));
    true
}

fn reflect(v: V, n: [f32; 3]) -> V { crate::moby_update::services::reflect(v4(v), v4([n[0], n[1], n[2], 0.0])).map(|x| x.to_f32()) }

/// Level08 `0x2d23f0` (module doc).
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x40 { w.mm(id).pvars.resize(0x40, 0); }
    if w.m(id).state != 0x37 {
        if let Some(o) = w.coll_sphere(v4(c::pos(w, id)), pf(0.5), 0x11, Some(id)) {
            if o.moby.is_some() { hit_moby(w, id, &o); }
        }
    }
    match w.m(id).state {
        1 => fly(w, id),
        2 => {
            if c::dec_timer_pvar_i32(w, id, 0x20) == 0 {
                let t = c::pi32(w, id, 0x20);
                if t == w.ticks(0x3c) || t == w.ticks(0x28) || t == w.ticks(0x14) {
                    c::set_pu8(w, id, 7, 0xfa);
                    flash::start(w, id, 0);
                }
            } else {
                w.mm(id).state = 99;
            }
            flash::update(w, id, 0);
        }
        0x62 | 99 => {
            let explode = w.m(id).state == 0x62;
            w.play_sound(0, 0, id);
            let tmpl = HitTemplate { attacker: Some(id), flags: 0x81_0001, damage: Pf::ONE, ..Default::default() };
            let p = c::pos(w, id);
            w.sphere_mobys(Pf::ONE, v4(p), 0x10, Some(id), Some(&tmpl));
            if explode {
                const B: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 3.0, flash2: f32::from_bits(0x3fd9_999a), flash_dist: 4.0, scale: 1.0, light: 7.0, streaks: 7, sparks: 10, puffs: 0x14, debris: -1, sound: -1, shake: true };
                fx::beam_explosion(w, &B, Some(id), p);
            } else {
                fx::death_explosion(w, 0.5, 13.0, Some(id), p, -1);
            }
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    crate::shadows::blob(w, 0.2, id);
}

/// State 1 (module doc).
fn fly(w: &mut World, id: MobyId) {
    let vel = c::pv4(w, id, 0x10);
    let spin = c::add_rot(w.m(id).rotation[0], c::len3(vel) * 0.5);
    w.mm(id).rotation[0] = spin;
    if c::pi32(w, id, 0x24) == 0 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let d = c::set_len3([x, y, z, 0.0], DT);
        let k = w.rng.randi(4) as usize;
        let life = w.ticks(0x2d);
        let a = crate::particles::type04::Spawn { pos: c::pos(w, id), vel: d, c1: SPARKS[k], c2: 0x4fff, life, base: 0x32, growth: 0x78, additive: true };
        fx::part04(w, &a);
    }
    let pos = c::pos(w, id);
    let from = [pos[0], pos[1], pos[2] - 0.16, pos[3]];
    let p = c::add(pos, vel);
    c::set_pos(w, id, p);
    let mut vel = vel;
    vel[2] -= GRAVITY * DT2;
    c::set_pv4(w, id, 0x10, vel);
    let to = [p[0], p[1], p[2] - 0.16, p[3]];
    let tmpl = HitTemplate { dir: v4(vel), attacker: Some(id), flags: 0x81_0000, damage: Pf::ONE, w20: 1, ..Default::default() };
    let hit = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, v4(from), v4(to), 0, Some(id), &tmpl);
    let Some(o) = hit else {
        if c::dec_timer_pvar_i32(w, id, 0x20) != 0 { w.mm(id).state = off_state(w, id); }
        return;
    };
    if o.moby.is_some() {
        hit_moby(w, id, &o);
        return;
    }
    if o.kind < 1 { return; }
    if w.m(id).cmd & 1 == 0 {
        w.mm(id).state = 0x62;
        c::set_pos(w, id, [o.point[0], o.point[1], o.point[2], 1.0]);
        let v = reflect(c::pv4(w, id, 0x10), o.normal);
        c::set_pv4(w, id, 0x10, c::set_len3(v, DT + DT));
        return;
    }
    w.play_sound(1, 0, id);
    c::set_pos(w, id, [o.point[0], o.point[1], o.point[2] + 0.16, 1.0]);
    let v = reflect(c::pv4(w, id, 0x10), o.normal);
    let n = [o.normal[0], o.normal[1], o.normal[2], 0.0];
    let side = cross(v, n);
    let s = w.rng.randf(-(DT * 1.5), DT * 1.5);
    let side = c::set_len3(side, s);
    let v = c::scale(c::add(v, side), f32::from_bits(0x3ecc_cccd));
    c::set_pv4(w, id, 0x10, v);
    let b = c::pi32(w, id, 0x24) + 1;
    c::set_pi32(w, id, 0x24, b);
    let t180 = w.ticks(0xb4);
    c::set_pi32(w, id, 0x20, t180);
    if c::len3(v) < DT {
        w.mm(id).state = 2;
        let t120 = w.ticks(0x78);
        c::set_pi32(w, id, 0x20, t120);
        c::set_pf(w, id, 0x28, 254.0 / t120 as f32);
        c::set_pv4(w, id, 0x10, [0.0; 4]);
    }
}

fn cross(a: V, b: V) -> V { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0], 0.0] }
