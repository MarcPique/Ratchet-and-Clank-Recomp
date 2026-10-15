//! **Drek's fleet's item scene** (class 1428, level17 `0x2f1790`, census U561, one instance): a spinning, glowing item
//! (scale × gp−0x4860, 1.0). Within 3 (3-D) of Ratchet, unless he flies a ship (state 0x32), it hides and plays scene
//! 1; after the scene: banner 17002 (countdown `ticks(180)`), global flags 0x78 and 2 set, a save; then it is gone
//! (at once when flag 2 is set). The whole update is skipped on level 15. Read from the level17 decomp.
//!
//! **System or not.** Per-class code; the glow is the infobot's (`gold_bolt`, base +0x00).
//!
//! | address | what | status |
//! |---|---|---|
//! | head | level 15 (0x15ed84 = 0xf): nothing; yaw += dt·π/2; scale = class scale × 1.0 | [`update`] |
//! | state 0 | the glow init (`0x2f1488`); flag 2 (0x13d38a) set → `DeleteMoby`; else → 1, z += 1 | [`update`] |
//! | state 1 | the glow (`0x2f1568`: z + 0.5); within 3 (3-D) and Ratchet not in state 0x32: mode \|= 0x41, `DialogStreamStart(1)`, → 2 | [`update`] |
//! | state 2 | game mode ≠ 2: `ShowBanner(17002, −1)`, countdown `ticks(180)`, flag 0x78 (0x13d400) = 1, flag 2 = 1, → 3, `memcard_Save` | [`update`] |
//! | state 3 | `DeleteMoby` | [`update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2f_1790;
pub const CLASSES: [i16; 1] = [1428];

/// Level17 `0x2f1790` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.svc.level == 0xf { return; }
    story::pvars(w, id, 0x40);
    let y = c::add_rot(c::yaw(w, id), DT * std::f32::consts::FRAC_PI_2);
    c::set_yaw(w, id, y);
    let k = w.classes.info(w.m(id).o_class).map_or(1.0, |i| i.scale);
    w.mm(id).scale = k;
    match w.m(id).state {
        0 => {
            gold_bolt::glow_init(w, id, 0);
            if story::flag(w, 2) != 0 {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            w.mm(id).position[2] += 1.0;
        }
        1 => {
            let mut centre = w.m(id).position;
            centre[2] += 0.5;
            gold_bolt::item_glow(w, id, 0, [centre[0], centre[1], centre[2]], 1.0, true);
            if c::len3(c::sub(c::pos(w, id), story::hero4(w))) < 3.0 && w.hero.state != 0x32 {
                w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
                crate::cinematic::start_scene(w, 1, false);
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode == 2 { return; }
            let t = w.ticks(0xb4);
            crate::cinematic::show_banner(w, 0x426a, t);
            story::set_flag(w, story::flag_index(0x13_d400), 1);
            story::set_flag(w, 2, 1);
            w.mm(id).state = 3;
            crate::cinematic::save(w);
        }
        3 => w.delete_moby(id),
        _ => {}
    }
}
