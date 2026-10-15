//! U521 (census 2026-09-29): class 1445, the area stalkers of Kalebo (level16 `0x2e5e08`, 24 created). A creature
//! (header: damage +0x20, flash +0x110, knockback +0x120, suck record +0x60) that waits at home until a target
//! (Ratchet or a decoy inside its area path, within 16 of home and 3 in height) turns up, walks to it (a steered walk
//! with a heading offset, kept off the area's walls), bites within 1.25 (key 14 of sequence 1: the joint hit on
//! joint 0, damage 1), walks back home when the target leaves. **Any** weapon hit (mask 0x330000; no damage
//! accounting, no resolver) kills it: a death flight, then the death explosion, `SetDeathBits`, the delete. The Suck
//! Cannon takes it (the table [`react::HELD7`], held 7).
//!
//! **Pvars**: +0x20 the damage record, +0x60 the suck record (+0xd0 the sequence table), +0x110 the flash, +0x120 the
//! knockback record, +0x180 the target record (+0x1c0 the moby, +0x1c4 the kind), +0x1d0 home, +0x1f0 the area path,
//! +0x1f4 the turn velocity, +0x1f8 the walk's heading offset, +0x204 the walk speed, +0x208 the last tick's drop, +0x220
//! the big-head cheat's.
//!
//! ## Coverage (level16 `0x2e5e08`, its tick `0x2e6208`, its walk `0x2e6428`)
//!
//! | address | what it does | ported / not |
//! |---|---|---|
//! | `0x2e6208` (state ≠ 0): `MobyGetHitMessage(m, 0x330000, 0)` (0x255d58) → K: +0x15d 0, zoff 0.5, speeds 15·dt / 8·dt (gp−0x4da0 / −0x4da4), gravity 20·dt², drag 20·dt², air speed 2·dt, flags 0x29, radius 0x200; untargetable; `0x256480` (= `0x26fa48`) aim; `0x257e50(…, K, 5, 1, 0)` (= `0x271418`); keys 8 / 16; state 8; +0x94 0; flash 0x78, `0x258d50` | every weapon's hit record kills it (the death flight) | [`tick`] (`World::get_hit`, `knock::*`, `flash::start`) |
//! | +0xa4 = 0xff; `0x258e30` flash update | | [`tick`] |
//! | `0x25b830(16, m, rec, …, area +0x1f0)` (= `0x274df8`); beyond 16 of home or 3 in height → kind 2; no moby → Ratchet's, the record at his position | the target search (decoys win) | [`tick`] (`target::acquire_in`) |
//! | `0x25e9f0(2.1, m, 1, +0x220)` | the big-head cheat manipulator | [`update`] (`manip::big_head`) |
//! | drawn, within 29 of the camera: `0x255a58` (= `0x26f020`), +0x7f = 0x17 | the shadow probe | [`update`] (`shadows::probe_down`) |
//! | case 0: no area → deleted; meter 1, column 0, health 1, +0xd0 gp−0x4db8, `rand() & 1` mirror, home, 1, blend 2 (`ticks(20)`), +0x58 8, +0x5a 3 | init | [`update`] (+0xd0: `react::SEQS_1445`) |
//! | case 1: a target → 4 (blend 0) | wait | [`update`] |
//! | case 4: `0x2e6428` to the target: within 1.25 → 5 (blend 1); no target → 6 | walk to the target | [`walk`] |
//! | case 5: `0x2576f8` (= `0x270cc0`) toward the target (12.57·dt², 12.57·dt, …); key 14 passed (`0x25cfa8` = `0x2765b0`) → `0x255320(0.333, 1, 1, m, 0, 1, 0, 1, 0)` (= level00 `0x2599e8`); wrapped beyond 1.5 → 4 (blend 0) | the bite on Ratchet (his moby's hit record, flags 1, damage 1) | [`update`] (`attack::joint_hit`) |
//! | case 6: `0x2e6428` home: within 1 → 1 (blend 2); a target → 4 | walk home | [`walk`] |
//! | case 7: `0x2db3f0` (= `0x305260`) done → 1, record state 0 | the Suck Cannon's carried update | [`update`] (`react::carried`) |
//! | case 8: `0x257f90` (= `0x271558`) & 0x140 → `0x25a988(0.5, 13, m, pos, 6)` (= `0x273f50`), `SetDeathBits(m, 0, −1)`, deleted | the death, the bolts and the save bit | [`update`] (`fx::death_explosion`, `crate_::set_death_bits`) |
//! | `0x2e6428`: heading = atan(point − pos) + +0x1f8; `0x270cc0`; speed `0x257268(distance, 8·dt², 12·dt², 4·dt, local, +0x204)` (= `0x270830`); v = row 0 at that speed, v.z = +0x208 − 10·dt²; `0x253f08(0.5, 0.5, 0, m, v, 0x10)` (= `0x26d610`); `0x25d270(0.333, area, pos, pos)` (= level00 `0x261d78`); +0x208 = z before − z after | the walk | [`walk`] (`turn::spring`, `walker::move_collide`, `path::push_from_walls`) |
//! | table (level16 0x1e9050): 0x2e65d0 / 0x2e6620 / 0x2e6670 / 0x2e66c0 (held 7, refused → 1), 0x2e66f0 (+0x60), `DeleteMoby` | the Suck Cannon | `react::slot_*` with [`react::HELD7`] |
//!
//! Native `f32`; the rand draws at the game's points.

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::crate_::set_death_bits;
use crate::moby_update::creature::{self as c, attack, flash, fx, ground, knock, react, target, turn, walker};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_5e08;
pub const REFERENCE_LEVEL: u32 = 16;
pub const CLASSES: [i16; 1] = [1445];

