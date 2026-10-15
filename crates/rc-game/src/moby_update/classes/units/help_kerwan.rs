//! The Kerwan help director, class 1342 (census U145; level03 `0x2df520`, one instance). Read from the level03 decomp
//! (docs/plan/level_scripting.md §4 has the summary). Records: `H[n]` help record n (0x141968 + 8n), `M[n]` move
//! record n (0x141848 + 8n). "ok" = group ok (0x1413dc < 2 or 9), "idle" = the box idle and nothing pending.
//!
//! **Pvars** (s32): cuboids at +0x00, +0x0c, +0x18, +0x24, +0x28, +0x2c, +0x30, +0x34, +0x38, +0x3c, +0x40, +0x44,
//! +0x70, +0x74, +0x78, +0x7c; +0x50 the fall counter, +0x54 the Swingshot-miss counter, +0x58 a 60-tick timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | `FastDecTimer(+0x58)` | [`update`] |
//! | state 0 | +0x54 = +0x50 = 0, → 1, update distance 0xff | [`update`] |
//! | 0x2df584 | in cuboid +0x78, ok → global flag 0x13d3a2 (0x1a) := 1 | [`update`] (G-SAV-010) |
//! | | flag 0x1a clear: cuboids +0x00 / +0x0c / +0x18, ok, idle, record never shown (mask ≥ 0) → `Help_Request(3000, 0x14)` / `(3001, 0x15)` / `(3003, 0x17)` | [`update`] |
//! | 0x2df758 | in cuboid +0x30, `M[3]` unused, ok → `M[3]` := 1 (count), time / mask | [`update`] |
//! | 0x2df7e0 | in cuboid +0x24 with `M[3]` unused and **not** ok (falling): the timer 0 → +0x50 += 1, timer := `ScaleTicks(60)`; ok with the timer running → timer := 0 | [`update`] |
//! | 0x2df8a8 | in cuboid +0x24, `M[3]` unused, ok, +0x50 a non-zero multiple of 6, idle, `H[0x16]` 0 → `(3002, 0x16)` | [`update`] |
//! | 0x2df938 | in cuboid +0x28, ok, idle, `H[0x18]` 0 and more than an hour (`ScaleTicks(0x34bc0)`) since `M[9]`'s time → `(3004, 0x18)` | [`update`] |
//! | 0x2df9c8 | creatures near: run-list mobys whose class type (header +0x46) is 5 within 8 of Ratchet | [`update`] |
//! | 0x2dfa60 | in cuboid +0x3c, none near, ok: hero state 0x1e → `M[19]` bumped; else the Blaster (item 15) owned with ammo ≥ 21 (`0x249530`), idle, `H[0x4a]` and `M[19]` 0 → `(20000, 0x4a)`. Outside: state 0x1e → `M[19]` bumped | [`update`] |
//! | 0x2dfb90 | in cuboid +0x38, ok, looks 0x15f68c ≤ 2, idle, `H[0x40]` 0 → `(1004, 0x40)`; ok, state 1, look timer 0x15f688 = −1, idle, `H[0x40]` used and `H[0x41]` not → `(1005, 0x41)` | [`update`] |
//! | 0x2dfcc4 | in cuboid +0x40, ok, idle, `H[0x54]` 0 → `(3008, 0x54)` | [`update`] |
//! | 0x2dfd38 | game mode 0, the acquired flag 0x13d4f4 (0x13d4e8[0xc]), idle, `H[0x19]` 0 → `(3005, 0x19)` | [`update`] |
//! | 0x2dfd8c | in cuboid +0x34, `M[15]` unused, ok → `M[15]` := 1, time / mask | [`update`] |
//! | 0x2dfe24 | the Swingshot miss flag 0x13fcd8 set with `M[15]` unused → 0x13fcd8 := 0, +0x54 += 1; then (flag clear too) `M[15]` unused, ok, +0x54 > 2, idle, `H[0x1a]` 0 → `(3006, 0x1a)` | [`update`] (`HeroFields::swing_help_clear`) |
//! | 0x2dfeec | hero state 0x74 (the cable), `M[16]` unused → `M[16]` := 1, time / mask (the "cable used" stat) | [`update`] |
//! | 0x2dff8c | in cuboid +0x2c, `M[16]` unused, group 6, the wrench in hand (0x140408 = 8), idle, `H[0x1b]` 0 → `(3007, 0x1b)` (the cable help) | [`update`] |
//! | 0x2e0010 | in cuboid +0x44, ok, idle, `H[0x79]` 0 → `(20015, 0x79)` | [`update`] |
//! | 0x2e0080 | in cuboid +0x7c, group < 2, `H[0x4f]` and `M[20]` 0, two or more owned hand items (slot type 0, not 8 / 0x18; `0x179bc8` = the item definitions' +8 on this level) → `(20005, 0x4f)` | [`update`] (`Inventory::slot_type`) |
//! | 0x2e0140 | in cuboid +0x70 → global flag 0x13d3a0 (0x18) := 1 | [`update`] (G-SAV-010) |
//! | 0x2e0168 | in cuboid +0x74, skill point 0x13d40b not earned → skill point, `allocate_voice_for_bank_entry(1, 0, 0)`, banner 0x53d6 | [`update`] (`story::award_skill_point`) |
//! | no particle, save, other moby written | | n/a |

