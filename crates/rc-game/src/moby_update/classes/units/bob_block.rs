//! Bobbing blocks, classes 1091–1098 and 1103: level06 0x300df0 (census U221; 21 created instances on Kalebo III). A
//! block hovers at its placed height (kept in its pvars) and bobs 3 units up and down over 600 ticks of the tick
//! counter, all in step; while Ratchet is held by a cutscene (state 0x72) it rests at the placed height. State 2 (set
//! by another moby) makes it drift away with its pvar velocity and spin, slowing by 1.5 % a tick while that vector's
//! w is above 1, and deletes it once the cutscene hold ends. Read from the level06 decomp and disassembly
//! (0x300df0; `$gp` 3.0 at 0x162090). Native `f32`.
//!
//! **Pvar block**: +0x00 the drift velocity (its w = +0x0c, the placed height from the init), +0x10 the spin.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x300df0 | no pvar block → `DeleteMoby` | [`update`] |
//! | state 0 | draw distance 0x200, update distance 0xff, +0x0c = z, → 1 | [`update`] |
//! | state 1 | Ratchet in state 0x72: z = +0x0c; else z = +0x0c + 3·sin(2π·(tick mod 600)/600) (0x221710; tick `0x15f5cc`) | [`update`] |
//! | state 2 | +0x0c > 1 → velocity.xyz ·= 0.985 (`VecScale`, 0x221210); position += velocity; Euler += spin (0x221188); Ratchet not in 0x72 → `DeleteMoby` | [`update`] |
//! | | no sound, particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{add, pf, pv4, set_pf, set_pv4};
use crate::moby_update::services::World;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x30_0df0;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 9] = [1091, 1092, 1093, 1094, 1095, 1096, 1097, 1098, 1103];

/// Ratchet held by a cutscene (`0x1413d4`).
const HELD: i32 = 0x72;
/// The bob's amplitude (`$gp` 3.0) and period (600 ticks).
pub const AMPLITUDE: f32 = 3.0;
pub const PERIOD: i64 = 600;
/// The game's 2π (`0x40c90fd0` = 6.28318).
const TWO_PI: f32 = f32::from_bits(0x40c9_0fd0);

/// Level06 `0x300f70(m, velocity, spin)`: a block set drifting (state 2; Blarg's shuttle's blast,
/// `super::blarg_shuttle`).
pub fn drift(w: &mut World, id: MobyId, vel: [f32; 4], spin: [f32; 4]) {
    if w.m(id).pvars.len() < 0x20 { w.mm(id).pvars.resize(0x20, 0); }
    w.mm(id).state = 2;
    set_pv4(w, id, 0, vel);
    set_pv4(w, id, 0x10, spin);
}

/// Level06 0x300df0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { w.delete_moby(id); return; }
    let held = w.hero.state == HELD;
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            set_pf(w, id, 0xc, z);
            let m = w.mm(id);
            m.draw_dist = 0x200;
            m.update_dist = 0xff;
            m.state = 1;
        }
        1 => {
            let base = pf(w, id, 0xc);
            let z = if held {
                base
            } else {
                let t = (w.counter as i32 % PERIOD as i32) as f32;
                base + AMPLITUDE * (t / 600.0 * TWO_PI).sin()
            };
            w.mm(id).position[2] = z;
        }
        2 => {
            let mut v = pv4(w, id, 0);
            if 1.0 < v[3] {
                for x in v.iter_mut().take(3) { *x *= f32::from_bits(0x3f7c_28f6); }
                set_pv4(w, id, 0, v);
            }
            let spin = pv4(w, id, 0x10);
            let m = w.mm(id);
            m.position = add(m.position, v);
            m.rotation = add(m.rotation, spin);
            if !held { w.delete_moby(id); }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    #[test]
    fn bobs_on_the_tick_counter_and_rests_in_cutscenes() {
        let mut t = MobyTable::new(vec![Moby { o_class: 1091, position: [0.0, 0.0, 20.0, 1.0], pvars: vec![0; 0x20], ..Moby::default() }], 4);
        let mut hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        {
            let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 150);
            update(&mut w, 0);
            update(&mut w, 0);
            assert_eq!((w.m(0).draw_dist, w.m(0).update_dist), (0x200, 0xff));
            assert!((w.m(0).position[2] - 23.0).abs() < 1e-4, "a quarter period: the top");
        }
        hero.state = HELD;
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 150);
        update(&mut w, 0);
        assert_eq!(w.m(0).position[2], 20.0);
    }
}