pub mod pv {
    pub const D: usize = 0x20;
    pub const SUCK: usize = 0x60;
    pub const FLASH: usize = 0x110;
    pub const K: usize = 0x120;
    pub const TGT: usize = 0x180;
    pub const TGT_MOBY: usize = 0x1c0;
    pub const TGT_KIND: usize = 0x1c4;
    pub const HOME: usize = 0x1d0;
    pub const AREA: usize = 0x1f0;
    pub const TURN_V: usize = 0x1f4;
    pub const OFFSET: usize = 0x1f8;
    pub const SPEED: usize = 0x204;
    pub const DROP: usize = 0x208;
    pub const SIZE: usize = 0x224;
}

pub mod st {
    pub const INIT: u8 = 0;
    pub const WAIT: u8 = 1;
    pub const WALK: u8 = 4;
    pub const BITE: u8 = 5;
    pub const HOME: u8 = 6;
    pub const HELD: u8 = 7;
    pub const DYING: u8 = 8;
}

/// Level16 gp−0x4da0 / −0x4da4 / −0x4dac / −0x4da8 (0x161e60, 0x161e5c, 0x161e54, 0x161e58): the death flight's out
/// and up speeds, gravity and drag.
const DEATH_OUT: f32 = 15.0;
const DEATH_UP: f32 = 8.0;
const GRAVITY: f32 = 20.0;
const DRAG: f32 = 20.0;

fn state(w: &World, id: MobyId) -> u8 { w.m(id).state }
fn set_state(w: &mut World, id: MobyId, s: u8) { w.mm(id).state = s; }
fn blend(w: &mut World, id: MobyId, s: u8) {
    if w.m(id).anim.seq_b != s {
        let t = w.ticks(0x14);
        w.anim_blend(id, s, 0, t);
    }
}
fn area(w: &World, id: MobyId) -> Option<usize> { usize::try_from(c::pi32(w, id, pv::AREA)).ok().filter(|&p| p < w.svc.splines.len()) }
fn kind(w: &World, id: MobyId) -> i32 { c::pi32(w, id, pv::TGT_KIND) }
fn face(w: &mut World, id: MobyId, h: f32) {
    let a = c::DT2 * 12.566_371;
    turn::turn_toward_pvar(w, id, h, a, a, c::DT * 12.566_371, pv::TURN_V);
}

