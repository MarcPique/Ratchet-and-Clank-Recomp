//! **Package P6 — the hand items, one mechanism.** The game's per-tick hand-0 logic, general over every hand
//! item (docs/plan/hero_states.md "P6"):
//!
//! 1. The request `0x141408` (the quick-select ring, the double-tap △, `GiveItem`, the debug give) is taken by
//!    `UpdateWrenchSelected(0)` 0x2307e0 in the slot loop, which puts the held item away and creates the new one
//!    (`items::update_hand_selected`, `items::create_hand`): the item held is `0x140408` = the slot's id.
//! 2. The weapon check `HeroPdaGadget` 0x240ed8 (the first call of the transitions after the hit intake) runs the
//!    held item's **fire** case: [`HandItemKind::fire`] from [`HAND_ITEMS`] ([`pda_item`]), then — when the state
//!    did not change — the Thruster hover epilogue ([`super::packs::pda_epilogue`]). The early returns before
//!    the switch (0x1413fc for a non-wrench, not Ratchet, no hand moby, the slot not ready) skip the epilogue, as
//!    the game does (level00 0x227fa0 / level01 0x240ed8 disassembly).
//! 3. The item moby's own update `(*moby+0x74)(moby)` in the slot loop 0x231088: [`HandItemKind::update`] —
//!    [`ItemUpdate::Slot`] runs inside `items::slot_loop` (the wrench's hit window), [`ItemUpdate::Hero`] needs the
//!    hero's context (SetState, the collision lines: the Swingshot's hook) and runs right after the slot loop
//!    ([`after_items`], called by the tick), which is the same point of the frame.
//!
//! A new gadget or weapon adds one row to [`HAND_ITEMS`] (its fire case and / or its item update); nothing else in
//! the hero or the tick changes.
//!
//! Ported rows: the wrench (item 8, fire in [`super::melee`] ahead of the table, update `melee::wrench_update`, its
//! thrown flight [`super::comet`]), the Swingshot (item 12, [`super::swingshot`]) and the Bomb Glove (item 10, the
//! throw gloves' fire case and the glove's update: [`super::weapons`]) and the Pyrocitor (item 16, no fire case: its
//! update fires it, [`super::pyrocitor`]); the guns (docs/plan/hero_gameplay.md §9, shared parts in [`super::guns`]):
//! the Blaster (item 15, case 0xf draws it, [`super::blaster`]), the R.Y.N.O. (23, [`super::ryno`]), the Devastator
//! (11, [`super::devastator`]) and the Tesla Claw (19, [`super::tesla`]), their updates firing; the Suck Cannon (9), the Taunter (14) and the Morph-o-Ray (21, [`super::morph_ray`]), which act
//! on creatures through `crate::moby_update::creature::react`; the Walloper (18, case 0x12 → the lunge 0x20, [`super::walloper`]); the
//! Visibomb (13, [`super::visibomb`]: its update launches the missile). The utility gadgets (2026-10-01, G-WPN-006):
//! the Hydrodisplacer (22, [`super::hydrodisplacer`]: its update and the use poses 0x38..0x3a on the pads 341), the
//! Trespasser (26, [`super::trespasser`]: its update tells the locks 615 that ○ is held), the Metal Detector (27,
//! [`super::metal_detector`]: aim, beep, dig, scan), the Hologuise (31, case 0x1f: [`super::hologuise`], the disguise
//! body 3) and the PDA (32, case 0x20: [`super::pda`], the remote vendor). The game's other cases, not ported yet: the
//! other throw weapons 0x11 / 0x14 / 0x18 / 0x19 (→ 0x23 / `0x22ee08`: their updates), 0x15.
//!
//! **The shared parts the gadgets added** (plug-and-play for the next item): the hand records 0x140c40 (three
//! joint-modifier records of kind 5 on the hand moby, run by the slot loop: an item update writes their joint, k / d,
//! scale or targets, [`hand_records`], drawn with the item through [`Gadgets::hand_mods`]); the calls into the hero an
//! item update makes inside the slot loop ([`ItemCall`]: `SetState` / `SetAnim`, run by [`after_items`]); the item's
//! +0xbc ([`Gadgets::item_bc`]) that the classes read; the pad's released-mask writes ([`Gadgets::released_or`]).
//!
//! **Hand-item slot leftovers (G-HERO-025, closed 2026-10-01):** the holster check 0x2405f8 has no per-item cases (one
//! rule for a raised weapon whose item def +0x18 byte is clear: `super::weapons::holster_check`, ported); the swap
//! timers 0x13f52a / 0x13f52c are counted down by `HeroTickStateTimer` (`physics::post_move`; nothing reads them in
//! the ported code); the swap's store of 0x20 into the requested item's definition +0x20 (0x179f60 + 0x4c·id) has no
//! reader in any exported overlay (n/a: the port's definitions are read-only data); the Suck Cannon's put-away ORs 5
//! into the pad's released mask 0x13cae8 while L1 / L2 are held ([`Gadgets::released_or`], applied by the tick).

