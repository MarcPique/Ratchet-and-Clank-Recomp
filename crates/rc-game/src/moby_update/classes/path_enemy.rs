//! Class 459, the Novalis robot troopers (`PathEnemyUpdate` 0x2e6bf0, level01 only), and their fire shots, class 722
//! (`0x2fa4c0`, spawned by `0x2fa068`, burst `0x2fa1b0`). A thin state machine over the shared creature layer
//! ([`crate::moby_update::creature`]); spec `docs/plan/creatures.md` "Robot trooper 459".
//!
//! **Arrival.** The init (state 0) puts the trooper on point 0 of its patrol path (pvar+0x164) — or of its arrival path
//! (+0x134) — and, unless +0x1b4 is set, lifts it 20 units: above the ground it is hidden (mode 0x41, no collision) and
//! waits in state 2 until Ratchet is within +0x1a8 (xy; 30 on Novalis, 0 = never), stands in the trigger cuboid +0x1c0,
//! or a carrier sets +0x1e8 (the dropship 666 on release, the gunship 688 at the end of its fly-by). It then shows up
//! and comes down on its jetpack (state 3, sequence 8, the exhaust [`jet_exhaust`] every tick): straight down at
//! 10 units/s onto the ground; along its arrival path at 5 units/s; or, dropped from a dropship, gliding from the
//! ship's velocity (+0x1d0, kept vertical for +0x1ec ticks) towards the centre of cuboid +0x1e4 at 5–10 units/s.
//!
//! **Patrol and attack.** State 1 turns to Ratchet (`SpringTurn2` 0.02 / 0.3 / 0.1) and, when he is within the range
//! +0x1a0 (doubled for 600 ticks after an alert or a hit), within 2 in height, the trooper was drawn last frame
//! (+0x31), the 60-tick fire cooldown +0x190 is out and it faces him within 5°, checks the muzzle (joint list 0) is not
//! inside a wall and opens fire (state 6, sequence 4): at key 14 of each loop a fire glob (722) leaves the muzzle
//! towards a point 4 ahead at 24 units/s (four per volley, then state 7, sequence 5 at half speed). When Ratchet comes
//! within 4 (and 6 in height), or an alert makes the patrol pick ([`pick`] = `0x2e6790`) choose a point other than the
//! nearest, it jet-hops along its path (states 9 → 10 → 11: sequence 8 at 1.75 speed, accelerating by 7.75·dt² up to
//! 7.75·dt per tick with the body tilted forward, braking by three times that at the point) and lands (sequence 10).
//! On the ground (states 1, 5, 6, 7, 11) it falls under 9.8·dt² onto the ground; landing on water (surface 0) kills it.
//!
//! **Hits** (mask 0x210000) through the resolver (column 4) when the class cooldown +0x26 is 0: the alert timer +0x1e2 =
//! 600 ticks, health (2) −= damage; the reaction picks the flight: 1 / 2 (or no health left) the death flight (state
//! 0xe, sequence 9, not targetable, 40·dt² gravity), 3 / 6 a stagger (0xc, sequences 0xc → 0xb), 4 / 5 a knock-down
//! (0xc, sequence 0xe); after a stagger it hops back to its patrol point. The death flight ends on a touch or out of
//! the world in `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, …, 5 streaks, 2 sparks, 4 puffs, sound 4, shake, 1 debris)`,
//! four body pieces (1736–1738, 1770) and the bolt drop.
//!
//! Pvars (0x240; header at 0: damage +0x20, flash +0x60, knockback +0x70, walker +0xd0): +0x38 alert, +0x120 arrival
//! index / +0x124 s8 step, **+0x130 arrival spline + 1** (the game's pointer; 0 none), +0x134 arrival spline, +0x150
//! patrol point / +0x154 s8 step, **+0x160 patrol spline + 1** (pointer), +0x164 patrol spline, +0x180 aim point,
//! +0x190 fire cooldown, +0x194 turn velocity, +0x198 aim distance, +0x1a0 range, +0x1a4 s16 shots, +0x1a6 s16 hop
//! point, +0x1a8 wake range, +0x1ac s16 one-way path, +0x1ae s16 "path has marked points", +0x1b4 no lift, +0x1b8 jet
//! speed, +0x1bc tilt velocity, +0x1c0 trigger cuboid, +0x1c4 camera cuboid, +0x1c8 camera timer, +0x1cc last key
//! time, +0x1d0 drop velocity, +0x1e0 s16 draw distance, +0x1e2 s16 alert timer, +0x1e4 drop cuboid, +0x1e8 woken,
//! +0x1ec s16 drop hold, +0x1ee s16 no-brake.
//!
//! A trooper with a camera cuboid +0x1c4 (none on Novalis) shows its arrival: Ratchet's state 0x1f and the camera
//! script on that cuboid for 360 ticks, queued in `creature::Globals::scripts` like the gunship's, and released from
//! state 1 when the timer +0x1c8 is out. The big-head cheat `0x278720(2.5, m, 2, +0x200)` (0x15edb7): `manip::big_head`. The shadow slab
//! `0x26eff8` (moby +0x84/+0x88) is set from the ground height of the gravity step (crate::shadows).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::projectile;
use crate::moby_update::creature::{self as c, damage, flash, fx, ground, knock, target, turn, walker};
use crate::moby_update::services::{pf as to_pf, pv, World};
use std::f32::consts::PI;