/// `0x2e6208` (module doc).
fn tick(w: &mut World, id: MobyId) {
    if state(w, id) == st::INIT { return; }
    if let Some(h) = w.get_hit(id, 0x33_0000, false) {
        let k = pv::K;
        c::set_pu8(w, id, k + 0x3d, 0);
        c::set_pf(w, id, k + knock::k::ZOFF, 0.5);
        c::set_pf(w, id, k + knock::k::UP, DEATH_UP * c::DT);
        c::set_pf(w, id, k + knock::k::GRAVITY, GRAVITY * c::DT2);
        c::set_pf(w, id, k + knock::k::DRAG, DRAG * c::DT2);
        c::set_pf(w, id, k + knock::k::AIR_SPEED, c::DT + c::DT);
        c::set_pf(w, id, k + knock::k::SPEED, DEATH_OUT * c::DT);
        c::set_pi32(w, id, k + knock::k::FLAGS, 0x29);
        c::set_pi32(w, id, k + knock::k::RADIUS, 0x200);
        w.mm(id).mode &= !mode::TARGETABLE;
        let dir = h.dir.map(|x| f32::from_bits(x.0));
        let (mut sp, mut up) = (c::pf(w, id, k + knock::k::SPEED), c::pf(w, id, k + knock::k::UP));
        let a = knock::aim(dir, &mut sp, &mut up);
        c::set_pf(w, id, k + knock::k::SPEED, sp);
        c::set_pf(w, id, k + knock::k::UP, up);
        knock::start(w, id, k, a, 5, 1, 0);
        c::set_pf(w, id, k + knock::k::KEY_APEX, 8.0);
        c::set_pf(w, id, k + knock::k::KEY_LAND, 16.0);
        set_state(w, id, st::DYING);
        w.mm(id).has_collision = false;
        c::set_pu8(w, id, pv::FLASH + 7, 0x78);
        flash::start(w, id, pv::FLASH);
    }
    w.mm(id).hit_slot = 0xff;
    flash::update(w, id, pv::FLASH);
    let t = target::acquire_in(w, id, 16.0, area(w, id));
    c::set_pv4(w, id, pv::TGT, t.pos);
    c::set_pv4(w, id, pv::TGT + 0x10, t.rot);
    c::set_pv4(w, id, pv::TGT + 0x20, t.aim);
    c::set_pv4(w, id, pv::TGT + 0x30, t.body);
    c::set_pi32(w, id, pv::TGT_MOBY, t.moby.map_or(0, |m| m as i32 + 1));
    c::set_pi32(w, id, pv::TGT_KIND, t.kind as i32);
    if t.kind != 2 {
        let h = c::pv4(w, id, pv::HOME);
        if 16.0 < c::dist2(h, t.pos) || 3.0 < (w.m(id).position[2] - t.pos[2]).abs() { c::set_pi32(w, id, pv::TGT_KIND, 2); }
    }
    if c::pi32(w, id, pv::TGT_MOBY) == 0 {
        let h = w.hero_moby.map_or(0, |m| m as i32 + 1);
        c::set_pi32(w, id, pv::TGT_MOBY, h);
        let hp = crate::moby_update::classes::units::hero_pos(w);
        c::set_pv4(w, id, pv::TGT, hp);
    }
}

