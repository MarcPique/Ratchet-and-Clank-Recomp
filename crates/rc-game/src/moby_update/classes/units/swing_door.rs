//! Swing doors, class 93: level15 0x2a3ba8 (census U473; 12 created instances on Quartu). A door stands turned −35°
//! (+35° mirrored) from its placed yaw and swings a further 35° open in 0.2 s (175°/s) when Ratchet, the camera or
//! a targetable moby with a target record is inside its first cuboid, and back when nothing is in either cuboid
//! (a door linked to a 1179 opens while that moby is in state 2 and never closes). Sound 0 at each start. Read from
//! the level15 decomp (0x2a3ba8; gp−0x5704 = −35°, gp−0x5700 = 0.2 s). Native `f32`.
//!
//! **Pvar block**: +0x00 the closed yaw, +0x04 / +0x08 the cuboids, +0x0c the swing, +0x10 mirrored, +0x14 the linked
//! moby (runtime index, −1 none), +0x18 the sound's voice slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2a3ba8 | no link, state ≠ 0: Ratchet (0x13f3d0) or the camera (0x1673c0) in cuboid 1 → open; else Ratchet in cuboid 2 → neither; else clear = camera not in cuboid 2; then every moby of the run list (0x15ffe4) with a target record (`0x2711f8`): in cuboid 1 → open, in neither → clear (`PointInCuboid` 0x274820) | [`update`] (`scheduler::build_active_list`, `targeting::record`, `triggers::point_in_cuboid`); the run list is rebuilt for the door [L] |
//! | | a link: class 1179 → open = its state 2 | [`update`] |
//! | | open → not clear | [`update`] |
//! | state 0 | mirrored → mode \| 0x8000; closed yaw = yaw + (mirrored ? −35° : 35°); swing 0; → 1 | [`update`] |
//! | state 1 | swing 0, yaw = closed; open → 2, `PlayClassSound(0, 0, m)` → +0x18 | [`update`] |
//! | state 2 | swing += −175°·dt (`fast_add_rotations`); below −35° → −35°, → 3; yaw = closed ± swing | [`update`] |
//! | state 3 | clear → 4, sound 0 | [`update`] |
//! | state 4 | swing += 175°·dt; above 0 → 0, → 1; yaw = closed ± swing; open → 2 | [`update`] |
//! | | no particle, light, save flag, other moby written | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::{add_rot, pf, pi32, set_pf, set_pi32, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers::point_in_cuboid;

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2a_3ba8;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [93];

/// gp−0x5704 / gp−0x5700: the swing (degrees) and its time (s).
pub const SWING: f32 = -35.0;
pub const TIME: f32 = 0.2;
/// The linked opener class.
pub const OPENER: i16 = 1179;
const DEG: f32 = 0.017453292;

/// (open, clear) this tick (module doc).
fn sense(w: &World, id: MobyId) -> (bool, bool) {
    let link = pi32(w, id, 0x14);
    if link != -1 {
        let open = usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| m.o_class == OPENER && m.state == 2);
        return (open, false);
    }
    if w.m(id).state == 0 { return (false, false); }
    let (c1, c2) = (pi32(w, id, 4), pi32(w, id, 8));
    let v = &w.svc.volumes;
    let h = super::hero_pos(w);
    let h = [h[0], h[1], h[2]];
    let cam = w.camera_point();
    let (mut open, mut clear) = (false, false);
    if point_in_cuboid(v, h, c1) || point_in_cuboid(v, cam, c1) {
        open = true;
    } else if !point_in_cuboid(v, h, c2) {
        clear = !point_in_cuboid(v, cam, c2);
    }
    let run = crate::moby_update::scheduler::build_active_list(w.table, w.camera, &w.svc.groups).0;
    for k in run {
        let m = &w.table.mobys[k];
        if crate::targeting::record(m).is_none() { continue; }
        let p = [m.position[0], m.position[1], m.position[2]];
        if point_in_cuboid(v, p, c1) { open = true; } else if !point_in_cuboid(v, p, c2) { clear = true; }
    }
    (open, clear && !open)
}

fn start(w: &mut World, id: MobyId, state: u8) {
    w.mm(id).state = state;
    let s = w.play_sound(0, 0, id);
    set_pi32(w, id, 0x18, s);
}

/// Level15 0x2a3ba8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { return; }
    let (open, clear) = sense(w, id);
    let mirrored = pi32(w, id, 0x10) != 0;
    let step = (SWING / TIME) * DEG * DT;
    let closed = pf(w, id, 0);
    let swung = |w: &mut World, s: f32| { let a = add_rot(closed, if mirrored { -s } else { s }); w.mm(id).rotation[2] = a; };
    match w.m(id).state {
        0 => {
            if mirrored { w.mm(id).mode |= mode::MIRROR; }
            let a = if mirrored { SWING * DEG } else { -(SWING * DEG) };
            let base = add_rot(w.m(id).rotation[2], a);
            set_pf(w, id, 0xc, 0.0);
            set_pf(w, id, 0, base);
            let m = w.mm(id);
            m.state = 1;
            m.rotation[2] = base;
        }
        1 => {
            set_pf(w, id, 0xc, 0.0);
            w.mm(id).rotation[2] = closed;
            if open { start(w, id, 2); }
        }
        2 => {
            let mut s = add_rot(pf(w, id, 0xc), step);
            if s < SWING * DEG { s = SWING * DEG; w.mm(id).state = 3; }
            set_pf(w, id, 0xc, s);
            swung(w, s);
        }
        3 => if clear { start(w, id, 4); },
        4 => {
            let mut s = add_rot(pf(w, id, 0xc), -step);
            if 0.0 < s { s = 0.0; w.mm(id).state = 1; }
            set_pf(w, id, 0xc, s);
            swung(w, s);
            if open { w.mm(id).state = 2; }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn a_linked_door_swings_open_on_state_two() {
        let mut m = Moby { o_class: 93, pvars: vec![0; 0x1c], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0x14, 1);
        let opener = Moby { o_class: OPENER, state: 1, ..Moby::default() };
        let mut t = MobyTable::new(vec![m, opener], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert!((w.m(0).rotation[2] - 35f32.to_radians()).abs() < 1e-5, "closed at +35°");
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        w.mm(1).state = 2;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 2);
        for _ in 0..13 { update(&mut w, 0); }
        assert_eq!(w.m(0).state, 3);
        assert!(w.m(0).rotation[2].abs() < 1e-5, "swung 35° to the placed yaw");
        assert_eq!(w.svc.sounds.len(), 1);
    }
}
