//! Oltanis's pop-up turrets, class 30: level14 0x2b3bf0 (census U440; 27 created instances), with their private hit
//! handler `0x2b4340`, and the **shot** 681 (spawner `0x2eccf0`, update `0x2ece00`; created by code, so the census
//! never saw it). A turret sits sunk 0.7 below its placed height, collision and targeting off; told to rise (+0x88 ≠ 0,
//! written by other code) and its re-arm timer run out, it rises in one second (sequence 1, then 2), and waits. The
//! target (Ratchet, or a decoy) within 34 and inside its waking cuboid wakes it (sequence 3): it sweeps its aim ±1° a
//! tick along its aiming cuboid's row 1 and fires a shot from one of three muzzles every 4th tick, until the target
//! has been gone two seconds at the end of a cycle. Weapon hits (not its own class's, not its shots') take health
//! from its damage record; at 0 it blows up (bolts / save bit, the beam explosion). Read from the level14 decomp and
//! the disassembly of the explosion call (0x2b42b0) and the shot's line test (0x2ece48). Native `f32`.
//!
//! **Pvar block**: +0x20 the damage record (health +0x20), +0x60 the flash record (+0x67 the red), +0x70 home z,
//! +0x74 s16 the phase timer, +0x76 s16 the wake timer, +0x78 1 / rise ticks, +0x80 the aiming cuboid, +0x84 home
//! yaw, +0x88 s16 raise, +0x8a u8 started up, +0x8b u8 woken flag, +0x8c the sweep, +0x90 the waking cuboid, +0x94
//! the re-arm ticks, +0x98 s32 the re-arm timer, +0x9c s16 the fire delay, +0x9e s16 the sweep direction.
//!
//! ## Coverage: the turret 30 (`0x2b3bf0`, `0x2b4340`)
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2b4340` | `MobyGetHitMessage(m, 0x330000, 0)`; a hit whose attacker is a class-30 or a 681 is dropped; `0x26f378(m, hit, +0x20, 0, &out, &dmg, 0, 4)` | [`hits`] (`damage::resolve`) |
//! | | out ≠ 1, state ≠ 6: health −= dmg (≤ 0 → reaction 1); 1 / 2: → 6, mode & ~0x1000, red 0xf0, `SetDeathBits(m, 0, −1)` (0x26c250); 3–8: red 200; 9 / 10: red 100; the flash `0x272318` | [`hits`] (`crate_::set_death_bits`, `flash::start`) |
//! | | +0xa4 = 0xff; every tick the flash `0x2723f8` | [`hits`] (`flash::update`) |
//! | state 0 | home z, home yaw, sweep 0, timer 0; no aiming / waking cuboid → a debug `printf`, `DeleteMoby`; wake timer ticks(300) (gp−0x56cc); +0x8b 0; raise 0 → 1 (collision off, z −= 0.7 (gp−0x56f8), +0x8a 0, mode & ~0x1000), else → 3 (+0x8a 1); re-arm timer = ticks(+0x94) | [`update`] |
//! | state 1 | raise ≠ 0 and the re-arm timer done → 2: timer ticks(60) (gp−0x56f4), +0x78 = 1/timer, `moby_change_sequence(m, 1, 0)`, anim speed 1/(timer·0.08333) (gp−0x56f0) | [`update`] |
//! | state 2 | z = home − 0.7·timer·+0x78; sequence 1 wrapped → sequence 2, speed 1; timer < ticks(30): collision on (class +0x10), mode \| 0x1000; timer done → wake timer ticks(300), → 3, timer ticks(60), z = home, sweep 0 | [`update`] |
//! | state 3 | `0x274b78(34, m)` not kind 2 and its point in the waking cuboid (`PointInCuboid`) → 4, sequence 3, timer ticks(120), fire delay ticks(20); else not started up: the wake timer done → +0x8b = 1 | [`update`] (`target::acquire`, `triggers::point_in_cuboid`) |
//! | state 4 | sweep += ±1° (gp−0x56ec; −1° with +0x9e ≠ 0), aim = sweep + home yaw·π/180 (sic: a radian yaw scaled again); yaw = atan to row 1 · sin(aim) + centre of the aiming cuboid; fire delay −= 1 | [`update`] |
//! | | every 4th tick (gp−0x56d0, 0x15f5cc): `randi(3)` muzzle: row 2·0.55, row 2·0.525 ± row 1·0.1 (gp−0x56e0..−0x56d4), + row 0·2 + position; velocity row 0·40·dt (gp−0x56e4); fire delay 0 → the shot (`0x2eccf0(p, v, m, 1)`) with ambient (0xe0, 0xe0, 0xe0) (0x2650d0) | [`update`], [`spawn_shot`] |
//! | | the target lost (kind 2 or outside the cuboid), the timer done, the animation wrapped in sequence 3 → wake timer ticks(300), → 3, sequence 2, speed 1 | [`update`] |
//! | state 5 (entered by other code) | f = timer·+0x78: z = home − (0.7 − 0.7·f), yaw = home + (sweep − home)·f (sic); sequence 4 wrapped → sequence 0, speed 1; timer < ticks(30): collision off; done → 1, yaw home, timer ticks(60), raise 0, re-arm timer ticks(+0x94), mode & ~0x1000, z = home − 0.7 | [`update`] |
//! | state 6 | `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, m, +0x40, pos, 5, 2, 4, sound 0, shake, debris 1, −1, 0)` (level14 0x2667d0), `DeleteMoby` | [`update`] (`fx::beam_explosion`) |
//! | `0x2b44a8` / `0x2b4670` / `0x2b4500` | the sentries' group calls: raise the sunk ones; any raised (or none alive); sink the risen ones once the rest are down (state 5's entry: sequence 4, timer ticks(60) (gp−0x56f4), speed 1/(timer·0.08333) (gp−0x56f0), sweep = yaw) | [`raise_group`], [`any_raised`], [`lower_group`] |
//!
//! ## Coverage: the shot 681 (`0x2eccf0`, `0x2ece00`)
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2eccf0` | `CreateMoby(0x2a9)`; Euler (0, −atan(\|v.xy\|, v.z), atan(v)); draw distance 0x7f, update 0xff, drawn; pos; +0x10 start, +0x00 velocity; blend 1 (`ticks(5)`); mode 0x208; +0x24 the flag (1), +0x20 the shooter; alpha 0x10; the matrix | [`spawn_shot`] |
//! | `0x2ece00` | pos += velocity; the template `0x261e80(1, …, shot, 0x10001, (v.xy normalised, 1))`, type bytes 1 / 1 (+0x1a is not written: 0 here [L]) | [`shot_update`] |
//! | | xy beyond 30 of the start: drawn → three type-51 puffs (level14 `0x279aa0` = level13 `0x27d8d0`); deleted | [`shot_update`] (`particles::type51`) |
//! | | else up to three `CollLine_Fix(old, new, +0x24 = 0, ignore)`: a hit moby of class 0xfc is ignored and the line retried; any other hit: the line again with the template (the hit), the shot at the point, drawn → five impact sparks off the face (type 27), class sound 0, deleted | [`shot_update`] (`services::line_hit_in`, `blaster_shot::impact_sparks`) |
//! | | outside [2, 1021]³ → deleted | [`shot_update`] |
//! | | no light or save flag | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{self as c, damage, flash, fx, target};
use crate::moby_update::services::{HitTemplate, World};
use crate::moby_update::triggers::point_in_cuboid;
use crate::ps2v::Pf;