pub const UPDATE_FN: u32 = 0x2e6bf0;
pub const CLASSES: [i16; 1] = [459];
/// The fire glob 722 (`CreateMoby(0x2d2)` in `0x2fa068`) and its update.
pub const SHOT_CLASS: i16 = 0x2d2;
pub const SHOT_CLASSES: [i16; 1] = [SHOT_CLASS];
pub const SHOT_UPDATE_FN: u32 = 0x2fa4c0;
/// The body pieces of the death (`BreakFxB` classes; `FxGroupUpdate`).
pub const PIECES: [i16; 4] = [0x6c8, 0x6c9, 0x6ca, 0x6ea];

/// gp−0x52ac (0x161954): the jet speed, gp−0x52b0 the shot speed, gp−0x52a4 the jet animation speed.
const JET: f32 = 7.75;
const SHOT_SPEED: f32 = 24.0;
const JET_ANIM: f32 = 1.75;
/// 5° and 2° (0x3db2b8c2 / 0x3d0efa35).
const DEG5: f32 = 0.087_266_46;
const DEG2: f32 = 0.034_906_585;

const D: usize = 0x20;
const HIT_CD: usize = 0x26;
const ALERT: usize = 0x38;
const ZERO40: usize = 0x40;
const FLASH: usize = 0x60;
const K: usize = 0x70;
const J: usize = 0xd0;
const VZ: usize = 0xe8;
const ARR_IDX: usize = 0x120;
const ARR_STEP: usize = 0x124;
const ARR_PATH: usize = 0x130;
const ARR_SPLINE: usize = 0x134;
const PT: usize = 0x150;
const PT_STEP: usize = 0x154;
const PATH: usize = 0x160;
const SPLINE: usize = 0x164;
const AIM: usize = 0x180;
const FIRE_CD: usize = 0x190;
const TURN_V: usize = 0x194;
const AIM_DIST: usize = 0x198;
const RANGE: usize = 0x1a0;
const SHOTS: usize = 0x1a4;
const HOP_PT: usize = 0x1a6;
const WAKE: usize = 0x1a8;
const ONE_WAY: usize = 0x1ac;
const MARKED: usize = 0x1ae;
const NO_LIFT: usize = 0x1b4;
const JET_V: usize = 0x1b8;
const TILT_V: usize = 0x1bc;
const TRIGGER: usize = 0x1c0;
const CAMERA: usize = 0x1c4;
const CAMERA_T: usize = 0x1c8;
const KEY_PREV: usize = 0x1cc;
const DROP_V: usize = 0x1d0;
const DRAW0: usize = 0x1e0;
const ALERT_T: usize = 0x1e2;
const DROP_CUBOID: usize = 0x1e4;
const WOKEN: usize = 0x1e8;
const DROP_HOLD: usize = 0x1ec;
const NO_BRAKE: usize = 0x1ee;

fn st(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_st(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn blend_if(w: &mut World, id: MobyId, seq: u8, frame: i32, t: i32) {
    if w.m(id).anim.seq_b != seq {
        let t = w.ticks(t);
        w.anim_blend(id, seq, frame, t);
    }
}
fn set_speed(w: &mut World, id: MobyId, s: f32) { w.mm(id).anim.speed = s; }

/// A spline stored as "index + 1" (the game's pointer; 0 = none).
fn spline(w: &World, id: MobyId, o: usize) -> Option<usize> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&s| s < w.svc.splines.len()) }
fn count(w: &World, s: usize) -> i32 { w.svc.splines[s].len() as i32 }
fn point(w: &World, s: usize, i: i32) -> c::V {
    w.svc.splines[s].get(i.max(0) as usize).map(|p| p.map(f32::from_bits)).unwrap_or([0.0; 4])
}
fn cuboid_centre(w: &World, i: i32) -> Option<c::V> {
    let s = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i)?;
    let m = s.matrix[3];
    Some([m[0], m[1], m[2], m[3]])
}
fn hero_pos(w: &World) -> c::V { w.hero.pos.map(|x| f32::from_bits(x.0)) }
fn in_cuboid(w: &World, p: c::V, i: i32) -> bool { w.in_cuboid([p[0], p[1], p[2]], i) }

fn alerted(w: &World, id: MobyId) -> bool { c::pi32(w, id, ALERT) != 0 || c::pi16(w, id, ALERT_T) != 0 }
fn range(w: &World, id: MobyId, alert: bool) -> f32 {
    let r = c::pf(w, id, RANGE);
    if alert { r + r } else { r }
}

