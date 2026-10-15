//! U96 (census 2026-09-30): class 612, Aridia's flame-throwing sentries (level02 `0x2d7748`: 15 created instances,
//! the only copy). A creature of the shared layer (mode 0x20 header: damage record +0x20, flash +0x60, knockback +0x70,
//! walker +0xd0) that guards the centre of a cuboid (+0x188): asleep (8 / 9: smoking from its joint 0 every 4th tick)
//! or standing (10) until Ratchet comes within +0x184 of the post or enters the wake cuboid +0x18c, it rises (2 / 3:
//! onto the ground at the post), walks there (5) and guards it (7: a wander about it, 6), and when a target (Ratchet,
//! a decoy, a gold chicken; `0x261c80` = `0x274b78`, range 14) comes within +0x180 of the post it faces it (0xb, the
//! animation at double speed) and sprays fire (0xc: the flame of its own emitter `0x264e70`, range 4; the flames it
//! keeps hit every moby they touch: damage 1, flags 0x10001, type 5 / 1). Health 3: a hit of 1 or more knocks it back
//! (0x10); the last one blows it up (0x11: `SetDeathBits`, the beam explosion, five pieces 1751, 1752, 1753, 1769,
//! 1922). Its own flames do not hurt it (hits from class 612 are ignored).
//!
//! **Pvars** (0x240): +0x20 damage record (health, +0x28 column 2, +0x29 0, +0x30 1), +0x40 the walker's move, +0x48
//! the rise speed, +0x58 byte 15, +0x5a byte 8, +0x5c byte 1, +0x60 flash, +0x70 knockback, +0xd0 walker J (+0xd8 / +0xdc
//! probes 2 / 2, +0xf4 top speed 4·dt, +0x10c / +0x110 / +0x114 its turn 0.03 / 0.3 / 0.25), +0x180 the guard radius,
//! +0x184 the wake radius, +0x188 the post cuboid, +0x18c the wake cuboid, +0x194 the start mode (1 standing, 2 / 3
//! asleep), +0x1a0 the big-head cheat's, +0x1e0 the wander point, +0x1f0 the kept flames (seven records) and +0x20c
//! (s16) the smoke timer, +0x214 the turn velocity, +0x220 the target (the port stores moby + 1).
//!
//! The level02 words (gp = 0x166c00): gp−0x519c 10 / −0x5198 12 (the death flight out / up ×dt), −0x5194 / −0x5190 6 / 6
//! (the knockback's out / up ×dt), −0x518c / −0x5188 20 / 20 (drags ×dt²), −0x5184 5.5, −0x5180 4 (the flame's
//! range), −0x517c 25 (the gravities ×dt²), −0x5178 / −0x5174 0x40808080 (the smoke colour), −0x5170 / −0x516c 60 /
//! 120 (the smoke's life), −0x5168 1.5 (its drift ×dt), −0x5164 2 (the flights' air speed ×dt); the emitter's
//! gp−0x6b30.. (level02 0x1600d0: 16, 2, 2, 1, 3: the Pyrocitor's numbers).
//!
//! ## Coverage
//!
//! **The tick** `0x2d85b8`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `MobyGetHitMessage(m, 0x330000, 0)`; a hit from class 0x264 (its own) is dropped; `0x25cae0(…, col 4)` | | [`pre`] |
//! | a hit (out5 ≠ 1) out of 0x11: health −= damage; K radius `trunc(614.4)`, height 0.6 | | [`pre`] |
//! | health left, damage < 1: flash 100; ≥ 1: flash 0x78, K gravity 25·dt², drag 20·dt², out 6·dt, up 6·dt, bounce 0.5, flags 0x19, air speed 2·dt, +0xad 0; `0x25eb80(atan(pos − Ratchet), m, K, 0xb, 1, 0)`; keys 6.5 / 11; 0x10 | the knockback (away from Ratchet, not along the hit) | [`pre`] (`knock::start`) |
//! | none left: K flags 9, gravity 25·dt², drag 20·dt², out 10·dt, up 12·dt, air 2·dt; start (0xe); keys 6.5 / 13; untargetable; flash 0xfa; 0x11 | the death flight | [`pre`] |
//! | `0x25fa80` flash start; +0xa4 = 0xff | | [`pre`] (`flash::start`) |
//! | the target kept while flaming (0xc) when it is Ratchet (class 0), a gold chicken 0x10e or a decoy 0xcb and alive; else `0x261c80(14, m, &rec)` (= `0x274b78`); none → Ratchet | the target | [`pre`] (`target::acquire`) |
//! | `0x25fb60` flash update | | [`pre`] |
//!
//! **The update** `0x2d7748`:
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x264dd0(2.5, m, 1, +0x1a0)` (= `0x278720`) | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn within 30 of the camera: `0x25c788` shadow probe, +0x7f = 0x18 | | [`update`] (`shadows::probe_down`) |
//! | 0: `rand() & 1` → mirror; targetable; +0x58 15, +0x30 1, +0x5c 1, +0x5a 8, +0x29 0; Ratchet's light word and ambient (+0x38); `SeedJumpPattern(J)`; column 2; J turn 0.03 / 0.3 / 0.25, top 4·dt, probes 2 / 2; health 3; +0x40 zeroed; the flames cleared (`0x264e40`); mode 1 → 10 (blend 0), 2 → 8 (blend 2), 3 → 9 (blend 0xf), else 7 (blend 0) | init | [`init`] |
//! | 2: wrapped → 3 (blend 4) | wake | [`update`] |
//! | 3: `SpringTurn2(atan(target − pos), 0.02, 0.3, π/2·dt)`; toward the post (xy, at most 6·dt a tick, `0x2616f8` = `ClampLen`); the rise speed → 40·dt by 25·dt²; z → the ground by it; on the ground → 4 (blend 5 at frame 4) | rise | [`rise`] |
//! | 4: wrapped → 5 (blend 6) | | [`update`] |
//! | 5: `0x25b710(m, J, post, +0x40)` (= `0x26de80`) arrived (exactly 4) → 7 (blend 0, 30) | walk to the post | [`update`] (`walker::walk_to`) |
//! | 6: walk to the wander point; bits 6 → 7 (blend 0); the target within the guard radius of the post and 4 in z → 5 (blend 6) | wander | [`update`] |
//! | 7: the target within the guard radius and 4 in z: at the post (within 1) turn to it (0.02, 0.3, 0.1), facing within 5° → 0xb (blend 8); away from the post → 5 (blend 6); else, wrapped: a wander point `randf(1, 2)` off the post along `rand_angle` → 6 (blend 6) | guard | [`guard`] |
//! | 8 / 9: every 4th tick while drawn: smoke from joint 0 (`PartType21Spawn(40000, p, randf(0, 1.5)·dt along rand_angle, 0x40808080 ×2, rr(ticks(60), ticks(120)), 1)`); then as 10 | asleep | [`asleep`] |
//! | 10: no wake cuboid: Ratchet within the wake radius of the post; else Ratchet's ground point (0x13f5f0) in the cuboid → 8: blend 4, 9: blend 0x10, then 2; 10 → 5 (blend 6) | wake up | [`asleep`] |
//! | 0xb: anim speed 2; turn to the target (0.02, 0.3, 0.1); wrapped → speed 1, 0xc (blend 9) | aim | [`update`] |
//! | 0xc: anim speed 0.5; the flame from joint 0 along (cos a, sin a, −0.1), a = the heading of joint 0 from the moby (`0x264e70`, range 4); the template `0x25bf70(1, tmpl, m, 0x10001, (cos a, sin a, 1, 5627.97))` type 5 / 1, the class; the kept flames' hits `0x2651d0`; wrapped → speed 1, 0xd, `randi(255) & 1` ? blend 10 : blend 8 | flame | [`flame`] |
//! | 0xd: turn to the target; wrapped: beyond the guard radius + 1 (xy) or 3 in z → 7 (blend 0); else 0xc (blend 9) | between bursts | [`update`] |
//! | 0x10: `0x25ecc0` & 0x40 → 7 (blend 0) | knocked back | [`update`] (`knock::update`) |
//! | 0x11: & 0x140 → `SetDeathBits(m, 0, −1)`; `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, +0x40, pos + 1 z, 5, 2, 4, −1, 1, 1, −1, 0)`; `BreakFxB` 0x6d7, 0x6d8, 0x6d9, 0x6e9, 0x782 (zero vectors); `DeleteMoby` | the death | [`update`] (`fx::beam_explosion`, `fx::break_piece`) |
//! | every tick but the death: `0x25c788` shadow probe | | [`update`] |
//!
//! **The emitter** `0x264e70(range, flames, start, dir)` (level02; the Pyrocitor's flame with its own numbers): the
//! length along `dir` (a `CollLine_Fix` to `range + 0.75`, flags 2: the hit less 0.75, at least 0), the launch speed
//! (`0x25df98` = `0x270830`: `crate::hero::pyrocitor::launch_speed`), 2 glow puffs (1 when shorter than 4) and 2
//! flames of type 12 a tick, each `rand_vec(0, (3 − 2·len/range)·dt)` + the launch step, at `randf(0, 1)` of the step;
//! every 4th tick the third is kept (the first of seven free slots); the smoke timer (s16 +0x1c) → `randf(10, 30)`
//! ticks, a type-25 spark from `start` along `step·randf(0.5, 0.1) + rand_vec(0, 2·dt)`. `creature::flame::emit` (shared
//! with Kalebo's 541: level16's copy `0x25eab8` is the same code and numbers).
//! **The hits** `0x2651d0(flames, m, tmpl)`: each kept record still a type-12 particle and alive hits a sphere of
//! `size / 210000 / 4` at its position (`coll_sphere_mobys(…, 0, m, tmpl)`), dropped below life 5; others dropped.
//! (`creature::flame::hits`). **The clear** `0x264e40` (`creature::flame::clear`).
//!
//! **The reaction table** (level02 0x1fb6ac): the default wrappers: no Suck Cannon reaction.
//!
//! **Not the game's, noted [L]:** without a particle system (a headless world) the kept flames are no records and hit
//! nothing. Native `f32`; the rand draws at the game's points (the type-12 / type-25 spawners' own draws only with a
//! free record, as the game's).

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, damage, flame, flash, fx, ground, knock, target, turn, walker};
use crate::moby_update::services::{pv as v4, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_FN: u32 = 0x2d_7748;
pub const CLASSES: [i16; 1] = [612];
/// The flame's joint (list 0).
pub const JOINTS: [i16; 1] = [612];

/// Pvar offsets (module doc).
pub mod pv {
    pub const D: usize = 0x20;
    pub const MOVE: usize = 0x40;
    pub const RISE_V: usize = 0x48;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const GUARD_R: usize = 0x180;
    pub const WAKE_R: usize = 0x184;
    pub const POST: usize = 0x188;
    pub const WAKE_CUBOID: usize = 0x18c;
    pub const MODE: usize = 0x194;
    pub const WANDER: usize = 0x1e0;
    pub const FLAMES: usize = 0x1f0;
    pub const SMOKE_T: usize = 0x20c;
    pub const TURN_V: usize = 0x214;
    pub const TGT_MOBY: usize = 0x220;
    pub const SIZE: usize = 0x224;
}

/// The states (module doc).
pub mod st {
    pub const INIT: u8 = 0;
    pub const WAKE: u8 = 2;
    pub const RISE: u8 = 3;
    pub const LANDED: u8 = 4;
    pub const TO_POST: u8 = 5;
    pub const WANDER: u8 = 6;
    pub const GUARD: u8 = 7;
    pub const ASLEEP: u8 = 8;
    pub const ASLEEP_B: u8 = 9;
    pub const STANDING: u8 = 10;
    pub const AIM: u8 = 0xb;
    pub const FLAME: u8 = 0xc;
    pub const PAUSE: u8 = 0xd;
    pub const KNOCKED: u8 = 0x10;
    pub const DYING: u8 = 0x11;
}

/// The flame's range (gp−0x5180).
pub const RANGE: f32 = 4.0;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn blend(w: &mut World, id: MobyId, seq: u8, n: i32) {
    let t = w.ticks(n);
    c::blend_to(w, id, seq, 0, t);
}
fn wrapped(w: &World, id: MobyId) -> bool { w.m(id).anim.flags & 2 != 0 }
fn target_moby(w: &World, id: MobyId) -> Option<MobyId> {
    let v = c::pi32(w, id, pv::TGT_MOBY);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn tpos(w: &World, id: MobyId) -> c::V { target_moby(w, id).map(|m| c::pos(w, m)).unwrap_or([0.0; 4]) }
/// The post: the centre of cuboid +0x188 (`0x1600ec[i] + 0x30`).
fn post(w: &World, id: MobyId) -> c::V {
    let i = c::pi32(w, id, pv::POST);
    let p = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).map(|s| s.centre()).unwrap_or([0.0; 3]);
    [p[0], p[1], p[2], 1.0]
}
fn spring_to(w: &mut World, id: MobyId, t: c::V, max: f32) -> f32 {
    let p = c::pos(w, id);
    let a = c::atan(t[0] - p[0], t[1] - p[1]);
    turn::spring_turn2_pvar(w, id, a, f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3e99_999a), max, pv::TURN_V);
    a
}

/// `0x2d85b8`: the tick (module doc).
fn pre(w: &mut World, id: MobyId) {
    let mut hit = w.get_hit(id, 0x33_0000, false);
    if hit.and_then(|h| h.attacker).is_some_and(|a| w.m(a).o_class == 0x264) { hit = None; }
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let kr = pv::K;
        c::set_pi32(w, id, kr + knock::k::RADIUS, f32::from_bits(0x4419_999a) as i32);
        c::set_pf(w, id, kr + knock::k::ZOFF, f32::from_bits(0x3f19_999a));
        let hp3 = crate::moby_update::classes::units::hero_pos(w);
        let p = c::pos(w, id);
        let away = c::atan(p[0] - hp3[0], p[1] - hp3[1]);
        if 0.0 < hp {
            if res.damage < 1.0 {
                c::set_pu8(w, id, pv::FLASH + 7, 100);
            } else {
                c::set_pu8(w, id, pv::FLASH + 7, 0x78);
                c::set_pf(w, id, kr + knock::k::GRAVITY, 25.0 * c::DT2);
                c::set_pf(w, id, kr + knock::k::DRAG, 20.0 * c::DT2);
                c::set_pf(w, id, kr + knock::k::BOUNCE, 0.5);
                c::set_pf(w, id, kr + knock::k::UP, 6.0 * c::DT);
                c::set_pi32(w, id, kr + knock::k::FLAGS, 0x19);
                c::set_pf(w, id, kr + knock::k::SPEED, 6.0 * c::DT);
                c::set_pf(w, id, kr + knock::k::AIR_SPEED, 2.0 * c::DT);
                c::set_pu8(w, id, kr + 0x3d, 0);
                knock::start(w, id, kr, away, 0xb, 1, 0);
                c::set_pf(w, id, kr + knock::k::KEY_APEX, 6.5);
                c::set_pf(w, id, kr + knock::k::KEY_LAND, 11.0);
                set_state(w, id, st::KNOCKED);
            }
        } else {
            c::set_pi32(w, id, kr + knock::k::FLAGS, 9);
            c::set_pf(w, id, kr + knock::k::GRAVITY, 25.0 * c::DT2);
            c::set_pf(w, id, kr + knock::k::DRAG, 20.0 * c::DT2);
            c::set_pf(w, id, kr + knock::k::SPEED, 10.0 * c::DT);
            c::set_pf(w, id, kr + knock::k::UP, 12.0 * c::DT);
            c::set_pf(w, id, kr + knock::k::AIR_SPEED, 2.0 * c::DT);
            c::set_pu8(w, id, kr + 0x3d, 0);
            knock::start(w, id, kr, away, 0xe, 1, 0);
            c::set_pf(w, id, kr + knock::k::KEY_APEX, 6.5);
            c::set_pf(w, id, kr + knock::k::KEY_LAND, 13.0);
            w.mm(id).mode &= !mode::TARGETABLE;
            c::set_pu8(w, id, pv::FLASH + 7, 0xfa);
            set_state(w, id, st::DYING);
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    let keep = target_moby(w, id).is_some_and(|t| {
        let m = w.m(t);
        m.state < 0x80 && matches!(m.o_class, 0 | 0x10e | 0xcb)
    }) && state(w, id) == st::FLAME;
    if !keep {
        let t = target::acquire(w, id, 14.0);
        c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let hm = w.hero_moby;
        c::set_pi32(w, id, pv::TGT_MOBY, hm.map_or(0, |m| m as i32 + 1));
    }
    flash::update(w, id, pv::FLASH);
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    let r = w.rng.rand();
    let m = w.mm(id);
    if r & 1 != 0 { m.mode |= mode::MIRROR; }
    m.mode |= mode::TARGETABLE;
    c::set_pu8(w, id, 0x58, 0xf);
    c::set_pf(w, id, pv::D + 0x10, 1.0);
    c::set_pu8(w, id, 0x5c, 1);
    c::set_pu8(w, id, 0x5a, 8);
    c::set_pu8(w, id, pv::D + 9, 0);
    crate::moby_update::classes::units::take_hero_light(w, id);
    walker::seed(&mut w.mm(id).pvars, pv::J);
    c::set_pu8(w, id, pv::D + 8, 2);
    c::set_pf(w, id, pv::J + 0x3c, f32::from_bits(0x3cf5_c28f));
    c::set_pf(w, id, pv::J + 0x40, f32::from_bits(0x3e99_999a));
    c::set_pf(w, id, pv::J + 0x44, 0.25);
    c::set_pf(w, id, pv::J + 0x24, c::DT * 4.0);
    c::set_pf(w, id, pv::J + 8, 2.0);
    c::set_pf(w, id, pv::D, 3.0);
    c::set_pf(w, id, pv::J + 0xc, 2.0);
    c::set_pv4(w, id, pv::MOVE, [0.0; 4]);
    flame::clear(w, id, pv::FLAMES);
    match c::pi32(w, id, pv::MODE) {
        1 => { c::blend_to(w, id, 0, 0, 0); set_state(w, id, st::STANDING); }
        2 => { c::blend_to(w, id, 2, 0, 0); set_state(w, id, st::ASLEEP); }
        3 => { c::blend_to(w, id, 0xf, 0, 0); set_state(w, id, st::ASLEEP_B); }
        _ => { c::blend_to(w, id, 0, 0, 0); set_state(w, id, st::GUARD); }
    }
}

/// State 3: rise onto the ground at the post.
fn rise(w: &mut World, id: MobyId) {
    let t = tpos(w, id);
    spring_to(w, id, t, c::DT * std::f32::consts::FRAC_PI_2);
    let mut v = c::sub(post(w, id), c::pos(w, id));
    v[2] = 0.0;
    v = c::clamp_len3(v, c::DT * 6.0);
    let mut p = c::add(c::pos(w, id), v);
    c::set_pos(w, id, p);
    let g = ground::ground(w, p, 0.5, 0).z;
    let mut rv = c::pf(w, id, pv::RISE_V);
    turn::approach(c::DT * 40.0, 25.0 * c::DT2, &mut rv);
    c::set_pf(w, id, pv::RISE_V, rv);
    turn::approach(g, rv, &mut p[2]);
    c::set_pos(w, id, p);
    if p[2] == g {
        set_state(w, id, st::LANDED);
        if w.m(id).anim.seq_b != 5 {
            let t = w.ticks(10);
            w.anim_blend(id, 5, 4, t);
        }
    }
}

/// State 7: guard the post (module doc).
fn guard(w: &mut World, id: MobyId) {
    let (t, c0) = (tpos(w, id), post(w, id));
    if c::dist3(t, c0) < c::pf(w, id, pv::GUARD_R) && (t[2] - c::pos(w, id)[2]).abs() < 4.0 {
        if c::dist3(c::pos(w, id), c0) < 1.0 {
            let a = spring_to(w, id, t, f32::from_bits(0x3dcc_cccd));
            if c::diff_rots(c::yaw(w, id), a) < f32::from_bits(0x3db2_b8c2) {
                set_state(w, id, st::AIM);
                blend(w, id, 8, 6);
            }
        } else {
            set_state(w, id, st::TO_POST);
            blend(w, id, 6, 10);
        }
        return;
    }
    if !wrapped(w, id) { return; }
    let a = w.rng.rand_angle();
    let r1 = w.rng.randf(1.0, 2.0);
    let x = a.cos() * r1;
    let r2 = w.rng.randf(1.0, 2.0);
    let y = a.sin() * r2;
    let old_w = c::pf(w, id, pv::WANDER + 0xc);
    c::set_pv4(w, id, pv::WANDER, c::add([x, y, 0.0, old_w], c0));
    set_state(w, id, st::WANDER);
    blend(w, id, 6, 10);
}

/// States 8 / 9 / 10: asleep or standing, waiting for Ratchet (module doc).
fn asleep(w: &mut World, id: MobyId) {
    let s = state(w, id);
    if s != st::STANDING && w.counter & 3 == 0 && w.m(id).visible != 0 {
        let f = w.rng.randf(0.0, 1.5) * c::DT;
        let a = w.rng.rand_angle();
        let v = [a.cos() * f, a.sin() * f, 0.0, 0.0];
        let p = w.joint_point(id, 0);
        let lo = w.ticks(0x3c);
        let hi = w.ticks(0x78);
        let life = w.rng.rand_range(lo, hi);
        *w.svc.fx.part_spawns.entry(21).or_default() += 1;
        if let Some(sys) = w.particles.as_deref_mut() {
            if crate::particles::type21::spawn_rng(sys, w.rng, 40000.0, p, v, 0x4080_8080, 0x4080_8080, life, 1).is_none() { w.svc.fx.part_failed += 1; }
        }
    }
    let hp = crate::moby_update::classes::units::hero_pos(w);
    let cub = c::pi32(w, id, pv::WAKE_CUBOID);
    let wake = if cub < 0 {
        c::dist3(hp, post(w, id)) < c::pf(w, id, pv::WAKE_R)
    } else {
        let g = w.hero.ground_point.map(|x| f32::from_bits(x.0));
        w.in_cuboid([g[0], g[1], g[2]], cub)
    };
    if !wake { return; }
    match s {
        st::ASLEEP => blend(w, id, 4, 10),
        st::ASLEEP_B => blend(w, id, 0x10, 10),
        _ => {
            set_state(w, id, st::TO_POST);
            blend(w, id, 6, 6);
            return;
        }
    }
    set_state(w, id, st::WAKE);
}

/// The hit template of the flame (`0x25bf70(1, tmpl, m, 0x10001, dir)`, type 5 / 1, the class).
fn template(w: &World, id: MobyId, dir: c::V) -> HitTemplate {
    HitTemplate { dir: v4(dir), attacker: Some(id), flags: 0x1_0001, b18: 5, b19: 1, h1a: w.m(id).o_class as u16, damage: Pf::ONE, w20: 1 }
}

/// State 0xc: the flame (module doc).
fn flame(w: &mut World, id: MobyId) {
    w.mm(id).anim.speed = 0.5;
    let j = w.joint_point(id, 0);
    let p = c::pos(w, id);
    let a = c::atan(j[0] - p[0], j[1] - p[1]);
    let (ca, sa) = c::cs(a);
    let mut dir = [ca, sa, -0.1, 0.0];
    flame::emit(w, id, pv::FLAMES, RANGE, j, dir);
    dir[3] = f32::from_bits(0x45af_df66);
    dir[2] = 1.0;
    let t = template(w, id, dir);
    flame::hits(w, id, pv::FLAMES, &t);
    if !wrapped(w, id) { return; }
    w.mm(id).anim.speed = 1.0;
    set_state(w, id, st::PAUSE);
    if w.rng.randi(0xff) & 1 == 0 { blend(w, id, 8, 6); } else { blend(w, id, 10, 6); }
}

/// `0x2d7748`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    // 0x264dd0(2.5, m, 1, +0x1a0): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.5, id, 1, id, 0x1a0);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 30.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x18;
        }
    }
    pre(w, id);
    match state(w, id) {
        st::INIT => init(w, id),
        st::WAKE => {
            if wrapped(w, id) {
                set_state(w, id, st::RISE);
                c::blend_to(w, id, 4, 0, 0);
            }
        }
        st::RISE => rise(w, id),
        st::LANDED => {
            if wrapped(w, id) {
                set_state(w, id, st::TO_POST);
                blend(w, id, 6, 6);
            }
        }
        st::TO_POST => {
            let c0 = post(w, id);
            let mut out = [0.0; 4];
            let r = walker::walk_to(w, id, pv::J, c0, &mut out);
            c::set_pv4(w, id, pv::MOVE, out);
            if r == 4 {
                set_state(w, id, st::GUARD);
                blend(w, id, 0, 30);
            }
        }
        st::WANDER => {
            let g = c::pv4(w, id, pv::WANDER);
            let mut out = [0.0; 4];
            let r = walker::walk_to(w, id, pv::J, g, &mut out);
            c::set_pv4(w, id, pv::MOVE, out);
            if r & 6 != 0 {
                set_state(w, id, st::GUARD);
                blend(w, id, 0, 10);
            } else {
                let (t, c0) = (tpos(w, id), post(w, id));
                if c::dist3(t, c0) < c::pf(w, id, pv::GUARD_R) && (t[2] - c::pos(w, id)[2]).abs() < 4.0 {
                    set_state(w, id, st::TO_POST);
                    blend(w, id, 6, 10);
                }
            }
        }
        st::GUARD => guard(w, id),
        st::ASLEEP | st::ASLEEP_B | st::STANDING => asleep(w, id),
        st::AIM => {
            w.mm(id).anim.speed = 2.0;
            let t = tpos(w, id);
            spring_to(w, id, t, f32::from_bits(0x3dcc_cccd));
            if wrapped(w, id) {
                w.mm(id).anim.speed = 1.0;
                set_state(w, id, st::FLAME);
                blend(w, id, 9, 6);
            }
        }
        st::FLAME => flame(w, id),
        st::PAUSE => {
            let t = tpos(w, id);
            spring_to(w, id, t, f32::from_bits(0x3dcc_cccd));
            if wrapped(w, id) {
                let far = c::pf(w, id, pv::GUARD_R) + 1.0 <= c::dist2(c::pos(w, id), t) || 3.0 <= (t[2] - c::pos(w, id)[2]).abs();
                if far {
                    set_state(w, id, st::GUARD);
                    blend(w, id, 0, 6);
                } else {
                    set_state(w, id, st::FLAME);
                    blend(w, id, 9, 6);
                }
            }
        }
        st::KNOCKED => {
            if knock::update(w, id, pv::K) & 0x40 != 0 {
                set_state(w, id, st::GUARD);
                blend(w, id, 0, 6);
            }
        }
        st::DYING if knock::update(w, id, pv::K) & 0x140 != 0 => {
            let p = c::pos(w, id);
            set_death_bits(w, id, 0, -1);
            fx::beam_explosion(w, &crate::moby_update::classes::units::aridia_sandshark::NEST_BLAST, Some(id), [p[0], p[1], p[2] + 1.0, p[3]]);
            let rot = w.m(id).rotation;
            for class in [0x6d7, 0x6d8, 0x6d9, 0x6e9, 0x782] { fx::break_piece(w, id, class, p, rot, 0, 0); }
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    crate::shadows::probe_down(w, id);
}
