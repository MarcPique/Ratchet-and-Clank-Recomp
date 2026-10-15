//! Drek's Fleet sliding doors, classes 1359–1364, 1367, 1369, 1372, 1373 (level 17, 52 instances): level17 0x2e87d8,
//! one update for all ten (census unit U533). A door slides 2 units along its heading (or across it) while Ratchet,
//! a linked moby or the camera is in its trigger cuboid, and back when they have left its hold cuboid, with its
//! class sound 0 looping while it moves. Read from the level17 decomp and disassembly. Native `f32`.
//!
//! **Pvar block** (P, words): 0..3 the home position, 4 a linked moby (index, −1 none), 5 the slide offset, 6 the
//! trigger cuboid, 7 the hold cuboid, 8 the sound slot (−1 none).
//!
//! Every tick: a door without both cuboids is deleted; nothing runs in a cutscene (`0x15f5c4 == 2`).
//! * **0**: 1360, 1361, 1363, 1367, 1373 slide the positive way (+0xbc = 1); home = pos; → **1**.
//! * **1** shut: offset 0; its sound released (`release_voice_slot` when the slot is still this door's and live);
//!   a door tipped 45° or more (|rot x|) opens only while Ratchet is on a magnetic floor (0x13f658 ≠ 0); Ratchet or the
//!   linked moby in the trigger cuboid (`PointInCuboid`): the sound (class sound 0, flags 4, when the class has sound
//!   defs and it is not already playing), → **2**.
//! * **2** opening: the sound kept going; offset ±3·dt; pos = home + offset·(cos a, sin a, 0), a = rot z (+ π/2 for
//!   1367, 1369, 1372, 1373); past 2: the sound released, → **4**.
//! * **4** open: the sound released; waits while Ratchet or the camera (0x1676c0) is in the hold cuboid or the linked
//!   moby in the trigger cuboid; then the sound, → **5**.
//! * **5** shutting: the sound kept going; Ratchet, the camera or the linked moby in the trigger cuboid: → **2**; else
//!   the offset steps back 3·dt toward 0 (0 when within a step) and the door follows; at 0 the sound is released,
//!   → **1**.

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_87d8;
pub const REFERENCE_LEVEL: u32 = 17;
pub const CLASSES: [i16; 10] = [1359, 1360, 1361, 1362, 1363, 1364, 1367, 1369, 1372, 1373];
/// The classes that slide the positive way (0x550, 0x551, 0x553, 0x557, 0x55d).
pub const POSITIVE: [i16; 5] = [1360, 1361, 1363, 1367, 1373];
/// The classes that slide across their heading (0x55c, 0x55d, 0x557, 0x559).
pub const ACROSS: [i16; 4] = [1372, 1373, 1367, 1369];

pub mod pv_ {
    pub const HOME: usize = 0x00;
    pub const LINK: usize = 0x10;
    pub const OFFSET: usize = 0x14;
    pub const TRIGGER: usize = 0x18;
    pub const HOLD: usize = 0x1c;
    pub const SLOT: usize = 0x20;
    pub const LEN: usize = 0x24;
}
use pv_ as o;

fn in_cuboid(w: &World, p: [f32; 4], k: i32) -> bool { w.in_cuboid([p[0], p[1], p[2]], k) }

/// Ratchet, the camera (when `camera`) or the linked moby in cuboid `k`.
fn someone_in(w: &World, id: MobyId, k: i32, camera: bool) -> bool {
    if in_cuboid(w, super::hero_pos(w), k) { return true; }
    if camera && in_cuboid(w, w.camera.map(|x| x.to_f32()), k) { return true; }
    link_in(w, id, k)
}

fn link_in(w: &World, id: MobyId, k: i32) -> bool {
    let l = c::pi32(w, id, o::LINK);
    l != -1 && w.table.mobys.get(l as usize).is_some_and(|m| in_cuboid(w, m.position, k))
}

/// The slot's owner is this door and it still plays: `release_voice_slot`; the slot is forgotten either way.
fn release(w: &mut World, id: MobyId) {
    let s = c::pi32(w, id, o::SLOT);
    if s != -1 && w.sound_alive(s, id) { w.release_sound(s, id); }
    c::set_pi32(w, id, o::SLOT, -1);
}

/// Class sound 0 (flags 4) when the class has sound defs and the slot is not playing.
fn keep_sound(w: &mut World, id: MobyId) {
    let oc = w.m(id).o_class;
    if w.classes.info(oc).is_some_and(|i| i.has_sounds) && !w.sound_alive(c::pi32(w, id, o::SLOT), id) {
        let s = w.play_sound(0, 4, id);
        c::set_pi32(w, id, o::SLOT, s);
    }
}

/// pos = home + offset along the slide direction.
fn place(w: &mut World, id: MobyId) {
    let m = w.m(id);
    let a = if ACROSS.contains(&m.o_class) { c::add_rot(std::f32::consts::FRAC_PI_2, m.rotation[2]) } else { m.rotation[2] };
    let d = c::pf(w, id, o::OFFSET);
    let home = c::pv4(w, id, o::HOME);
    w.mm(id).position = c::add(home, [a.cos() * d, a.sin() * d, 0.0, 0.0]);
}

fn step(w: &World, id: MobyId) -> f32 { if w.m(id).cmd == 0 { -(DT * 3.0) } else { DT * 3.0 } }

/// Level17 0x2e87d8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < o::LEN { return; }
    let (trigger, hold) = (c::pi32(w, id, o::TRIGGER), c::pi32(w, id, o::HOLD));
    if trigger == -1 || hold == -1 {
        w.delete_moby(id);
        return;
    }
    if w.svc.game_mode == 2 { return; }
    match w.m(id).state {
        0 => {
            if POSITIVE.contains(&w.m(id).o_class) { w.mm(id).cmd = 1; }
            let p = c::pos(w, id);
            c::set_pv4(w, id, o::HOME, p);
            w.mm(id).state = 1;
        }
        1 => {
            c::set_pf(w, id, o::OFFSET, 0.0);
            release(w, id);
            if std::f32::consts::FRAC_PI_4 <= w.m(id).rotation[0].abs() && w.hero.f658 == 0 { return; }
            if !in_cuboid(w, super::hero_pos(w), trigger) && !link_in(w, id, trigger) { return; }
            keep_sound(w, id);
            w.mm(id).state = 2;
        }
        2 => {
            keep_sound(w, id);
            let d = c::pf(w, id, o::OFFSET) + step(w, id);
            c::set_pf(w, id, o::OFFSET, d);
            place(w, id);
            if 2.0 < d.abs() {
                release(w, id);
                w.mm(id).state = 4;
            }
        }
        4 => {
            release(w, id);
            if in_cuboid(w, super::hero_pos(w), hold) || in_cuboid(w, w.camera.map(|x| x.to_f32()), hold) || link_in(w, id, trigger) { return; }
            keep_sound(w, id);
            w.mm(id).state = 5;
        }
        5 => {
            keep_sound(w, id);
            if someone_in(w, id, trigger, true) {
                w.mm(id).state = 2;
                return;
            }
            let d = c::pf(w, id, o::OFFSET);
            let d = if DT * 3.0 < d.abs() { d - step(w, id) } else { 0.0 };
            c::set_pf(w, id, o::OFFSET, d);
            place(w, id);
            if d == 0.0 {
                release(w, id);
                w.mm(id).state = 1;
            }
        }
        _ => {}
    }
}
