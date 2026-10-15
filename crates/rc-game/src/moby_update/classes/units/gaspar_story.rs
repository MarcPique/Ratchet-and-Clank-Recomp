//! **The Pilot's Helmet pickup** (class 1290, level01 `0x30a6d0`: the same code in all 19 overlays, placed once, on
//! Gaspar (level09 `0x308818`); census U321; G-CUT-004's "0x30a6d0" scene source). A spinning, glowing helmet: within
//! 3 (XY) and 2 (z) of a living Ratchet it hides and plays scene P[0]; after it: banner 9002 (countdown 180), the
//! Pilot's Helmet (item 7) given and put on (the head slot's request 0x141410 = 7), the help 9000 when its record
//! 0x34 was never shown, the moby P[1] made permanent, its mission set, a save; then it is gone. Read from the level01
//! decomp.
//!
//! **System or not.** Per-class code; the glow is the infobot's (`gold_bolt::glow_init` / `item_glow` at base +0x10).
//!
//! | address | what | status |
//! |---|---|---|
//! | head | yaw += dt·π/2 | [`update`] |
//! | state 0 | the glow init (`0x30a9d8`); P[0] = −1: `STUB_printf`; Pilot's Helmet owned (0x13d4c7) → `STUB_printf`, `DeleteMoby`; else → 1, z += 1 | [`update`] |
//! | state 1 | the glow (`0x30aab8`, every tick); Ratchet within 3 (XY), \|Δz\| < 2 and health ≠ 0: mode \|= 0x41, `DialogStreamStart(P[0])` (−1: `STUB_printf`), → 2 | [`update`] |
//! | state 2 | game mode ≠ 2: `ShowBanner(9002, −1)`, countdown 0x15f640 = 180, `GiveItem(7, 1)`, help record 0x34 unused → `Help_Request(9000, 0x34)`; the moby P[1] made permanent (killed / collected bytes; −1: `STUB_printf`); mission ≠ 0xff → `SetMissionDone`; 0x141410 = 7; → 3 | [`update`] (`HeroFields::head_request`) |
//! | state 3 | `memcard_Save`, `DeleteMoby` | [`update`] |
//! | `0x30a610` / `0x30a670` | the riding-platform and leave helpers next to it (not called by this class) | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::interact;
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 1;
pub const UPDATE_FN: u32 = 0x30_a6d0;
pub const CLASSES: [i16; 1] = [1290];
/// The Pilot's Helmet.
const HELMET: usize = 7;

/// Level01 `0x30a6d0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x50);
    let y = c::add_rot(c::yaw(w, id), DT * std::f32::consts::FRAC_PI_2);
    c::set_yaw(w, id, y);
    match w.m(id).state {
        0 => {
            gold_bolt::glow_init(w, id, 0x10);
            if p::i32(&w.m(id).pvars, 0) == -1 { w.svc.unported("helmet 1290: no scene (STUB_printf)"); }
            if w.inventory.owned(HELMET) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            w.mm(id).position[2] += 1.0;
        }
        1 => {
            let pos = w.m(id).position;
            gold_bolt::item_glow(w, id, 0x10, [pos[0], pos[1], pos[2]], 1.0, true);
            let h = story::hero4(w);
            if 3.0 <= c::dist2(c::pos(w, id), h) || 2.0 <= (pos[2] - h[2]).abs() || w.hero.health == 0 { return; }
            w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
            match usize::try_from(p::i32(&w.m(id).pvars, 0)) {
                Ok(scene) => crate::cinematic::start_scene(w, scene, false),
                Err(_) => w.svc.unported("helmet 1290: no scene (STUB_printf)"),
            }
            w.mm(id).state = 2;
        }
        2 => {
            if w.svc.game_mode == 2 { return; }
            crate::cinematic::show_banner(w, 0x232a, 0xb4);
            interact::give_item(w, HELMET, true);
            if w.svc.help.records.help[0x34].count == 0 { w.svc.help.request(9000, 0x34); }
            match story::link(w, p::i32(&w.m(id).pvars, 4)) {
                Some(m) => story::kill_record(w, m),
                None => w.svc.unported("helmet 1290: no linked moby (STUB_printf)"),
            }
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            w.hero_fields_mut().head_request = Some(HELMET as i32);
            w.mm(id).state = 3;
        }
        3 => {
            crate::cinematic::save(w);
            w.delete_moby(id);
        }
        _ => {}
    }
}
