//! Umbris' sinking floats, class 1080 (level 07, 8 created instances): level07 0x311bc8 (census U256). A float that
//! sits a little askew (±2° pitch and roll, ±0.02 height) until Ratchet stands on it, then rocks for half a second
//! while settling 0.002 a tick, and sinks: it tilts toward 80°, drops faster and faster (9.8 units/s²) and is deleted
//! below z 2. Its riders are carried by its displacement only (the game passes the same rotation twice). Read from the
//! level07 decomp and disassembly of 0x311bc8 (the sinking state's `fast_sin` argument is the tilt step) and its
//! private `HeroOnMoby` copy 0x311b70. Native `f32`.
//!
//! **Pvar block**: +0x20 the moby record's words (+0x20 s32, +0x24 s16, +0x28 byte, +0x3e s16), +0x60 the platform
//! block, +0xa0 s16 the timer, +0xa2 s16 the class sound (−1 none), +0xa4 the fall speed, +0xa8 / +0xac the tilt at
//! the step.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | +0x20 = 0, +0x24 = 0, +0x28 = 4, +0x3e = 0xd; ambient (0x50, 0x40, 0x10) (0x278d18 = L01 0x2650d0); rot.x += `randf(−2°, 2°)`, rot.y += `randf(−2°, 2°)`, z += `randf(−0.02, 0.02)`; → 1 | [`update`] |
//! | state 1 | Ratchet on it (0x311b70 = `HeroOnMoby` 0x277fb8: the ledge moby in group 3 / state 0x1c, else standing on it): +0xa8 / +0xac = rot.x / rot.y, timer = `ticks(30)`, sound +0xa2 ≠ −1 → `PlayClassSound(+0xa2, 0, m)`, → 2 | [`update`] (`World::hero_on_moby`, `World::play_sound`) |
//! | state 2 | rot.x = sin(wrap(timer·0.75))·1°, rot.y = cos(wrap(timer·0.89))·1° (0x286d30 = L01 0x2731d0); z −= 0.002·[0x15ed60]; `FastDecTimer(s16 +0xa0)` out → +0xa4 = 0.002·[0x15ed60], → 3 | [`update`] |
//! | state 3 | s = (80° − rot.x)·0.05; rot.x += s; z −= 2·sin s; +0xa4 += 9.8·dt²; z −= +0xa4; z < 2 → `DeleteMoby` (no carry that tick) | [`update`] |
//! | every state | `CarryRiders(+0x60, position − old position, rotation, rotation)` (0x288d88 = L01 0x2755f8) | `triggers::carry_riders` |
//! | | no particle, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::flyer::wrap_frac;
use crate::moby_update::creature::{self as c, DT2, SPEED};
use crate::moby_update::services::{fast_dec_timer_s16, World};
use crate::moby_update::triggers;

/// The update in the level07 class table.
pub const UPDATE_FN: u32 = 0x31_1bc8;
pub const REFERENCE_LEVEL: u32 = 7;
pub const CLASSES: [i16; 1] = [1080];
const DEG: f32 = 0.017_453_292;
const TWO_DEG: f32 = f32::from_bits(0x3d0e_fa35);
const TILT: f32 = 1.396_263_4;

/// Level07 0x311bc8 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xb0 { return; }
    let old = w.m(id).position;
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, 0x20, 0);
            c::set_pi16(w, id, 0x24, 0);
            c::set_pu8(w, id, 0x28, 4);
            c::set_pi16(w, id, 0x3e, 0xd);
            w.mm(id).ambient = [0x50, 0x40, 0x10, 0];
            let a = w.rng.randf(-TWO_DEG, TWO_DEG);
            w.mm(id).rotation[0] += a;
            let b = w.rng.randf(-TWO_DEG, TWO_DEG);
            w.mm(id).rotation[1] += b;
            let z = w.rng.randf(-f32::from_bits(0x3ca3_d70a), f32::from_bits(0x3ca3_d70a));
            let m = w.mm(id);
            m.state = 1;
            m.position[2] += z;
        }
        1 => {
            if w.hero_on_moby(id) {
                let r = w.m(id).rotation;
                c::set_pf(w, id, 0xa8, r[0]);
                c::set_pf(w, id, 0xac, r[1]);
                let t = w.ticks(0x1e);
                c::set_pi16(w, id, 0xa0, t as i16);
                let s = c::pi16(w, id, 0xa2);
                if s != -1 { w.play_sound(s as i32, 0, id); }
                w.mm(id).state = 2;
            }
        }
        2 => {
            let t = c::pi16(w, id, 0xa0) as f32;
            let m = w.mm(id);
            m.rotation[0] = wrap_frac(t * 0.75).sin() * DEG;
            m.rotation[1] = wrap_frac(t * 0.89).cos() * DEG;
            m.position[2] -= SPEED * 0.002;
            let mut tm = c::pi16(w, id, 0xa0);
            let fired = fast_dec_timer_s16(&mut tm);
            c::set_pi16(w, id, 0xa0, tm);
            if fired != 0 {
                c::set_pf(w, id, 0xa4, SPEED * 0.002);
                w.mm(id).state = 3;
            }
        }
        3 => {
            let s = (TILT - w.m(id).rotation[0]) * 0.05;
            let v = c::pf(w, id, 0xa4) + DT2 * 9.8;
            c::set_pf(w, id, 0xa4, v);
            let m = w.mm(id);
            m.rotation[0] += s;
            m.position[2] -= s.sin() + s.sin();
            m.position[2] -= v;
            if m.position[2] < 2.0 {
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    let m = w.mm(id);
    let (d, r) = (c::sub(m.position, old), m.rotation);
    triggers::carry_riders(&mut m.pvars, 0x60, d, r, r);
}
