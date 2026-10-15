//! Hoven's animated idlers, class 339: level12 0x2ec1d0 (census U390; 9 created instances). An animation-only prop:
//! a byte timer (+0xbc) picks when it plays one of its idle clips (sequence 0 → 3 or 5 at random, 1 → 4, 2 → 6);
//! at the end of a clip it blends back (4 / 6 → 0, 3 → 1, 5 → 2) and waits a random 0..3 s. Ratchet within 9 (xy)
//! holds it in sequence 0 (the wait kept at 2 or more); within 12 and settled in a clip's end pose (key A = key B ≠
//! 0) it moves on at once.
//! Read from the level12 decomp (0x2ec1d0). Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | | d = Ratchet's xy distance (`fun_001f9b80`, 0x13f3d0) | [`update`] |
//! | state 0 | sequence ≠ 0 → `MobyAnimBlend(m, 0, 0, 0)`; → 1; +0xbc = `randi(ticks(180))` (0x26c930) | [`update`] |
//! | state 1 | key A 0, d < 9, +0xbc < 2 → +0xbc = 2; d < 12, key A = key B ≠ 0 → +0xbc = 0; the byte timer (`0x220ed8`) not done → wait | [`update`] |
//! | | key A 0: `randi(256)` even → blend 5, odd → 3; 1 → blend 4; 2 → blend 6 (each only when key B differs); → 2 | [`update`] |
//! | state 2 | the animation wrapped (+0x70 bit 1): key A 4 / 6 → blend 0, 3 → 1, 5 → 2; → 1, +0xbc = `randi(ticks(180))` | [`update`] |
//! | | no sound, particle, hit or other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::blend_to;
use crate::moby_update::services::{fast_dec_timer_u8, World};

/// The update in the level12 class table.
pub const UPDATE_FN: u32 = 0x2e_c1d0;
pub const REFERENCE_LEVEL: u32 = 12;
pub const CLASSES: [i16; 1] = [339];

fn rest(w: &mut World, id: MobyId) {
    w.mm(id).state = 1;
    let n = w.ticks(180);
    let t = w.rng.randi(n);
    w.mm(id).cmd = t as u8;
}

/// Level12 0x2ec1d0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    let d = crate::moby_update::creature::dist2(w.m(id).position, super::hero_pos(w));
    let (a, b) = (w.m(id).anim.seq_a, w.m(id).anim.seq_b);
    match w.m(id).state {
        0 => {
            blend_to(w, id, 0, 0, 0);
            rest(w, id);
        }
        1 => {
            if a == 0 && d < 9.0 && w.m(id).cmd < 2 { w.mm(id).cmd = 2; }
            if d < 12.0 && a == b && a != 0 { w.mm(id).cmd = 0; }
            let mut t = w.m(id).cmd;
            let done = fast_dec_timer_u8(&mut t);
            w.mm(id).cmd = t;
            if done == 0 { return; }
            match a {
                0 => {
                    let seq = if w.rng.randi(0x100) & 1 == 0 { 5 } else { 3 };
                    blend_to(w, id, seq, 0, 0);
                }
                1 => blend_to(w, id, 4, 0, 0),
                2 => blend_to(w, id, 6, 0, 0),
                _ => {}
            }
            w.mm(id).state = 2;
        }
        2 => {
            if w.m(id).anim.flags & 2 == 0 { return; }
            match a {
                4 | 6 => blend_to(w, id, 0, 0, 0),
                3 => blend_to(w, id, 1, 0, 0),
                5 => blend_to(w, id, 2, 0, 0),
                _ => {}
            }
            rest(w, id);
        }
        _ => {}
    }
}
