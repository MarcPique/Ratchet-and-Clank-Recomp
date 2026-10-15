//! Ratchet's inventory and equipment: what he owns, what each item slot has equipped, and the rules that change
//! them (docs/plan/gadgets.md). One model for the whole game: the save-block fields ([`GameState`] /
//! [`SessionState`]) are the source of truth; the hero's item slots (`crate::hero::idle::ItemSlot`, one per slot)
//! carry them out tick by tick, and the menus (quick select, the Gadgets page, the vendor) only ever write the
//! same fields the game's menus write. Addresses are level01.elf.
//!
//! **Owned** `0x13d4c0 + id` (save chunk 10, `GameState::global.owned`), set by `GiveItem` 0x275760
//! ([`GameState::give_item`]); **acquired** `0x13d4e8 + id` (chunk 11).
//!
//! **Slots** (the item definitions' `+8` word, [`Slot`]): 0 hand, 1 feet, 2 head, 3 back; 4 and 5 are the
//! always-on extras (the Map-o-matic / Bolt Grabber and the Persuader: created by `HeroItemsCreate` 0x22f3c0
//! whenever they are owned, never equipped). Per slot `s` the game keeps:
//! * the **saved (equipped) item** `0x141660 + 4·s` (save chunk 32, `GameState::global.equipped[s]`);
//! * the **request** `0x141408 + 4·s` (session: `SessionState::temp_hand` / `temp_feet` / `temp_head` /
//!   `temp_back`; 0x26 = "take it off", [`NOTHING`]);
//! * the **target** `0x141424 + 4·s`, the **restore item** `0x141440 + 4·s` and its **request** `0x14145c + 4·s`
//!   (hero block, cleared at every level start: `crate::hero::idle::ItemSlot`);
//! * the **slot record** `0x1403e0 + 0x50·s`: its moby(s), `+0x24` state (2 ready, 3 being put away, 0 empty) and
//!   `+0x28` item. `GetClankModule(s)` 0x22ddd8 is the item while the state is 2, else −1 — the **ready item**.
//!
//! The Gadgets page equips through a copy of the four saved items (`0x1ba1a0[4]`, [`menu_select`]) and, on
//! close, requests every slot whose copy changed ([`menu_close_requests`], `PageMenuClose` 0x28c6c8).

use crate::game_state::{GameState, SessionState};

/// Item ids (the `0x13d4c0` table index; names from the item definitions' `+0` text ids, level01 0x179f40).
pub mod item {
    pub const CLANK: i32 = 1;
    pub const HELI_PACK: i32 = 2;
    pub const THRUSTER_PACK: i32 = 3;
    pub const HYDRO_PACK: i32 = 4;
    pub const SONIC_SUMMONER: i32 = 5;
    pub const O2_MASK: i32 = 6;
    pub const PILOTS_HELMET: i32 = 7;
    pub const WRENCH: i32 = 8;
    pub const BOMB_GLOVE: i32 = 10;
    pub const SWINGSHOT: i32 = 12;
    pub const HYDRODISPLACER: i32 = 22;
    pub const TRESPASSER: i32 = 26;
    pub const METAL_DETECTOR: i32 = 27;
    pub const MAGNEBOOTS: i32 = 28;
    pub const GRINDBOOTS: i32 = 29;
    pub const HOVERBOARD: i32 = 30;
    pub const HOLOGUISE: i32 = 31;
    pub const PDA: i32 = 32;
    pub const MAP_O_MATIC: i32 = 33;
    pub const BOLT_GRABBER: i32 = 34;
    pub const PERSUADER: i32 = 35;
}

/// The request value that empties a slot (`UpdateWrenchSelected` 0x2307e0: target = saved = 0).
pub const NOTHING: i32 = 0x26;

/// Item slots (item definition `+8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    Hand = 0,
    Feet = 1,
    Head = 2,
    Back = 3,
}

