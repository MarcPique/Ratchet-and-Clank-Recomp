//! U300 (census 2026-09-29): class 52, the buzz bombs of Gaspar and Kalebo (level09 `0x2c5990`, level16 `0x2a1088`,
//! the same code; 46 created). A small flyer that hovers half a unit over the ground until a target (Ratchet inside its
//! area path within 12 of it and 3 in height; any target within 24 while alerted) turns up; then it calls its group
//! and flies at him with a looping buzz (class sound 1, loop), holding at 0.8 over his height and stopping within 1.4.
//! Within 6 it winds up (sequence 3), then circles in (sequence 4) with a fuse of `ticks(180)`, blinking (flash 0xf0)
//! at 179, 120, 60, 40, 20 and 10 ticks left; at 0 it blows up: a beam explosion with a 2-unit damage sphere of 1 on
//! Ratchet and every moby, class sound 0, `SetDeathBits`. A weapon hit knocks it back (3 hits: health 3) or kills it
//! (a flight, `SetDeathBits`, the death explosion). The Taunter or a group call alerts it for `randf(180, 240)`
//! ticks. No Suck Cannon (its table is the default). Private pvar layout: the creature header (damage +0x20, flash
//! +0x60, knockback +0x70, walker +0xd0), the target record +0x120 (+0x160 the moby, +0x164 the kind), +0x180 the
//! area path, +0x184 the fuse, +0x188 the range, +0x18c the turn velocity, +0x190 the alert, +0x194 the buzz's voice
//! slot, +0x198 "moving", +0x280 the big-head cheat's.
//!
//! ## Coverage (level09 `0x2c5990` and its tick `0x2c6230`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x2c6230` (state ≠ 0): scale → class scale × 0.5 (gp−0x57f4) at 10 % a tick | the grow-in | [`tick`] |
//! | `FastDecTimer(+0x184)` = 2 → state 7, done; +0x184 = `ticks(10 / 20 / 40 / 60 / 120 / 179)` → flash 0xf0, `0x27cf10` | the fuse and its blinks | [`tick`] (`flash::start`) |
//! | `0x279f18` / `0x279f70` (col 4); out5 ≠ 1, not dying: health −= damage (≤ 0 → reaction 1); K: zoff 0, flags 1, +0xad 0, radius 512, gravity 20·dt², drag 30·dt²; class sound 8 | every weapon's hit record through the resolver; the hurt sound | [`tick`] (`damage::resolve`, `World::play_sound`) |
//! | reaction 1 / 2: untargetable, keys 14 / 28, 24·dt / 8·dt (gp−0x57f8 / −0x57fc), `0x27a640` (= `0x26fa48`) aim, `0x27c010(…, K, 4, 1, 0)`, `SetDeathBits(m, 0, −1)`, state 6, flash 0xf0 | the death flight, the bolts and the save bit | [`tick`] (`knock::*`, `crate_::set_death_bits`) |
//! | 3–8: keys 3 / 6, 12·dt / 0, aim, start (seq 4), state 5, group call `(group, 1)`, flash 0x78; 9 / 10: flash 0xfa; flash start | knockback; the group | [`tick`] (`attack::group_command`) |
//! | +0xa4 = 0xff; `0x27cff0` flash update | | [`tick`] |
//! | lure +0x38 or +0xbc = 1 → alert `trunc(randf(180, 240))`; both cleared; `FastDecTimer(+0x190)` | the Taunter, the group's call | [`tick`] |
//! | `randi(4)` = 0: not alerted → range 12, `0x27f9f0(12, m, rec, area)` (= `0x274df8`), beyond 12 or 3 in height → kind 2; alerted → 24, `0x27f770(24)` (= `0x274b78`); else the record follows its moby (gone → none, kind 2); none → Ratchet | the target search (decoys win) | [`tick`] (`target::acquire_in` / `acquire`) |
//! | `0x282a90(2.7, m, 0, +0x280)` | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn and within 29 of the camera: `0x279c18` (= `0x26f020`), +0x7f = 0x17; game mode 2 → nothing | the shadow probe | [`update`] (`shadows::probe_down`) |
//! | case 0: health 3, meter 3, column 2, +0x5a 6, +0x58 8; no area path → printf, deleted; walker (probes 0.5 / 0.5, radius 512, top speed 7.75·dt), state 1, blend 1 (10), slot −1 | init | [`update`] |
//! | case 1: z += (ground + 0.5 − z)·0.1; `0x27b8b8(atan(target − pos), 8.73·dt², 8.73·dt², 4π·dt, yaw, +0x18c)` (= `0x270cc0`); wrapped → 1 in 19 blend 2 (`rand_range(7, 12)`), else blend 1 (`rand_range(7, 14)`), anim speed `randf(0.95, 1.05)`; a target → group call, the buzz (sound 1, loop 4) unless playing, moving, 2 | idle | [`update`] (`turn::turn_toward_pvar`) |
//! | cases 2 / 4: turn; moving beyond 1.9 xy, stopped within 1.4; the move `0x278280(0, 0.5, 0, m, v, 0)` (= `0x26d610`): stopped → only z toward Ratchet's z + 0.2 (×0.1 below, ×0.05 above, ±0.3·speed); moving → +0xf4 along the heading and z toward his z + 0.8; 2 within 6 → 3 (blend 3) | the chase | [`chase`] (`walker::move_collide`) |
//! | case 3: wrapped → group call, moving, 4, the buzz, blend 4, the fuse `ticks(180)` if 0 | the wind-up | [`update`] |
//! | case 5: `0x27c150` (= `0x271558`) with z kept, K vz 0; K speed < 2·dt → the buzz, moving, 4, the fuse | knockback | [`update`] (`knock::update`) |
//! | case 6: the buzz stopped (`release_voice_slot` when its own), slot −1; `0x27c150` & 0x121 → `0x27eb48(0.5, 13, m, pos, 0)` (= `0x273f50`), deleted | the death | [`update`] (`fx::death_explosion`, `World::release_sound`) |
//! | case 7: `SetDeathBits(m, 0, −1)`, the buzz stopped, `0x27df08(2, 1, 4, 2, 9, 1, 15, m, 0, pos, 10, 3, 16, 0, 0, 0, …)` (= `SpawnBeamExplosion`), deleted | the blast on Ratchet (the damage sphere: his moby's hit record) | [`update`] ([`BLAST`], `fx::beam_explosion`) |
//!
//! Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::fx::{beam_explosion, death_explosion, Beam};
use crate::moby_update::creature::{self as c, attack, damage, flash, ground, knock, target, turn, walker};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2c_5990;
pub const REFERENCE_LEVEL: u32 = 9;
pub const CLASSES: [i16; 1] = [52];

