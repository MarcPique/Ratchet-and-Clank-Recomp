//! Rising blocks, classes 852 / 853: level05 0x314eb0 (census U185; 21 created instances on Rilgar). Each block starts
//! 8 units below its placed height; when its linked moby's command byte (+0xbc) becomes 1 it waits its delay, then
//! springs up to 1 unit above the placed height, while (after its own timer) its y tilt eases back to 0; command 2
//! puts it there at once. 853 plays sound 0 once near the top, 852 when its tilt timer runs out on 1. Read from the
//! level05 decomp and disassembly (0x314eb0); the `$gp` constants 50, 10, 18, 360, 360, 1440 (0x161e80..94). Native
//! `f32`.
//!
//! **Pvar block**: +0x00 the linked moby (a pvar moby link), +0x04 the rise delay, +0x08 the tilt timer, +0x0c f32 the
//! target height, +0x10 f32 the rise velocity, +0x14 f32 the tilt velocity.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | target = z + 1, z −= 8, → 1 | [`update`] |
//! | state 1 | the link's +0xbc: 1 → 2; 2 → 3 with z = target, tilt 0 | [`update`] |
//! | state 2 | `FastDecTimer(+0x04)` ≠ 0 → `0x270830(target, 50·dt², 10·dt², 18·dt, &z, &v)`; then 853 with `|z − target|` < 3 and `|target − z + v|` > 3 → sound 0; `FastDecTimer(+0x08)`: running → sound 0 on 852 when it reads 1; else `0x270cc0(0, 360°·dt², 360°·dt², 1440°·dt, &rot.y, &w)`, rot.y = 0 → 3 | [`update`] (`turn::spring`, `turn::turn_toward`) |
//! | | no particle, light, save flag, other moby written | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::turn::{spring, turn_toward};
use crate::moby_update::creature::{dec_timer_pvar_i32, pf, pi32, set_pf, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x31_4eb0;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 2] = [852, 853];

const DEG: f32 = f32::from_bits(0x3c8e_fa35);

/// Level05 0x314eb0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let oc = w.m(id).o_class;
    match w.m(id).state {
        0 => {
            let z = w.m(id).position[2];
            set_pf(w, id, 0xc, z + 1.0);
            let m = w.mm(id);
            m.state = 1;
            m.position[2] = z - 8.0;
        }
        1 => {
            let link = usize::try_from(pi32(w, id, 0)).ok().and_then(|l| w.table.mobys.get(l)).map_or(0, |m| m.cmd);
            match link {
                1 => w.mm(id).state = 2,
                2 => {
                    let t = pf(w, id, 0xc);
                    let m = w.mm(id);
                    m.state = 3;
                    m.position[2] = t;
                    m.rotation[1] = 0.0;
                }
                _ => {}
            }
        }
        2 => {
            if dec_timer_pvar_i32(w, id, 4) != 0 {
                let target = pf(w, id, 0xc);
                let (mut z, mut v) = (w.m(id).position[2], pf(w, id, 0x10));
                spring(target, 50.0 * DT2, 10.0 * DT2, 18.0 * DT, &mut z, &mut v);
                w.mm(id).position[2] = z;
                set_pf(w, id, 0x10, v);
                if oc == 853 && (z - target).abs() < 3.0 && 3.0 < (target - z + v).abs() { w.play_sound(0, 0, id); }
            }
            if dec_timer_pvar_i32(w, id, 8) == 0 {
                if pi32(w, id, 8) == 1 && oc == 852 { w.play_sound(0, 0, id); }
            } else {
                let (mut a, mut v) = (w.m(id).rotation[1], pf(w, id, 0x14));
                turn_toward(0.0, 360.0 * DEG * DT2, 360.0 * DEG * DT2, 1440.0 * DEG * DT, &mut a, &mut v);
                w.mm(id).rotation[1] = a;
                set_pf(w, id, 0x14, v);
                if a == 0.0 { w.mm(id).state = 3; }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;

    #[test]
    fn rises_on_the_links_command() {
        let mut b = Moby { o_class: 853, position: [0.0, 0.0, 10.0, 1.0], rotation: [0.0, 0.5, 0.0, 0.0], pvars: vec![0; 0x20], ..Moby::default() };
        p::set_i32(&mut b.pvars, 0, 1);
        p::set_i32(&mut b.pvars, 8, 300);
        let mut t = MobyTable::new(vec![b, Moby::default()], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!((w.m(0).state, w.m(0).position[2], pf(&w, 0, 0xc)), (1, 2.0, 11.0));
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        w.mm(1).cmd = 1;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 2);
        for _ in 0..600 { update(&mut w, 0); }
        assert_eq!((w.m(0).state, w.m(0).position[2], w.m(0).rotation[1]), (3, 11.0, 0.0));
        assert_eq!(w.svc.sounds.len(), 1, "853: one sound near the top");
    }
}
