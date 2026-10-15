//! Veldin's hidden props, class 1432 (level 18, 13 instances): level18 0x2f7ab0 (census unit U559). The first update
//! hides the moby for good: state 1, mode |= 3 (hidden, no update), +0x31 = 0. Read from the level18 disassembly
//! (10 words).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2f7ab0 | state 0: state 1, mode \|= 3, +0x31 = 0; other states: nothing | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2f_7ab0;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1432];

/// Level18 0x2f7ab0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    if m.state == 0 {
        m.state = 1;
        m.mode |= 3;
        m.visible = 0;
    }
}