pub mod pv {
    pub const D: usize = 0x20;
    pub const LURE: usize = 0x38;
    pub const FLASH: usize = 0x60;
    pub const K: usize = 0x70;
    pub const J: usize = 0xd0;
    pub const TGT: usize = 0x120;
    pub const TGT_MOBY: usize = 0x160;
    pub const TGT_KIND: usize = 0x164;
    pub const AREA: usize = 0x180;
    pub const FUSE: usize = 0x184;
    pub const RANGE: usize = 0x188;
    pub const TURN_V: usize = 0x18c;
    pub const ALERT: usize = 0x190;
    pub const SLOT: usize = 0x194;
    pub const MOVING: usize = 0x198;
    pub const SIZE: usize = 0x19c;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const IDLE: u8 = 1;
    pub const CHASE: u8 = 2;
    pub const WIND_UP: u8 = 3;
    pub const CIRCLE: u8 = 4;
    pub const KNOCKED: u8 = 5;
    pub const DYING: u8 = 6;
    pub const BLAST: u8 = 7;
}

/// The fuse's blast (`0x27df08`).
pub const BLAST: Beam = Beam { damage_r: 2.0, damage: 1.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: 0, shake: false };
/// Level09 gp−0x57f4 (0x16140c): the scale factor; gp−0x5808 / −0x57fc / −0x5800 (0x1613f8, 0x161408, 0x161404,
/// 0x161400): the flights' gravity 20, the death's out / up 24 / 8, the knockback's out 12.
const SCALE: f32 = 0.5;
const GRAVITY: f32 = 20.0;
const DEATH_OUT: f32 = 24.0;
const DEATH_UP: f32 = 8.0;
const KNOCK_OUT: f32 = 12.0;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn seq(w: &World, id: MobyId) -> u8 { w.m(id).anim.seq_b }
fn blend(w: &mut World, id: MobyId, s: u8, n: i32) {
    if seq(w, id) != s {
        let t = w.ticks(n);
        w.anim_blend(id, s, 0, t);
    }
}
fn moby_ref(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    (v > 0).then(|| (v - 1) as MobyId).filter(|&m| m < w.table.mobys.len())
}
fn set_moby_ref(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }
fn area(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::AREA)).ok().filter(|&p| p < w.svc.splines.len()) }
fn group_call(w: &mut World, id: MobyId) {
    let g = w.m(id).group;
    if g != -1 { attack::group_command(w, g, 1); }
}
/// The buzz (class sound 1, looping) unless its slot is taken.
fn buzz(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::SLOT) == -1 {
        let s = w.play_sound(1, 4, id);
        c::set_pi32(w, id, pv::SLOT, s);
    }
}
fn buzz_off(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, pv::SLOT);
    if s != -1 { w.release_sound(s, id); }
    c::set_pi32(w, id, pv::SLOT, -1);
}
fn arm(w: &mut World, id: MobyId) {
    if c::pi32(w, id, pv::FUSE) == 0 {
        let t = w.ticks(0xb4);
        c::set_pi32(w, id, pv::FUSE, t);
    }
}
fn face(w: &mut World, id: MobyId, t: c::V) {
    let p = c::pos(w, id);
    turn::turn_toward_pvar(w, id, c::atan(t[0] - p[0], t[1] - p[1]), c::DT2 * 8.726_646, c::DT2 * 8.726_646, c::DT * 12.566_371, pv::TURN_V);
}