use super::idle::JointRec;
use super::items::{HitSink, ItemData, ItemEnv};
use super::states::Ctx;
use super::Hero;
use crate::moby_runtime::MobyTable;

/// How the port runs a hand item's moby update.
#[derive(Clone, Copy)]
pub enum ItemUpdate {
    /// Not ported (the item only follows the hand).
    None,
    /// Inside the slot loop (no hero context needed).
    Slot(fn(&mut Hero, &mut MobyTable, &dyn super::anim::AnimCtl, &ItemEnv, &mut dyn HitSink, &mut crate::rng::Rng)),
    /// Right after the slot loop, with the hero's context.
    Hero(fn(&mut Hero, &mut Ctx, &ItemData)),
}

/// One hand item: its case of the weapon check and its moby update.
#[derive(Clone, Copy)]
pub struct HandItemKind {
    /// Item id (`0x140408`).
    pub id: i32,
    pub name: &'static str,
    /// The weapon check's case for the item (None: not ported; only the epilogue runs).
    pub fire: Option<fn(&mut Hero, &mut Ctx)>,
    pub update: ItemUpdate,
}

/// The hand items the port knows (see the module doc).
pub static HAND_ITEMS: [HandItemKind; 22] = [
    HandItemKind { id: super::items::item::WRENCH, name: "wrench", fire: None, update: ItemUpdate::Slot(super::melee::wrench_update) },
    HandItemKind { id: super::swingshot::SWINGSHOT, name: "Swingshot", fire: Some(super::swingshot::fire), update: ItemUpdate::Hero(super::swingshot::item_update) },
    // The throw gloves' case of the weapon check (0x23 / the arm, super::weapons) and their shared update (the Bomb
    // Glove's 0x2d8330 and its copies: super::gloves).
    HandItemKind { id: super::items::item::BOMB_GLOVE, name: "Bomb Glove", fire: Some(super::weapons::fire), update: ItemUpdate::Slot(super::gloves::update) },
    HandItemKind { id: crate::moby_update::classes::mine::MINE_GLOVE, name: "Mine Glove", fire: Some(super::weapons::fire), update: ItemUpdate::Slot(super::gloves::update) },
    HandItemKind { id: crate::moby_update::classes::decoy::DECOY_GLOVE, name: "Decoy Glove", fire: Some(super::weapons::fire), update: ItemUpdate::Slot(super::gloves::update) },
    HandItemKind { id: crate::moby_update::classes::doom_canister::GLOVE_OF_DOOM, name: "Glove of Doom", fire: Some(super::weapons::fire), update: ItemUpdate::Slot(super::gloves::update) },
    // No weapon-check case: the Pyrocitor's update 0x2cd458 fires it (super::pyrocitor).
    HandItemKind { id: super::pyrocitor::PYROCITOR, name: "Pyrocitor", fire: None, update: ItemUpdate::Slot(super::pyrocitor::update) },
    // The weapon check's case 0xf draws it; its update 0x2ca610 fires (super::blaster).
    HandItemKind { id: super::blaster::BLASTER, name: "Blaster", fire: Some(super::blaster::fire), update: ItemUpdate::Slot(super::blaster::update) },
    // No weapon-check case: its update 0x2e4e60 fires the salvo (super::ryno).
    HandItemKind { id: super::ryno::RYNO, name: "R.Y.N.O.", fire: None, update: ItemUpdate::Slot(super::ryno::update) },
    // No weapon-check case: its update 0x2c7d68 fires (super::devastator).
    HandItemKind { id: super::devastator::DEVASTATOR, name: "Devastator", fire: None, update: ItemUpdate::Slot(super::devastator::update) },
    // No weapon-check case: its update 0x2ce448 fires the beam (super::tesla).
    HandItemKind { id: super::tesla::TESLA, name: "Tesla Claw", fire: None, update: ItemUpdate::Slot(super::tesla::update) },
    // No weapon-check case: its update 0x303000 pulls and fires (super::suck_cannon, through creature::react).
    HandItemKind { id: super::suck_cannon::SUCK_CANNON, name: "Suck Cannon", fire: None, update: ItemUpdate::Slot(super::suck_cannon::update) },
    // No weapon-check case: its update 0x2ccb78 whistles and lures (super::taunter).
    HandItemKind { id: super::taunter::TAUNTER, name: "Taunter", fire: None, update: ItemUpdate::Slot(super::taunter::update) },
    // No weapon-check case: its update 0x2d2450 fires the beam and morphs (super::morph_ray, the chicken 270).
    HandItemKind { id: super::morph_ray::MORPH, name: "Morph-o-Ray", fire: None, update: ItemUpdate::Slot(super::morph_ray::update) },
    // The weapon check's case 0x12 starts the gadget lunge 0x20; its update 0x2d11c0 makes the arcs (super::walloper).
    HandItemKind { id: super::walloper::WALLOPER, name: "Walloper", fire: Some(super::walloper::fire), update: ItemUpdate::Slot(super::walloper::update) },
    // No weapon-check case: its update 0x2c8cc0 launches the missile 172 (super::visibomb).
    HandItemKind { id: super::visibomb::VISIBOMB, name: "Visibomb", fire: None, update: ItemUpdate::Slot(super::visibomb::update) },
    // No weapon-check case: its update 0x309cd0 starts the use poses 0x38..0x3a on a pad 341 (super::hydrodisplacer).
    HandItemKind { id: super::hydrodisplacer::HYDRODISPLACER, name: "Hydrodisplacer", fire: None, update: ItemUpdate::Slot(super::hydrodisplacer::update) },
    // No weapon-check case: its update 0x2d7380 publishes ○ held in its +0xbc for the locks 615 (super::trespasser).
    HandItemKind { id: super::trespasser::TRESPASSER, name: "Trespasser", fire: None, update: ItemUpdate::Slot(super::trespasser::update) },
    // No weapon-check case: its update 0x2f2280 aims, beeps and digs (super::metal_detector).
    HandItemKind { id: super::metal_detector::METAL_DETECTOR, name: "Metal Detector", fire: None, update: ItemUpdate::Slot(super::metal_detector::update) },
    // The weapon check's case 0x1f starts the disguise's timer (super::hologuise); its moby (class 483) has no update.
    HandItemKind { id: super::hologuise::HOLOGUISE, name: "Hologuise", fire: Some(super::hologuise::fire), update: ItemUpdate::None },
    // The weapon check's case 0x20 opens the remote vendor (super::pda); its moby (class 619) has no update.
    HandItemKind { id: super::pda::PDA, name: "PDA", fire: Some(super::pda::fire), update: ItemUpdate::None },
    // Kalebo III's board weapon (case 0x24 of level16 0x223350; its missiles 1475: super::hoverboard).
    HandItemKind { id: super::hoverboard::WEAPON_ITEM, name: "board weapon", fire: Some(super::hoverboard::weapon_fire), update: ItemUpdate::Slot(super::hoverboard::weapon_update) },
];

