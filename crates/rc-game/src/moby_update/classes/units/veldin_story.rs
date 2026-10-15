//! **Veldin's Clank** (class 834, level00 `0x2d9dc8`, census U31, one instance; talk block +0x20): the robot Ratchet
//! meets on Veldin. His first update hides Clank on Ratchet's back (0x141628 = 1) and sets global flag 8 (the
//! "Clank met" bit the music and the transition read: `GameState::on_veldin_clank_init` is the direct-boot copy of it);
//! the talk then plays its nodes, and once a node past 3 has played the level is left for Novalis (planet 1:
//! `0x2a29a0(1)`, the travel lane's `DoSpaceTransition`). Read from the level00 decomp.
//!
//! **System or not.** Per-class code on the talk system, `story` (flags, the level exit), the hero-block channel.
//!
//! | address | what | status |
//! |---|---|---|
//! | head | game mode 2: not drawn (+0x31 = 0), mode \|= 1; else drawn, mode &= ~1 | [`update`] |
//! | state 0 | 0x141628 = 1; `NpcTalkRegister(+0x20)`; flag 8 (0x13d390) clear: radius 255; set: node (+0x56) = 1, radius 3, node played (+0x24) = 0; flag 8 = 1; → 1; update distance 0xff | [`update`] (`HeroFields::clank_hidden`, `story::set_flag`) |
//! | state 1 | node ≠ 0: radius 16 inside cuboid +0x60, else 3; `NpcTalkUpdate(+0x20)` → 2, radius 3; node played > 3 and game mode ≠ 6: `0x2a29a0(1)` (`FUN_0028eeb0`) | [`update`] (`story::level_exit`) |
//! | state 2 | game mode ≠ 2 → 1 | [`update`] |
//! | | no look-at, idle, sound or item of his own | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 0;
pub const UPDATE_FN: u32 = 0x2d_9dc8;
pub const CLASSES: [i16; 1] = [834];

const T: usize = 0x20;
/// Global flag 8 (0x13d390): Clank met.
const FLAG_CLANK: usize = crate::game_state::FLAG_VELDIN_CLANK;

/// Level00 `0x2d9dc8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x64);
    interact::poll_scene_end_at(w, id, T);
    if w.svc.game_mode == 2 {
        w.mm(id).visible = 0;
        w.mm(id).mode |= mode::HIDDEN;
    } else {
        w.mm(id).visible = 1;
        w.mm(id).mode &= !mode::HIDDEN;
    }
    match w.m(id).state {
        0 => {
            w.hero_fields_mut().clank_hidden = Some(1);
            interact::talk_register_at(w, id, T);
            if story::flag(w, FLAG_CLANK) == 0 {
                p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, 255.0);
            } else {
                let pv = &mut w.mm(id).pvars;
                p::set_i16(pv, T + talk::NODE, 1);
                p::set_ff(pv, T + talk::RADIUS, 3.0);
                p::set_i16(pv, T + talk::LAST, 0);
            }
            story::set_flag(w, FLAG_CLANK, 1);
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
        }
        1 => {
            if p::i16(&w.m(id).pvars, T + talk::NODE) != 0 {
                let inside = story::hero_in(w, p::i32(&w.m(id).pvars, 0x60));
                p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, if inside { 16.0 } else { 3.0 });
            }
            if interact::talk_update_at(w, id, T) {
                w.mm(id).state = 2;
                p::set_ff(&mut w.mm(id).pvars, T + talk::RADIUS, 3.0);
            }
            if p::i16(&w.m(id).pvars, T + talk::LAST) > 3 && w.svc.game_mode != 6 { story::level_exit(w, 1); }
        }
        2 if w.svc.game_mode != 2 && w.svc.interact.talker != Some(id) => { w.mm(id).state = 1; }
        _ => {}
    }
}
