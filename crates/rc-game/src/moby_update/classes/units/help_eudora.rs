//! The Eudora help director, class 1343 (census U165; level04 `0x2e4418`, one instance). Read from the level04
//! decomp. `H[n]` / `M[n]` / `G[n]`: help, move and gadget-help records (0x141968 / 0x141848 / 0x141720 + 8n); "ok" =
//! group ok; "idle" = the box idle and nothing pending.
//!
//! **Pvars** (s32): cuboids +0x00, +0x04, +0x2c, +0x30, +0x34, +0x38, +0x3c, +0x50; mobys +0x40 / +0x44 (cranks 280)
//! and +0x48 / +0x4c (Trespassers 615); counters +0x08, +0x0c (never set here), +0x14 (the first-entry mark), +0x1c,
//! +0x20 (crank tries), +0x24 (falls), +0x28 (fall latch).
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, update distance 0xff, +0x14 := −1 | [`update`] |
//! | 0x2e4474 | in cuboid +0x00, idle, the moby's mission (+0xb0) not done: the first time (+0x14 = −1) +0x14 := the death count (a mark), `M[24]` bumped; later, `M[24]` > 2: item 20 not owned → `(4001, 0x1d)` while `H[0x1d]` is 0; owned, more than `ScaleTicks(18000)` since `G[20]`'s time and `H[0x1c]` 0 → `(4000, 0x1c)` | [`update`] |
//! | 0x2e4598 | `H[0x42]` < 4, in cuboid +0x34 or +0x38: the crank hint as 1341's (counter +0x20, `(1006, 0x42)` on the 3rd and 6th try) | [`update`] ([`super::hints::crank_tries`]) |
//! | 0x2e46e8 | the acquired flag 0x13d4f1 (0x13d4e8[9]), idle, `H[0x1e]` 0 → `(4002, 0x1e)` | [`update`] |
//! | 0x2e476c | in cuboid +0x3c, `M[31]` unused, idle, `H[0x77]` 0 → `(20013, 0x77)` | [`update`] |
//! | 0x2e47c8 | the Trespassers +0x48 / +0x4c in their state 4 → global flags 0x1c / 0x1d; the cranks +0x40 / +0x44 in their state 5 → flags 0x1e / 0x1f | [`update`] (G-SAV-010 writes) |
//! | 0x2e48dc | in cuboid +0x04, ok, idle: item 26 not owned → the reminder on `H[0x1f]` → `(4003, 0x1f)`; owned: the Trespasser +0x48 in state 4 → `M[17]` bumped; else with +0x0c = 0 and `M[17]` unused: +0x08 += 1 and the arm / reminder on `H[0x20]` → `(4004, 0x20)`. Out of the cuboid, +0x0c := 0 | [`update`] |
//! | 0x2e4c60 | in cuboid +0x50, ok, idle, an hour since `M[9]`'s time and `H[0x43]` without this level's bit → `(1007, 0x43)` | [`update`] |
//! | 0x2e4d04 | `H[0x46]` or `H[0x47]` unused: in state 0x72 on a Trespasser (ground moby 0x13f64c of class 615) inside cuboid +0x04: `H[0x46]` 0 → `(2011, 0x46)`; else `H[0x47]` 0 → +0x1c += 1, past `ScaleTicks(1800)` → `(2012, 0x47)`; otherwise +0x1c := 0 | [`update`] |
//! | 0x2e4df0 | in cuboid +0x30, `M[25]` unused, ok → `M[25]` := 1, time / mask | [`update`] |
//! | 0x2e4e84 | in cuboid +0x2c with `M[25]` unused and not ok (falling), the latch +0x28 clear → +0x24 += 1, +0x28 := `ScaleTicks(180)`; ok → +0x28 := 0 | [`update`] |
//! | 0x2e4f24 | in cuboid +0x2c, `M[25]` unused, ok, +0x24 > 2, idle, `H[0x49]` 0 → `(4005, 0x49)` | [`update`] |
//! | no sound, particle, save, other moby written | | n/a |

use super::hints::{arm_or_remind, bump_move, class_state, crank_tries, group_ok, idle, in_cuboid, mission_done, older_than, owned, pvi, remind, request, set_flag, set_pvi, ticks};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_4418;
pub const REFERENCE_LEVEL: u32 = 4;
pub const CLASSES: [i16; 1] = [1343];

fn h(w: &World, r: usize) -> u16 { w.svc.help.records.help[r].count }
fn m(w: &World, r: usize) -> u16 { w.svc.help.records.moves[r].count }

