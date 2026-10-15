//! **Orxon's story pickups and the nanotech seller** (level 10; read from the level10 decomp):
//!
//! * **18 the Magneboots** (`0x298668`, census U329, one instance; in Clank's section): a spinning, glowing item
//!   (scaled to 0.667). Within 3 (XY) and 2 (z) of a living hero it hides and plays scene 2; after it: banner 10011
//!   (countdown 180), the Magneboots (item 28) given and equipped, the checkpoint at the moby P[1], a save; then gone.
//! * **1326 the Nanotech seller** (`0x2e8358`, U350, one instance; G-SAV-005): faced within 4 (XY), not while his
//!   2-second cool-down runs, he offers the Premium Nanotech (4,000 bolts; global flag 4, max health 5) and then the
//!   Ultra Nanotech (30,000; flag 5, max health 8) through the △ prompt ("not enough bolts" below the price). A sale
//!   takes the bolts, sets the flag, max health and health, plays scene 0 / 1, and after the scene saves and shows
//!   banner 10012 / 10013.
//!
//! **System or not.** Per-class code: the item glow is the infobot's (`gold_bolt`), the prompt is the shared
//! `try_set_help_message` (`Interact::try_prompt`), the stores are `story`'s (G-SAV-005: `set_flag`, `set_max_hp`,
//! `set_health`, `add_bolts`).
//!
//! ## 18 coverage (level10 `0x298668`)
//!
//! | address | what | status |
//! |---|---|---|
//! | head | yaw += dt·π/2 | [`magneboots_update`] |
//! | state 0 | the glow init (`0x298860`); Magneboots acquired (0x13d504) → `DeleteMoby`; else → 1, z += 0.5, scale × 0.667 | [`magneboots_update`] |
//! | state 1 | the glow (`0x298940`: z + 0.5); within 3 (XY), \|Δz\| < 2, health ≠ 0: mode \|= 0x41, `DialogStreamStart(2)`, → 2 | [`magneboots_update`] |
//! | state 2 | game mode ≠ 2: `ShowBanner(10011, −1)`, countdown 180, `GiveItem(28, 1)`, the checkpoint at moby P[1] (−1: none), `memcard_Save`, → 3 | [`magneboots_update`] |
//! | state 3 | `DeleteMoby` | [`magneboots_update`] |
//!
//! ## 1326 coverage (level10 `0x2e8358`)
//!
//! | address | what | status |
//! |---|---|---|
//! | head | `FastDecTimer(P+4)` | [`nanotech_update`] |
//! | state 0 | Ratchet's feet within π/2 of his yaw and 4 (XY), P+4 = 0, game mode ≠ 2: `SetTalked(m, 1)`; flag 4 clear: bolts < 4001 → prompt 10016; else prompt 10014 and △ with the prompt held: flag 4 = 1, max health 5, health 5, bolts −= 4000, scene 0, → 1; flag 4 set and flag 5 clear (P+4 = 0): bolts < 30001 → prompt 10017; else prompt 10015 and △: flag 5 = 1, max health 8, health 8, bolts −= 30000, scene 1, → 1 | [`nanotech_update`] |
//! | state 1 | game mode ≠ 2: `memcard_Save`, `ShowBanner(10012 / 10013 with flag 5, −1)`, countdown 180, P+4 = `ticks(240)`, → 0 | [`nanotech_update`] |
//! | | no look-at, idle or sound of his own | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::classes::gold_bolt;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::interact::{self, owner};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 10;
pub const MAGNEBOOTS_FN: u32 = 0x29_8668;
pub const MAGNEBOOTS_CLASSES: [i16; 1] = [18];
pub const NANOTECH_FN: u32 = 0x2e_8358;
pub const NANOTECH_CLASSES: [i16; 1] = [1326];

/// The two upgrades: (flag, price, max health, scene, offer prompt, short prompt).
const PREMIUM: (usize, i32, i32, usize, i32, i32) = (story::FLAG_PREMIUM_NANOTECH, 4000, 5, 0, 0x271e, 0x2720);
const ULTRA: (usize, i32, i32, usize, i32, i32) = (story::FLAG_ULTRA_NANOTECH, 30000, 8, 1, 0x271f, 0x2721);