/// The row of item `id`.
pub fn hand_item(id: i32) -> Option<&'static HandItemKind> { HAND_ITEMS.iter().find(|k| k.id == id) }

/// The hand-item fields the mechanism keeps between the slot loop and [`after_items`], and the hero-block fields of
/// the utility gadgets (the hand item's joint records, the Hydrodisplacer's water, the disguise's timer).
#[derive(Clone, Debug)]
pub struct Gadgets {
    /// The slot loop reached the item's update and it is an [`ItemUpdate::Hero`] one: [`after_items`] runs it.
    pub pending: Option<i32>,
    /// The tick the slot loop deferred it in (`ItemEnv::frame`: the screen markers the update registers).
    pub pending_tick: u64,
    /// `UpdateWrenchSelected` took 0x141345: the drones' launch `0x2e8c20` is made right after it by the slot loop
    /// ([`launch_drones`]).
    pub drone_launch: bool,
    /// 0x140c40 + 0xb0·k (k < 3): the hand item's joint records (kind 5), cleared by `HeroItemsCreate` (joint +0xa0 =
    /// −1) and run by the slot loop (`0x227050`) for every record whose joint is set ([`hand_records`]).
    pub hand_joints: [JointRec; 3],
    /// The hand moby's joint-modifier list (+0x64), head first: the records attached ([`hand_records`]).
    pub hand_manips: Vec<u8>,
    /// The hand moby's modifiers as the evaluator reads them (the records', then an item's own node: the Metal
    /// Detector's head), rebuilt every tick; the engine draws the hand item with them.
    pub hand_mods: Vec<rc_formats::moby_anim::JointModifier>,
    /// The calls into the hero code an item update made inside the slot loop (`SetState`, `SetAnim`, leaving the
    /// body), run in order right after it by [`after_items`] (the same point of the frame: nothing reads the hero in
    /// between).
    pub calls: Vec<ItemCall>,
    /// The hand item moby's byte +0xbc (cleared at its creation): the Hydrodisplacer's cutaway phase, the
    /// Trespasser's "○ held" the locks 615 read.
    pub item_bc: u8,
    /// 0x141400 (hero block): the Hydrodisplacer holds water ([`super::hydrodisplacer`]).
    pub hydro_full: bool,
    /// The Hydrodisplacer's pvars ([`super::hydrodisplacer::Hydro`]).
    pub hydro: super::hydrodisplacer::Hydro,
    /// The Metal Detector's pvars and node ([`super::metal_detector::Detector`]).
    pub detector: super::metal_detector::Detector,
    /// The Hologuise's timers 0x14162c / 0x14162e ([`super::hologuise`]).
    pub disguise: super::hologuise::Disguise,
    /// Bits an item update ORed into the pad's released mask 0x13cae8 (the Suck Cannon going away with L1 / L2 held:
    /// 5): the tick applies them to its pad right after the item updates (the hand items get the pad read-only).
    pub released_or: u32,
    /// 0x15f5c4 (the game mode) as the tick saw it before the hero update (`crate::tick::MobySystem::game_mode`).
    pub game_mode: i32,
    /// 0x141660 / 0x141408 (`items::ItemGlobals::saved` / `request`) as the hero code outside the slot loop reads them:
    /// the tick mirrors them in before the hero update and writes the request back after it (the Hologuise's weapon
    /// check, the disguise's way out).
    pub hand_saved: i32,
    pub hand_request: i32,
    /// The PDA's `OpenVendorMenu(0)` of this tick's weapon check, handed to the engine by the slot loop ([`super::pda`]).
    pub pda_open: bool,
    /// The disguise's transitions called `UpdateWrenchSelected(0)` (□): made by the tick right after the body's update.
    pub wrench_select: bool,
    /// The board weapon pickup's hand switch (`crate::hero::hoverboard`, level16 `0x2252b0`): 0x1413fc = 0,
    /// `UpdateWrenchSelected(0)`, 0x1413fc = 1, then the request 0x24; made by the tick right after the hero update.
    pub board_select: bool,
}