/// Level04 0x2e4418 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    match w.m(id).state {
        0 => {
            let mo = w.mm(id);
            mo.state = 1;
            mo.update_dist = 0xff;
            set_pvi(w, id, 0x14, -1);
            return;
        }
        1 => {}
        _ => return,
    }
    let ok = group_ok(w);
    let mission = w.m(id).mission as i32;
    if in_cuboid(w, id, 0) && idle(w) && !mission_done(w, mission) {
        if pvi(w, id, 0x14) == -1 {
            set_pvi(w, id, 0x14, 0);
            bump_move(w, 24);
        } else if m(w, 24) > 2 {
            if !owned(w, 20) {
                if h(w, 0x1d) == 0 { request(w, 0xfa1, 0x1d); }
            } else if older_than(w, w.svc.help.records.gadget[20].time, 18000) && h(w, 0x1c) == 0 {
                request(w, 4000, 0x1c);
            }
        }
    }
    if h(w, 0x42) < 4 && (in_cuboid(w, id, 0x34) || in_cuboid(w, id, 0x38)) { crank_tries(w, id, 0x20); }
    let acquired = w.svc.interact.game.acquired.get(9).is_some_and(|&b| b != 0);
    if acquired && idle(w) && h(w, 0x1e) == 0 { request(w, 0xfa2, 0x1e); }
    if in_cuboid(w, id, 0x3c) && m(w, 31) == 0 && idle(w) && h(w, 0x77) == 0 { request(w, 0x4e2d, 0x77); }
    for (off, class, state, fl) in [(0x48usize, 0x267i16, 4u8, 0x1cusize), (0x4c, 0x267, 4, 0x1d), (0x40, 0x118, 5, 0x1e), (0x44, 0x118, 5, 0x1f)] {
        if class_state(w, pvi(w, id, off)) == Some((class, state)) { set_flag(w, fl); }
    }
    let gate = in_cuboid(w, id, 4);
    if gate && ok && idle(w) {
        if !owned(w, 26) {
            remind(w, 0x1f, 0xfa3, 0x1f);
        } else if class_state(w, pvi(w, id, 0x48)) == Some((0x267, 4)) {
            bump_move(w, 17);
        } else if pvi(w, id, 0xc) == 0 && m(w, 17) == 0 {
            set_pvi(w, id, 8, pvi(w, id, 8) + 1);
            arm_or_remind(w, 0x20, 0xfa4, 0x20);
        }
    } else if !gate && pvi(w, id, 0xc) != 0 {
        set_pvi(w, id, 0xc, 0);
    }
    if in_cuboid(w, id, 0x50) && ok && idle(w) {
        let bit = 1u32 << (w.svc.help.level as u32 & 31);
        if older_than(w, w.svc.help.records.moves[9].time, 0x34bc0) && w.svc.help.records.help[0x43].mask & bit == 0 { request(w, 0x3ef, 0x43); }
    }
    if h(w, 0x46) == 0 || h(w, 0x47) == 0 {
        let on_trespasser = w.hero.state == 0x72 && w.hero.ground_moby.and_then(|g| w.table.mobys.get(g)).is_some_and(|g| g.o_class == 0x267);
        if on_trespasser && gate {
            if h(w, 0x46) == 0 {
                request(w, 0x7db, 0x46);
            } else {
                let n = pvi(w, id, 0x1c) + 1;
                set_pvi(w, id, 0x1c, n);
                if ticks(0x708) < n { request(w, 0x7dc, 0x47); }
            }
        } else {
            set_pvi(w, id, 0x1c, 0);
        }
    }
    if in_cuboid(w, id, 0x30) && m(w, 25) == 0 && ok {
        w.svc.help.records.moves[25].count = 1;
        let (l, t) = (w.svc.help.level, w.svc.help.play_time);
        crate::help::touch(&mut w.svc.help.records.moves[25], l, t);
    }
    let fall = in_cuboid(w, id, 0x2c);
    if fall && m(w, 25) == 0 && !ok {
        if pvi(w, id, 0x28) == 0 {
            set_pvi(w, id, 0x24, pvi(w, id, 0x24) + 1);
            set_pvi(w, id, 0x28, ticks(0xb4));
        }
    } else if pvi(w, id, 0x28) != 0 && ok {
        set_pvi(w, id, 0x28, 0);
    }
    if fall && m(w, 25) == 0 && ok && pvi(w, id, 0x24) > 2 && idle(w) && h(w, 0x49) == 0 { request(w, 0xfa5, 0x49); }
}
