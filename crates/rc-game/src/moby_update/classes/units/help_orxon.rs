//! The Orxon help director, class 1344 (census U341; level10 `0x2e85b8`, one instance). Read from the level10
//! decomp.
//!
//! **Pvars** (s32): +0x00 / +0x04 / +0x08 cuboids, +0x10 / +0x14 two mobys, +0x18 a cuboid, +0x1c the level-word
//! cuboid, +0x20 a moby (class 1015).
//!
//! | address | what | port |
//! |---|---|---|
//! | entry | the level word gp−0x4cc8 (0x161f38) := Ratchet in cuboid +0x1c (read by the level's `0x2dfc38`, not ported) | [`update`] (`Services::units` word [`AIR_WORD`]) |
//! | state 0 | → 1, update distance 0xff | [`update`] |
//! | state 1 | in cuboid +0x00, group ok, box idle and `H[0x35]` count 0 → `Help_Request(10000, 0x35)` | [`update`] |
//! | | in cuboid +0x04, group ok, idle: global flag 4 (0x13d38c) clear → `(10002, 0x37)` while `H[0x37]` is 0; set, flag 5 clear and `H[0x38]` 0 → `(10003, 0x38)` | [`update`] |
//! | | game mode 0, the acquired flag 0x13d504 (0x13d4e8[0x1c]) set, idle and `H[0x36]` 0 → `(10001, 0x36)` | [`update`] |
//! | | in cuboid +0x08, group ok, item 11 not owned, idle and `H[0x5c]` 0 → `(10005, 0x5c)` | [`update`] |
//! | | any class-857 moby of the run list (0x15ffe4) not in its state 1: Ratchet within 15 of both mobys +0x10 and +0x14 (≠ −1) and `H[0x6a]` 0 → `(10007, 0x6a)`; in cuboid +0x18 with the moby +0x20 of class 1015 not in state 4 → the arm / reminder on `H[0x6b]` → `(10008, 0x6b)` | [`update`] |
//! | no sound, particle, save flag write, other moby written | | n/a |

use super::hints::{arm_or_remind, class_state, flag, group_ok, hero_dist, idle, in_cuboid, owned, pvi, request, run_list};
use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2e_85b8;
pub const REFERENCE_LEVEL: u32 = 10;
pub const CLASSES: [i16; 1] = [1344];
/// The level word gp−0x4cc8.
pub const AIR_WORD: u32 = 0x16_1f38;

/// Level10 0x2e85b8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let inside = in_cuboid(w, id, 0x1c);
    w.svc.units.set_word(AIR_WORD, inside as u32);
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            m.state = 1;
            m.update_dist = 0xff;
            return;
        }
        1 => {}
        _ => return,
    }
    let h = |w: &World, r: usize| w.svc.help.records.help[r].count;
    if in_cuboid(w, id, 0) && group_ok(w) && idle(w) && h(w, 0x35) == 0 { request(w, 10000, 0x35); }
    if in_cuboid(w, id, 4) && group_ok(w) && idle(w) {
        if !flag(w, 4) {
            if h(w, 0x37) == 0 { request(w, 0x2712, 0x37); }
        } else if !flag(w, 5) && h(w, 0x38) == 0 {
            request(w, 0x2713, 0x38);
        }
    }
    let acquired = w.svc.interact.game.acquired.get(0x1c).is_some_and(|&b| b != 0);
    if w.svc.game_mode == 0 && acquired && idle(w) && h(w, 0x36) == 0 { request(w, 0x2711, 0x36); }
    if in_cuboid(w, id, 8) && group_ok(w) && !owned(w, 11) && idle(w) && h(w, 0x5c) == 0 { request(w, 0x2715, 0x5c); }
    let list = run_list(w);
    if !list.iter().any(|&m| w.m(m).o_class == 0x359 && w.m(m).state != 1) { return; }
    let near = |w: &World, off: usize| { let m = pvi(w, id, off); m != -1 && usize::try_from(m).ok().filter(|&i| i < w.table.mobys.len()).is_some_and(|i| hero_dist(w, i) < 15.0) };
    if near(w, 0x10) && near(w, 0x14) && h(w, 0x6a) == 0 { request(w, 0x2717, 0x6a); }
    if in_cuboid(w, id, 0x18) && class_state(w, pvi(w, id, 0x20)).is_some_and(|(c, s)| c == 0x3f7 && s != 4) {
        arm_or_remind(w, 0x6b, 0x2718, 0x6b);
    }
}
