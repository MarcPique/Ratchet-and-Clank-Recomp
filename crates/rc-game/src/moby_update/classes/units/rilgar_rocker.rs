//! Rilgar's rocking floats, class 810 (level 05, 4 created instances): level05 0x30bdd8 (census U180). A platform that
//! rocks in place (pitch 1°·sin a, roll 2°·sin b, a and b turning 47.2°/s and 63.7°/s), carrying its riders through
//! the turn, and plays its class sound 0 every 3–6 s. Read from the level05 decomp of 0x30bdd8 and the overlay's
//! data (gp−0x4f2c .. −0x4f18). Native `f32`.
//!
//! **Pvar block**: +0x20 the platform block, +0x60 / +0x6c the two phases, +0x78 s32 the sound timer.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | timer = trunc(`multiply_global_scale(randf(180, 360))`) (gp−0x4f1c / −0x4f18, ints as floats); +0x60 = `rand_angle`, +0x6c = `rand_angle`; → 1 | [`update`] |
//! | state 1 | `FastDecTimer(+0x78)` out → `PlayClassSound(0, 0, m)`, timer re-drawn as in state 0 | [`update`] (`World::play_sound`) |
//! | | +0x60 += 47.2°·dt, +0x6c += 63.7°·dt (gp−0x4f2c / −0x4f28, `fast_add_rotations`); rot.y = 1°·sin +0x60, rot.x = 2°·sin +0x6c (gp−0x4f24 / −0x4f20); `CarryRiders(+0x20, position − old position (0), old rotation, rotation)` (0x28a7c0 = L01 0x2755f8) | [`update`] (`triggers::carry_riders`) |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gold_bolt::fast_dec_timer;
use crate::moby_update::creature::{self as c, DT};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x30_bdd8;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [810];
pub const BLOCK: usize = 0x20;
pub const PHASE_A: usize = 0x60;
pub const PHASE_B: usize = 0x6c;
pub const TIMER: usize = 0x78;
const DEG: f32 = 0.017_453_292;

fn reroll(w: &mut World, id: MobyId) {
    let r = w.rng.randf(180.0, 360.0);
    let t = (r * w.svc.timing.timer_scale.to_f32()) as i32;
    c::set_pi32(w, id, TIMER, t);
}

/// Level05 0x30bdd8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < TIMER + 4 { return; }
    match w.m(id).state {
        0 => {
            reroll(w, id);
            let a = w.rng.rand_angle();
            c::set_pf(w, id, PHASE_A, a);
            let b = w.rng.rand_angle();
            c::set_pf(w, id, PHASE_B, b);
            w.mm(id).state = 1;
        }
        1 => {
            let mut t = c::pi32(w, id, TIMER);
            let fired = fast_dec_timer(&mut t);
            c::set_pi32(w, id, TIMER, t);
            if fired != 0 {
                w.play_sound(0, 0, id);
                reroll(w, id);
            }
            let (pos, rot0) = (w.m(id).position, w.m(id).rotation);
            let a = c::add_rot(c::pf(w, id, PHASE_A), 47.2 * DEG * DT);
            c::set_pf(w, id, PHASE_A, a);
            let b = c::add_rot(c::pf(w, id, PHASE_B), 63.7 * DEG * DT);
            c::set_pf(w, id, PHASE_B, b);
            let m = w.mm(id);
            m.rotation[1] = DEG * a.sin();
            m.rotation[0] = 2.0 * DEG * b.sin();
            let (d, r) = (c::sub(m.position, pos), m.rotation);
            triggers::carry_riders(&mut m.pvars, BLOCK, d, rot0, r);
        }
        _ => {}
    }
}
