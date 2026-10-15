//! **Eudora's story drivers** (level 04; read from the level04 decomp):
//!
//! * **1120 the Suck Cannon** (`0x2e1a10`, census U169, one instance): a spinning, glowing item. Within 3 (XY) of
//!   Ratchet it hides and plays scene 2; after the scene: the banner (3995, or 21471 when the item is owned
//!   already), the Suck Cannon (item 9, equipped), the checkpoint at the moby +0x04, a save; then it is gone.
//! * **1190 the Blarg informant** (`0x2e3078`, U170, one instance): a talker whose talk's node 2 unlocks Blarg
//!   (planet 6) and sets his mission; Ratchet is put 2 in front of him facing him when the scene ends, the game is
//!   saved and he hides for good. With his mission done at load he is hidden at once.
//!
//! **System or not.** Per-class code on the shared systems (talk, the item glow = the infobot's / gold bolt's code,
//! checkpoint, `story`, cinematic).
//!
//! ## 1120 coverage (level04 `0x2e1a10`)
//!
//! | address | what | status |
//! |---|---|---|
//! | head | yaw += dt·π/2; game mode 2: mode \|= 0x41; else mode &= ~0x41 when hidden | [`suck_cannon_update`] |
//! | state 0 | the glow init (`0x2e1c20`: the infobot's, base +0x10); Suck Cannon acquired (0x13d4f1) → `DeleteMoby`; else z += 1, → 1 | [`suck_cannon_update`] (`gold_bolt::glow_init`) |
//! | state 1 | the glow (`0x2e1d00`: z + 0.25, the sprites only outside game mode 2); Ratchet within 3 (XY): mode \|= 0x41, `DialogStreamStart(2)`, → 2 | [`suck_cannon_update`] (`gold_bolt::item_glow`) |
//! | state 2 | game mode ≠ 2: `ShowBanner(3995, −1)` (21471 when owned, 0x13d4c9), the countdown 0x15f640 = 180 (unscaled), `GiveItem(9, 1)`, the checkpoint at moby +0x04's position / rotation (−1: `STUB_printf`), → 3, `memcard_Save` | [`suck_cannon_update`] |
//! | state 3 | `DeleteMoby` | [`suck_cannon_update`] |
//!
//! ## 1190 coverage (level04 `0x2e3078`)
//!
//! | address | what | status |
//! |---|---|---|
//! | head | drawn and within 30 of the camera (0x166f40): the shadow probe, +0x7f = 0x18 | [`informant_update`] (`shadows::probe_down`) |
//! | head | `0x2e2f90`: the big-head cheat's scene manipulator | `manip::scene_big_head` |
//! | state 0 | mission (+0xb0) not done → 1, `NpcTalkRegister`; done → 3 (collision off, mode \|= 0x41) | [`informant_update`] |
//! | state 1 | talk radius 255 in cuboid +0x140, else 1; `NpcTalkUpdate` → 2 | [`informant_update`] |
//! | state 2 | node played (talk +0x04) ≠ 2: outside game mode 2, `NpcTalkUpdate` again (the dialogue goes on); node 2: `UnlockPlanet(6)`, `SetMissionDone(+0xb0)`, the scene-end place 2 along row 0 facing him (yaw + 0x40490fd0, his pitch / roll), `memcard_Save`, → 3 hidden | [`informant_update`] |
//! | state 3 | collision off, mode \|= 0x41 | [`informant_update`] |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::interact::{self, talk};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 4;
pub const SUCK_FN: u32 = 0x2e_1a10;
pub const SUCK_CLASSES: [i16; 1] = [1120];
pub const INFORMANT_FN: u32 = 0x2e_3078;
pub const INFORMANT_CLASSES: [i16; 1] = [1190];

