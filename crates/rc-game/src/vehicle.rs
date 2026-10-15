//! **The ridden vehicle record** (boot globals 0x140940..0x14095f): the one block every flown or ridden vehicle class
//! writes when Ratchet takes it, and the HUD, the pause triggers, the water managers and the level's other classes
//! read. One record for the game, not per class: Gemlik's ship 69 (level13 `0x2bb068`), Pokitaru's jet 1242 and the
//! fleet's ship 1379 fill the same words ([`Record::take`] is the common part of their mounts).
//!
//! | address | field | written by / read by |
//! |---|---|---|
//! | 0x140940 | [`Record::moby`] | the mount; read by the pause triggers (`menus::mode`, kind 1 for [`crate::menus::mode::VEHICLE_CLASSES`]), Gemlik's water managers' pause (`0x309e88`), the HUD callbacks, Qwark's ship 388 |
//! | 0x140944 (s16) | [`Record::class`] | the mount (the moby's class) |
//! | 0x140946 / 0x140947 (u8) | [`Record::missiles`] / [`Record::missiles_max`] | the mount (10 / 20 on Gemlik); the missile fire spends one; the HUD draws the pips |
//! | 0x140948..0x14094b (u8) | [`Record::b48`], [`Record::b49`], [`Record::b4a`], [`Record::b4b`] | the mount (20, 3, 3, 3 on Gemlik); [L] other vehicles' counters (no Gemlik reader) |
//! | 0x14094c (f32) | [`Record::health`] | the mount (255); hits and wall scrapes take from it |
//! | 0x140950 (f32) | [`Record::health_max`] | the mount (256); the HUD gauge's full scale |
//! | 0x140954 (f32) | [`Record::f54`] | the mount (100) [L: no Gemlik reader] |
//! | 0x140958 (s32) | [`Record::hud`] | each tick while ridden: `health · 200 / 256` |
//! | 0x14095c (s32) | [`Record::hud_handle`] | a HUD element handle the exit hands to `FUN_0024b090(h, 0)` (the HUD's set-flags) [L: no Gemlik writer: 0] |
//! | 0x14095e (u8) | [`Record::b5e`] | the mount (1) |
//! | 0x14095f (u8) | [`Record::quit`] | the freeze menu's kind-1 "Quit?" yes (`|= 1`, `crate::menus::freeze`); the vehicle polls bit 0 and clears it at the mount |
//!
//! Hoven's turret 1267 (not a vehicle of the pause table) polls [`Record::quit`] too.

use crate::moby_runtime::MobyId;

/// The record (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub moby: Option<MobyId>,
    pub class: i16,
    pub missiles: u8,
    pub missiles_max: u8,
    pub b48: u8,
    pub b49: u8,
    pub b4a: u8,
    pub b4b: u8,
    pub health: f32,
    pub health_max: f32,
    pub f54: f32,
    pub hud: i32,
    pub hud_handle: i32,
    pub b5e: u8,
    pub quit: u8,
}

/// The values a vehicle's mount writes into the record (each class its own constants).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mount {
    pub missiles: u8,
    pub missiles_max: u8,
    pub b48: u8,
    pub b49: u8,
    pub b4a: u8,
    pub b4b: u8,
    pub health: f32,
    pub health_max: f32,
    pub f54: f32,
    pub b5e: u8,
}

impl Record {
    /// A mount: the record names `moby` (class `class`), with the vehicle's values, and the quit bit cleared.
    pub fn take(&mut self, moby: MobyId, class: i16, m: &Mount) {
        self.class = class;
        self.health = m.health;
        self.missiles = m.missiles;
        self.b48 = m.b48;
        self.b5e = m.b5e;
        self.missiles_max = m.missiles_max;
        self.moby = Some(moby);
        self.b4b = m.b4b;
        self.b49 = m.b49;
        self.b4a = m.b4a;
        self.quit = 0;
        self.health_max = m.health_max;
        self.f54 = m.f54;
    }

    /// The vehicle moby when it is `id`.
    pub fn is(&self, id: MobyId) -> bool { self.moby == Some(id) }

    /// The freeze menu's kind-1 yes: 0x14095f |= 1.
    pub fn request_quit(&mut self) { self.quit |= 1; }
}