/// Level10 `0x298668` (18; module doc).
pub fn magneboots_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x50);
    let y = c::add_rot(c::yaw(w, id), DT * std::f32::consts::FRAC_PI_2);
    c::set_yaw(w, id, y);
    match w.m(id).state {
        0 => {
            gold_bolt::glow_init(w, id, 0x10);
            if w.svc.interact.game.acquired.get(28).is_some_and(|&b| b != 0) {
                w.delete_moby(id);
                return;
            }
            let m = w.mm(id);
            m.state = 1;
            m.position[2] += 0.5;
            m.scale *= 0.667;
        }
        1 => {
            let mut centre = w.m(id).position;
            centre[2] += 0.5;
            gold_bolt::item_glow(w, id, 0x10, [centre[0], centre[1], centre[2]], 1.0, true);
            let (pos, h) = (w.m(id).position, story::hero4(w));
            if c::dist2(pos, h) < 3.0 && (pos[2] - h[2]).abs() < 2.0 && w.hero.health != 0 {
                w.mm(id).mode |= mode::HIDDEN | mode::NO_ANIM;
                crate::cinematic::start_scene(w, 2, false);
                w.mm(id).state = 2;
            }
        }
        2 => {
            if w.svc.game_mode == 2 { return; }
            crate::cinematic::show_banner(w, 0x271b, 0xb4);
            interact::give_item(w, 28, true);
            if let Some(m) = story::link(w, p::i32(&w.m(id).pvars, 4)) {
                let (pos, rot) = (w.m(m).position, w.m(m).rotation);
                crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos: [pos[0], pos[1], pos[2]], rot: [rot[0], rot[1], rot[2]] });
            }
            crate::cinematic::save(w);
            w.mm(id).state = 3;
        }
        3 => w.delete_moby(id),
        _ => {}
    }
}

/// One sale: the prompt (or the "not enough bolts" one), and with △ while the prompt is held the purchase.
fn offer(w: &mut World, id: MobyId, o: (usize, i32, i32, usize, i32, i32)) {
    let (flag, price, hp, scene, msg, short) = o;
    if w.svc.counters.bolts < price + 1 {
        w.svc.interact.try_prompt(owner::VENDOR, short);
        return;
    }
    let held = w.svc.interact.try_prompt(owner::VENDOR, msg);
    if !w.svc.interact.triangle() || held == 0 { return; }
    story::set_flag(w, flag, 1);
    story::set_max_hp(w, hp);
    story::set_health(w, hp);
    story::add_bolts(w, -price);
    crate::cinematic::start_scene(w, scene, false);
    w.mm(id).state = 1;
}

/// Level10 `0x2e8358` (1326; module doc).
pub fn nanotech_update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 8);
    c::dec_timer_pvar_i32(w, id, 4);
    match w.m(id).state {
        0 => {
            let (pos, h) = (c::pos(w, id), story::hero4(w));
            let facing = c::diff_rots(c::yaw(w, id), c::atan(h[0] - pos[0], h[1] - pos[1])) < std::f32::consts::FRAC_PI_2;
            if !(facing && c::dist2(pos, h) < 4.0 && p::i32(&w.m(id).pvars, 4) == 0 && w.svc.game_mode != 2) { return; }
            interact::set_talked(w, id, 1);
            if story::flag(w, PREMIUM.0) == 0 {
                offer(w, id, PREMIUM);
            } else if story::flag(w, ULTRA.0) == 0 && p::i32(&w.m(id).pvars, 4) == 0 {
                offer(w, id, ULTRA);
            }
        }
        1 => {
            if w.svc.game_mode == 2 { return; }
            crate::cinematic::save(w);
            let msg = if story::flag(w, ULTRA.0) != 0 { 0x271d } else { 0x271c };
            crate::cinematic::show_banner(w, msg, 0xb4);
            let t = w.ticks(0xf0);
            p::set_i32(&mut w.mm(id).pvars, 4, t);
            w.mm(id).state = 0;
        }
        _ => {}
    }
}