/// `PathEnemyUpdate` 0x2e6bf0.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1f0 { return; }
    let key = ground::key_time(w, id);
    let alert = alerted(w, id);
    // 0x278720(2.5, m, 2, +0x200): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 2, id, 0x200);
    let old = c::pos(w, id);
    let cam_t = c::pi32(w, id, CAMERA_T);
    if 0 < cam_t { c::set_pi32(w, id, CAMERA_T, cam_t - 1); }
    let mut next = c::pi32(w, id, PT);
    let mut moved = false;
    if st(w, id) != 0 { moved = pick(w, id, &mut next); }
    let tg = target::acquire(w, id, range(w, id, alert));
    let t = tg.pos;
    let hit = w.get_hit(id, 0x21_0000, false);
    let res = damage::resolve(w, id, hit, D, 0, 4);
    c::dec_timer_pvar_s16(w, id, HIT_CD);
    if hit.is_some() && c::pi16(w, id, HIT_CD) == 0 {
        if let Some(h) = res.hit { on_hit(w, id, &h, res.reaction, res.damage); }
    }
    w.mm(id).hit_slot = 0xff;
    if c::pi32(w, id, ALERT) != 0 {
        let a = w.ticks(600);
        c::set_pi16(w, id, ALERT_T, a as i16);
    }
    c::set_pi32(w, id, ALERT, 0);
    c::dec_timer_pvar_s16(w, id, ALERT_T);
    let pos = c::pos(w, id);
    match st(w, id) {
        0 => {
            if !init(w, id) { return; }
        }
        1 => {
            turn::spring_turn2_pvar(w, id, c::atan(t[0] - pos[0], t[1] - pos[1]), 0.02, 0.3, 0.1, TURN_V);
            c::dec_timer_pvar_i32(w, id, FIRE_CD);
            if try_fire(w, id, t, alert, moved) == Fire::No {
                let d = c::dist2(c::pos(w, id), t);
                let hop = alert || (d < 4.0 && (c::pos(w, id)[2] - t[2]).abs() < 6.0);
                if hop && moved { hop_to(w, id, next); }
            }
            camera_end(w, id);
        }
        2 => wait(w, id, t),
        3 => {
            arrive(w, id);
            jet_exhaust(w, id, old);
        }
        6 => {
            if c::pf(w, id, KEY_PREV) < 14.0 && 14.0 <= key {
                fire(w, id, t);
                if st(w, id) != 6 { return end(w, id, key); }
            }
            if key < 2.0 {
                let stop = if c::pi16(w, id, SHOTS) < 4 {
                    let d = c::dist2(c::pos(w, id), t);
                    d > range(w, id, alert) || 6.0 < (c::pos(w, id)[2] - t[2]).abs()
                } else {
                    true
                };
                if stop {
                    blend_if(w, id, 5, 5, 10);
                    set_st(w, id, 7);
                    set_speed(w, id, 0.5);
                }
            }
        }
        7 => {
            turn::spring_turn2_pvar(w, id, c::atan(t[0] - pos[0], t[1] - pos[1]), 0.02, 0.3, 0.1, TURN_V);
            if w.m(id).anim.seq_b != 0 {
                if w.m(id).anim.flags & 2 != 0 {
                    set_speed(w, id, 1.0);
                    let p = c::pos(w, id);
                    let far = !alert && (8.0 <= c::dist2(p, t) || 6.0 <= (p[2] - t[2]).abs());
                    let at_point = patrol_point(w, id, next).is_none_or(|q| c::dist3(p, q) <= 0.25);
                    if far || !moved || at_point {
                        if try_fire(w, id, t, alert, moved) != Fire::Fired {
                            blend_if(w, id, 0, 0, 5);
                            set_st(w, id, 1);
                        }
                    } else {
                        c::set_pi32(w, id, PT, next);
                        blend_if(w, id, 8, 0, 10);
                        set_st(w, id, 9);
                        set_speed(w, id, JET_ANIM);
                    }
                }
            } else if c::dec_timer_pvar_i32(w, id, FIRE_CD) != 0 {
                blend_if(w, id, 5, 0, 15);
            }
        }
        9 => {
            set_speed(w, id, 1.0);
            c::set_pf(w, id, JET_V, 0.0);
            c::set_pf(w, id, TILT_V, 0.0);
            set_st(w, id, 10);
            jet_exhaust(w, id, old);
        }
        10 => {
            hop(w, id);
            jet_exhaust(w, id, old);
        }
        11 => {
            land(w, id, t);
            jet_exhaust(w, id, old);
        }
        0xc => {
            let r = knock::update(w, id, K);
            if r & knock::res::ANIM_WRAPPED != 0 {
                let p = c::pos(w, id);
                if patrol_point(w, id, next).is_some_and(|q| 0.25 < c::dist3(p, q)) {
                    blend_if(w, id, 8, 0, 10);
                    c::set_pi32(w, id, PT, next);
                    set_st(w, id, 9);
                    set_speed(w, id, JET_ANIM);
                    return;
                }
                blend_if(w, id, 0, 0, 6);
                set_st(w, id, 1);
            }
            if !projectile::in_world(c::pos(w, id)) {
                explode(w, id);
                set_death_bits(w, id, 0, -1);
                w.delete_moby(id);
                return;
            }
        }
        0xe => {
            w.mm(id).mode &= !mode::TARGETABLE;
            let r = knock::update(w, id, K);
            if r & 6 != 0 || !projectile::in_world(c::pos(w, id)) {
                die(w, id);
                return;
            }
        }
        _ => {}
    }
    end(w, id, key);
}

/// `LAB_002e89ac`: the hit flash, the last key time, gravity on the ground states, the water death.
fn end(w: &mut World, id: MobyId, key: f32) {
    flash::update(w, id, FLASH);
    c::set_pf(w, id, KEY_PREV, key);
    let s = st(w, id);
    let g = if matches!(s, 1 | 5 | 6 | 7 | 11) {
        let v = c::pf(w, id, VZ) + c::DT2 * 9.8;
        c::set_pf(w, id, VZ, v);
        let mut p = c::pos(w, id);
        p[2] -= v - 2.0;
        c::set_pos(w, id, p);
        let g = ground::ground(w, p, 0.5, 0);
        p[2] -= 2.0;
        if p[2] < g.z {
            p[2] = g.z;
            c::set_pf(w, id, VZ, 0.0);
        }
        c::set_pos(w, id, p);
        g
    } else {
        ground::ground(w, c::pos(w, id), 0.5, 0)
    };
    crate::shadows::set_ground(w.mm(id), g.z);
    if g.surface == 0 && c::pos(w, id)[2] < g.z + 1.0 { die(w, id); }
}

/// The beam explosion of 0x2e6bf0's three death sites.
fn explode(w: &mut World, id: MobyId) {
    let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: 4, shake: true };
    fx::beam_explosion(w, &b, Some(id), c::pos(w, id));
}