/// The updates in the level14 class table.
pub const UPDATE_FN: u32 = 0x2b_3bf0;
pub const SHOT_FN: u32 = 0x2e_ce00;
pub const REFERENCE_LEVEL: u32 = 14;
pub const CLASSES: [i16; 1] = [30];
pub const SHOT_CLASSES: [i16; 1] = [681];
/// The class whose line hits the shot passes through (0xfc).
pub const PASS_CLASS: i16 = 0xfc;

/// Pvar offsets.
pub mod pv {
    pub const RECORD: usize = 0x20;
    pub const FLASH: usize = 0x60;
    pub const HOME_Z: usize = 0x70;
    pub const TIMER: usize = 0x74;
    pub const WAKE: usize = 0x76;
    pub const INV: usize = 0x78;
    pub const AIM: usize = 0x80;
    pub const HOME_YAW: usize = 0x84;
    pub const RAISE: usize = 0x88;
    pub const STARTED: usize = 0x8a;
    pub const WOKEN: usize = 0x8b;
    pub const SWEEP: usize = 0x8c;
    pub const WAKE_CUBOID: usize = 0x90;
    pub const REARM: usize = 0x94;
    pub const REARM_T: usize = 0x98;
    pub const FIRE: usize = 0x9c;
    pub const DIR: usize = 0x9e;
    pub const SIZE: usize = 0xa0;
}

