//! **Umbris' path lifts**, classes 38 (level07 `0x2cd2b0` with its rider test `0x2cd258`; 1 placed; census U254) and
//! 1474 (`0x2cdb28` / `0x2cdad0`; 1 placed; U255; the names are descriptive [L]). A platform that rides its path
//! (+0xb4, points evenly timed) from the first point to the last when Ratchet steps on after having been away: the
//! ride eases in (its speed ramps to 1 / (travel seconds +0xb0 · 60) of the path a tick), holds Ratchet still unless
//! +0xc8 is set, loops its sound, and pauses half a second while it would come down on him. At the end the lift
//! shrinks away over a second and reappears at the start (1474 first waits there until he steps off). 1474 exists
//! only while its mission's loaded byte is −1 (else hidden, no collision). Its riders ride along. Read from the
//! level07 decomp. Native `f32`.
//!
//! **Pvars**: +0x20 the moby record (+0x20 s32, +0x24 s16, +0x28 byte 4, +0x3e s16 5), +0x60 the platform block,
//! +0xa0 byte the starting end (0: at the last point, waiting to vanish), +0xa4 the place along the path (0..1),
//! +0xa8 its speed, +0xac the ramp (= the top speed), +0xb0 the travel time (s), +0xb4 the path, +0xb8 s16 armed
//! (Ratchet was off it), +0xba s16 a hold timer, +0xbc the height change last tick, +0xc4 the loop sound's slot,
//! +0xc8 no hold on Ratchet.
//!
//! | address | what | port |
//! |---|---|---|
//! | top | 1474: the mission's loaded byte (0x15fc88) ≠ −1 → collision off, hidden, nothing else; no path → nothing; the hold timer `FastDecTimer` | [`update`] |
//! | state 0 | the record's words; +0xbc = +0xa0; at its end (0 → the last point, place 1), timers 0, update / draw distance 0xff, no sound; 1474: shown, the class's collision; → 1 | [`update`] |
//! | +0xbc 1 | armed and Ratchet on it (`HeroOnMoby`): `SetState(0x72, 1)` unless +0xc8; → 2, the ramp `1 / multiply_global_scale(+0xb0·60)` | [`update`] |
//! | +0xbc 0 | scale −= class scale / 60; below 0: the class scale, the first point, → 1, place 0 | [`update`] |
//! | +0xbc 3 (1474) | Ratchet off → 0 | [`update`] |
//! | +0xbc 2 | not held: coming down (dz < 0) on Ratchet (xy < 2, \|dz\| < 4, he 1 below) → hold `ticks(30)`, stop; else the loop sound (`PlayClassSound(0, 4)` when not alive), speed += ramp·dt (at most the ramp), place += speed; past an end: his per-body idle (`0x24f4a0`) if he is on it and +0xc8 is 0, → 0 (1474: 3) at place 1 or → 1 at 0, speed 0, hold `ticks(15)`, the sound released; the point at the place (the last at 1, else the segment's lerp); dz | [`update`] (`HeroCall::BodyIdle`) |
//! | every tick | `CarryRiders(+0x60, moved, rot, rot)`; Ratchet off, beyond 2 (xy) and no hold → armed | `triggers::carry_riders` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::{HeroCall, World};

pub const REFERENCE_LEVEL: u32 = 7;
pub const UPDATE_FN: u32 = 0x2c_d2b0;
pub const CLASSES: [i16; 1] = [38];
pub const GATED_FN: u32 = 0x2c_db28;
pub const GATED_CLASSES: [i16; 1] = [1474];
const PVARS: usize = 0xcc;

mod pv {
    pub const REC: usize = 0x20;
    pub const PLATFORM: usize = 0x60;
    pub const START: usize = 0xa0;
    pub const PLACE: usize = 0xa4;
    pub const SPEED: usize = 0xa8;
    pub const RAMP: usize = 0xac;
    pub const TIME: usize = 0xb0;
    pub const PATH: usize = 0xb4;
    pub const ARMED: usize = 0xb8;
    pub const HOLD: usize = 0xba;
    pub const DZ: usize = 0xbc;
    pub const SLOT: usize = 0xc4;
    pub const FREE: usize = 0xc8;
}

/// Level07 `0x2cd2b0` (module doc).
pub fn update(w: &mut World, id: MobyId) { run(w, id, false); }

/// Level07 `0x2cdb28` (module doc).
pub fn gated_update(w: &mut World, id: MobyId) {
    let mission = w.m(id).mission;
    if w.missions.mission_slot(mission) != 0xff {
        let m = w.mm(id);
        m.has_collision = false;
        m.mode |= 0x41;
        return;
    }
    run(w, id, true);
}

fn point(pts: &[[u32; 4]], i: usize) -> c::V { pts[i].map(f32::from_bits) }

fn stop_sound(w: &mut World, id: MobyId) {
    c::set_pf(w, id, pv::SPEED, 0.0);
    let slot = c::pi32(w, id, pv::SLOT);
    if slot != -1 { w.release_sound(slot, id); }
    c::set_pi32(w, id, pv::SLOT, -1);
}

