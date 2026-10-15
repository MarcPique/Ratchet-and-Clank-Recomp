//! Blarg's small level classes (level 06; read from the level06 decomp; the names are descriptive [L]):
//!
//! * **55, the landing bay doors** (`0x2b4770`, 2 placed; census U221): two halves turning about the centre of their
//!   cuboid (pvar +0x00). While the ship lands (mode 6, substate 8) they start 20° open and close; as it flies away
//!   (substate 3) they open 20°. The half whose placed rotation has y ≠ 0 turns the other way.
//! * **1551, the scene FX driver** (`0x309348`, 1 placed; U249): in scene 5, up to `ticks(127)`, the infobot's
//!   thrusters on actor 2 (the pattern of Rilgar's 1550, [`super::rilgar_small`]).
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2b4770` | mode 6: not yet started (+0x10) → substate 8: started, angle +0x18 = 20°, target +0x14 = 0; substate 3: started, target 20°, angle 0; other modes: +0x10 = 0 | [`door_update`] |
//! | | angle ≠ target → `0x26ac10(target, π/2·dt², π/2·dt², π/2·dt, &angle, &+0x08)` (= `0x270cc0`) | [`door_update`] (`turn::turn_toward`) |
//! | | 0: radius +0x0c = the xy distance to the cuboid's centre, base yaw +0x04; rot.y ≠ 0 → 1, else 2 | [`door_update`] |
//! | | 1: at `angle − π/2` about the centre (radius +0x0c, z kept), yaw = base + angle; 2: at `−π/2 − angle`, yaw = base − angle | [`door_update`] |
//! | `0x309348` | 0: → 1, +0x30 = 0xff; 1: game mode 2, scene (0x16d010) 5 and its tick (0x16d014) ≤ `ticks(127)` → `0x272718(actor 2)` (L01 `0x278450`) | [`scene_fx_update`] (`cutscene_fx::infobot_thrusters`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::cutscene_fx::infobot_thrusters;
use crate::moby_update::creature::{self as c, turn, DT, DT2};
use crate::moby_update::services::World;
use std::f32::consts::FRAC_PI_2;

pub const REFERENCE_LEVEL: u32 = 6;
pub const DOOR_FN: u32 = 0x2b_4770;
pub const DOOR_CLASSES: [i16; 1] = [55];
pub const SCENE_FX_FN: u32 = 0x30_9348;
pub const SCENE_FX_CLASSES: [i16; 1] = [1551];
/// 0x3eb2b8c2: the doors' opening (20°).
const OPEN: f32 = f32::from_bits(0x3eb2_b8c2);
/// 0xbfc90fd0: −π/2 as the game stores it.
const QUARTER: f32 = f32::from_bits(0xbfc9_0fd0);

fn centre(w: &World, id: MobyId) -> [f32; 3] {
    let i = c::pi32(w, id, 0);
    w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).map(|s| s.centre()).unwrap_or([0.0; 3])
}

/// Level06 `0x2b4770` (module doc).
pub fn door_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x1c { w.mm(id).pvars.resize(0x1c, 0); }
    if w.svc.game_mode == 6 {
        if c::pi32(w, id, 0x10) == 0 {
            match w.svc.travel.sub {
                8 => {
                    c::set_pi32(w, id, 0x10, 1);
                    c::set_pf(w, id, 0x18, OPEN);
                    c::set_pf(w, id, 0x14, 0.0);
                }
                3 => {
                    c::set_pi32(w, id, 0x10, 1);
                    c::set_pf(w, id, 0x14, OPEN);
                    c::set_pf(w, id, 0x18, 0.0);
                }
                _ => {}
            }
        }
    } else {
        c::set_pi32(w, id, 0x10, 0);
    }
    let (t, mut a) = (c::pf(w, id, 0x14), c::pf(w, id, 0x18));
    if a != t {
        let mut v = c::pf(w, id, 0x08);
        turn::turn_toward(t, DT2 * FRAC_PI_2, DT2 * FRAC_PI_2, DT * FRAC_PI_2, &mut a, &mut v);
        c::set_pf(w, id, 0x18, a);
        c::set_pf(w, id, 0x08, v);
    }
    let ctr = centre(w, id);
    match w.m(id).state {
        0 => {
            let p = c::pos(w, id);
            let r = c::dist2(p, [ctr[0], ctr[1], ctr[2], 0.0]);
            c::set_pf(w, id, 0x0c, r);
            let yaw = c::yaw(w, id);
            c::set_pf(w, id, 0x04, yaw);
            w.mm(id).state = if w.m(id).rotation[1] == 0.0 { 2 } else { 1 };
        }
        s @ (1 | 2) => {
            let (r, base) = (c::pf(w, id, 0x0c), c::pf(w, id, 0x04));
            let (h, yaw) = if s == 1 { (c::add_rot(a, QUARTER), c::add_rot(base, a)) } else { (c::sub_rot(QUARTER, a), c::sub_rot(base, a)) };
            let m = w.mm(id);
            m.position[0] = h.cos() * r + ctr[0];
            m.position[1] = h.sin() * r + ctr[1];
            m.rotation[2] = yaw;
        }
        _ => {}
    }
}

/// Level06 `0x309348` (module doc).
pub fn scene_fx_update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if w.svc.game_mode != 2 { return; }
            let Some(scene) = w.svc.cinematic.scene.clone() else { return };
            if scene.id != 5 || w.ticks(0x7f) < scene.tick { return; }
            if let Some(a) = scene.actors.get(2) { infobot_thrusters(w, a); }
        }
        _ => {}
    }
}