/// The shot's pvars.
pub mod sv {
    pub const VEL: usize = 0x00;
    pub const START: usize = 0x10;
    pub const SHOOTER: usize = 0x20;
    pub const FLAG: usize = 0x24;
}

/// The level14 `$gp` words (gp = 0x166c00).
pub mod k {
    /// gp−0x56f8: the sink depth.
    pub const SINK: f32 = 0.7;
    /// gp−0x56f4: the rise ticks.
    pub const RISE: i32 = 60;
    /// gp−0x56f0: the rise animation's rate factor.
    pub const RISE_RATE: f32 = f32::from_bits(0x3daa_a8eb);
    /// gp−0x56ec: the sweep step (1°).
    pub const SWEEP: f32 = 0.017_453_3;
    /// gp−0x56e4: the shot speed (· dt).
    pub const SHOT_SPEED: f32 = 40.0;
    /// gp−0x56e0 / −0x56dc / −0x56d8 / −0x56d4: the muzzle offsets.
    pub const MUZZLE_A: f32 = 0.55;
    pub const MUZZLE_B: f32 = 0.525;
    pub const MUZZLE_FWD: f32 = 2.0;
    pub const MUZZLE_SIDE: f32 = 0.1;
    /// gp−0x56d0: the ticks between shots.
    pub const FIRE_EVERY: u64 = 4;
    /// gp−0x56cc: the wake timer.
    pub const WAKE: i32 = 300;
    /// The target search range (0x42080000).
    pub const RANGE: f32 = 34.0;
    /// The shot's reach (xy).
    pub const REACH: f32 = 30.0;
}

/// State 6's blast.
pub const BLAST: fx::Beam = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: 0, shake: true };

/// `moby_change_sequence(m, seq, 0)` (0x26c5a8 on level 01).
fn cut(w: &mut World, id: MobyId, seq: u8) {
    let o = w.m(id).o_class;
    let Some(class) = w.classes.anim(o) else { return };
    let class = class.clone();
    if rc_formats::moby_anim::hard_cut(&mut w.mm(id).anim, &class, seq, 0) {
        crate::moby_update::anim_sound::after_sequence_change(w.mm(id), &class);
    }
}

fn wrapped_in(w: &World, id: MobyId, seq: u8) -> bool { w.m(id).anim.flags & 2 != 0 && w.m(id).anim.seq_a == seq }

// ---------------------------------------------------------------------------------------------------
// The group calls the searchlight sentries 8 make on their turret group (`super::oltanis_sentry`).

fn group(w: &World, g: i32) -> Vec<MobyId> { crate::moby_update::scheduler::group_walk(w, g, crate::moby_update::scheduler::GroupWalk::Any) }
fn alive(w: &World, m: MobyId) -> bool { (w.m(m).state as i8) >= 0 }