impl Slot {
    pub const ALL: [Slot; 4] = [Slot::Hand, Slot::Feet, Slot::Head, Slot::Back];
    pub fn from_type(t: i32) -> Option<Slot> {
        match t {
            0 => Some(Slot::Hand),
            1 => Some(Slot::Feet),
            2 => Some(Slot::Head),
            3 => Some(Slot::Back),
            _ => None,
        }
    }
    pub fn index(self) -> usize { self as usize }
}

/// One item definition (0x4c bytes at level01 0x179f40 + 0x4c·id) as far as the inventory reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemInfo {
    /// +0x00 / +0x04: the name text id and the gold version's (0 none).
    pub name: i32,
    pub gold_name: i32,
    /// +0x08: the slot type (0 hand, 1 feet, 2 head, 3 back, 4 / 5 extras, −1 none).
    pub slot: i32,
    /// +0x0c: the attach word (index of Ratchet's attach matrix, `HeroComputeAttachMatrices`).
    pub attach: i32,
    /// +0x10 / +0x14: the moby class and the second moby's (the right boot), −1 none.
    pub o_class: i32,
    pub o_class2: i32,
    /// +0x38: the HUD icon (60000 + id; 0 none).
    pub icon: u16,
    /// +0x3a: the ammo pickup's class (the Weapons page's ammo model, 0x2919a0), −1 none.
    pub ammo_class: i16,
    /// +0x40 / +0x42 / +0x44: the Help pages' text ids (the item's help, the gold version's, the gadget help's title).
    pub help: [i16; 3],
}

/// The item definitions, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemInfos(pub Vec<ItemInfo>);

impl ItemInfos {
    /// From the raw definition table (`ITEM_COUNT` × 0x4c bytes; `ItemTables::item_defs_addr` in the overlay).
    pub fn from_raw(raw: &[u8]) -> ItemInfos {
        let sz = rc_formats::save_game::ITEM_DEF_SIZE;
        let n = raw.len() / sz;
        let w = |i: usize, o: usize| i32::from_le_bytes(raw[i * sz + o..i * sz + o + 4].try_into().unwrap());
        ItemInfos(
            (0..n)
                .map(|i| ItemInfo {
                    name: w(i, 0),
                    gold_name: w(i, 4),
                    slot: w(i, 8),
                    attach: w(i, 0xc),
                    o_class: w(i, 0x10),
                    o_class2: w(i, 0x14),
                    icon: u16::from_le_bytes([raw[i * sz + 0x38], raw[i * sz + 0x39]]),
                    ammo_class: i16::from_le_bytes([raw[i * sz + 0x3a], raw[i * sz + 0x3b]]),
                    help: [0x40, 0x42, 0x44].map(|o| i16::from_le_bytes([raw[i * sz + o], raw[i * sz + o + 1]])),
                })
                .collect(),
        )
    }
    pub fn get(&self, id: i32) -> Option<&ItemInfo> { usize::try_from(id).ok().and_then(|i| self.0.get(i)) }
    /// The item's slot (`+8`), None for the extras and the unused ids.
    pub fn slot(&self, id: i32) -> Option<Slot> { self.get(id).and_then(|d| Slot::from_type(d.slot)) }
    pub fn class(&self, id: i32) -> i32 { self.get(id).map_or(-1, |d| d.o_class) }
}

// ------------------------------------------------------------------------------------------------
// The equipment view of the saved state.

/// `0x13d4c0[id] != 0`.
pub fn owns(gs: &GameState, id: i32) -> bool { usize::try_from(id).ok().and_then(|i| gs.global.owned.get(i)).is_some_and(|&b| b != 0) }

/// The saved (equipped) item of `slot` (`0x141660 + 4·slot`; 0 none).
pub fn equipped(gs: &GameState, slot: Slot) -> i32 { gs.global.equipped[slot.index()] }

/// The session request of `slot` (`0x141408 + 4·slot`).
pub fn request(sess: &SessionState, slot: Slot) -> i32 {
    match slot {
        Slot::Hand => sess.temp_hand,
        Slot::Feet => sess.temp_feet,
        Slot::Head => sess.temp_head,
        Slot::Back => sess.temp_back,
    }
}

