//! Eudora's crank followers, classes 432 / 1052 (level 04, 4 placed 432s; census U154): level04 0x2c6858, the one
//! update the level's class table names for both classes. A moby whose frozen animation (sequence 1, speed 0) is posed
//! at t = the progress of a bolt crank (class 280, `classes::bolt_crank`) while Ratchet winds it. It **swaps its own
//! class** (`moby_update::class_swap`, G-CLS-031): a placed 432 becomes 1052 at its init and turns back into 432 for
//! good when its crank is fully wound (progress 1). Its class sound 0 loops while the progress moves (from the live
//! class's sound table: 1052's until the end), sound 1 plays when the progress reaches 0 or 1. Read from the level04
//! decomp and disassembly of 0x2c6858. Native `f32`.
//!
//! **Pvar block**: +0x00 s32 the crank (moby index, −1 none), +0x04 s32 the voice slot.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2c6874 | no pvar block → `DeleteMoby` (L04 0x241558) | [`update`] |
//! | 0x2c687c | t0 = +0x54 (the animation t as the tick found it) | [`update`] |
//! | state 0 | `MobyAnimBlend(m, 1, 0, ticks(300))` (0x24a5b8, 0x1f1988); → 1; +0x58 speed = 0; slot = −1 | [`update`] (`World::anim_blend`) |
//! | | o_class 432 → the class swap to 1052 (0x2c690c..0x2c6964; the state stays 1) | [`update`] (`class_swap::swap`) |
//! | state 1 | crank −1 → nothing; crank not class 0x118 (280) → nothing | [`update`] |
//! | | t (+0x54) = the crank's progress (crank pvar +0x00), written before the compare | [`update`] |
//! | | progress = t0 → the voice released (`SoundIsAlive` 0x27e820; `release_voice_slot` 0x27e878 when the slot is ≠ −1, owned by this moby and active), slot = −1 | [`release`] |
//! | | progress ≠ t0, not 1 and not 0 (`c.eq.s`) → voice alive: nothing; else slot = `PlayClassSound(0, 4, m)` (0x27eb48, the loop) | [`update`] |
//! | | progress ≠ t0 and 1 or 0 → the voice released, slot = −1; `PlayClassSound(1, 0, m)` (its return dropped); then o_class 1052 and the progress (re-read) 1 → the class swap to 432 (0x2c6a84..0x2c6adc), → 2 | [`update`] (`class_swap::swap`) |
//! | state 2 | the voice released, slot = −1 (every tick: no other effect, for good) | [`release`] |
//! | other states | nothing | [`update`] |
//! | | no particle, hit, flag, light, bolt, other moby written (the crank is only read) | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::class_swap;
use crate::moby_update::creature as c;
use crate::moby_update::services::{pvar as p, World};

/// The update in the level04 class table (both classes).
pub const UPDATE_FN: u32 = 0x2c_6858;
pub const REFERENCE_LEVEL: u32 = 4;
/// The placed class, and the class it is while its crank runs.
pub const PLACED: i16 = 432;
pub const RUNNING: i16 = 1052;
pub const CLASSES: [i16; 2] = [PLACED, RUNNING];
/// The bolt crank's class.
pub const CRANK: i16 = 0x118;

/// The voice release of the tail (0x2c6b14 / 0x2c6b3c): `SoundIsAlive(m, slot)` → the guarded `release_voice_slot`,
/// slot = −1.
fn release(w: &mut World, id: MobyId) {
    let slot = c::pi32(w, id, 4);
    if w.sound_alive(slot, id) {
        w.release_sound(slot, id);
        c::set_pi32(w, id, 4, -1);
    }
}

/// Level04 0x2c6858 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.is_empty() {
        w.delete_moby(id);
        return;
    }
    if w.m(id).pvars.len() < 8 { return; }
    let t0 = w.m(id).anim.t;
    match w.m(id).state {
        0 => {
            let t = w.ticks(300);
            w.anim_blend(id, 1, 0, t);
            let m = w.mm(id);
            m.state = 1;
            m.anim.speed = 0.0;
            c::set_pi32(w, id, 4, -1);
            if w.m(id).o_class == PLACED { class_swap::swap(w, id, RUNNING); }
        }
        1 => {
            let Ok(k) = usize::try_from(c::pi32(w, id, 0)) else { return };
            let Some(cm) = w.table.mobys.get(k) else { return };
            if cm.o_class != CRANK || cm.pvars.len() < 4 { return; }
            let prog = p::ff(&cm.pvars, 0);
            w.mm(id).anim.t = prog;
            if prog == t0 {
                release(w, id);
                return;
            }
            if prog != 1.0 && prog != 0.0 {
                let slot = c::pi32(w, id, 4);
                if !w.sound_alive(slot, id) {
                    let s = w.play_sound(0, 4, id);
                    c::set_pi32(w, id, 4, s);
                }
                return;
            }
            release(w, id);
            w.play_sound(1, 0, id);
            if w.m(id).o_class != RUNNING { return; }
            if p::ff(&w.m(k).pvars, 0) != 1.0 { return; }
            class_swap::swap(w, id, PLACED);
            w.mm(id).state = 2;
        }
        2 => release(w, id),
        _ => {}
    }
}
