//! Sliders, class 367: level06 0x2d9d10 (census U204; 8 created instances on Kalebo III). The init records the slider's
//! home and a far point 11 units along its second row (its left); in state 2 (set by another moby, with a target in
//! +0x28) its position springs along home → far to the target fraction (`0x270830(target, 10·dt², 10·dt², 20·dt)`)
//! and it rests (state 1) on arrival. Read from the level06 decomp (0x2d9d10). Native `f32`.
//!
//! **Pvar block**: +0x00 home, +0x10 far, +0x20 the fraction, +0x24 its velocity, +0x28 the target fraction.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | home = position; far = home + rows[1] at length 11 (0x221410, 0x221188); → 1 | [`update`] |
//! | state 2 | `0x270830(+0x28, 10·dt², 10·dt², 20·dt, &+0x20, &+0x24)`; position.xyz = `lerp(+0x20, home, far)` (0x2211e8); fraction = target → 1 | [`update`] (`turn::spring`) |
//! | | no sound, particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::turn::spring;
use crate::moby_update::creature::{add, pf, pv4, set_len3, set_pf, set_pv4, DT, DT2};
use crate::moby_update::services::World;

/// The update in the level06 class table.
pub const UPDATE_FN: u32 = 0x2d_9d10;
pub const REFERENCE_LEVEL: u32 = 6;
pub const CLASSES: [i16; 1] = [367];

/// Level06 0x2d9d10 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x2c { return; }
    match w.m(id).state {
        0 => {
            let home = w.m(id).position;
            let r = w.m(id).rows[1];
            set_pv4(w, id, 0, home);
            set_pv4(w, id, 0x10, add(set_len3([r[0], r[1], r[2], r[3]], 11.0), home));
        }
        2 => {
            let target = pf(w, id, 0x28);
            let (mut t, mut v) = (pf(w, id, 0x20), pf(w, id, 0x24));
            spring(target, DT2 * 10.0, DT2 * 10.0, DT * 20.0, &mut t, &mut v);
            set_pf(w, id, 0x20, t);
            set_pf(w, id, 0x24, v);
            let (a, b) = (pv4(w, id, 0), pv4(w, id, 0x10));
            let m = w.mm(id);
            for k in 0..3 { m.position[k] = a[k] + (b[k] - a[k]) * t; }
            if t != target { return; }
        }
        _ => return,
    }
    w.mm(id).state = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    #[test]
    fn slides_to_the_target_fraction() {
        let mut m = Moby { o_class: 367, position: [5.0, 0.0, 0.0, 1.0], pvars: vec![0; 0x70], ..Moby::default() };
        m.rows[1] = [0.0, 1.0, 0.0, 0.0];
        let mut t = MobyTable::new(vec![m], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!((w.m(0).state, pv4(&w, 0, 0x10)[1]), (1, 11.0));
        w.mm(0).state = 2;
        set_pf(&mut w, 0, 0x28, 1.0);
        for _ in 0..300 { update(&mut w, 0); }
        assert_eq!((w.m(0).state, w.m(0).position[1]), (1, 11.0));
    }
}
