//! The Blarg help director, class 1348 (census U232; level06 `0x3083b0`, one instance). Read from the level06 decomp
//! (the eight "unreachable" blocks are `div` traps).
//!
//! **Pvars** (s32): +0x00..+0x14 six mobys (−1 none), +0x18 a moby group, +0x20 / +0x24 / +0x28 cuboids and a moby,
//! +0x2c a switch moby.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x3083c8 | `H[0x2a]` count 0 and in cuboid +0x24 → `Help_Request(6003, 0x2a)` | [`update`] |
//! | 0x3083f8 | `H[0x29]` count 0: each of the six mobys (≠ −1) not in its state 1 → `Help_Request(6002, 0x29)` | [`update`] |
//! | 0x308450 | `H[0x2e]` count 0, or `H[0x2f]` count 0: the moby +0x28 deleted → item 14 (0x13d4ce) not owned: `Help_Request(6008, 0x2f)` and `H[0x2e]` bumped; owned: `Help_Request(6007, 0x2e)` and `H[0x2f]` bumped (the game crosses the two records: kept) | [`update`] |
//! | 0x308618 | in cuboid +0x20 and `MobyGroupCount(P+0x18, −1)` = 0: for each of the six mobys (≠ −1) alive and not in state 1 → the arm / reminder on `H[0x28]` with `Help_Request(6001, 0x28)`; a deleted one → `H[0x28]` count := 0xffff | [`update`] ([`super::hints::arm_or_remind`]) |
//! | | a slot of −1 reads the moby before the table (index −1) | n/a [L]: treated as neither live nor deleted |
//! | 0x3087f8 | global flag 0x13d3ad (0x25) clear, the switch +0x2c (≠ −1) with its command byte +0xbc set → flag := 1 | [`update`] (G-SAV-010 write) |
//! | no sound, particle, save, other moby written | | n/a |

use super::hints::{arm_or_remind, bump, class_state, flag, gone, in_cuboid, owned, pvi, request, set_flag};
use crate::moby_runtime::MobyId;
use crate::moby_update::scheduler::group_count;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x30_83b0;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [1348];

/// Level06 0x3083b0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let h = |w: &World, r: usize| w.svc.help.records.help[r].count;
    if h(w, 0x2a) == 0 && in_cuboid(w, id, 0x24) { request(w, 0x1773, 0x2a); }
    if h(w, 0x29) == 0 {
        for k in 0..6 {
            let m = pvi(w, id, 4 * k);
            if m != -1 && class_state(w, m).is_none_or(|(_, s)| s != 1) { request(w, 0x1772, 0x29); }
        }
    }
    if (h(w, 0x2e) == 0 || h(w, 0x2f) == 0) && gone(w, pvi(w, id, 0x28)) {
        if !owned(w, 14) {
            request(w, 0x1778, 0x2f);
            bump(w, 0x2e);
        } else {
            request(w, 0x1777, 0x2e);
            bump(w, 0x2f);
        }
    }
    if in_cuboid(w, id, 0x20) && group_count(w, pvi(w, id, 0x18), -1) == 0 {
        for k in 0..6 {
            let m = pvi(w, id, 4 * k);
            let Some((_, s)) = class_state(w, m).filter(|_| m != -1) else { continue };
            if s == 0xfe || s == 0xfd {
                w.svc.help.records.help[0x28].count = 0xffff;
            } else if s != 1 {
                arm_or_remind(w, 0x28, 0x1771, 0x28);
            }
        }
    }
    let sw = pvi(w, id, 0x2c);
    if !flag(w, 0x25) && sw != -1 && usize::try_from(sw).ok().and_then(|i| w.table.mobys.get(i)).is_some_and(|m| m.cmd != 0) { set_flag(w, 0x25); }
}
