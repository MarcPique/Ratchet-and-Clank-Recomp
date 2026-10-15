//! U110 (census 2026-10-02): class 762, Aridia's boulder that only an explosion breaks (level02 0x2e0720, the only
//! copy: 1 placed). Read from the level02 decomp. Native `f32`; the `rand` draws in the game's order.
//!
//! Novalis's breakable rock ([`crate::moby_update::classes::breakables::novalis`]) with a heavier trigger and bigger
//! bursts, sharing its two burst helpers: a hit with flags 0x800000 (a blast) bursts it into 200 dusty puffs and 50
//! rock bits and remembers it (the global flag 0x13d39d), so it stays gone on later visits.
//!
//! | address | what | port |
//! |---|---|---|
//! | | `MobyGetHitMessage(m, 0x800000, 0)` | [`update`] |
//! | 0 | the global flag 0x13d39d (`story::flag(0x15)`) set → `DeleteMoby`; else 1 | [`update`] |
//! | 1 | a hit: the flag = 1; class sound 0; 200 puffs over 4 × 2 × 4 along the yaw (speed `randf(1.5, 3.5)·dt`, size `randf(1.5, 3.5)·210000`, life `ticks(rand_range(120, 240))`, colours 0x5f243632 / 0x061311, gp−0x4f3c / −0x4f38); 50 rock bits (z `randf(1, 4)`, scale `randf(0.05, 0.15)`); `DeleteMoby` (no death bits) | [`update`] (`breakables::novalis::puffs`, `bits`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::breakables::novalis::{bits, puffs};
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 2;
pub const UPDATE_FN: u32 = 0x2e_0720;
pub const CLASSES: [i16; 1] = [762];
/// The global flag byte 0x13d39d (0x13d388 + 0x15).
const BROKEN: usize = 0x15;
const SMOKE: (u32, u32) = (0x5f24_3632, 0x06_1311);

/// Level02 0x2e0720 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let hit = w.get_hit(id, 0x80_0000, false);
    match w.m(id).state {
        0 => {
            if story::flag(w, BROKEN) != 0 {
                w.delete_moby(id);
            } else {
                w.mm(id).state = 1;
            }
        }
        1 if hit.is_some() => {
            story::set_flag(w, BROKEN, 1);
            w.play_sound(0, 0, id);
            puffs(w, id, 200, 2.0, 1.0, 4.0, (1.5, 3.5), (1.5, 3.5), (0x78, 0xf0), SMOKE);
            bits(w, id, 50, 2.0, 1.0, (1.0, 4.0), (0.05, 0.15));
            w.delete_moby(id);
        }
        _ => {}
    }
}
