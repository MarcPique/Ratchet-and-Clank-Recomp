//! **Orxon's flame vents, class 702** (level10 `0x2c7a20`, census U349; 3 placed, a row of them). Off for pvar 2
//! ticks (180), on for pvar 1 (120), alternately (the timer +0x0c; the first wait from 0). On: three type-40 flames
//! a tick along the vent's axis (`EulerToMatrix` of its rotation · (0, 0, pvar 0·3·dt): at the vent and 0.33 and
//! 0.66 of that step out, flags 0x40: their hits push nothing), the roar (sound 0 of class 0x519, flags 4) kept
//! alive; the burst over: the roar released.
//!
//! The flames hurt through their lines (`particles::type40`, the moby loop's next tick). Read from the level10 decomp.
//! Native `f32`.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2c7a20` | the vent | [`update`] |
//! | `0x262b08` (= L01 `0x284d88`) | `PartType40Spawn` | `crate::particles::type40::spawn` |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, V, DT};
use crate::moby_update::services::World;
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 10;
pub const UPDATE_FN: u32 = 0x2c_7a20;
pub const CLASSES: [i16; 1] = [702];

/// `PartType40Spawn(p, v, flags, owner)`.
fn flame(w: &mut World, p: V, v: V, owner: MobyId) {
    *w.svc.fx.part_spawns.entry(40).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else { return };
    if crate::particles::type40::spawn(sys, w.rng, p, v, 0x40, owner as u32 + 1).is_none() { w.svc.fx.part_failed += 1; }
}

/// Level10 `0x2c7a20` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, 0x14);
    match w.m(id).state {
        0 => {
            c::set_pi32(w, id, 0x10, -1);
            if c::dec_timer_pvar_i32(w, id, 0xc) != 0 {
                let t = c::pi32(w, id, 4);
                c::set_pi32(w, id, 0xc, t);
                w.mm(id).state = 1;
            }
        }
        1 => {
            let rows = crate::moby_update::services::euler_rows(crate::moby_update::services::pv(w.m(id).rotation));
            let z = c::pf(w, id, 0) * 3.0 * DT;
            let v: V = std::array::from_fn(|i| if i == 3 { 0.0 } else { f32::from_bits(rows[2][i].0) * z });
            let step = v.map(|x| x * f32::from_bits(0x3ea8_f5c3));
            let mut p = c::pos(w, id);
            for k in 0..3 {
                if 0 < k { p = c::add(p, step); }
                flame(w, p, v, id);
            }
            let s = c::pi32(w, id, 0x10);
            if !w.sound_alive(s, id) {
                let s = w.play_sound_as(0, 4, id, 0x519);
                c::set_pi32(w, id, 0x10, s);
            }
            if c::dec_timer_pvar_i32(w, id, 0xc) != 0 {
                let t = c::pi32(w, id, 8);
                c::set_pi32(w, id, 0xc, t);
                w.mm(id).state = 0;
                let s = c::pi32(w, id, 0x10);
                if s != -1 { w.release_sound(s, id); }
                c::set_pi32(w, id, 0x10, -1);
            }
        }
        _ => {}
    }
}
