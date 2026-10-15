//! **Kalebo III's Map-O-Matic giver** (class 1377, level16 `0x2e3190`, census U529, one instance; talk block +0x20):
//! talkable inside his cuboid +0x160 while Ratchet is on the ground and not grinding (groups 0xf / 0x16); after the
//! talk Ratchet is put at (300.73, 206.04, 123.22), Euler (0, 0.11, 0), help 16002, the Map-O-Matic (item 33), his mission,
//! the checkpoint at Ratchet and a save. He also sets global flag 0x70 when Ratchet enters cuboid +0x164 and flag 0x71
//! the first time Ratchet grinds. Read from the level16 decomp.
//!
//! **System or not.** Per-class code from the NPC template (look-at layout row), the talk system, `story`.
//!
//! | address | what | status |
//! |---|---|---|
//! | top | `0x2e30a8` (the big-head scene manipulator) | `manip::scene_big_head` |
//! | state 0 | `NpcTalkRegister(+0x20)`, → 1, radius (+0x2c) 20, update distance 0xff | [`update`] |
//! | state 1 | air ticks (0x13f65e) 0, group ≠ 0xf / 0x16, in cuboid +0x160, `NpcTalkUpdate(+0x20)` → 2 | [`update`] |
//! | state 2 | game mode ≠ 2: `HeroTeleport((300.73, 206.04, 123.22), (0, 0.11, 0), 0, 1)` (0x1d96c0 / 0x1d96d0), `Help_Request(16002, 0x84)`, `GiveItem(33, 1)`, `SetMissionDone(+0xb0)`, the checkpoint at Ratchet, `memcard_Save`, → 3 | [`update`] |
//! | always | flag 0x70 (0x13d3f8) clear and Ratchet in cuboid +0x164 → set; flag 0x71 (0x13d3f9) clear and grinding (group 0xf) → set | [`update`] |
//! | tail | the look-at (records +0x60 list 0 / +0xe0 list 2, glance +0x170, s32 +0x168 / +0x16c, eye 1.5, pitch × 1, yaw 0.7 / 0.3) | [`talking_npc::look_at_layout`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::talking_npc::{look_at_layout, LookLayout};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2e_3190;
pub const CLASSES: [i16; 1] = [1377];

const T: usize = 0x20;
const LOOK: LookLayout = LookLayout { pitch: (0x60, 0), yaw: (0xe0, 2), glance: 0x170, seen: 0x168, glance_timer: 0x16c, gate_seq_b: true, eye: 1.5, pitch_k: 1.0, yaw_a: 0.7, yaw_b: 0.3, short_timers: false, gate_main: 0, gate_alt: 0xff, k_seen: 0.04 };
/// 0x1d96c0 / 0x1d96d0: where Ratchet stands after the talk.
const AFTER: ([f32; 3], [f32; 3]) = ([300.73, 206.04, 123.22], [0.0, 0.11, 0.0]);

/// Level16 `0x2e3190` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x180);
    interact::poll_scene_end_at(w, id, T);
    // 0x2e30a8: the actors' big-head cheat on the scene actors of [1377] (list 1, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[1377], 1, 2.75);
    match w.m(id).state {
        0 => {
            interact::talk_register_at(w, id, T);
            w.mm(id).state = 1;
            p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, 20.0);
            w.mm(id).update_dist = 0xff;
        }
        1 => {
            let g = w.hero.group;
            if w.hero.air_ticks == 0 && g != 0xf && g != 0x16 && story::hero_in(w, p::i32(&w.m(id).pvars, 0x160)) && interact::talk_update_at(w, id, T) {
                w.mm(id).state = 2;
            }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => {
            crate::cinematic::hero_teleport(w, AFTER.0, AFTER.1, 0, true);
            w.svc.help.request(0x3e82, 0x84);
            interact::give_item(w, 33, true);
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            let (pos, yaw) = (crate::hero::physics::to_f32x3(w.hero.pos), w.hero.yaw().to_f32());
            crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos, rot: [0.0, 0.0, yaw] });
            crate::cinematic::save(w);
            w.mm(id).state = 3;
        }
        _ => {}
    }
    if story::flag(w, 0x70) == 0 && story::hero_in(w, p::i32(&w.m(id).pvars, 0x164)) { story::set_flag(w, 0x70, 1); }
    if story::flag(w, 0x71) == 0 && w.hero.group == 0xf { story::set_flag(w, 0x71, 1); }
    look_at_layout(w, id, &LOOK);
}