/// The explosion, the four body pieces, the bolt drop and `DeleteMoby`.
fn die(w: &mut World, id: MobyId) {
    explode(w, id);
    let (p, rot) = (c::pos(w, id), w.m(id).rotation);
    for cl in PIECES { fx::break_piece(w, id, cl, p, rot, 0, 0); }
    set_death_bits(w, id, 0, -1);
    w.delete_moby(id);
}

/// The hit branch (module doc).
fn on_hit(w: &mut World, id: MobyId, h: &crate::moby_update::services::HitRecord, reaction: u8, dmg: f32) {
    let t = w.ticks(600);
    c::set_pi16(w, id, ALERT_T, t as i16);
    let hp = c::pf(w, id, D) - dmg;
    c::set_pf(w, id, D, hp);
    let r = if hp <= 0.0 { 1 } else { reaction };
    let dir = h.dir.map(|x| f32::from_bits(x.0));
    let kk = |o: usize| K + o;
    let aim = |w: &mut World| {
        let (mut s, mut u) = (c::pf(w, id, kk(knock::k::SPEED)), c::pf(w, id, kk(knock::k::UP)));
        let a = knock::aim(dir, &mut s, &mut u);
        c::set_pf(w, id, kk(knock::k::SPEED), s);
        c::set_pf(w, id, kk(knock::k::UP), u);
        a
    };
    match r {
        1 | 2 => {
            let drag = c::DT2 * 10.0;
            c::set_pf(w, id, kk(knock::k::ZOFF), 0.3);
            c::set_pf(w, id, kk(knock::k::DRAG), drag);
            c::set_pf(w, id, kk(knock::k::GRAVITY), c::DT2 * 40.0);
            c::set_pf(w, id, kk(knock::k::SPEED), ((drag + drag) * 10.0).sqrt());
            c::set_pf(w, id, kk(knock::k::KEY_APEX), 5.0);
            c::set_pf(w, id, kk(knock::k::KEY_LAND), 10.0);
            c::set_pf(w, id, kk(knock::k::UP), c::DT * 15.0);
            c::set_pi32(w, id, kk(knock::k::FLAGS), 1);
            c::set_pu8(w, id, K + 0x3d, 0);
            let a = aim(w);
            let t = w.ticks(2);
            knock::start(w, id, K, a, 9, t, 0);
            c::set_pu8(w, id, FLASH + 7, 0xb4);
            set_st(w, id, 0xe);
            w.mm(id).mode &= !mode::TARGETABLE;
            flash::start(w, id, FLASH);
            let t = w.ticks(60);
            c::set_pi16(w, id, HIT_CD, t as i16);
        }
        3..=6 => {
            let stagger = matches!(r, 3 | 6);
            c::set_pu8(w, id, FLASH + 7, if stagger { 0x78 } else { 0xfa });
            c::set_pi32(w, id, kk(knock::k::RADIUS), 0x200);
            c::set_pf(w, id, kk(knock::k::GRAVITY), f32::from_bits(0x3c03_126f));
            if stagger { c::set_pf(w, id, kk(knock::k::ZOFF), 0.5); }
            let drag = c::pf(w, id, kk(knock::k::DRAG));
            c::set_pf(w, id, kk(knock::k::SPEED), ((drag + drag) * 1.5).sqrt());
            c::set_pf(w, id, kk(knock::k::UP), c::DT * if stagger { 8.5 } else { 5.0 });
            c::set_pf(w, id, kk(knock::k::KEY_APEX), if stagger { 5.0 } else { 7.0 });
            c::set_pf(w, id, kk(knock::k::KEY_LAND), if stagger { 10.0 } else { 13.5 });
            c::set_pi32(w, id, kk(knock::k::FLAGS), 1);
            c::set_pu8(w, id, K + 0x3d, 0);
            let a = aim(w);
            knock::start(w, id, K, a, if stagger { 0xc } else { 0xe }, 6, 0);
            if stagger { blend_if(w, id, 0xb, 0, 6); }
            set_st(w, id, 0xc);
            flash::start(w, id, FLASH);
        }
        _ => {}
    }
}

/// State 0: the init (module doc). False when the trooper was deleted (no patrol path).
fn init(w: &mut World, id: MobyId) -> bool {
    let sp = c::pi32(w, id, SPLINE);
    if sp < 0 || sp as usize >= w.svc.splines.len() {
        w.delete_moby(id);
        return false;
    }
    c::set_pi32(w, id, PT, 0);
    c::set_pi32(w, id, PATH, sp + 1);
    let arr = c::pi32(w, id, ARR_SPLINE);
    if arr < 0 || arr as usize >= w.svc.splines.len() {
        let p0 = point(w, sp as usize, 0);
        c::set_pos(w, id, p0);
        if c::pi32(w, id, NO_LIFT) == 0 {
            c::set_pi32(w, id, ARR_PATH, 0);
            let mut p = p0;
            p[2] += 20.0;
            c::set_pos(w, id, p);
        }
    } else {
        c::set_pi32(w, id, ARR_IDX, 0);
        c::set_pi32(w, id, ARR_PATH, arr + 1);
        let p0 = point(w, arr as usize, 0);
        c::set_pos(w, id, p0);
    }
    c::set_pi16(w, id, MARKED, 0);
    let s = sp as usize;
    for k in 0..count(w, s) {
        if point(w, s, k)[3] == 1.0 { c::set_pi16(w, id, MARKED, 1); }
    }
    w.mm(id).mode |= mode::TARGETABLE;
    c::set_pu8(w, id, D + 9, 0);
    if let Some(h) = w.hero_moby {
        let (l, a) = (w.m(h).light, w.m(h).ambient);
        let m = w.mm(id);
        m.light = l;
        m.ambient = a;
    }
    c::set_pu8(w, id, 0x5a, 8);
    c::set_pu8(w, id, 0x58, 8);
    walker::seed(&mut w.mm(id).pvars, J);
    c::set_pf(w, id, J + 0x3c, 0.03);
    c::set_pf(w, id, J + 0x40, 0.3);
    c::set_pf(w, id, J + 0x44, 0.25);
    c::set_pf(w, id, J + 0x1c, c::DT2 * 5.0);
    c::set_pf(w, id, J + 0x24, JET * c::DT);
    c::set_pf(w, id, J + 0xc, 2.0);
    c::set_pf(w, id, J + 8, 2.0);
    c::set_pf(w, id, D, 2.0);
    c::set_pi16(w, id, D + 4, 2);
    c::set_pu8(w, id, D + 9, 0);
    c::set_pu8(w, id, D + 8, 1);
    c::set_pv4(w, id, ZERO40, [0.0; 4]);
    c::set_pi32(w, id, CAMERA_T, -1);
    if w.m(id).anim.seq_b != 0 {
        let t = w.ticks(5);
        w.anim_blend(id, 0, 0, t);
    }
    let dd = w.m(id).draw_dist;
    c::set_pi16(w, id, DRAW0, dd);
    let g = ground::ground(w, c::pos(w, id), 0.5, 0).z;
    if 0.5 < c::pos(w, id)[2] - g {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode |= 0x41;
    }
    set_st(w, id, 2);
    true
}

