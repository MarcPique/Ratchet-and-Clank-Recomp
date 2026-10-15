//! Gold-weapon offers, classes 304 (the offer) and 1456–1465 (the props that go with each gold weapon):
//! `ItemOfferUpdate` level01 0x2e1ac0. Spec: `docs/plan/moby_update_catalogue.md` "In the port: teleporter pads and
//! item offers"; the buy prompt is the shared NPC talk system (`docs/plan/interaction.md` §3,
//! [`crate::moby_update::interact`]).
//!
//! The pvar block starts with the talk block (`interact::talk`); pvar+0x40 (s32) is the offer's index k into the
//! gold-weapon table 0x1deb10 (level01 `.data`: `[19, 10, 11, 16, 17, 15, 21, 20, 25, 9]`, item ids); the gold
//! weapon is owned when `0x13e520[item]` ≠ 0. On Novalis the offers are talk slots 3..12 (instance +0x74), whose
//! node lists read "You must own the Tesla Claw …" (condition kind 2: the plain weapon owned), "You need 60,000
//! bolts and 4 Gold Bolts …" (kind 6 false) and "△ Buy the Gold Tesla Claw for 60,000 bolts and 4 Gold Bolts"
//! (kind 6 true; the price is printed from the record's +0x14).
//!
//! Per tick:
//! * **the challenge gate**: k ≠ −1, k even and the times-completed count `0x15ee20` = 0 → `DeleteMoby`, return
//!   (on a first playthrough the even offers and their props are gone after the load pass);
//! * state 0: class 304: gold weapon not owned and `NpcTalkRegister` ≠ −1 → state 1, else state 3. The props
//!   (1456–1465) → state 2;
//! * state 1: talk radius +0x0c = 1.9, `NpcTalkUpdate`; when the node advanced last (+0x04) is 2 (the buy node, which
//!   has no scene): the gold weapon owned, bolts −= its gold price (`0x1c4544 + 0x18·item`), `FUN_002aea70(offer)`
//!   (the upgrade's demo: [`Handoff::GoldUpgrade`], played by the engine's vendor mode), state 4;
//! * state 2: the prop is deleted once its gold weapon is owned;
//! * state 4: outside game mode 0 wait; else `memcard_Save(0, −1)` (`crate::cinematic::save`) and state 3.

use crate::moby_runtime::MobyId;
use crate::moby_update::interact::{self, talk, GameWrite, Handoff};
use crate::moby_update::services::{pvar as p, World};

/// The update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2e1ac0;

/// Classes that run [`update`].
pub const CLASSES: [i16; 11] = [304, 1456, 1457, 1458, 1459, 1460, 1461, 1462, 1463, 1464, 1465];

/// The offer class (0x130).
const OFFER: i16 = 304;

/// `0x1deb10`: offer index → gold weapon item id.
pub const GOLD_ITEMS: [i32; 10] = [19, 10, 11, 16, 17, 15, 21, 20, 25, 9];

/// The talk radius state 1 writes (0x3ff33333).
pub const RADIUS: f32 = 1.9;

fn gold_item(k: i32) -> Option<usize> { usize::try_from(k).ok().and_then(|k| GOLD_ITEMS.get(k)).map(|&i| i as usize) }

fn owned(w: &World, k: i32) -> bool {
    let Some(item) = gold_item(k) else { return false };
    w.svc.counters.gold_weapons.get(item).is_some_and(|&b| b != 0)
}

/// `ItemOfferUpdate` (0x2e1ac0).
pub fn update(w: &mut World, id: MobyId) {
    let k = if w.m(id).pvars.len() >= 0x44 { p::i32(&w.m(id).pvars, 0x40) } else { -1 };
    if k != -1 && k & 1 == 0 && w.svc.counters.times_completed == 0 {
        w.delete_moby(id);
        return;
    }
    match w.m(id).state {
        0 => {
            let next = if w.m(id).o_class == OFFER {
                if !owned(w, k) && interact::talk_register(w, id) != -1 { 1 } else { 3 }
            } else {
                2
            };
            w.mm(id).state = next;
        }
        1 => {
            p::set_ff(&mut w.mm(id).pvars, talk::RADIUS, RADIUS);
            interact::talk_update(w, id);
            if p::i16(&w.m(id).pvars, talk::LAST) != 2 { return; }
            let Some(item) = gold_item(k) else { return };
            if let Some(b) = w.svc.counters.gold_weapons.get_mut(item) { *b = 1; }
            if let Some(b) = w.svc.interact.game.gold_weapons.get_mut(item) { *b = 1; }
            w.svc.interact.writes.push(GameWrite::GoldWeapon(item));
            w.mm(id).state = 4;
            let price = w.svc.interact.tables.shop.gold_price(item) as i32;
            w.svc.counters.bolts -= price;
            w.svc.interact.handoffs.push(Handoff::GoldUpgrade { offer: id, item: item as i32 });
            // FUN_002aea70 (the upgrade's mode 5) closes the help box (`FUN_002258b0`).
            w.svc.help.kill();
        }
        2 => {
            if owned(w, k) { w.delete_moby(id); }
        }
        4 => {
            if w.svc.game_mode != 0 { return; }
            crate::cinematic::save(w);
            w.mm(id).state = 3;
        }
        _ => {}
    }
}