impl Default for Gadgets {
    fn default() -> Self {
        Gadgets {
            pending: None,
            pending_tick: 0,
            drone_launch: false,
            hand_joints: [cleared_record(0), cleared_record(1), cleared_record(2)],
            hand_manips: Vec::new(),
            hand_mods: Vec::new(),
            calls: Vec::new(),
            item_bc: 0,
            hydro_full: false,
            hydro: Default::default(),
            detector: Default::default(),
            disguise: Default::default(),
            released_or: 0,
            game_mode: 0,
            hand_saved: 0,
            hand_request: 0,
            pda_open: false,
            wrench_select: false,
            board_select: false,
        }
    }
}

/// A call into the hero code made by a hand item's update (run by [`after_items`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemCall {
    /// `SetState(id, play)` 0x23cf98.
    SetState { id: i32, play: bool },
    /// `SetAnim(blend, seq, frame)` 0x247a90 (`blend` = `(float)ticks(n)`).
    SetAnim { blend: i32, seq: u8, frame: i32 },
}

/// The kind `0x231088` gives the hand records before `0x227050` (`FUN_00226ff8(5)`: the hand moby).
pub const HAND_KIND: u8 = 5;

/// One hand record as `HeroItemsCreate` leaves it: `FastMemSet(0, 0xb0)` and the joint +0xa0 = −1 (unused).
fn cleared_record(k: u8) -> JointRec {
    let mut r = JointRec::of_kind(0xf0 + k, -1, HAND_KIND, 0, 0);
    r.scale = 0.0;
    r.node_scale = 0.0;
    r
}