/// State 2: dormant until Ratchet is near, in the trigger cuboid, or a carrier woke it.
fn wait(w: &mut World, id: MobyId, t: c::V) {
    let d = c::dist2(c::pos(w, id), t);
    if c::pf(w, id, WAKE) <= d && !in_cuboid(w, hero_pos(w), c::pi32(w, id, TRIGGER)) && c::pi32(w, id, WOKEN) == 0 { return; }
    let cam = c::pi32(w, id, CAMERA);
    if cam != -1 {
        // SetState(0x1f, 0), CameraScript(cuboid +0x1c4) for 360 ticks (no Novalis trooper has one).
        let t = w.ticks(0x168);
        if let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cam) {
            let (centre, euler) = (s.centre(), s.euler);
            w.svc.creatures.scripts.push(c::ScriptRequest::Start { moby: id, hero_state: 0x1f, cuboid: cam, centre, euler, ticks: t });
        }
        w.svc.creatures.cutscene = true;
        c::set_pi32(w, id, CAMERA_T, t);
    }
    let has = w.classes.info(w.m(id).o_class).map(|i| i.has_collision && !i.no_header).unwrap_or(false);
    let m = w.mm(id);
    m.has_collision = has;
    m.mode &= !0x41;
    blend_if(w, id, 8, 0, 10);
    set_st(w, id, 3);
}

/// The end of a +0x1c4 camera script (state 1): `CameraScript2(0)`, `SetState(0, 1)`, 0x15f404 = 0.
fn camera_end(w: &mut World, id: MobyId) {
    if c::pi32(w, id, CAMERA_T) != 0 || c::pi32(w, id, CAMERA) == -1 { return; }
    w.svc.creatures.scripts.push(c::ScriptRequest::End { moby: id });
    w.svc.creatures.cutscene = false;
    c::set_pi32(w, id, CAMERA, -1);
}

/// State 3: the jetpack arrival (module doc).
fn arrive(w: &mut World, id: MobyId) {
    let pos = c::pos(w, id);
    let done = match spline(w, id, ARR_PATH) {
        None => {
            let cub = c::pi32(w, id, DROP_CUBOID);
            match (cub, cuboid_centre(w, cub)) {
                (-1, _) | (_, None) => {
                    let dz = ground::ground(w, pos, 0.5, 0).z - pos[2];
                    let v = [0.0, 0.0, dz.max(-(c::DT * 10.0)), 0.0];
                    c::set_pos(w, id, c::add(pos, v));
                    c::len3(v) < c::DT
                }
                (_, Some(cc)) => {
                    turn::spring_turn2_pvar(w, id, c::atan(cc[0] - pos[0], cc[1] - pos[1]), 0.01, 0.3, 0.1, TURN_V);
                    let v = c::pv4(w, id, DROP_V);
                    let l = c::len3(v);
                    let vz = v[2];
                    let mut v = c::set_len3(v, l * 0.95);
                    v = c::add(v, c::set_len3(c::sub(cc, pos), l * 0.05));
                    if c::DT * 10.0 < l {
                        v = c::add(v, c::set_len3(v, -(c::DT2 * 10.0)));
                    } else if l < c::DT * 5.0 {
                        v = c::add(v, c::set_len3(v, c::DT2 * 5.0));
                    }
                    if c::dec_timer_pvar_s16(w, id, DROP_HOLD) == 0 { v[2] = vz; }
                    c::set_pv4(w, id, DROP_V, v);
                    let p = c::add(pos, v);
                    c::set_pos(w, id, p);
                    c::dist3(p, cc) < c::DT * 10.0
                }
            }
        }
        Some(s) => {
            let n = count(w, s);
            if n == 0 { return; }
            let i = (c::pi32(w, id, ARR_IDX) + n + c::pu8(w, id, ARR_STEP) as i8 as i32) % n;
            let q = point(w, s, i);
            turn::spring_turn2_pvar(w, id, c::atan(q[0] - pos[0], q[1] - pos[1]), 0.01, 0.3, 0.1, TURN_V);
            if c::pi32(w, id, ARR_IDX) == n - 1 {
                true
            } else {
                let p = c::add(pos, c::set_len3(c::sub(q, pos), c::DT * 5.0));
                c::set_pos(w, id, p);
                if c::dist3(q, p) < 0.5 { c::set_pi32(w, id, ARR_IDX, i); }
                false
            }
        }
    };
    if done {
        blend_if(w, id, 0, 0, 5);
        set_st(w, id, 1);
    }
}