/// `0x2e6428(m, point)`: one step of the walk toward `to` (module doc); returns the distance before the step.
fn walk(w: &mut World, id: MobyId, to: c::V) -> f32 {
    let p = c::pos(w, id);
    let h = c::add_rot(c::atan(to[0] - p[0], to[1] - p[1]), c::pf(w, id, pv::OFFSET));
    let z0 = p[2];
    let d = c::dist3(p, to);
    face(w, id, h);
    let mut x = 0.0;
    let mut v = c::pf(w, id, pv::SPEED);
    turn::spring(d, c::DT2 * 8.0, c::DT2 * 12.0, c::DT * 4.0, &mut x, &mut v);
    c::set_pf(w, id, pv::SPEED, v);
    let row0 = w.m(id).rows[0];
    let mut mv = c::set_len3(row0, v);
    mv[2] = c::pf(w, id, pv::DROP) - c::DT2 * 10.0;
    walker::move_collide(w, id, 0.5, 0.5, 0.0, &mut mv, 0x10);
    if let Some(a) = area(w, id) {
        let pts = w.svc.splines[a].clone();
        if let Some(q) = crate::path::push_from_walls(&pts, f32::from_bits(0x3eaa_7efa), c::pos(w, id)) { c::set_pos(w, id, q); }
    }
    c::set_pf(w, id, pv::DROP, z0 - w.m(id).position[2]);
    d
}

/// Level16 `0x2e5e08` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    tick(w, id);
    // 0x25e9f0(2.1, m, 1, +0x220): the big-head cheat (0x15edb7).
    crate::moby_update::manip::big_head(w, 2.1, id, 1, id, 0x220);
    let cam = w.camera.map(|x| f32::from_bits(x.0));
    if w.m(id).visible != 0 && c::dist3(c::pos(w, id), cam) < 29.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x17;
    }
    let t = c::pv4(w, id, pv::TGT);
    let s = state(w, id);
    match s {
        st::INIT => {
            if c::pi32(w, id, pv::AREA) == -1 {
                w.delete_moby(id);
                return;
            }
            c::set_pi16(w, id, pv::D + 4, 1);
            c::set_pu8(w, id, pv::D + 8, 0);
            c::set_pf(w, id, pv::D, 1.0);
            c::set_pi32(w, id, pv::SUCK + react::rec::SEQS, react::seq_table_id(react::SEQS_1445));
            if w.rng.rand() & 1 != 0 { w.mm(id).mode |= mode::MIRROR; }
            let p = c::pos(w, id);
            c::set_pv4(w, id, pv::HOME, p);
            set_state(w, id, st::WAIT);
            blend(w, id, 2);
            c::set_pu8(w, id, 0x58, 8);
            c::set_pu8(w, id, 0x5a, 3);
        }
        st::WAIT => {
            if kind(w, id) != 2 {
                set_state(w, id, st::WALK);
                blend(w, id, 0);
            }
        }
        st::WALK => {
            if 1.25 <= walk(w, id, t) {
                if kind(w, id) == 2 { set_state(w, id, st::HOME); }
                return;
            }
            set_state(w, id, st::BITE);
            blend(w, id, 1);
        }
        st::BITE => {
            let p = c::pos(w, id);
            face(w, id, c::atan(t[0] - p[0], t[1] - p[1]));
            if ground::passed_frame(w, id, 14.0) {
                attack::joint_hit(w, f32::from_bits(0x3eaa_7efa), 1.0, id, 0, 1, 0, 1, 0);
                return;
            }
            if w.m(id).anim.flags & 2 != 0 && 1.5 < c::dist3(c::pos(w, id), t) {
                set_state(w, id, st::WALK);
                blend(w, id, 0);
            }
        }
        st::HOME => {
            let h = c::pv4(w, id, pv::HOME);
            if walk(w, id, h) < 1.0 {
                set_state(w, id, st::WAIT);
                blend(w, id, 2);
            } else if kind(w, id) != 2 {
                set_state(w, id, st::WALK);
            }
        }
        st::HELD => {
            if react::carried(w, id, pv::K) != 0 {
                set_state(w, id, st::WAIT);
                c::set_pi16(w, id, pv::SUCK + react::rec::STATE, 0);
            }
        }
        st::DYING if knock::update(w, id, pv::K) & 0x140 != 0 => {
            let p = c::pos(w, id);
            fx::death_explosion(w, 0.5, 13.0, Some(id), p, 6);
            set_death_bits(w, id, 0, -1);
            w.delete_moby(id);
        }
        _ => {}
    }
}
