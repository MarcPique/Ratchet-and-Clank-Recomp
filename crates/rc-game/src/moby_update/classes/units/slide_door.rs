//! Sliding doors, classes 196, 197, 1958: level15 0x2bddb0, the same code on 17 (census U477; 10 created instances).
//! A door slides along its yaw by three times its instance scale (its scale over the class scale) in three seconds:
//! 196 / 1958 backwards and with sound 0 at each start, 197 forwards and silent. A linked door (+0x10, a runtime
//! index) opens when its link reaches state 4 and stays open; a door with +0x10 = 0 opens while Ratchet is in cuboid A
//! and, when unlinked (+0x10 = −1), closes once he has left cuboid B. A door with no link and no cuboids is deleted.
//! Read from the level15 decomp (0x2bddb0). Native `f32`.
//!
//! **Pvar block**: +0x00 the closed position, +0x10 the link, +0x14 the slide, +0x18 / +0x1c cuboids A / B.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2bddb0 | link −1 and a cuboid −1 → `DeleteMoby` | [`update`] |
//! | state 0 | +0x00 = position; link 0 → 4, else 1 | [`update`] |
//! | state 1 | the link (moby +0x10) in state 4 → sound 0 (196 / 1958), → 2 | [`update`] |
//! | state 2 | slide ±= travel/3·dt (travel = 3·scale / class scale, − for 196 / 1958); position = closed + (cos yaw, sin yaw, 0)·slide; \|slide\| > travel → link −1 ? 5 : 3 | [`update`] |
//! | state 4 | slide 0; Ratchet (0x13f3d0) in cuboid A (`PointInCuboid`) → sound, → 2 | [`update`] (`triggers::point_in_cuboid`) |
//! | state 5 | Ratchet out of cuboid B → sound, → 6 | [`update`] |
//! | state 6 | \|slide\| above one step → slide ∓= step, else 0; position; slide 0 → 4 | [`update`] |
//! | | no particle, light, save flag, other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{pf, pi32, pv4, set_pf, set_pv4, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers::point_in_cuboid;

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2b_ddb0;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 3] = [196, 197, 1958];

fn place(w: &mut World, id: MobyId, s: f32) {
    let home = pv4(w, id, 0);
    let a = w.m(id).rotation[2];
    let m = w.mm(id);
    m.position = [home[0] + a.cos() * s, home[1] + a.sin() * s, home[2], home[3]];
}

/// Level15 0x2bddb0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let oc = w.m(id).o_class;
    let loud = oc == 196 || oc == 1958;
    let link = pi32(w, id, 0x10);
    if link == -1 && (pi32(w, id, 0x18) == -1 || pi32(w, id, 0x1c) == -1) {
        w.delete_moby(id);
        return;
    }
    let sound = |w: &mut World| if loud { w.play_sound(0, 0, id); };
    let travel = w.m(id).scale * 3.0 / super::class_scale(w, oc);
    let step = travel / 3.0 * DT;
    let hero = { let h = super::hero_pos(w); [h[0], h[1], h[2]] };
    let next = match w.m(id).state {
        0 => {
            let p = w.m(id).position;
            set_pv4(w, id, 0, p);
            if link == 0 { 4 } else { 1 }
        }
        1 => {
            if usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_none_or(|m| m.state != 4) { return; }
            sound(w);
            2
        }
        2 => {
            let s = pf(w, id, 0x14) + if loud { -step } else { step };
            set_pf(w, id, 0x14, s);
            place(w, id, s);
            if s.abs() <= travel { return; }
            if link == -1 { 5 } else { 3 }
        }
        4 => {
            set_pf(w, id, 0x14, 0.0);
            if !point_in_cuboid(&w.svc.volumes, hero, pi32(w, id, 0x18)) { return; }
            sound(w);
            2
        }
        5 => {
            if point_in_cuboid(&w.svc.volumes, hero, pi32(w, id, 0x1c)) { return; }
            sound(w);
            6
        }
        6 => {
            let s = pf(w, id, 0x14);
            let s = if step < s.abs() { s - if loud { -step } else { step } } else { 0.0 };
            set_pf(w, id, 0x14, s);
            place(w, id, s);
            if s != 0.0 { return; }
            4
        }
        _ => return,
    };
    w.mm(id).state = next;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn a_linked_door_slides_when_the_link_reaches_state_4() {
        let mut m = Moby { o_class: 196, scale: 1.0, pvars: vec![0; 0x20], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0x10, 1);
        let link = Moby { o_class: 1, state: 3, ..Moby::default() };
        let mut t = MobyTable::new(vec![m, link], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        w.mm(1).state = 4;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 2);
        for _ in 0..30 { update(&mut w, 0); }
        assert!(w.m(0).position[0] < 0.0, "sliding back along its yaw: {:?}", w.m(0).position);
        assert_eq!(w.svc.sounds.len(), 1);
    }
}
