//! Class 1750 on Veldin's finale (census U570, level 18, 1 instance): the save made before the last boss. Its update
//! (level18 0x2fad08) is `FUN_00281fa8(0x1dfc10)`: while the page menu's ending-buffer word (L18 0x1ba7d0, L01
//! 0x1ba250) is 0, store the buffer there and `MakeWholeSave` the game into it; every later tick does nothing. The end
//! page's "Timewarp to before you defeated Drek" restores that buffer (`process_global_state_flags`,
//! `crate::menus::pause::saves::time_warp`). Spec: docs/plan/progression.md `## saves`.
//!
//! | address | what | port |
//! |---|---|---|
//! | level18 0x2fad08 | `FUN_00281fa8(0x1dfc10)` | [`update`] |
//! | level18 0x281fa8 | the word +0xe0 of 0x1ba6f0 = 0: word = buffer, `memcard_MakeWholeSave(buffer)` (0x24f150) | [`update`] (`cinematic::ending_save`: the engine keeps the bytes once per level load) |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

pub const UPDATE_FN: u32 = 0x2f_ad08;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1750];

/// Level18 0x2fad08 (the once-only test is the engine's: the buffer word is set on the first call).
pub fn update(w: &mut World, _: MobyId) { crate::cinematic::ending_save(w); }