fn patrol_point(w: &World, id: MobyId, i: i32) -> Option<c::V> { spline(w, id, PATH).map(|s| point(w, s, i)) }

/// The outcome of [`try_fire`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fire {
    /// A condition failed (state 1 goes on to its hop check).
    No,
    /// All held but the muzzle is inside a wall: the cooldown restarts (60 ticks).
    Blocked,
    /// State 6.
    Fired,
}

/// The fire check of states 1 and 7: in range, level, drawn last frame (+0x31), cooled down, facing Ratchet within
/// 5°, the muzzle clear → state 6.
fn try_fire(w: &mut World, id: MobyId, t: c::V, alert: bool, moved: bool) -> Fire {
    let pos = c::pos(w, id);
    if range(w, id, alert) <= c::dist2(pos, t) { return Fire::No; }
    if 2.0 <= (pos[2] - t[2]).abs() || w.m(id).visible == 0 { return Fire::No; }
    if alert && moved { return Fire::No; }
    if c::pi32(w, id, FIRE_CD) != 0 { return Fire::No; }
    if DEG5 <= c::diff_rots(c::yaw(w, id), c::atan(t[0] - pos[0], t[1] - pos[1])) { return Fire::No; }
    let j = w.joint_point(id, 0);
    let a = [pos[0], pos[1], j[2], pos[3]];
    if w.line(pv(a), pv(j), 0, Some(id)).is_none() {
        c::set_pi16(w, id, SHOTS, 0);
        c::set_pf(w, id, AIM_DIST, 4.0);
        set_st(w, id, 6);
        blend_if(w, id, 4, 0, 6);
        c::set_pi32(w, id, FIRE_CD, 0);
        Fire::Fired
    } else {
        let cd = w.ticks(60);
        c::set_pi32(w, id, FIRE_CD, cd);
        Fire::Blocked
    }
}

/// The hop decision of state 1 (`LAB_002e7584`): to the picked point when it is more than 0.25 away (a one-way path
/// stops at its end).
fn hop_to(w: &mut World, id: MobyId, next: i32) {
    let Some(s) = spline(w, id, PATH) else { return };
    if c::dist3(c::pos(w, id), point(w, s, next)) <= 0.25 { return; }
    if c::pi16(w, id, ONE_WAY) != 0 && count(w, s) - 1 <= c::pi32(w, id, PT) { return; }
    blend_if(w, id, 8, 0, 10);
    c::set_pi32(w, id, PT, next);
    set_speed(w, id, JET_ANIM);
    set_st(w, id, 9);
}

/// State 6 at key 14: a glob from the muzzle (joint list 0) towards the aim point 4 ahead (the yaw, or the direction to
/// Ratchet when within 5 of it), unless the muzzle is inside a wall (then state 1).
fn fire(w: &mut World, id: MobyId, t: c::V) {
    let mut yaw = c::yaw(w, id);
    let j = w.joint_point(id, 0);
    c::set_pv4(w, id, AIM, j);
    let a = c::atan(t[0] - j[0], t[1] - j[1]);
    if c::diff_rots(a, yaw) < DEG5 { yaw = a; }
    let d = c::pf(w, id, AIM_DIST);
    let mut aim = j;
    aim[0] += yaw.cos() * d;
    aim[1] += yaw.sin() * d;
    c::set_pv4(w, id, AIM, aim);
    let v = c::set_len3(c::sub(aim, j), SHOT_SPEED * c::DT);
    let pos = c::pos(w, id);
    let from = [pos[0], pos[1], j[2], pos[3]];
    if w.line(pv(from), pv(j), 0, Some(id)).is_some() {
        blend_if(w, id, 0, 0, 5);
        set_st(w, id, 1);
        return;
    }
    let life = w.svc.timing.scale(to_pf(10.0)).to_f32() as i32;
    spawn_shot(w, id, j, v, life);
    let n = c::pi16(w, id, SHOTS) + 1;
    c::set_pi16(w, id, SHOTS, n);
}

/// State 10: the jet hop towards the patrol point (module doc).
fn hop(w: &mut World, id: MobyId) {
    let Some(s) = spline(w, id, PATH) else { return };
    let tgt = c::pi32(w, id, PT);
    let i = if c::pi16(w, id, MARKED) == 0 {
        tgt as i16
    } else {
        let pos = c::pos(w, id);
        let (mut best, mut k) = (1e11f32, 0i32);
        for n in 0..count(w, s) {
            let d = c::dist3(point(w, s, n), pos);
            if d < best {
                best = d;
                k = n;
            }
        }
        if tgt < k { (k - 1) as i16 } else if k < tgt { (k + 1) as i16 } else { tgt as i16 }
    };
    c::set_pi16(w, id, HOP_PT, i);
    let mut q = point(w, s, i as i32);
    q[2] += 0.8;
    let pos = c::pos(w, id);
    let h = c::atan(q[0] - pos[0], q[1] - pos[1]);
    turn::spring_turn2_pvar(w, id, h, c::DT2 * 2.0 * PI, c::DT2 * PI, c::DT * 2.0 * PI, TURN_V);
    let h = c::atan(q[0] - pos[0], q[1] - pos[1]);
    if c::diff_rots(c::yaw(w, id), h) < PI / 4.0 {
        let mut v = c::pf(w, id, JET_V);
        if v < JET * c::DT { v += JET * c::DT2; }
        c::set_pf(w, id, JET_V, v);
        let pitch = c::atan(c::dist2(pos, q), q[2] - pos[2]);
        let step = fx::polar(v, c::yaw(w, id), pitch);
        c::set_pos(w, id, c::add(pos, step));
        tilt(w, id, (v / (JET * c::DT)) * (PI / 6.0), c::DT * 1.5 * PI);
    }
    let mut q = point(w, s, tgt);
    q[2] += 0.8;
    if c::dist3(c::pos(w, id), q) < 0.25 { set_st(w, id, 11); }
}

