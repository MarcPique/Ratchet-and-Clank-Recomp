//! **The Gadgetron PDA** (item 32 = 0x20, gadget class 619, which has no update in any class table): the weapon check
//! `HeroPdaGadget` 0x240ed8's case 0x20 opens the remote vendor (`OpenVendorMenu(0)` 0x2ae1a0). Read from the level01
//! decompiler output.
//!
//! **Coverage** (`address | what | status`):
//!
//! | address | what | status |
//! |---|---|---|
//! | 0x240ed8 case 0x20 | movement group 0x1413dc < 3, 4 or 5, and ○ (the slot's fire mask) pressed within `ticks(8)` → `OpenVendorMenu(0)`; then the epilogue (the Thruster hover, `packs::pda_epilogue`) | ported ([`fire`]; the epilogue by `gadgets::pda_item`) |
//! | 0x2ae1a0 (vendor 0) | the remote vendor: the list with the PDA's prices (`VendorBuildItemList(1)`, ammo only), the selection, the HUD emptied, sound 3, the help box closed, 0x15f3fc = 1, game mode 5, `SetState(100, 1)`, 0x1413f5 = 0x1413fc = 1, Ratchet hidden | ported: the hero's part here ([`fire`]: `SetState(100, 1)`, 0x1413f5 / 0x1413fc), the hand-off `Handoff::OpenVendor { vendor: None }` and the help box through the moby world ([`open`]); the menu, its list and prices by `crate::menus::vendor` (`remote`) and `rc-engine` interact_render |
//! | 0x2ae1a0 (vendor 0) | `CreateMoby(0xb)` at the camera + (gp−0x5ba0 x, 0, −0x161068 z), yaw π, MobyBuildMatrix, state 3; its `hard_cut(3, 0)` and speed 0.5·0x15ed60 (the remote vendor's own presentation; no `FadeToBlack(4)`, no camera script) | ported by the engine's vendor (`rc-engine` interact_render `remote_vendor`, on this hand-off; `crate::menus::vendor::Vendor::open(remote)`) |
//! | sounds, particles, lights, stats, bolts, save flags | sound 3 by the menu's open; nothing else | — |

use super::items::{HitSink, ItemEnv};
use super::physics::ticks;
use super::states::Ctx;
use super::Hero;
use crate::moby_runtime::MobyTable;
use crate::rng::Rng;

pub const PDA: i32 = 0x20;

/// `HeroPdaGadget` case 0x20.
pub fn fire(h: &mut Hero, c: &mut Ctx) {
    if !(h.group < 3 || h.group == 4 || h.group == 5) { return; }
    if c.env.pad.pressed_within(h.items.slot.fire_mask, ticks(8)).is_none() { return; }
    // `OpenVendorMenu(0)`'s hero part: `SetState(100, 1)`, 0x1413f5 = 1, 0x1413fc = 1 (Ratchet hidden: the engine's
    // vendor mode); the menu itself is handed to the engine right after the transitions ([`open`]).
    h.set_state(c, 100, true);
    h.f13f5 = 1;
    h.items.f13fc = 1;
    h.gadgets.pda_open = true;
}

/// The hand-off of [`fire`]'s `OpenVendorMenu(0)` (made by the slot loop of the same tick, which has the moby world):
/// `Handoff::OpenVendor { vendor: None }` and the help box closed (`FUN_002258b0`).
pub(super) fn open(hero: &mut Hero, table: &mut MobyTable, env: &ItemEnv, hits: &mut dyn HitSink, rng: &mut Rng) {
    if !std::mem::take(&mut hero.gadgets.pda_open) { return; }
    let hero_ref: &Hero = hero;
    hits.world(table, hero_ref, rng, env.frame as u64, &mut |w| {
        w.svc.help.kill();
        w.svc.interact.handoffs.push(crate::moby_update::interact::Handoff::OpenVendor { vendor: None });
    });
}