pub fn set_request(sess: &mut SessionState, slot: Slot, v: i32) {
    match slot {
        Slot::Hand => sess.temp_hand = v,
        Slot::Feet => sess.temp_feet = v,
        Slot::Head => sess.temp_head = v,
        Slot::Back => sess.temp_back = v,
    }
}

/// The owned items of a slot, in id order.
pub fn owned_in(gs: &GameState, infos: &ItemInfos, slot: Slot) -> Vec<i32> {
    (0..rc_formats::save_game::ITEM_COUNT as i32).filter(|&i| owns(gs, i) && infos.slot(i) == Some(slot)).collect()
}

// ------------------------------------------------------------------------------------------------
// The Gadgets / Weapons page rules.

/// What ✕ on a grid cell did (`FUN_0028f260`'s ✕ branch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Select {
    /// No item there or not owned: sound 2 (denied).
    Denied,
    /// The slot's pending item became this one (sound 0).
    Equipped,
    /// The feet / head item already pending was taken off (sound 0; slots 1 and 2 only).
    Unequipped,
    /// The hand / back item already pending: nothing changes (sound 0).
    Unchanged,
    /// The Drone Device (0x18): its own path (`0x141345`), not an equip.
    Drone,
}

/// `FUN_0028f260` ✕ on a cell holding `id`: `pending` is the menu's copy of the saved items (`0x1ba1a0[4]`, made
/// when the menu opens). A hand or back item can only be replaced (the game has no empty hand or back); a feet or
/// head item selected again is taken off.
pub fn menu_select(pending: &mut [i32; 4], id: i32, owned: bool, slot_type: i32) -> Select {
    if id == 0 || !owned { return Select::Denied; }
    let Ok(t) = usize::try_from(slot_type) else { return Select::Denied };
    let Some(p) = pending.get_mut(t) else { return Select::Denied };
    if *p == id {
        if t == 0 || t == 3 {
            if id == 0x18 { return Select::Drone; }
            *p = id;
            return Select::Unchanged;
        }
        *p = 0;
        return Select::Unequipped;
    }
    if id == 0x18 { return Select::Drone; }
    *p = id;
    Select::Equipped
}

/// `PageMenuClose` 0x28c6c8: for every slot whose menu copy differs from the saved item, the request
/// `0x141408 + 4·s` = the copy (0 → [`NOTHING`]). None: unchanged.
pub fn menu_close_requests(saved: &[i32], pending: &[i32; 4]) -> [Option<i32>; 4] {
    std::array::from_fn(|s| (saved.get(s).copied().unwrap_or(0) != pending[s]).then_some(if pending[s] == 0 { NOTHING } else { pending[s] }))
}

/// Applies [`menu_close_requests`] to the session.
pub fn apply_close_requests(sess: &mut SessionState, req: &[Option<i32>; 4]) {
    for (s, r) in Slot::ALL.iter().zip(req) {
        if let Some(v) = r { set_request(sess, *s, *v); }
    }
}

// ------------------------------------------------------------------------------------------------
// Debug grants (port-only, documented in docs/plan/gadgets.md §6).

/// What a debug grant does beyond ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantEquip {
    /// Only own the items (and their `GiveItem` ammo): nothing is equipped; equip them in the Gadgets menu.
    None,
    /// `RC_GIVE_ITEMS`' rule: the last back, feet and head item among them become the saved items of their slots
    /// (so they are worn from the start) and the last hand item is requested into the hand.
    LastPerSlot,
}