/// The forward tilt (moby +0x44) spring of states 10 / 11.
fn tilt(w: &mut World, id: MobyId, target: f32, max: f32) {
    let mut v = c::pf(w, id, TILT_V);
    let cur = w.m(id).rotation[1];
    let r = turn::spring_turn(cur, target, c::DT2 * 2.0 * PI, c::DT2 * PI, max, &mut v);
    c::set_pf(w, id, TILT_V, v);
    w.mm(id).rotation[1] = r;
}

/// State 11: brake, turn to Ratchet, land (sequence 10) and go back to state 1 when it ends.
fn land(w: &mut World, id: MobyId, t: c::V) {
    let pos = c::pos(w, id);
    let f = if c::pf(w, id, JET_V) <= 0.0 || c::pi16(w, id, NO_BRAKE) != 0 {
        turn::spring_turn2_pvar(w, id, c::atan(t[0] - pos[0], t[1] - pos[1]), 0.02, 0.3, 0.1, TURN_V);
        c::set_pf(w, id, JET_V, 0.0);
        0.0
    } else {
        let v = c::pf(w, id, JET_V) - JET * 3.0 * c::DT2;
        c::set_pf(w, id, JET_V, v);
        let y = c::yaw(w, id);
        c::set_pos(w, id, c::add(pos, [y.cos() * v, y.sin() * v, 0.0, 0.0]));
        v / (JET * c::DT)
    };
    tilt(w, id, f * (PI / 6.0), c::DT * 2.0 * PI);
    let an = w.m(id).anim;
    let seq = if an.seq_b == 10 {
        an.seq_a
    } else if c::pf(w, id, JET_V) <= 0.0 {
        let p = c::pos(w, id);
        if c::diff_rots(c::yaw(w, id), c::atan(t[0] - p[0], t[1] - p[1])) < DEG2 {
            c::set_pf(w, id, JET_V, 0.0);
            blend_if(w, id, 10, 0, 3);
            return;
        }
        an.seq_a
    } else {
        an.seq_a
    };
    if seq == 10 && w.m(id).anim.flags & 2 != 0 {
        blend_if(w, id, 0, 0, 6);
        set_st(w, id, 1);
    }
}

/// `0x2e6518(moby, old)`: three jet-exhaust particles (type 22) from joint list 1, per particle
/// `randf(1, −1)·0.2·dt` ×2 and `randf(0.8, 1.2)·−7·dt` (rotated into the body, plus this tick's move), `randi(6)` ×2
/// colours, `randf(105000, 157500)` size, `ticks(rand_range(5, 15))` life.
/// The exhaust colours (level01 .data 0x20af50 / 0x20af68).
const EXHAUST_C1: [u32; 6] = [0x4f00_8fff, 0x4f00_8fff, 0x4f00_7fff, 0x4f00_6fff, 0x2fff_ffff, 0x2fff_ffff];
const EXHAUST_C2: [u32; 6] = [0x2f00_5f7f, 0x2f00_4f7f, 0x2f00_3f7f, 0x2f00_004f, 0x2f00_0000, 0x3f00_0000];

fn jet_exhaust(w: &mut World, id: MobyId, old: c::V) {
    for _ in 0..3 {
        let x = w.rng.randf(1.0, -1.0) * c::DT * 0.2;
        let y = w.rng.randf(1.0, -1.0) * c::DT * 0.2;
        let z = w.rng.randf(0.8, 1.2) * c::DT * -7.0;
        let j = w.joint_point(id, 1);
        let r = w.m(id).rows;
        let v = c::add(std::array::from_fn(|k| if k == 3 { 0.0 } else { r[0][k] * x + r[1][k] * y + r[2][k] * z }), c::sub(c::pos(w, id), old));
        let a = w.rng.randi(6);
        let b = w.rng.randi(6);
        let size = w.rng.randf(f32::from_bits(0x47cd_1400), f32::from_bits(0x4819_cf00));
        let n = w.rng.rand_range(5, 15);
        let life = w.ticks(n);
        let (c1, c2) = (EXHAUST_C1[a as usize], EXHAUST_C2[b as usize]);
        projectile::part22(w, &crate::particles::type22::Spawn { size, pos: j, vel: v, c1, c2, life });
    }
}