impl Gadgets {
    /// `HeroItemsCreate` 0x22f3c0 making a hand item: the three hand records cleared, the new moby's +0xbc and pvars
    /// clear (`CreateMoby` hands out a cleared pvar block [L], as for the gloves).
    pub fn on_create(&mut self) {
        self.hand_joints = [cleared_record(0), cleared_record(1), cleared_record(2)];
        self.hand_manips.clear();
        self.hand_mods.clear();
        self.item_bc = 0;
        self.hydro = Default::default();
        self.detector = Default::default();
    }
}

/// The slot loop `0x231088`'s pass over the hand records (slot 0, state 2, after `UpdateWrenchSelected`): every
/// record whose joint +0xa0 is set gets kind 5 and its update `0x227050` (the springs, attach / detach on the hand
/// moby's list, targets cleared, scale reset). Then the hand moby's modifier list for the draw ([`Gadgets::hand_mods`],
/// with `targets` = the item class's joint lists' targets).
pub(super) fn hand_records(hero: &mut Hero, targets: &[u8]) {
    let g = &mut hero.gadgets;
    for k in 0..g.hand_joints.len() {
        let r = &mut g.hand_joints[k];
        if r.joint == -1 { continue; }
        r.kind = HAND_KIND;
        r.update();
        let i = k as u8;
        if r.attached {
            if !g.hand_manips.contains(&i) { g.hand_manips.insert(0, i); }
        } else {
            g.hand_manips.retain(|&x| x != i);
        }
    }
    hand_modifiers(hero, targets);
}

