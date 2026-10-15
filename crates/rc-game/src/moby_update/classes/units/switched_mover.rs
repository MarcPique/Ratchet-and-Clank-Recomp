//! Eudora's switched movers, classes 1101, 1102, 1531, 1532: level04 0x2e17d8 (census U162; 8 created instances).
//! A block that waits for its link (+0x00: a 615 in state 4 or a 1190 in state 2), or finds itself already done
//! (its collected byte or death bit set), then moves along z for one second at its class's speed (1101 / 1532 up
//! 1.3 a second, 1531 down 2.55, 1102 down 2.05), sets its death bits and stops. 1101 / 1532 play 1101's sound 0 as
//! they start; 1101 plays sound 1 as it stops. Read from the level04 decomp and disassembly (0x2e17d8, the speed
//! table 0x2e1768). Native `f32`.
//!
//! **Pvar block**: +0x00 the link (moby index, −1 none), +0x04 the start z, +0x08 s32 the move timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x04 = z, → 1 | [`update`] |
//! | state 1 | the collected byte (`0x1bb804[id]`, level01 `0x1bbb04`) or the death bit (`0x14c190[level]`) set, or the link a 615 in state 4 / a 1190 in state 2 → 1101 / 1532: `PlayClassSoundByClass(0, 0, m, 1101)` (0x2a16c0); → 2, timer ticks(60) | [`update`] |
//! | state 2 | z += speed (0x2e1768: 1101 / 1532 1.3, 1531 −2.55, else −2.05) · dt; the timer done → the death bits (`0x14c190[level]`, `0x1ba650` = level01 `0x1ba950`), 1101: sound 1; → 3 | [`update`] |
//! | | no particle, hit, light, other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{dec_timer_pvar_i32, pi32, set_pi32, DT};
use crate::moby_update::services::World;

/// The update in the level04 class table.
pub const UPDATE_FN: u32 = 0x2e_17d8;
pub const REFERENCE_LEVEL: u32 = 4;
pub const CLASSES: [i16; 4] = [1101, 1102, 1531, 1532];
/// The link classes and the state each must be in.
pub const LINKS: [(i16, u8); 2] = [(0x267, 4), (0x4a6, 2)];

/// `0x2e1768`: the class's z speed (units a second).
pub fn speed(o_class: i16) -> f32 {
    match o_class {
        0x5fb => f32::from_bits(0xc023_3333),
        0x44d | 0x5fc => f32::from_bits(0x3fa6_6666),
        _ => f32::from_bits(0xc003_3333),
    }
}

fn done_before(w: &World, id: MobyId) -> bool {
    let b2 = w.m(id).spawn_id;
    b2 >= 0 && (w.svc.save.collected.get(&b2).is_some_and(|&v| v != 0) || w.svc.save.death.contains(&(w.svc.level, b2)))
}

/// Level04 0x2e17d8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    let oc = w.m(id).o_class;
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            set_pi32(w, id, 4, z.to_bits() as i32);
            w.mm(id).state = 1;
        }
        1 => {
            if !done_before(w, id) {
                let Some(l) = usize::try_from(pi32(w, id, 0)).ok().and_then(|l| w.table.mobys.get(l)) else { return };
                if !LINKS.iter().any(|&(c, s)| l.o_class == c && l.state == s) { return; }
            }
            if oc == 0x44d || oc == 0x5fc { w.play_sound_as(0, 0, id, 0x44d); }
            w.mm(id).state = 2;
            let t = w.ticks(60);
            set_pi32(w, id, 8, t);
        }
        2 => {
            w.mm(id).position[2] += speed(oc) * DT;
            if dec_timer_pvar_i32(w, id, 8) == 0 { return; }
            let (b2, lvl) = (w.m(id).spawn_id, w.svc.level);
            if b2 >= 0 {
                w.svc.save.death.insert((lvl, b2));
                w.svc.save.death_level.insert(b2);
            }
            if oc == 0x44d { w.play_sound(1, 0, id); }
            w.mm(id).state = 3;
        }
        _ => {}
    }
}