/// `0x2e6790(moby, &point)`: the patrol pick (module doc). Two-way paths: among the points before, at and after the
/// nearest (only the marked ones, w = 1, when the path has any), the one nearest Ratchet when alerted; else the one
/// turning the most away from him (< 90°), or one at least 12 away. One-way paths: the next marked point (or the end).
/// True when the pick is not the nearest point.
pub fn pick(w: &mut World, id: MobyId, idx: &mut i32) -> bool {
    let alert = alerted(w, id);
    let t = target::acquire(w, id, range(w, id, alert)).pos;
    let Some(s) = spline(w, id, PATH) else { return false };
    let n = count(w, s);
    if n == 0 { return false; }
    let pos = c::pos(w, id);
    let marked = c::pi16(w, id, MARKED) != 0;
    let ok = |k: i32| !marked || point(w, s, k)[3] == 1.0;
    if c::pi16(w, id, ONE_WAY) != 0 {
        loop {
            if *idx < n - 1 { *idx = (*idx + n + c::pu8(w, id, PT_STEP) as i8 as i32) % n; }
            if *idx == n - 1 || point(w, s, *idx)[3] == 1.0 { break; }
        }
        return true;
    }
    let (mut best, mut near, mut last, mut prev, mut next) = (10000.0f32, 0i32, 0i32, 0i32, 0i32);
    for k in 0..n {
        if !ok(k) { continue; }
        let d = c::dist2(point(w, s, k), pos);
        if d < best {
            prev = last;
            near = k;
            best = d;
        } else if last == near {
            next = k;
        }
        last = k;
    }
    if near == n - 1 { next = 0; } else if near == 0 { prev = n - 1; }
    let (mut bd, mut ba, mut chosen) = (if alert { 10000.0f32 } else { 0.0 }, 0.0f32, 0i32);
    let step = c::pu8(w, id, PT_STEP) as i8 as i32;
    *idx = 0;
    for _ in 0..n {
        let i = *idx;
        if (i == prev || i == near || i == next) && ok(i) {
            let e = point(w, s, i);
            if alert {
                let d = c::dist3(t, e);
                if d < bd {
                    bd = d;
                    chosen = i;
                }
            } else {
                let diff = c::diff_rots(c::atan(e[0] - pos[0], e[1] - pos[1]), c::atan(t[0] - pos[0], t[1] - pos[1]));
                let dist = c::dist3(pos, e);
                let take = if diff < PI / 2.0 {
                    ba < diff
                } else if 12.0 <= dist {
                    dist < bd || bd < 12.0
                } else {
                    bd < dist
                };
                if take {
                    bd = dist;
                    ba = diff;
                    chosen = i;
                }
            }
        }
        *idx = (i + n + step) % n;
        if *idx == 0 { break; }
    }
    *idx = chosen;
    chosen != near
}

// -------------------------------------------------------------------------------------------------
// The fire glob, class 722

/// `0x2fa068(shooter, pos, vel, life)`: the glob moby (half scale, random Euler: 3 × `rand_angle`), pvars +0x00
/// velocity, +0x10 the shooter, +0x14 life; ambient (0x7f, 0x7f, 0x7f); two glow sprites (type 26: 210000 /
/// 115500, colours 0x2f00007f / 0x4f004f7f, for its life).
pub fn spawn_shot(w: &mut World, shooter: MobyId, p: c::V, v: c::V, life: i32) -> Option<MobyId> {
    let m = w.create_moby(SHOT_CLASS)?;
    let a = w.rng.rand_angle();
    let b = w.rng.rand_angle();
    let cc = w.rng.rand_angle();
    let mo = w.mm(m);
    mo.visible = 1;
    mo.draw_dist = 0x7f;
    mo.update_dist = 0x7f;
    mo.scale *= 0.5;
    mo.ambient = [0x7f, 0x7f, 0x7f, mo.ambient[3]];
    mo.rotation = [a, b, cc, 0.0];
    mo.position = p;
    if mo.pvars.len() < 0x18 { mo.pvars.resize(0x80, 0); }
    c::set_pv4(w, m, 0, v);
    c::set_pi32(w, m, 0x10, shooter as i32);
    c::set_pi32(w, m, 0x14, life);
    projectile::part26(w, 210000.0, m, 0x2f00_007f, life);
    projectile::part26(w, 115500.0, m, 0x4f00_4f7f, life);
    w.build_matrix(m);
    Some(m)
}

/// `0x2fa4c0`: the glob's flight. Gravity `2/ticks(10)²` per tick, the move; outside the world's positive octant or
/// more than 64 (xy) from the camera it vanishes; while its life lasts the step is swept (the line, then a 0.5 sphere:
/// template flags 0x10001, type 1 / 1, damage 1, push (vel.xy, 1, exact)) and a hit bursts it at the hit point; at the
/// end of its life it bursts where it is.
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let old = c::pos(w, id);
    let k = w.svc.timing.scale(to_pf(10.0)).to_f32();
    let mut v = c::pv4(w, id, 0);
    v[2] -= ((k * 0.0 + 1.0) * 2.0) / (k * k);
    c::set_pv4(w, id, 0, v);
    let p = c::add(old, v);
    c::set_pos(w, id, p);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if p[0] < 0.0 || p[1] < 0.0 || p[2] < 0.0 || 64.0 < c::dist2(p, cam) {
        w.delete_moby(id);
        return;
    }
    let shooter = usize::try_from(c::pi32(w, id, 0x10)).ok().filter(|&s| s < w.table.mobys.len());
    let tmpl = projectile::template(w, id, [v[0], v[1], 1.0, crate::hero::damage::EXACT_PUSH_W], 0x1_0001, (1, 1), 1.0, 1);
    if c::dec_timer_pvar_i32(w, id, 0x14) == 0 {
        let Some(at) = projectile::sweep(w, old, p, 0, 0.5, shooter, &tmpl) else { return };
        c::set_pos(w, id, at);
    }
    let at = c::pos(w, id);
    shot_burst(w, 0.5, id, at, 0);
    w.delete_moby(id);
}

/// `0x2fa1b0(size, 0, moby, pos, sound)`: the death explosion's code with its own flashes (2·size red-violet for
/// `ticks(20)`, 1.5·size for `ticks(29)`) and no shake (`fx::SHOT_BURST`, docs/plan/explosions.md §C).
fn shot_burst(w: &mut World, size: f32, id: MobyId, p: c::V, sound: i32) {
    fx::spark_burst(w, &fx::SHOT_BURST, size, 0.0, Some(id), p, sound);
}