/// [`Gadgets::hand_mods`]: the attached records' modifiers (head first), then the item's own node (the Metal
/// Detector's head, `AttachManipulator(item, 3, 0x1e1600)`).
pub(super) fn hand_modifiers(hero: &mut Hero, targets: &[u8]) {
    let g = &hero.gadgets;
    let mut mods: Vec<rc_formats::moby_anim::JointModifier> = g
        .hand_manips
        .iter()
        .filter_map(|&i| {
            let r = g.hand_joints.get(i as usize)?;
            let t = usize::try_from(r.joint).ok().and_then(|j| targets.get(j)).copied().filter(|&t| t != 0xff)?;
            Some(r.modifier(t))
        })
        .collect();
    if hero.items.slot.id == super::metal_detector::METAL_DETECTOR {
        if let Some(m) = super::metal_detector::node(hero, targets) { mods.insert(0, m); }
    }
    hero.gadgets.hand_mods = mods;
}

/// The item class's joint lists' modifier targets ([`super::fx::JointData::items`]; empty: not loaded).
pub fn item_targets(hero: &Hero, o_class: i16) -> Vec<u8> {
    hero.fx.joints.items.iter().find(|(o, _)| *o == o_class).map(|(_, t)| t.clone()).unwrap_or_default()
}

/// `0x2e8c20` (from `UpdateWrenchSelected` 0x2307e0 with 0x141345 set): with the Drone Device's ammo, the drones are
/// launched on the moby world (`crate::moby_update::classes::drone::launch`) and one ammo is used when any was
/// missing (`0x249450(0x18, 1)`).
pub(super) fn launch_drones(hero: &mut Hero, table: &mut MobyTable, env: &super::items::ItemEnv, hits: &mut dyn HitSink, rng: &mut crate::rng::Rng) {
    use crate::moby_update::classes::drone;
    if !std::mem::take(&mut hero.gadgets.drone_launch) { return; }
    if hero.weapons.has_ammo(drone::DRONE_DEVICE) == 0 { return; }
    let yaw = hero.rot[2].to_f32();
    let mut used = false;
    hits.world(table, hero, rng, env.frame as u64, &mut |w| used = drone::launch(w, yaw));
    if used { hero.weapons.use_ammo(drone::DRONE_DEVICE, 1); }
}

/// `HeroPdaGadget` 0x240ed8 for a ready hand item other than the wrench (the slot checks have passed). `t0` is
/// the state timer at entry; returns true when the state changed (`timer < t0`).
pub(super) fn pda_item(h: &mut Hero, c: &mut Ctx, t0: i32) -> bool {
    if let Some(f) = hand_item(h.items.slot.id).and_then(|k| k.fire) { f(h, c); }
    if h.timer == t0 { super::packs::pda_epilogue(h, c); }
    h.timer < t0
}

/// The holster check 0x2405f8 (after the water checks). True when it changed the state: one rule for every hand item
/// (a raised weapon 0x1413f8 whose def +0x18 byte 0x1413fb is clear; `super::weapons::holster_check`).
pub(super) fn holster_check(h: &mut Hero, c: &mut Ctx) -> bool { super::weapons::holster_check(h, c) }

/// The item moby's update in the slot loop 0x231088 (`(*moby+0x74)(moby)`), for the hand item of the slot.
pub(super) fn slot_item_update(hero: &mut Hero, table: &mut MobyTable, anim: &dyn super::anim::AnimCtl, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut crate::rng::Rng) {
    let id = hero.items.slot.id;
    match hand_item(id).map(|k| k.update) {
        Some(ItemUpdate::Slot(f)) => {
            // The wrench's update belongs to its class (0x47), as before the table.
            if id != super::items::item::WRENCH || hero.items.slot.item.as_ref().is_some_and(|it| it.o_class == 0x47) { f(hero, table, anim, env, hits, rng); }
        }
        Some(ItemUpdate::Hero(_)) => {
            hero.gadgets.pending = Some(id);
            hero.gadgets.pending_tick = env.frame as u64;
        }
        _ => {}
    }
}