/// A debug grant: `ids` owned (as `GiveItem` without its banner: owned, acquired, the ammo it grants, the quick
/// select slot of a hand item); with [`GrantEquip::LastPerSlot`] the last back / feet / head item is saved in
/// `equipped[3 / 1 / 2]` and the last hand item other than the wrench is requested (`temp_hand`). Returns the
/// equipped item per slot (hand = the request).
pub fn debug_grant(gs: &mut GameState, sess: &mut SessionState, ids: &[usize], tables: Option<&rc_formats::save_game::ItemTables>, mode: GrantEquip) -> [Option<i32>; 4] {
    for &id in ids {
        let Some(o) = gs.global.owned.get_mut(id) else { continue };
        *o = 1;
        gs.global.acquired[id] = 1;
        if let Some(t) = tables {
            if t.records[id].has_ammo() { gs.global.ammo[id] = gs.global.ammo[id].max(t.records[id].grant_ammo() as i32); }
            if t.slot_type[id] == 0 && id != item::WRENCH as usize && !gs.global.quick_select.contains(&(id as i32)) {
                if let Some(q) = gs.global.quick_select.iter().position(|&q| q == 0) { gs.global.quick_select[q] = id as i32; }
            }
        }
    }
    let mut out = [None; 4];
    if mode == GrantEquip::None { return out; }
    // The slot of each id: the definitions' +8 when read, else the known ids (packs 2..4, head 5..7, boots 28 / 29).
    let slot_of = |i: usize| -> Option<Slot> {
        match tables {
            Some(t) => Slot::from_type(*t.slot_type.get(i)?),
            None => match i as i32 {
                2..=4 => Some(Slot::Back),
                5..=7 => Some(Slot::Head),
                item::MAGNEBOOTS | item::GRINDBOOTS => Some(Slot::Feet),
                _ => None,
            },
        }
    };
    for s in [Slot::Feet, Slot::Head, Slot::Back] {
        if let Some(&i) = ids.iter().rev().find(|&&i| slot_of(i) == Some(s) && i != item::HOVERBOARD as usize) {
            gs.global.equipped[s.index()] = i as i32;
            out[s.index()] = Some(i as i32);
        }
    }
    // The Drone Device is never put in the hand: granting it launches its drones as buying it does (0x2af7e8: 0x141345).
    if ids.contains(&0x18) { sess.drone = true; }
    if let Some(&h) = ids.iter().rev().find(|&&i| slot_of(i) == Some(Slot::Hand) && i != item::WRENCH as usize && i != 0x18) {
        sess.temp_hand = h as i32;
        out[0] = Some(h as i32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_rules_per_slot() {
        // Hand 10, feet 0, head 6, back 2 saved.
        let mut p = [10, 0, 6, 2];
        assert_eq!(menu_select(&mut p, 3, false, 3), Select::Denied, "not owned");
        assert_eq!(menu_select(&mut p, 0, true, 3), Select::Denied, "empty cell");
        assert_eq!(menu_select(&mut p, 3, true, 3), Select::Equipped);
        assert_eq!(p, [10, 0, 6, 3]);
        assert_eq!(menu_select(&mut p, 3, true, 3), Select::Unchanged, "the back cannot be emptied");
        assert_eq!(menu_select(&mut p, 6, true, 2), Select::Unequipped, "the head item comes off");
        assert_eq!(p, [10, 0, 0, 3]);
        assert_eq!(menu_select(&mut p, 29, true, 1), Select::Equipped);
        assert_eq!(menu_select(&mut p, 28, true, 1), Select::Equipped, "one feet item at a time");
        assert_eq!(p, [10, 28, 0, 3]);
        assert_eq!(menu_select(&mut p, 28, true, 1), Select::Unequipped);
        assert_eq!(menu_select(&mut p, 12, true, 0), Select::Equipped);
        assert_eq!(menu_select(&mut p, 12, true, 0), Select::Unchanged, "the hand cannot be emptied");
        assert_eq!(menu_select(&mut p, 0x18, true, 0), Select::Drone);
        assert_eq!(p, [12, 0, 0, 3]);
    }

    #[test]
    fn close_requests_only_changed_slots() {
        let saved = [10, 29, 6, 2, 0, 0, 0];
        assert_eq!(menu_close_requests(&saved, &[10, 29, 6, 2]), [None; 4]);
        assert_eq!(menu_close_requests(&saved, &[12, 0, 6, 3]), [Some(12), Some(NOTHING), None, Some(3)]);
        let mut s = SessionState::default();
        apply_close_requests(&mut s, &[Some(12), Some(NOTHING), None, Some(3)]);
        assert_eq!((s.temp_hand, s.temp_feet, s.temp_head, s.temp_back), (12, NOTHING, 0, 3));
    }
}