/// Level04 `0x2e1a10` (1120; module doc).
pub fn suck_cannon_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x50);
    let y = c::add_rot(c::yaw(w, id), DT * std::f32::consts::FRAC_PI_2);
    c::set_yaw(w, id, y);
    if w.svc.game_mode == 2 {
        w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
    } else if w.m(id).mode & mode::HIDDEN != 0 {
        w.mm(id).mode &= !(mode::HIDDEN | mode::NO_ANIM);
    }
    match w.m(id).state {
        0 => {
            gold_bolt::glow_init(w, id, 0x10);
            if w.svc.interact.game.acquired.get(9).is_some_and(|&b| b != 0) {
                w.delete_moby(id);
                return;
            }
            w.mm(id).state = 1;
            w.mm(id).position[2] += 1.0;
        }
        1 => {
            let mut centre = w.m(id).position;
            centre[2] += 0.25;
            let spawn = w.svc.game_mode != 2;
            gold_bolt::item_glow(w, id, 0x10, [centre[0], centre[1], centre[2]], 1.0, spawn);
            if c::dist2(c::pos(w, id), story::hero4(w)) < 3.0 {
                w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
                crate::cinematic::start_scene(w, 2, false);
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode == 2 { return; }
            let msg = if w.inventory.owned(9) { 0x53df } else { 0xfa7 };
            crate::cinematic::show_banner(w, msg, 0xb4);
            interact::give_item(w, 9, true);
            match story::link(w, p::i32(&w.m(id).pvars, 4)) {
                Some(m) => {
                    let (pos, rot) = (w.m(m).position, w.m(m).rotation);
                    crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos: [pos[0], pos[1], pos[2]], rot: [rot[0], rot[1], rot[2]] });
                }
                None => w.svc.unported("eudora 1120: no checkpoint moby (STUB_printf)"),
            }
            w.mm(id).state = 3;
            crate::cinematic::save(w);
        }
        3 => w.delete_moby(id),
        _ => {}
    }
}

fn hide_for_good(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    m.state = 3;
    m.has_collision = false;
    m.mode |= mode::HIDDEN | mode::NO_ANIM;
}

/// Level04 `0x2e3078` (1190; module doc).
pub fn informant_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x144);
    interact::poll_scene_end(w, id);
    if w.m(id).visible != 0 && c::len3(c::sub(c::pos(w, id), w.camera.map(|x| f32::from_bits(x.0)))) < 30.0 {
        crate::shadows::probe_down(w, id);
        w.mm(id).b7f = 0x18;
    }
    // 0x2e2f90: the actors' big-head cheat on the scene actors of [1190] (list 0, 2.75).
    crate::moby_update::manip::scene_big_head(w, &[1190], 0, 2.75);
    match w.m(id).state {
        0 => {
            if !story::mission_done(w, w.m(id).mission as i32) {
                w.mm(id).state = 1;
                interact::talk_register(w, id);
            } else {
                hide_for_good(w, id);
            }
        }
        1 => {
            let inside = story::hero_in(w, p::i32(&w.m(id).pvars, 0x140));
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, if inside { 255.0 } else { 1.0 });
            if interact::talk_update(w, id) { w.mm(id).state = 2; }
        }
        2 => {
            if p::i16(&w.m(id).pvars, talk::LAST) != 2 {
                if w.svc.game_mode != 2 { interact::talk_update(w, id); }
                return;
            }
            crate::cinematic::unlock_planet(w, 6);
            let mission = w.m(id).mission;
            crate::cinematic::set_mission_done(w, mission);
            let m = w.m(id);
            let r = c::set_len3(m.rows[0], 2.0);
            let pos = [m.position[0] + r[0], m.position[1] + r[1], m.position[2] + r[2]];
            let yaw = c::add_rot(m.rotation[2], f32::from_bits(0x4049_0fd0));
            w.svc.interact.scene_end_place = Some((pos, yaw));
            crate::cinematic::save(w);
            hide_for_good(w, id);
        }
        3 => {
            w.mm(id).has_collision = false;
            w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
        }
        _ => {}
    }
}
