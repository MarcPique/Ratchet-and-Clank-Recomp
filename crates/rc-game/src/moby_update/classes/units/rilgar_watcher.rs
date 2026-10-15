//! **The watching bystanders, classes 920 (Rilgar) and 447 (Umbris, Gemlik)** (level05 `0x317aa0`; level07 `0x2f61c0`
//! and level13 `0x2edbe8` are the same code; census U210; the name is descriptive [L]). A bystander whose head follows
//! Ratchet when he is near and in front of it and moving, and otherwise glances about (the talking NPCs' look-at,
//! `talking_npc::look_at_layout`). With its links set (pvar +0x120 a cuboid, +0x124 / +0x128 mobys) it can stand in for
//! another: gone at the load when +0x124's moby has left state 0 (setting +0x128's to 3), and once Ratchet steps into
//! its cuboid its talked word is set. Read from the level05 decomp and disassembly.
//!
//! **Pvars** (0x130): +0x00 the head record (list 0: pitch and 0.6 of the yaw), +0x80 the neck record (list 1: 0.4 of
//! the yaw), +0x100 the glance point, +0x118 the "seen" timer, +0x11c the glance timer, +0x120 the cuboid, +0x124 /
//! +0x128 the linked mobys (−1 none).
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | `0x3179a8` (L01 `0x2fb4b8`): the scene actors' big head (classes 0x398 / 0x1bf, 2.1); drawn and within 30 (`vec_distance`) of the camera: `0x2843d8` (= `0x26f020`), +0x7f = 0x18 | [`update`] (`manip::scene_big_head`, `shadows::probe_down`) |
//! | state 0 | +0x30 = 0xff; the links checked (each −1 printed); +0x124's moby not in state 0 → +0x128's moby → 3, `DeleteMoby`; else → 1 | [`update`] |
//! | state 1 | +0x120 ≠ −1 and Ratchet in it → `0x2907b8(m, 1)` (L01 `0x27b438`, the talked word), → 2 | [`update`] (`interact::set_talked`) |
//! | state 2 | → 3 | [`update`] |
//! | every tick | the look-at (seq B 0; eye 1, pitch ×1, yaw 0.6 / 0.4, k 0.02 / 0.04 seen, d 0.3; the cheat 0x15edb0: +0x70 = 2.75) and the two records' springs `0x28cde8` (= `0x2777d8`) | [`update`] (`talking_npc::look_at_layout`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 5;
pub const UPDATE_FN: u32 = 0x31_7aa0;
pub const CLASSES: [i16; 2] = [920, 447];
/// The scene actors whose head the cheat swells, and by how much.
pub const SCENE_HEADS: [i16; 2] = [0x398, 0x1bf];
pub const SCENE_HEAD_SCALE: f32 = f32::from_bits(0x4006_6666);
const LOOK: LookLayout = LookLayout { pitch: (0x00, 0), yaw: (0x80, 1), glance: 0x100, seen: 0x118, glance_timer: 0x11c, gate_seq_b: true, eye: 1.0, pitch_k: 1.0, yaw_a: 0.6, yaw_b: 0.4, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
const CUBOID: usize = 0x120;
const LINK_A: usize = 0x124;
const LINK_B: usize = 0x128;
const SIZE: usize = 0x130;

fn moby(w: &World, idx: i32) -> Option<MobyId> { usize::try_from(idx).ok().filter(|&m| m < w.table.mobys.len()) }

/// Level05 `0x317aa0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { w.mm(id).pvars.resize(SIZE, 0); }
    crate::moby_update::manip::scene_big_head(w, &SCENE_HEADS, 0, SCENE_HEAD_SCALE);
    if w.m(id).visible != 0 {
        let cam = w.camera.map(|x| f32::from_bits(x.0));
        if c::dist3(c::pos(w, id), cam) < 30.0 {
            crate::shadows::probe_down(w, id);
            w.mm(id).b7f = 0x18;
        }
    }
    match w.m(id).state {
        0 => {
            w.mm(id).update_dist = 0xff;
            if let Some(a) = moby(w, c::pi32(w, id, LINK_A)) {
                if w.m(a).state != 0 {
                    if let Some(b) = moby(w, c::pi32(w, id, LINK_B)) { w.mm(b).state = 3; }
                    w.delete_moby(id);
                    return;
                }
            }
            w.mm(id).state = 1;
        }
        1 => {
            let cub = c::pi32(w, id, CUBOID);
            if cub != -1 && w.in_cuboid(w.hero_point(), cub) {
                crate::moby_update::interact::set_talked(w, id, 1);
                w.mm(id).state = 2;
            }
        }
        2 => w.mm(id).state = 3,
        _ => {}
    }
    look_at_layout(w, id, &LOOK);
}