use super::hints::{bump_move, flag, group_ok, hero_dist, idle, in_cuboid, older_than, owned, pvi, request, run_list, set_flag, set_pvi, ticks};
use crate::moby_runtime::MobyId;
use crate::moby_update::creature::dec_timer_i32;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2d_f520;
pub const REFERENCE_LEVEL: u32 = 3;
pub const CLASSES: [i16; 1] = [1342];

/// Pvar offsets.
pub mod pv {
    pub const FALLS: usize = 0x50;
    pub const MISSES: usize = 0x54;
    pub const TIMER: usize = 0x58;
}

fn h(w: &World, r: usize) -> u16 { w.svc.help.records.help[r].count }
fn m(w: &World, r: usize) -> u16 { w.svc.help.records.moves[r].count }
fn never_shown(w: &World, r: usize) -> bool { (w.svc.help.records.help[r].mask as i32) >= 0 }

/// Move record `r` := count 1 with the time / mask update (the directors' "set once").
fn set_move(w: &mut World, r: usize) {
    w.svc.help.records.moves[r].count = 1;
    let (l, t) = (w.svc.help.level, w.svc.help.play_time);
    crate::help::touch(&mut w.svc.help.records.moves[r], l, t);
}

/// Level03 0x2df520 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let mut t = pvi(w, id, pv::TIMER);
    dec_timer_i32(&mut t);
    set_pvi(w, id, pv::TIMER, t);
    match w.m(id).state {
        0 => {
            set_pvi(w, id, pv::MISSES, 0);
            set_pvi(w, id, pv::FALLS, 0);
            let mo = w.mm(id);
            mo.state = 1;
            mo.update_dist = 0xff;
            return;
        }
        1 => {}
        _ => return,
    }
    let ok = group_ok(w);
    if in_cuboid(w, id, 0x78) && ok { set_flag(w, 0x1a); }
    if !flag(w, 0x1a) {
        for (off, msg, rec) in [(0x00usize, 3000, 0x14usize), (0x0c, 0xbb9, 0x15), (0x18, 0xbbb, 0x17)] {
            if in_cuboid(w, id, off) && ok && idle(w) && never_shown(w, rec) { request(w, msg, rec as i32); }
        }
    }
    if in_cuboid(w, id, 0x30) && m(w, 3) == 0 && ok { set_move(w, 3); }
    let fall = in_cuboid(w, id, 0x24);
    if fall && m(w, 3) == 0 && !ok {
        if pvi(w, id, pv::TIMER) == 0 {
            set_pvi(w, id, pv::FALLS, pvi(w, id, pv::FALLS) + 1);
            set_pvi(w, id, pv::TIMER, ticks(0x3c));
        }
    } else if pvi(w, id, pv::TIMER) != 0 && ok {
        set_pvi(w, id, pv::TIMER, 0);
    }
    let falls = pvi(w, id, pv::FALLS);
    if fall && m(w, 3) == 0 && ok && falls % 6 == 0 && falls != 0 && idle(w) && h(w, 0x16) == 0 { request(w, 0xbba, 0x16); }
    if in_cuboid(w, id, 0x28) && ok && idle(w) && h(w, 0x18) == 0 && older_than(w, w.svc.help.records.moves[9].time, 0x34bc0) { request(w, 0xbbc, 0x18); }
    let near = run_list(w).into_iter().filter(|&k| w.classes.info(w.m(k).o_class).is_some_and(|i| i.ty == 5) && hero_dist(w, k) < 8.0).count();
    if in_cuboid(w, id, 0x3c) && near == 0 && ok && w.hero.state != 0x1e {
        let ammo = crate::moby_update::classes::pickup::item_ammo(w, 15);
        if owned(w, 15) && ammo >= 0x15 && idle(w) && h(w, 0x4a) == 0 && m(w, 19) == 0 { request(w, 20000, 0x4a); }
    } else if w.hero.state == 0x1e {
        bump_move(w, 19);
    }
    let look = in_cuboid(w, id, 0x38) && ok;
    if look && w.hero.help.looks <= 2 && idle(w) && h(w, 0x40) == 0 { request(w, 0x3ec, 0x40); }
    if look && w.hero.state == 1 && w.hero.help.look_timer == -1 && idle(w) && h(w, 0x40) != 0 && h(w, 0x41) == 0 { request(w, 0x3ed, 0x41); }
    if in_cuboid(w, id, 0x40) && ok && idle(w) && h(w, 0x54) == 0 { request(w, 0xbc0, 0x54); }
    let acquired = w.svc.interact.game.acquired.get(0xc).is_some_and(|&b| b != 0);
    if w.svc.game_mode == 0 && acquired && idle(w) && h(w, 0x19) == 0 { request(w, 0xbbd, 0x19); }
    if in_cuboid(w, id, 0x34) && m(w, 15) == 0 && ok { set_move(w, 15); }
    let miss = w.hero.swing.help != 0;
    if miss && m(w, 15) == 0 {
        w.hero_fields_mut().swing_help_clear = true;
        set_pvi(w, id, pv::MISSES, pvi(w, id, pv::MISSES) + 1);
    }
    if m(w, 15) == 0 && ok && pvi(w, id, pv::MISSES) > 2 && idle(w) && h(w, 0x1a) == 0 { request(w, 0xbbe, 0x1a); }
    if w.hero.state == 0x74 && m(w, 16) == 0 { set_move(w, 16); }
    if in_cuboid(w, id, 0x2c) && m(w, 16) == 0 && w.hero.group == 6 && w.hero.items.slot.id == 8 && idle(w) && h(w, 0x1b) == 0 { request(w, 0xbbf, 0x1b); }
    if in_cuboid(w, id, 0x44) && ok && idle(w) && h(w, 0x79) == 0 { request(w, 0x4e2f, 0x79); }
    if in_cuboid(w, id, 0x7c) && (w.hero.group as u32) < 2 && h(w, 0x4f) == 0 && m(w, 20) == 0 {
        let n = (0..0x25usize).filter(|&i| i != 8 && i != 0x18 && w.inventory.slot_type(i) == 0 && owned(w, i)).count();
        if n > 1 { request(w, 0x4e25, 0x4f); }
    }
    if in_cuboid(w, id, 0x70) { set_flag(w, 0x18); }
    // Skill point 0x13d40b (level03's `allocate_voice_for_bank_entry` 0x27a588 is `PlayLevelSoundAtMoby`'s code).
    if in_cuboid(w, id, 0x74) { crate::moby_update::story::award_skill_point(w, crate::moby_update::story::skill_index(0x13_d40b)); }
}
