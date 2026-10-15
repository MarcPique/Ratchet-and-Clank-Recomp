//! Sliding blocks placed by a cuboid, class 843: level05 0x30e508 (census U185; 16 created instances on Rilgar). A
//! block sits on its cuboid's y row at +0x18 from the cuboid's centre (the other side, mirrored, with +0x1c), facing
//! the cuboid's yaw. When its linked moby's command byte (+0xbc) turns non-zero it plays sound 0, slides out along
//! the row to three times that distance (a spring on the distance left), then sinks until it is 5.9 below the
//! cuboid's centre. Read from the level05 decomp (0x30e508). Native `f32`.
//!
//! **Pvar block**: +0x00 the velocity (v3; +0x08 the sink velocity), +0x10 the cuboid, +0x14 the linked moby (runtime
//! index), +0x18 the offset, +0x1c mirrored.
//!
//! | address | what | port |
//! |---|---|---|
//! | state 0 | position = the cuboid's centre (+0x30..+0x3c), yaw = its Euler z (+0x78); o = unit(row 1)·+0x18, +0x1c ≠ 0 → −o and mode \| 0x8000; position += o; → 1 | [`update`] |
//! | state 1 | the link's +0xbc ≠ 0 → `PlayClassSound(0, 0, m)`, → 2, velocity = 0 (0x221170) | [`update`] |
//! | state 2 | d = centre ± unit(row 1)·3·+0x18 − position; speed = \|v\|; `0x270830(\|d\|, 20·dt², 30·dt², 10·dt, &0, &speed)`; v = unit(d)·speed; position += v; \|d\| < 0.01 → 3 | [`update`] (`turn::spring`) |
//! | state 3 | `0x270830(centre z − 5.9, 20·dt², 40·dt², 10·dt, &z, &+0x08)`; z below it → 4 | [`update`] |
//! | | no particle, light, save flag, other moby written | n/a |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::turn::spring;
use crate::moby_update::creature::{pf, pi32, pv4, set_pf, set_pv4, DT, DT2};
use crate::moby_update::services::World;
use rc_formats::volumes::ShapeKind;

/// The update in the level05 class table.
pub const UPDATE_FN: u32 = 0x30_e508;
pub const REFERENCE_LEVEL: u32 = 5;
pub const CLASSES: [i16; 1] = [843];

/// How far below the cuboid's centre the block sinks.
pub const SINK: f32 = 5.9;

fn unit(v: [f32; 3], l: f32) -> [f32; 4] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n == 0.0 { [0.0; 4] } else { [v[0] * l / n, v[1] * l / n, v[2] * l / n, 0.0] }
}

/// Level05 0x30e508 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let Some(c) = w.svc.volumes.shape(ShapeKind::Cuboid, pi32(w, id, 0x10)).copied() else { return };
    let row1 = [c.matrix[1][0], c.matrix[1][1], c.matrix[1][2]];
    let flip = pi32(w, id, 0x1c) != 0;
    let sign = if flip { -1.0 } else { 1.0 };
    match w.m(id).state {
        0 => {
            let o = unit(row1, pf(w, id, 0x18));
            let m = w.mm(id);
            m.position = c.matrix[3];
            m.rotation[2] = c.euler[2];
            if flip { m.mode |= mode::MIRROR; }
            for (p, o) in m.position.iter_mut().zip(o).take(3) { *p += o * sign; }
            m.state = 1;
        }
        1 => {
            let link = pi32(w, id, 0x14);
            if usize::try_from(link).ok().and_then(|l| w.table.mobys.get(l)).is_some_and(|m| m.cmd != 0) {
                w.play_sound(0, 0, id);
                w.mm(id).state = 2;
                set_pv4(w, id, 0, [0.0; 4]);
            }
        }
        2 => {
            let o = unit(row1, pf(w, id, 0x18) * 3.0);
            let p = w.m(id).position;
            let d: [f32; 3] = std::array::from_fn(|k| c.matrix[3][k] + o[k] * sign - p[k]);
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let v0 = pv4(w, id, 0);
            let mut speed = (v0[0] * v0[0] + v0[1] * v0[1] + v0[2] * v0[2]).sqrt();
            let mut x = 0.0;
            spring(l, 20.0 * DT2, 30.0 * DT2, 10.0 * DT, &mut x, &mut speed);
            let v = unit(d, speed);
            set_pv4(w, id, 0, [v[0], v[1], v[2], v0[3]]);
            let m = w.mm(id);
            for (p, v) in m.position.iter_mut().zip(v).take(3) { *p += v; }
            if l < 0.01 { m.state = 3; }
        }
        3 => {
            let floor = c.matrix[3][2] - SINK;
            let mut z = w.m(id).position[2];
            let mut v = pf(w, id, 8);
            spring(floor, 20.0 * DT2, 40.0 * DT2, 10.0 * DT, &mut z, &mut v);
            set_pf(w, id, 8, v);
            w.mm(id).position[2] = z;
            if z < floor { w.mm(id).state = 4; }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::services::pvar as p;
    use rc_formats::volumes::{Shape, Volumes};

    #[test]
    fn slides_out_three_times_then_sinks() {
        let mut m = Moby { o_class: 843, pvars: vec![0; 0x20], ..Moby::default() };
        p::set_i32(&mut m.pvars, 0x14, 1);
        p::set_ff(&mut m.pvars, 0x18, 2.0);
        let link = Moby { o_class: 1, ..Moby::default() };
        let mut t = MobyTable::new(vec![m, link], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        let c = Shape { matrix: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [10.0, 20.0, 30.0, 1.0]], ..Shape::default() };
        svc.volumes = std::sync::Arc::new(Volumes { cuboids: vec![c], ..Default::default() });
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        update(&mut w, 0);
        assert_eq!(w.m(0).position, [10.0, 22.0, 30.0, 1.0]);
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 1, "waits for the link");
        w.mm(1).cmd = 1;
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 2);
        assert!(w.svc.sounds.iter().any(|s| (s.moby, s.index) == (0, 0)));
        let mut n = 0;
        while w.m(0).state == 2 && n < 600 { update(&mut w, 0); n += 1; }
        assert!((w.m(0).position[1] - 26.0).abs() < 0.02, "out to 3·2: {:?} after {n}", w.m(0).position);
        while w.m(0).state == 3 && n < 1200 { update(&mut w, 0); n += 1; }
        assert_eq!(w.m(0).state, 4);
        assert!(w.m(0).position[2] < 30.0 - SINK);
    }
}