/// `0x2c6230` (module doc).
fn tick(w: &mut World, id: MobyId) {
    if state(w, id) == st::INIT { return; }
    let o = w.m(id).o_class;
    let full = crate::moby_update::classes::units::class_scale(w, o) * SCALE;
    let s = w.m(id).scale;
    w.mm(id).scale = s + (full - s) * 0.1;
    if c::dec_timer_pvar_i32(w, id, pv::FUSE) == 2 {
        set_state(w, id, st::BLAST);
        return;
    }
    let fuse = c::pi32(w, id, pv::FUSE);
    if [10, 0x14, 0x28, 0x3c, 0x78, 0xb3].iter().any(|&n| fuse == w.ticks(n)) {
        c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
        flash::start(w, id, pv::FLASH);
    }
    let hit = w.get_hit(id, 0x33_0000, false);
    let res = damage::resolve(w, id, hit, pv::D, 0, 4);
    if res.out5 != 1 && state(w, id) != st::DYING {
        let hp = c::pf(w, id, pv::D) - res.damage;
        c::set_pf(w, id, pv::D, hp);
        let r = if hp <= 0.0 { 1 } else { res.reaction };
        let k = pv::K;
        c::set_pf(w, id, k + knock::k::ZOFF, 0.0);
        c::set_pi32(w, id, k + knock::k::FLAGS, 1);
        c::set_pu8(w, id, k + 0x3d, 0);
        c::set_pi32(w, id, k + knock::k::RADIUS, 512);
        c::set_pf(w, id, k + knock::k::GRAVITY, GRAVITY * c::DT2);
        c::set_pf(w, id, k + knock::k::DRAG, c::DT2 * 30.0);
        w.play_sound(8, 0, id);
        let dir = res.hit.map(|h| h.dir.map(|x| f32::from_bits(x.0))).unwrap_or([0.0; 4]);
        match r {
            1 | 2 => {
                w.mm(id).mode &= !mode::TARGETABLE;
                c::set_pf(w, id, k + knock::k::GRAVITY, GRAVITY * c::DT2);
                c::set_pf(w, id, k + knock::k::KEY_APEX, 14.0);
                c::set_pf(w, id, k + knock::k::KEY_LAND, 28.0);
                let (mut sp, mut up) = (DEATH_OUT * c::DT, DEATH_UP * c::DT);
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, k + knock::k::SPEED, sp);
                c::set_pf(w, id, k + knock::k::UP, up);
                knock::start(w, id, k, a, 4, 1, 0);
                set_death_bits(w, id, 0, -1);
                set_state(w, id, st::DYING);
                c::set_pu8(w, id, pv::FLASH + 7, 0xf0);
            }
            3..=8 => {
                c::set_pf(w, id, k + knock::k::KEY_APEX, 3.0);
                c::set_pf(w, id, k + knock::k::KEY_LAND, 6.0);
                let (mut sp, mut up) = (KNOCK_OUT * c::DT, 0.0);
                let a = knock::aim(dir, &mut sp, &mut up);
                c::set_pf(w, id, k + knock::k::SPEED, sp);
                c::set_pf(w, id, k + knock::k::UP, up);
                knock::start(w, id, k, a, 4, 1, 0);
                set_state(w, id, st::KNOCKED);
                group_call(w, id);
                c::set_pu8(w, id, pv::FLASH + 7, 0x78);
            }
            9 | 10 => c::set_pu8(w, id, pv::FLASH + 7, 0xfa),
            _ => {}
        }
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    if c::pi32(w, id, pv::LURE) != 0 || w.m(id).cmd == 1 {
        let f = w.rng.randf(180.0, 240.0);
        c::set_pi32(w, id, pv::ALERT, f as i32);
        c::set_pi32(w, id, pv::LURE, 0);
        w.mm(id).cmd = 0;
    }
    c::dec_timer_pvar_i32(w, id, pv::ALERT);
    if w.rng.randi(4) == 0 {
        let alerted = c::pi32(w, id, pv::ALERT) != 0;
        let range = if alerted { 24.0 } else { 12.0 };
        c::set_pf(w, id, pv::RANGE, range);
        let t = if alerted { target::acquire(w, id, range) } else { target::acquire_in(w, id, range, area(w, id)) };
        c::set_pv4(w, id, pv::TGT, t.pos);
        c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
        c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
        c::set_pv4(w, id, pv::TGT + 0x30, t.body);
        set_moby_ref(w, id, pv::TGT_MOBY, t.moby);
        c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
        if t.kind != 2 && !alerted {
            let p = c::pos(w, id);
            if range < c::dist2(p, t.pos) || 3.0 < (p[2] - t.pos[2]).abs() { c::set_pi32(w, id, pv::TGT_KIND, 2); }
        }
    } else {
        match moby_ref(w, id, pv::TGT_MOBY).filter(|&m| { let s = w.m(m).state; s != crate::moby_runtime::state::DELETED && s != crate::moby_runtime::state::DELETED_STATIC }) {
            Some(m) => { let p = c::pos(w, m); c::set_pv4(w, id, pv::TGT, p); }
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
}

/// Cases 2 / 4 (module doc).
fn chase(w: &mut World, id: MobyId, t: c::V) {
    let z0 = w.m(id).position[2];
    face(w, id, t);
    let d = c::dist2(c::pos(w, id), t);
    if 1.9 < d { c::set_pi32(w, id, pv::MOVING, 1); }
    if d < 1.4 { c::set_pi32(w, id, pv::MOVING, 0); }
    let hz = crate::moby_update::classes::units::hero_pos(w)[2];
    let speed = c::SPEED;
    let clamp = |x: f32| { let m = speed * 0.3; if m < x { m } else if x < -m { -m } else { x } };
    let mut v = if c::pi32(w, id, pv::MOVING) == 0 {
        let k = if w.m(id).position[2] < hz + 0.2 { 0.1 } else { 0.05 };
        [0.0, 0.0, clamp(0.0 + ((hz + 0.2) - z0) * k), 0.0]
    } else {
        let (co, si) = c::cs(c::yaw(w, id));
        let sp = c::pf(w, id, pv::J + 0x24);
        let k = if w.m(id).position[2] < hz + 0.8 { 0.1 } else { 0.05 };
        [co * sp, si * sp, clamp(((hz + 0.8) - z0) * k + 0.0), 0.0]
    };
    walker::move_collide(w, id, 0.0, 0.5, 0.0, &mut v, 0);
    if d < 6.0 && state(w, id) == st::CHASE {
        set_state(w, id, st::WIND_UP);
        blend(w, id, 3, 10);
    }
}

/// Level09 `0x2c5990` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    tick(w, id);
    // 0x282a90(2.7, m, 0, +0x280): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.7, id, 0, id, 0x280);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 29.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x17;
    }
    if w.svc.game_mode == 2 { return; }
    let t = moby_ref(w, id, pv::TGT_MOBY).map(|m| c::pos(w, m)).unwrap_or([0.0; 4]);
    match state(w, id) {
        st::INIT => {
            c::set_pf(w, id, pv::D, 3.0);
            c::set_pi16(w, id, pv::D + 4, 3);
            c::set_pu8(w, id, pv::D + 8, 2);
            c::set_pu8(w, id, 0x5a, 6);
            c::set_pu8(w, id, 0x58, 8);
            if c::pi32(w, id, pv::AREA) == -1 {
                // printf of the missing area, then DeleteMoby.
                w.delete_moby(id);
                return;
            }
            walker::seed(&mut w.mm(id).pvars, pv::J);
            c::set_pf(w, id, pv::J + 8, 0.5);
            c::set_pf(w, id, pv::J + 0xc, 0.5);
            c::set_pi32(w, id, pv::J, 512);
            c::set_pf(w, id, pv::J + 0x24, c::DT * 7.75);
            set_state(w, id, st::IDLE);
            blend(w, id, 1, 10);
            c::set_pi32(w, id, pv::SLOT, -1);
        }
        st::IDLE => {
            let g = ground::ground(w, c::pos(w, id), 0.5, 0).z;
            let z = w.m(id).position[2];
            w.mm(id).position[2] = z + ((g + 0.5) - z) * 0.1;
            face(w, id, t);
            if w.m(id).anim.flags & 2 != 0 {
                if w.rng.randi(0x13) == 0 {
                    if seq(w, id) != 2 {
                        let n = w.rng.rand_range(7, 0xc);
                        w.anim_blend(id, 2, 0, n);
                    }
                } else if seq(w, id) != 1 {
                    let n = w.rng.rand_range(7, 0xe);
                    w.anim_blend(id, 1, 0, n);
                }
                let s = w.rng.randf(f32::from_bits(0x3f73_3333), f32::from_bits(0x3f86_6666));
                w.mm(id).anim.speed = s;
            }
            if c::pi32(w, id, pv::TGT_KIND) != 2 {
                group_call(w, id);
                buzz(w, id);
                c::set_pi32(w, id, pv::MOVING, 1);
                set_state(w, id, st::CHASE);
            }
        }
        st::CHASE | st::CIRCLE => chase(w, id, t),
        st::WIND_UP => {
            if w.m(id).anim.flags & 2 == 0 { return; }
            group_call(w, id);
            c::set_pi32(w, id, pv::MOVING, 1);
            set_state(w, id, st::CIRCLE);
            buzz(w, id);
            blend(w, id, 4, 10);
            arm(w, id);
        }
        st::KNOCKED => {
            let z = w.m(id).position[2];
            knock::update(w, id, pv::K);
            w.mm(id).position[2] = z;
            c::set_pf(w, id, pv::K + 8, 0.0);
            if c::DT + c::DT <= c::len3(c::pv4(w, id, pv::K)) { return; }
            buzz(w, id);
            c::set_pi32(w, id, pv::MOVING, 1);
            set_state(w, id, st::CIRCLE);
            arm(w, id);
        }
        st::DYING => {
            buzz_off(w, id);
            if knock::update(w, id, pv::K) & 0x121 == 0 { return; }
            let p = c::pos(w, id);
            death_explosion(w, 0.5, 13.0, Some(id), p, 0);
            w.delete_moby(id);
        }
        st::BLAST => {
            set_death_bits(w, id, 0, -1);
            buzz_off(w, id);
            let p = c::pos(w, id);
            beam_explosion(w, &BLAST, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}
