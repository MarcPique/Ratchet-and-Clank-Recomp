//! **The pieces that turn over at the boss's checkpoint, classes 1434 / 1435** (level18 `0x2f7ad8`, census U597; three
//! placed at one point, #889..#891, by the last arena). They hide while scene 3 plays (the boss's fall) and, once the
//! arena's checkpoint word is set (0x1623a0, by the boss 1422 during that scene), turn over: upside down (Euler x
//! += π) and mirrored about the height 101.6. They show again when the scene ends; restarting at the checkpoint
//! turns them over in the level's first ticks. Read from the level18 decomp. Native `f32`.
//!
//! ## Coverage (`0x2f7ad8`)
//! | address | what | port |
//! |---|---|---|
//! | state 0 | → 1, then state 1's test | [`update`] |
//! | state 1 | game mode 2 (0x15f5c4) and scene 3 (0x16d290), or the tick counter 0x15f5cc < 5 with 0x1623a0 set and Euler x 0: → 2, hidden (+0x31 = 0), mode \| 1 | [`update`] |
//! | state 2 | 0x1623a0 set and Euler x 0: z = 101.6 + (101.6 − z), Euler x += π (`fast_add_rotations`) | [`update`] |
//! | | game mode ≠ 2 → 1, shown, mode & ~1 | [`update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::services::World;

use super::veldin_boss::words;

pub const REFERENCE_LEVEL: u32 = 18;
pub const UPDATE_FN: u32 = 0x2f_7ad8;
pub const CLASSES: [i16; 2] = [1434, 1435];

/// The height they turn over about.
const PIVOT: f32 = 101.6;

/// Level18 `0x2f7ad8` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    match st {
        0 => w.mm(id).state = 1,
        1 => {}
        2 => {
            if w.svc.units.word(words::CHECKPOINT) != 0 && w.m(id).rotation[0] == 0.0 {
                let m = w.mm(id);
                let d = PIVOT - m.position[2];
                m.rotation[0] = crate::moby_update::creature::add_rot(m.rotation[0], std::f32::consts::PI);
                m.position[2] = m.position[2] + d + d;
            }
            if w.svc.game_mode != 2 {
                let m = w.mm(id);
                m.state = 1;
                m.visible = 1;
                m.mode &= 0xfffe;
            }
            return;
        }
        _ => return,
    }
    let scene3 = w.svc.game_mode == 2 && w.svc.cinematic.scene.as_ref().is_some_and(|s| s.id == 3);
    let restart = w.counter < 5 && w.svc.units.word(words::CHECKPOINT) != 0 && w.m(id).rotation[0] == 0.0;
    if scene3 || restart {
        let m = w.mm(id);
        m.state = 2;
        m.visible = 0;
        m.mode |= 1;
    }
}
