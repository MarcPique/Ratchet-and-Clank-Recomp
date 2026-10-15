//! **The Thruster-Pack floor buttons, class 1179** (level11 `0x30ee00`, 2 placed; level15's copy `0x2d9100` is the
//! same code; census U389): Ratchet stomps one (state 0x22) standing on it with the Thruster-Pack and it sinks 1.5 for
//! good. On Pokitaru (planet 0xb) a pressed button with a mission byte is recorded (`0x1bb424` / `0x1bc084`: level 11's
//! killed and collected bytes) and comes back pressed. Near a button with a mission byte, on foot, the help asks for
//! the Thruster-Pack (message 0x2b02, record 0x71) or, with it, to stomp (0x2b01, record 0x6c). Read from the level11
//! decomp.
//!
//! **Pvars** (0x70, mode 0x20): +0x00 the creature record's offset (0x20; +0x3e its flags).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | help record 0x6c (0x141cc8): count and time = 0; +0x3e \|= 8; planet 0xb and a mission byte (+0xb0 ≠ 0xff): collected byte `0x1bc084[+0xb2]` ≠ 0 or the level's death bit (`0x14cc90`) → pressed; else → 1 | [`update`] (`SaveBits::collected` / `death`) |
//! | pressed | → 2, z −= 1.5, `PlayClassSound(0, 0)` | [`update`] (`press`) |
//! | state 1 | Ratchet on it (0x13f64c, grounded) with the Thruster-Pack (0x13d4c3) in state 0x22: planet 0xb and a mission byte → `0x1bb424[+0xb2]` = mission + 2, and the collected byte when the mission is loaded and not done (`story::kill_record`); pressed | [`update`] |
//! | | a mission byte, within 2 (`vec_distance2`) and the movement group (0x1413dc) ≤ 1: no Thruster-Pack → the reminder of record 0x71, `Help_Request(0x2b02, 0x71)`; else record 0x6c: count 0 → armed, else the reminder, `Help_Request(0x2b01, 0x6c)` | [`update`] (`hints::remind` / `hints::arm_or_remind`) |
//! | state 2 | nothing | n/a |

use super::hints;
use crate::moby_runtime::MobyId;
use crate::moby_update::creature as c;
use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x30_ee00;
pub const CLASSES: [i16; 1] = [1179];
/// The planet whose buttons are saved (0x15ed84 = 0xb).
pub const SAVED_LEVEL: u32 = 11;
/// The Thruster-Pack (0x13d4c0 + 3).
pub const THRUSTER_PACK: usize = 3;
/// Ratchet's stomp.
pub const STOMP: i32 = 0x22;
/// The record flags (+0x20 record, +0x1e).
const FLAGS: usize = 0x3e;
const NEED_PACK: (i32, usize) = (0x2b02, 0x71);
const STOMP_HINT: (i32, usize) = (0x2b01, 0x6c);

fn press(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    m.state = 2;
    m.position[2] -= 1.5;
    w.play_sound(0, 0, id);
}

/// Level11 `0x30ee00` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < FLAGS + 2 { return; }
    let (mission, sid) = (w.m(id).mission, w.m(id).spawn_id);
    let saved = w.svc.level == SAVED_LEVEL && mission != 0xff;
    match w.m(id).state {
        0 => {
            let r = &mut w.svc.help.records.help[STOMP_HINT.1];
            r.count = 0;
            r.time = 0;
            let f = c::pi16(w, id, FLAGS) | 8;
            c::set_pi16(w, id, FLAGS, f);
            let done = saved
                && (w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, sid)));
            if done { press(w, id) } else { w.mm(id).state = 1 }
        }
        1 => {
            let h = &w.hero;
            if h.ground_moby == Some(id) && h.air_ticks == 0 && h.state == STOMP && hints::owned(w, THRUSTER_PACK) {
                if saved { crate::moby_update::story::kill_record(w, id); }
                press(w, id);
            }
            if mission == 0xff || 4.0 <= c::dist2(w.m(id).position, super::hero_pos(w)) || 1 < w.hero.group as u32 { return; }
            if !hints::owned(w, THRUSTER_PACK) {
                hints::remind(w, NEED_PACK.1, NEED_PACK.0, NEED_PACK.1 as i32);
            } else {
                hints::arm_or_remind(w, STOMP_HINT.1, STOMP_HINT.0, STOMP_HINT.1 as i32);
            }
        }
        _ => {}
    }
}