/// `0x2b44a8(g)`: every turret of group `g` waiting sunk (state 1) told to rise (+0x88 = 1).
pub fn raise_group(w: &mut World, g: i32) {
    for m in group(w, g) {
        if w.m(m).state == 1 && w.m(m).pvars.len() >= pv::SIZE { c::set_pi16(w, m, pv::RAISE, 1); }
    }
}

/// `0x2b4670(g)`: no list, a live turret told to rise, or no live turret at all.
pub fn any_raised(w: &World, g: i32) -> bool {
    let list = group(w, g);
    if list.is_empty() { return true; }
    let mut live = 0;
    for m in list {
        if !alive(w, m) { continue; }
        live += 1;
        if w.m(m).pvars.len() >= pv::SIZE && c::pi16(w, m, pv::RAISE) != 0 { return true; }
    }
    live == 0
}

/// `0x2b4500(g)`: once every turret not woken (+0x8b = 0) is sunk or sinking (state 1 / 5), the risen ones (state 3)
/// sink: sequence 4, state 5, the timer `ticks(60)` (gp−0x56f4) with its inverse, the animation speed 1/(timer·0.08333),
/// the sweep = the yaw, +0x8b and the raise cleared. Returns whether they did (no list: false).
pub fn lower_group(w: &mut World, g: i32) -> bool {
    let list = group(w, g);
    if list.is_empty() { return false; }
    for &m in &list {
        if w.m(m).pvars.len() < pv::SIZE { return false; }
        if c::pu8(w, m, pv::WOKEN) == 0 && !matches!(w.m(m).state, 1 | 5) { return false; }
    }
    for m in list {
        if w.m(m).state != 3 { continue; }
        cut(w, m, 4);
        w.mm(m).state = 5;
        let t = w.ticks(k::RISE);
        c::set_pi16(w, m, pv::TIMER, t as i16);
        c::set_pf(w, m, pv::INV, 1.0 / t as f32);
        w.mm(m).anim.speed = 1.0 / (t as i16 as f32 * k::RISE_RATE);
        let yaw = w.m(m).rotation[2];
        c::set_pf(w, m, pv::SWEEP, yaw);
        c::set_pu8(w, m, pv::WOKEN, 0);
        c::set_pi16(w, m, pv::RAISE, 0);
    }
    true
}