/// Right after the hand's slot loop (`HeroItemsUpdate` 0x231268, same point of the frame): the pending item
/// update that needs the hero's context, and the item fields of an item that is gone. `data`: the hand classes.
pub fn after_items(hero: &mut Hero, c: &mut Ctx, data: &ItemData) {
    // The hero calls the item's update made (the Hydrodisplacer's `SetState(0x38, 0)` + `SetAnim`, …), in order.
    for call in std::mem::take(&mut hero.gadgets.calls) {
        match call {
            ItemCall::SetState { id, play } => {
                hero.set_state(c, id, play);
            }
            ItemCall::SetAnim { blend, seq, frame } => hero.set_anim(c.anim, c.rng, crate::ps2v::Pf::from_i32(blend), seq, frame),
        }
    }
    let pending = hero.gadgets.pending.take();
    if let Some(ItemUpdate::Hero(f)) = pending.and_then(hand_item).map(|k| k.update) { f(hero, c, data); }
    // The SetState an item update made inside the slot loop (the Bomb Glove's 1 → 0x1e).
    super::weapons::after_items(hero, c);
    if hero.items.slot.item.is_none() || hero.items.slot.id != super::swingshot::SWINGSHOT { super::swingshot::item_gone(hero); }
}

/// The hand item's class sounds of this tick (`PlayClassSound(index, 0, item)` inside its update: the wrench's hit,
/// the Swingshot's fire / hit / pull), played through the hero's sound layer right after the item's update (the
/// tick calls it after [`after_items`]), in the order the update made them. No hand moby: dropped.
///
/// Then Ratchet's own sounds of the item update (`fx.item_voices`, `moby` = Ratchet: the catch's voice, the thrown
/// wrench's whoosh loop and its release, the bomb glove's throw voice), in order.
pub fn flush_item_sounds(h: &mut Hero, moby: &crate::moby_runtime::Moby, sounds: &mut dyn super::HeroSounds, rng: &mut crate::rng::Rng) {
    let mut list = std::mem::take(&mut h.fx.item_sounds);
    list.append(&mut h.swing.item.sounds);
    if let Some(item) = h.items.slot.item.as_ref() {
        let (o_class, pos) = (item.o_class, item.position);
        for index in list { sounds.item_sound(o_class, pos, index, 0, rng); }
    }
    let item = h.items.slot.item.as_ref().map(|it| (it.o_class, it.position));
    for cmd in std::mem::take(&mut h.fx.item_voices) {
        match cmd {
            super::packs::SoundCmd::Loop { n, sound } => h.packs.loops[n] = sounds.voice(moby, sound, 4, rng),
            super::packs::SoundCmd::Voice { index, flags } => { sounds.voice(moby, index, flags, rng); }
            super::packs::SoundCmd::Release { slot } => sounds.release(moby, slot),
            super::packs::SoundCmd::MobyLoop { n, moby: m, o_class, pos, sound } => h.packs.loops[n] = sounds.moby_sound(m, o_class, pos, sound, 4, rng),
            super::packs::SoundCmd::ReleaseOf { slot, moby: m } => sounds.release_of(m, slot),
            super::packs::SoundCmd::ItemLoop { n, index, flags } => {
                let alive = h.fx.item_loops[n].is_some_and(|s| sounds.alive(s));
                if let (false, Some((o_class, pos))) = (alive, item) {
                    let s = sounds.item_sound(o_class, pos, index, flags, rng);
                    h.fx.item_loops[n] = (s >= 0).then_some(s);
                }
            }
            super::packs::SoundCmd::ItemRelease { n } => {
                if let Some(s) = h.fx.item_loops[n].take() { sounds.release(moby, s); }
            }
            super::packs::SoundCmd::ItemPitch { n, pb } => {
                if let Some(s) = h.fx.item_loops[n] { sounds.set_pitch_bend(s, pb); }
            }
            super::packs::SoundCmd::MobySound { moby: id, o_class, pos, index, flags } => { sounds.moby_sound(id, o_class, pos, index, flags, rng); }
        }
    }
    for n in 0..h.fx.item_loops.len() { h.fx.item_loop_alive[n] = h.fx.item_loops[n].is_some_and(|s| sounds.alive(s)); }
}
