//! Conveyor belts, classes 652 / 653 (level 02) and 296 (level 12): level02 0x2dc6b0, the same code on 12 (census U95;
//! 42 created instances). The belt does not move: it carries whoever rides it (mode 0x20, platform block at pvar
//! +0x60 through the loader's pointer at +0x08) by a velocity along its heading, `CarryRiders` every tick. A belt
//! with +0xa4 ≠ 0 (every belt on 02, 12 of 25 on 12) runs one way at 2.5 units/s; the others reverse every 180
//! ticks, easing down and up at 2.5 units/s², with the animation speed following the belt. Read from the level02
//! decomp (0x2dc6b0); the `$gp` constants (−2.5, 180, 2.5) are the same on both copies (02 0x161c14..1c, 12
//! 0x161960..68). Native `f32`.
//!
//! **Pvar block**: +0x40 the velocity (units per tick), +0x60..+0x9f the platform block (+0x9c its flags), +0xa0 the
//! reversal timer, +0xa4 one-way.
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2dc6b0 | no pvar block → return | [`update`] |
//! | state 0 | velocity = 0 (`0x221170`), timer = `ticks(180)`, flags +0x9c \|= 4, → 3 (one-way) or 1 | [`update`] |
//! | state 1 | anim speed +0x58 = 1; timer running: speed `v = |vel|` (0x2212e8) + 2.5·dt² while below `|−2.5·dt|` (0x221128), vel = −v·(cos, sin, 0) of the heading (0x2216f8 / 0x221710); run out: `v = |vel| − 2.5·dt²`, vel = −v·(cos, sin) while v > 0, else timer = `ticks(180)`, → 2 (vel kept); +0x58 = v / (−2.5·dt) | [`run`] |
//! | state 2 | the same with +0x58 = −1 first, vel = +v·(cos, sin), → 1, +0x58 = v / (2.5·dt) | [`run`] |
//! | state 3 | vel = 2.5·dt·(cos, sin, 0) | [`update`] |
//! | every state | `CarryRiders(+0x60, vel, +0x40, +0x40)` (0x2755f8) | `triggers::carry_riders` |
//! | | no sound, particle, light, save flag, other moby | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{len3, pi32, pv4, set_pi32, set_pv4, DT, DT2};
use crate::moby_update::services::{pvar as p, World};
use crate::moby_update::triggers;

/// The update in the level02 class table.
pub const UPDATE_FN: u32 = 0x2d_c6b0;
pub const REFERENCE_LEVEL: u32 = 2;
pub const CLASSES: [i16; 2] = [652, 653];

/// `$gp` −2.5: the belt speed factor (units per second; the belt runs "backwards" along its heading).
pub const SPEED: f32 = -2.5;
/// `$gp` 2.5: the easing, units per second².
pub const EASE: f32 = 2.5;
/// `$gp` 180: ticks between reversals.
pub const PERIOD: i32 = 180;

/// Level02 0x2dc6b0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xa8 { return; }
    match w.m(id).state {
        0 => {
            set_pv4(w, id, 0x40, [0.0; 4]);
            let t = w.ticks(PERIOD);
            set_pi32(w, id, 0xa0, t);
            let f = p::u32(&w.m(id).pvars, 0x9c) | 4;
            p::set_u32(&mut w.mm(id).pvars, 0x9c, f);
            w.mm(id).state = if pi32(w, id, 0xa4) == 0 { 1 } else { 3 };
        }
        1 => run(w, id, -1.0, 2),
        2 => run(w, id, 1.0, 1),
        3 => {
            let (c, s) = (w.m(id).rotation[2].cos(), w.m(id).rotation[2].sin());
            let k = -(SPEED * DT);
            set_pv4(w, id, 0x40, [c * k, s * k, 0.0, pv4(w, id, 0x40)[3]]);
        }
        _ => {}
    }
    let (vel, rot) = (pv4(w, id, 0x40), w.m(id).rotation);
    triggers::carry_riders(&mut w.mm(id).pvars, 0x60, vel, rot, rot);
}

/// States 1 (`dir` −1) and 2 (+1): ease up to full speed while the timer runs, then down, then reverse into `next`.
fn run(w: &mut World, id: MobyId, dir: f32, next: u8) {
    w.mm(id).anim.speed = -dir;
    let mut t = pi32(w, id, 0xa0);
    let fired = crate::moby_update::creature::dec_timer_i32(&mut t);
    set_pi32(w, id, 0xa0, t);
    let (c, s) = (w.m(id).rotation[2].cos(), w.m(id).rotation[2].sin());
    let w3 = pv4(w, id, 0x40)[3];
    let v = if fired == 0 {
        let mut v = len3(pv4(w, id, 0x40));
        if v < (SPEED * DT).abs() { v += EASE * DT2; }
        set_pv4(w, id, 0x40, [c * v * dir, s * v * dir, 0.0, w3]);
        v
    } else {
        let v = len3(pv4(w, id, 0x40)) - EASE * DT2;
        if 0.0 < v {
            set_pv4(w, id, 0x40, [c * v * dir, s * v * dir, 0.0, w3]);
        } else {
            let t = w.ticks(PERIOD);
            set_pi32(w, id, 0xa0, t);
            w.mm(id).state = next;
        }
        v
    };
    w.mm(id).anim.speed = v / (-dir * SPEED * DT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    fn belt(one_way: i32) -> Moby {
        let mut m = Moby { o_class: 296, mode: 0x20, pvars: vec![0; 0xb0], ..Moby::default() };
        p::set_i32(&mut m.pvars, 8, 0x60);
        p::set_i32(&mut m.pvars, 0xa4, one_way);
        m
    }

    #[test]
    fn one_way_and_reversing_belts_carry_riders() {
        let mut t = MobyTable::new(vec![belt(1), belt(0)], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        update(&mut w, 1);
        assert_eq!((w.m(0).state, w.m(1).state, p::u32(&w.m(0).pvars, 0x9c), pi32(&w, 1, 0xa0)), (3, 1, 4, 180));
        update(&mut w, 0);
        let d = triggers::platform_delta(w.m(0)).unwrap();
        assert_eq!(d.displacement, [2.5 * DT, 0.0, 0.0, 0.0], "one-way: +x at 2.5 u/s");
        // The reversing belt eases up by 2.5·dt² per tick, backwards along its heading.
        update(&mut w, 1);
        assert_eq!(pv4(&w, 1, 0x40)[0], -(EASE * DT2));
        assert_eq!(w.m(1).anim.speed, (EASE * DT2) / (SPEED * DT));
        for _ in 0..400 { update(&mut w, 1); }
        assert!(w.m(1).state == 2 || w.m(1).state == 1);
        let v = len3(pv4(&w, 1, 0x40));
        assert!(v <= 2.5 * DT + EASE * DT2 + 1e-6, "capped near full speed: {v}");
    }
}