/// `0x2b4340` (module doc).
pub fn hits(w: &mut World, id: MobyId) {
    let own = w.m(id).o_class;
    let mut hit = w.get_hit(id, 0x33_0000, false);
    if let Some(a) = hit.and_then(|h| h.attacker) {
        let ac = w.m(a).o_class;
        if ac == own || ac == SHOT_CLASSES[0] { hit = None; }
    }
    let r = damage::resolve(w, id, hit, pv::RECORD, 0, 4);
    if r.out5 != 1 && w.m(id).state != 6 {
        let h = c::pf(w, id, pv::RECORD) - r.damage;
        c::set_pf(w, id, pv::RECORD, h);
        let reaction = if h <= 0.0 { 1 } else { r.reaction };
        match reaction {
            1 | 2 => {
                let m = w.mm(id);
                m.state = 6;
                m.mode &= !mode::TARGETABLE;
                c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
                crate::moby_update::classes::crate_::set_death_bits(w, id, 0, -1);
            }
            3..=8 => c::set_pu8(w, id, pv::FLASH + 7, 200),
            9 | 10 => c::set_pu8(w, id, pv::FLASH + 7, 100),
            _ => {}
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
}

/// The target is in the waking cuboid (`0x274b78(34)` not kind 2, `PointInCuboid`).
fn target_in(w: &World, id: MobyId) -> bool {
    let t = target::acquire(w, id, k::RANGE);
    t.kind != 2 && point_in_cuboid(&w.svc.volumes, [t.pos[0], t.pos[1], t.pos[2]], c::pi32(w, id, pv::WAKE_CUBOID))
}

/// Level14 `0x2b3bf0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    hits(w, id);
    let home = c::pf(w, id, pv::HOME_Z);
    match w.m(id).state {
        0 => {
            let (z, yaw) = (w.m(id).position[2], w.m(id).rotation[2]);
            c::set_pf(w, id, pv::HOME_Z, z);
            c::set_pf(w, id, pv::SWEEP, 0.0);
            c::set_pf(w, id, pv::HOME_YAW, yaw);
            c::set_pi16(w, id, pv::TIMER, 0);
            if c::pi32(w, id, pv::AIM) == -1 || c::pi32(w, id, pv::WAKE_CUBOID) == -1 {
                w.delete_moby(id);
                return;
            }
            let t = w.ticks(k::WAKE);
            c::set_pi16(w, id, pv::WAKE, t as i16);
            c::set_pu8(w, id, pv::WOKEN, 0);
            if c::pi16(w, id, pv::RAISE) == 0 {
                let m = w.mm(id);
                m.state = 1;
                m.has_collision = false;
                m.position[2] -= k::SINK;
                m.mode &= !mode::TARGETABLE;
                c::set_pu8(w, id, pv::STARTED, 0);
            } else {
                w.mm(id).state = 3;
                c::set_pu8(w, id, pv::STARTED, 1);
            }
            let t = w.ticks(c::pi32(w, id, pv::REARM));
            c::set_pi32(w, id, pv::REARM_T, t);
        }
        1 => {
            if c::pi16(w, id, pv::RAISE) != 0 && c::dec_timer_pvar_i32(w, id, pv::REARM_T) != 0 {
                w.mm(id).state = 2;
                let t = w.ticks(k::RISE);
                c::set_pi16(w, id, pv::TIMER, t as i16);
                c::set_pf(w, id, pv::INV, 1.0 / t as i16 as f32);
                cut(w, id, 1);
                let t = c::pi16(w, id, pv::TIMER) as f32;
                w.mm(id).anim.speed = 1.0 / (t * k::RISE_RATE);
            }
        }
        2 => {
            let t = c::pi16(w, id, pv::TIMER);
            w.mm(id).position[2] = home - ((k::SINK - 0.0) * t as f32 * c::pf(w, id, pv::INV) + 0.0);
            if wrapped_in(w, id, 1) {
                cut(w, id, 2);
                w.mm(id).anim.speed = 1.0;
            }
            if (t as i32) < w.ticks(k::RISE >> 1) {
                let o = w.m(id).o_class;
                let col = super::class_collision(w, o);
                let m = w.mm(id);
                m.has_collision = col;
                m.mode |= mode::TARGETABLE;
            }
            if c::dec_timer_pvar_s16(w, id, pv::TIMER) != 0 {
                let t = w.ticks(k::WAKE);
                c::set_pi16(w, id, pv::WAKE, t as i16);
                w.mm(id).state = 3;
                let t = w.ticks(60);
                c::set_pi16(w, id, pv::TIMER, t as i16);
                w.mm(id).position[2] = home;
                c::set_pf(w, id, pv::SWEEP, 0.0);
            }
        }
        3 => {
            if target_in(w, id) {
                w.mm(id).state = 4;
                cut(w, id, 3);
                let t = w.ticks(120);
                c::set_pi16(w, id, pv::TIMER, t as i16);
                let t = w.ticks(20);
                c::set_pi16(w, id, pv::FIRE, t as i16);
                return;
            }
            if c::pu8(w, id, pv::STARTED) == 0 && c::dec_timer_pvar_s16(w, id, pv::WAKE) != 0 { c::set_pu8(w, id, pv::WOKEN, 1); }
        }
        4 => sweep_and_fire(w, id),
        5 => {
            let t = c::pi16(w, id, pv::TIMER);
            let f = t as f32 * c::pf(w, id, pv::INV);
            let (hy, sw) = (c::pf(w, id, pv::HOME_YAW), c::pf(w, id, pv::SWEEP));
            let m = w.mm(id);
            m.position[2] = home - (k::SINK + (0.0 - k::SINK) * f);
            m.rotation[2] = hy + (sw - hy) * f;
            if wrapped_in(w, id, 4) {
                cut(w, id, 0);
                w.mm(id).anim.speed = 1.0;
            }
            if (t as i32) < w.ticks(k::RISE >> 1) { w.mm(id).has_collision = false; }
            if c::dec_timer_pvar_s16(w, id, pv::TIMER) != 0 {
                let m = w.mm(id);
                m.state = 1;
                m.rotation[2] = hy;
                let t = w.ticks(60);
                c::set_pi16(w, id, pv::TIMER, t as i16);
                c::set_pi16(w, id, pv::RAISE, 0);
                let t = w.ticks(c::pi32(w, id, pv::REARM));
                c::set_pi32(w, id, pv::REARM_T, t);
                let m = w.mm(id);
                m.mode &= !mode::TARGETABLE;
                m.position[2] = home - k::SINK;
            }
        }
        6 => {
            let p = w.m(id).position;
            fx::beam_explosion(w, &BLAST, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// State 4 (module doc).
fn sweep_and_fire(w: &mut World, id: MobyId) {
    let step = if c::pi16(w, id, pv::DIR) != 0 { -k::SWEEP } else { k::SWEEP };
    let s = c::add_rot(c::pf(w, id, pv::SWEEP), step);
    c::set_pf(w, id, pv::SWEEP, s);
    let a = c::add_rot(s, c::pf(w, id, pv::HOME_YAW) * 0.017_453_292);
    if let Some(cub) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, pv::AIM)) {
        let (row, centre) = (cub.matrix[1], cub.centre());
        let sn = a.sin();
        let pt = [row[0] * sn + centre[0], row[1] * sn + centre[1]];
        let p = w.m(id).position;
        w.mm(id).rotation[2] = c::atan(pt[0] - p[0], pt[1] - p[1]);
    }
    c::dec_timer_pvar_s16(w, id, pv::FIRE);
    if w.counter.is_multiple_of(k::FIRE_EVERY) {
        let r = w.rng.randi(3);
        let rows = w.m(id).rows;
        let (r0, r1, r2) = (rows[0], rows[1], rows[2]);
        let mut p = if r == 0 {
            c::set_len3(r2, k::MUZZLE_A)
        } else {
            let side = if r == 1 { k::MUZZLE_SIDE } else { -k::MUZZLE_SIDE };
            c::add(c::set_len3(r2, k::MUZZLE_B), c::set_len3(r1, side))
        };
        p = c::add(p, c::set_len3(r0, k::MUZZLE_FWD));
        p = c::add(p, w.m(id).position);
        let v = c::set_len3(r0, k::SHOT_SPEED * c::DT);
        if c::pi16(w, id, pv::FIRE) == 0 {
            if let Some(s) = spawn_shot(w, p, v, id, 1) { w.mm(s).ambient = [0xe0, 0xe0, 0xe0, 0]; }
        }
    }
    if !target_in(w, id) && c::dec_timer_pvar_s16(w, id, pv::TIMER) != 0 && wrapped_in(w, id, 3) {
        let t = w.ticks(k::WAKE);
        c::set_pi16(w, id, pv::WAKE, t as i16);
        w.mm(id).state = 3;
        cut(w, id, 2);
        w.mm(id).anim.speed = 1.0;
    }
}

fn shooter(w: &World, id: MobyId) -> Option<MobyId> { usize::try_from(c::pi32(w, id, sv::SHOOTER) - 1).ok().filter(|&m| m < w.table.mobys.len()) }

/// `0x2eccf0(p, v, shooter, flag)` (module doc).
pub fn spawn_shot(w: &mut World, p: c::V, v: c::V, from: MobyId, flag: i32) -> Option<MobyId> {
    let s = w.create_moby(SHOT_CLASSES[0])?;
    {
        let m = w.mm(s);
        if m.pvars.len() < 0x30 { m.pvars.resize(0x80, 0); }
        m.rotation[0] = 0.0;
        m.draw_dist = 0x7f;
        m.update_dist = 0xff;
        m.visible = 1;
        m.rotation[1] = -c::atan(c::len2(v), v[2]);
        m.rotation[2] = c::atan(v[0], v[1]);
        m.position = p;
    }
    c::set_pv4(w, s, sv::START, p);
    c::set_pv4(w, s, sv::VEL, v);
    let t = w.ticks(5);
    w.anim_blend(s, 1, 0, t);
    w.mm(s).mode = 0x208;
    c::set_pi32(w, s, sv::FLAG, flag);
    c::set_pi32(w, s, sv::SHOOTER, from as i32 + 1);
    w.mm(s).alpha = 0x10;
    w.build_matrix(s);
    Some(s)
}

/// Level14 `0x2ece00`: the shot 681 (module doc).
pub fn shot_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x30 { return; }
    let old = c::pos(w, id);
    let v = c::pv4(w, id, sv::VEL);
    let p = c::add(old, v);
    c::set_pos(w, id, p);
    let mut dir = c::set_len2([v[0], v[1], 0.0, v[3]], 1.0);
    dir[2] = 1.0;
    let tmpl = HitTemplate { dir: dir.map(Pf::f), attacker: Some(id), flags: 0x1_0001, b18: 1, b19: 1, h1a: 0, damage: Pf::ONE, w20: 1 };
    let flags = (c::pi32(w, id, sv::FLAG) == 0) as u32;
    let start = c::pv4(w, id, sv::START);
    let drawn = w.m(id).visible != 0;
    if k::REACH < c::dist2(p, start) {
        if drawn {
            for _ in 0..3 {
                let x = w.rng.randf(-1.0, 1.0);
                let y = w.rng.randf(-1.0, 1.0);
                let z = w.rng.randf(-1.0, 1.0);
                let r = [x, y, z, 0.0];
                let a = c::add(c::set_len3(r, c::len3(r) * 0.1), r);
                let s = w.rng.randf(c::DT + c::DT, c::DT * 4.0);
                let v = c::set_len3(a, s);
                *w.svc.fx.part_spawns.entry(crate::particles::type51::TYPE).or_default() += 1;
                if let Some(sys) = w.particles.as_deref_mut() {
                    if crate::particles::type51::spawn(sys, p, v).is_none() { w.svc.fx.part_failed += 1; }
                }
            }
        }
        w.delete_moby(id);
        return;
    }
    let mut ignore = shooter(w, id);
    for _ in 0..3 {
        let Some(h) = w.coll_line(old.map(Pf::f), p.map(Pf::f), flags, ignore) else { break };
        if h.moby.is_some_and(|m| w.m(m).o_class == PASS_CLASS) {
            ignore = h.moby;
            continue;
        }
        let h2 = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, old.map(Pf::f), p.map(Pf::f), flags, ignore, &tmpl);
        let (point, normal) = h2.map_or((h.point, h.normal), |h| (h.point, h.normal));
        c::set_pos(w, id, [point[0], point[1], point[2], p[3]]);
        if drawn {
            let vel = c::pv4(w, id, sv::VEL);
            crate::moby_update::classes::blaster_shot::impact_sparks(w, point, [vel[0], vel[1], vel[2]], normal);
        }
        w.play_sound(0, 0, id);
        w.delete_moby(id);
        return;
    }
    if !crate::moby_update::creature::projectile::in_world(c::pos(w, id)) { w.delete_moby(id); }
}