fn run(w: &mut World, id: MobyId, gated: bool) {
    if w.m(id).pvars.len() < PVARS { w.mm(id).pvars.resize(PVARS, 0); }
    let Some(path) = usize::try_from(c::pi32(w, id, pv::PATH)).ok() else { return };
    let pts: Vec<[u32; 4]> = w.svc.splines.get(path).cloned().unwrap_or_default();
    if pts.is_empty() { return; }
    let n = pts.len();
    let old = c::pos(w, id);
    c::dec_timer_pvar_s16(w, id, pv::HOLD);
    if w.m(id).state == 0 {
        let start = c::pu8(w, id, pv::START);
        c::set_pu8(w, id, pv::REC + 8, 4);
        c::set_pi16(w, id, pv::REC + 0x1e, 5);
        c::set_pi32(w, id, pv::REC, 0);
        c::set_pi16(w, id, pv::REC + 4, 0);
        w.mm(id).cmd = start;
        let place = if start == 0 { 1.0 } else { 0.0 };
        c::set_pf(w, id, pv::PLACE, place);
        let p = point(&pts, if place as i32 != 0 { n - 1 } else { 0 });
        c::set_pos(w, id, p);
        c::set_pi16(w, id, pv::HOLD, 0);
        c::set_pf(w, id, pv::SPEED, 0.0);
        c::set_pf(w, id, pv::DZ, 0.0);
        let m = w.mm(id);
        m.state = 1;
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        c::set_pi32(w, id, pv::SLOT, -1);
        if gated {
            let oc = w.m(id).o_class;
            let coll = w.classes.info(oc).is_some_and(|i| i.has_collision);
            let m = w.mm(id);
            m.mode &= !0x41;
            m.has_collision = coll;
        }
    }
    let on = w.hero_on_moby(id);
    match w.m(id).cmd {
        1 => {
            if c::pi16(w, id, pv::ARMED) != 0 && on {
                if c::pi32(w, id, pv::FREE) == 0 && w.hero_on_moby(id) { crate::cinematic::hero_state(w, 0x72, true); }
                w.mm(id).cmd = 2;
                let t = crate::moby_update::services::fl(w.svc.timing.scale(crate::ps2v::Pf::f(c::pf(w, id, pv::TIME) * 60.0)));
                c::set_pi16(w, id, pv::ARMED, 0);
                c::set_pf(w, id, pv::RAMP, 1.0 / t);
            }
        }
        0 => {
            let oc = w.m(id).o_class;
            let cs = super::class_scale(w, oc);
            let s = w.m(id).scale - cs / 60.0;
            w.mm(id).scale = s;
            if s < 0.0 {
                w.mm(id).scale = cs;
                c::set_pos(w, id, point(&pts, 0));
                w.mm(id).cmd = 1;
                c::set_pf(w, id, pv::PLACE, 0.0);
            }
        }
        3 if gated => {
            if !on { w.mm(id).cmd = 0; }
        }
        2 => ride(w, id, &pts, old[2], gated),
        _ => {}
    }
    let now = c::pos(w, id);
    let r = w.m(id).rotation;
    crate::moby_update::triggers::carry_riders(&mut w.mm(id).pvars, pv::PLATFORM, c::sub(now, old), r, r);
    if !w.hero_on_moby(id) && 2.0 < c::dist2(now, super::hero_pos(w)) && c::pi16(w, id, pv::HOLD) == 0 { c::set_pi16(w, id, pv::ARMED, 1); }
}

/// +0xbc 2: the ride (module doc). `z0` is the height before this tick.
fn ride(w: &mut World, id: MobyId, pts: &[[u32; 4]], z0: f32, gated: bool) {
    if c::pi16(w, id, pv::HOLD) != 0 {
        stop_sound(w, id);
        return;
    }
    let pos = c::pos(w, id);
    let hero = super::hero_pos(w);
    if c::pf(w, id, pv::DZ) < 0.0 && c::dist2(pos, hero) < 2.0 && (hero[2] - pos[2]).abs() < 4.0 && 1.0 < pos[2] - hero[2] {
        let t = w.ticks(30);
        c::set_pi16(w, id, pv::HOLD, t as i16);
        stop_sound(w, id);
        return;
    }
    let slot = c::pi32(w, id, pv::SLOT);
    if !w.sound_alive(slot, id) {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, pv::SLOT, s);
    }
    let ramp = c::pf(w, id, pv::RAMP);
    let mut speed = c::pf(w, id, pv::SPEED) + ramp * DT;
    if ramp.abs() < speed.abs() { speed = ramp; }
    c::set_pf(w, id, pv::SPEED, speed);
    let mut place = c::pf(w, id, pv::PLACE) + speed;
    c::set_pf(w, id, pv::PLACE, place);
    if 0.5 < (place - 0.5).abs() {
        if c::pi32(w, id, pv::FREE) == 0 && w.hero_on_moby(id) { w.hero_fields_mut().call(HeroCall::BodyIdle); }
        if 0.0 < ramp {
            w.mm(id).cmd = if gated { 3 } else { 0 };
            place = 1.0;
        } else {
            w.mm(id).cmd = 1;
            place = 0.0;
        }
        c::set_pf(w, id, pv::PLACE, place);
        let t = w.ticks(15);
        c::set_pi16(w, id, pv::HOLD, t as i16);
        stop_sound(w, id);
    }
    let n = pts.len();
    let f = (n - 1) as f32 * place;
    let i = (f as i32).max(0) as usize;
    let p = if n - 1 <= i {
        point(pts, n - 1)
    } else {
        let a = point(pts, i);
        c::add(a, c::scale(c::sub(point(pts, i + 1), a), f - i as f32))
    };
    c::set_pos(w, id, p);
    c::set_pf(w, id, pv::DZ, p[2] - z0);
}
