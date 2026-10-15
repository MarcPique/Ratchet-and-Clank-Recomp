//! The alarm drones, class 77: level15 0x2a2488, the same code on 17 (census U470; 36 created instances). A drone
//! waits hidden in its moby group (state 1) until an alarm 408 ([`super::quartu_alarm`]) releases one ([`release`],
//! 0x2a2868) at the alarm's position; it rises for a second, then homes on Ratchet at 10 units/s with a random
//! heading offset, holding 0.4–0.6 above his shadow point, and blows up on him (or after 240 ticks, or when a weapon
//! hits it): the death explosion and a sphere hit. While the level's alarm word is set every live drone trails
//! type-60 sparks, and one member per group and tick registers the group's glow (four camera-facing quads per live
//! member, 0x2a29b8). Read from the level15 decomp (0x2a2488, 0x2a2868, 0x2a29b8); the data words (gp−0x574c..,
//! the corner / ST tables 0x1ce110 / 0x1ce210) are the same bytes on 17. Native `f32`; the `rand` draws in the
//! game's order.
//!
//! **Pvar block**: +0x64 f32 the heading offset, +0x68 s32 the timer, +0x6c the group's registration word (pvar
//! shared data, `Services::pvar_shared`).
//!
//! | address | what | port |
//! |---|---|---|
//! | 0x2a2488 | `MobyGetHitMessage(m, 0x330000, 0)` (= L01 0x26f320) → state 4, +0xa4 = 0xff | [`update`] (`World::get_hit`) |
//! | | state ≠ 1: `rand_vec(0, 2·dt)` (0x26cb58); p = position, z + 0.333; alarm word 0x161ab8 = 0 → state 1, +0x94 = 0, done | [`update`] |
//! | | `PartType60Spawn(0.25, p, v, 0x60206080, trunc(ticks(20)), randi(0xff), 0)` (0x288588) | [`update`] (`World::part60`) |
//! | | +0x6c word ≠ tick counter → = counter, `RegisterDrawCallback(0x2a29b8)` (list 1) | [`update`] (`Callback::UnitQuads`); a null +0x6c (register every tick) is not reachable in the port's data (the loader always points it) [L] |
//! | state 0 | → the reset: state 1, +0x31 = 0, mode \| 1, +0x94 = 0 | [`update`] |
//! | state 2 | z += 2·dt; `FastDecTimer(+0x68)` → state 3, seq 1 (`MobyAnimBlend`, ticks(10)), +0x68 = ticks(240) | [`update`] |
//! | state 3 | heading = `FastArcTan(Ratchet − m)` + +0x64; v = (cos, sin)·10·dt, vz = ∓3·dt outside 0.4..0.6 above the shadow point z (0x13f418); `0x26d610(0.25, 0.25, 0, m, v, 0)` | [`update`] (`walker::move_collide`) |
//! | | `VecDistance2(body point 0x13f420, m) < 0.5` or the timer → state 4; seq 1 wrapped → seq 2 | [`update`] |
//! | state 4 | `0x273f50(0.25, 13, m, pos, −1)` (death explosion, no sound); `0x26e830(1, 1, 1, m, pos, 0x10001, 0, 1, 0)` (sphere hit: Ratchet and every moby the sphere lists); the reset | [`update`] (`fx::death_explosion`, `attack::sphere_hit`) |
//! | 0x2a2868 | the group's first member in state 1: state 2, seq 0 (no blend), +0x31 = 0, mode \| 1, collision on (class +0x10), +0x64 = `randf(−15, 15)`°, +0x68 = ticks(60), position = the alarm's, z − 1, yaw toward Ratchet; 1 / 0 | [`release`] |
//! | 0x2a29b8 | FX 8, ALPHA 0x44; every member of the group not in state 1: rows = `EulerToMatrix(0, −atan(\|cam − m\|xy, cam.z − m.z), atan(cam − m))`, point = position + 0.333 z; 4 quads (0x1ce110: ±0.2, ±0.1, ±0.25, ±0.3 in y / z), colours gp−0x5738.. | [`fx_quads`] + `rc-engine` fx_draw; the camera `0x1673c0` is the one the update saw when it registered [L] |
//! | | no sound, light, save flag, bolt | n/a |

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::{add_rot, atan, attack, dec_timer_pvar_i32, dist2, fx, pf, pi32, set_pf, set_pi32, walker, DT};
use crate::moby_update::services::{Services, World};

use super::{FxQuad, FxQuads};

/// The update in the level15 class table.
pub const UPDATE_FN: u32 = 0x2a_2488;
pub const REFERENCE_LEVEL: u32 = 15;
pub const CLASSES: [i16; 1] = [77];

/// The level's alarm word (level15 0x161ab8 = gp−0x5148, written by the alarms 408: [`super::quartu_alarm`]).
pub const ALARM_WORD: u32 = 0x16_1ab8;
/// The camera the glow reads (`0x1673c0`), kept by the registering update (three words).
pub const CAMERA_WORD: u32 = 0x16_73c0;

/// Pvar offsets.
pub mod pv {
    pub const HEADING: usize = 0x64;
    pub const TIMER: usize = 0x68;
    pub const WORD: usize = 0x6c;
}

/// gp−0x574c: the homing speed.
pub const SPEED: f32 = 10.0;
/// gp−0x5748..−0x5740: the sparks' colour, size, life (ticks).
pub const SPARK_RGBA: u32 = 0x6020_6080;
pub const SPARK_SIZE: f32 = 0.25;
pub const SPARK_LIFE: i32 = 20;
/// gp−0x573c: the glow's height above the position (0x3eaa7efa).
pub const GLOW_Z: f32 = f32::from_bits(0x3eaa_7efa);
/// gp−0x5738..: the four quads' colours.
pub const GLOW_RGBA: [u32; 4] = [0x4060_2080, 0x40ff_8080, 0x2020_80ff, 0x1020_8080];
/// The FX texture (`GetEffectTex(8)`).
pub const FX: usize = 8;
/// 0x1ce110: the four quads' half sizes (corners (0, ±h, ±h) in the order (h, −h), (−h, −h), (h, h), (−h, h)).
pub const HALF: [f32; 4] = [0.2, 0.1, 0.25, 0.3];
/// 0x1ce210: the ST of the four corners.
pub const ST: [[f32; 2]; 4] = [[1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [0.0, 1.0]];

/// Level15 0x2a2488 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x70 { return; }
    if w.get_hit(id, 0x33_0000, false).is_some() {
        let m = w.mm(id);
        m.state = 4;
        m.hit_slot = 0xff;
    }
    if w.m(id).state != 1 {
        let v = w.rng.rand_vec(0.0, 2.0 * DT);
        let mut p = w.m(id).position;
        p[2] += GLOW_Z;
        if w.svc.units.word(ALARM_WORD) == 0 {
            let m = w.mm(id);
            m.state = 1;
            m.has_collision = false;
            return;
        }
        let life = w.ticks(SPARK_LIFE);
        let rot = w.rng.randi(0xff) as u8;
        w.part60(SPARK_SIZE, p, [v[0], v[1], v[2], 0.0], SPARK_RGBA, life as u16, rot, 0);
        register(w, id);
    }
    match w.m(id).state {
        0 => reset(w, id),
        2 => {
            w.mm(id).position[2] += 2.0 * DT;
            if dec_timer_pvar_i32(w, id, pv::TIMER) == 0 { return; }
            w.mm(id).state = 3;
            if w.m(id).anim.seq_b != 1 {
                let t = w.ticks(10);
                w.anim_blend(id, 1, 0, t);
            }
            let t = w.ticks(240);
            set_pi32(w, id, pv::TIMER, t);
        }
        3 => {
            let h = super::hero_pos(w);
            let q = w.m(id).position;
            let a = add_rot(pf(w, id, pv::HEADING), atan(h[0] - q[0], h[1] - q[1]));
            let dz = q[2] - crate::moby_update::services::fl(w.hero.shadow_point[2]);
            let vz = if 0.6 < dz { -(DT * 3.0) } else if dz < 0.4 { DT * 3.0 } else { 0.0 };
            let mut v = [a.cos() * SPEED * DT, a.sin() * SPEED * DT, vz, 0.0];
            walker::move_collide(w, id, 0.25, 0.25, 0.0, &mut v, 0);
            let b = w.hero_body_point();
            let d = dist2([b[0], b[1], b[2], 0.0], w.m(id).position);
            if d < 0.5 || dec_timer_pvar_i32(w, id, pv::TIMER) != 0 {
                w.mm(id).state = 4;
                return;
            }
            if w.m(id).anim.seq_b != 1 || w.m(id).anim.flags & 2 == 0 { return; }
            let t = w.ticks(10);
            w.anim_blend(id, 2, 0, t);
        }
        4 => {
            let p = w.m(id).position;
            fx::death_explosion(w, 0.25, 13.0, Some(id), p, -1);
            attack::sphere_hit(w, 1.0, 1.0, 1.0, id, p, 0x1_0001, 0, 1, 0);
            reset(w, id);
        }
        _ => {}
    }
}

/// The drone's rest: state 1, not drawn, collision off.
fn reset(w: &mut World, id: MobyId) {
    let m = w.mm(id);
    m.state = 1;
    m.visible = 0;
    m.mode |= mode::HIDDEN;
    m.has_collision = false;
}

/// One registration per group and tick (the pvar shared word), with the camera the glow faces.
fn register(w: &mut World, id: MobyId) {
    let word = pi32(w, id, pv::WORD);
    let tick = w.counter as i32;
    if w.svc.shared_i32(word) == tick { return; }
    w.svc.set_shared_i32(word, tick);
    let c = w.camera_point();
    for (k, x) in c.iter().enumerate() { w.svc.units.set_word(CAMERA_WORD + 4 * k as u32, x.to_bits()); }
    if let Some(i) = super::row(REFERENCE_LEVEL, UPDATE_FN) { w.svc.draw_callbacks.register(Callback::UnitQuads(i), id); }
}

/// Level15 0x2a2868(group, point): releases the first resting drone of moby group `group` at `point` (module doc).
/// True when one was released.
pub fn release(w: &mut World, group: i32, point: [f32; 4]) -> bool {
    let Some(list) = w.svc.groups.lists.get(group as u8 as usize).and_then(|l| l.clone()) else { return false };
    for e in list {
        let d = (e & 0x7fff) as usize;
        if w.table.mobys.get(d).is_none_or(|m| m.state != 1) { continue; }
        w.mm(d).state = 2;
        if w.m(d).anim.seq_b != 0 { w.anim_blend(d, 0, 0, 0); }
        let oc = w.m(d).o_class;
        let coll = super::class_collision(w, oc);
        let m = w.mm(d);
        m.visible = 0;
        m.mode |= mode::HIDDEN;
        m.has_collision = coll;
        let a = w.rng.randf(-15.0, 15.0) * f32::from_bits(0x3c8e_fa35);
        set_pf(w, d, pv::HEADING, a);
        let t = w.ticks(60);
        set_pi32(w, d, pv::TIMER, t);
        let h = super::hero_pos(w);
        let m = w.mm(d);
        m.position = point;
        m.position[2] -= 1.0;
        m.rotation[2] = atan(h[0] - m.position[0], h[1] - m.position[1]);
        return true;
    }
    false
}

/// Level15 0x2a29b8 for the registering drone `id` (module doc): the glow of every live member of its group.
pub fn fx_quads(table: &MobyTable, svc: &Services, id: MobyId) -> Option<FxQuads> {
    let me = table.mobys.get(id)?;
    let list = svc.groups.lists.get(me.group as u8 as usize)?.as_ref()?;
    let cam: [f32; 3] = std::array::from_fn(|k| f32::from_bits(svc.units.word(CAMERA_WORD + 4 * k as u32)));
    let mut quads = Vec::new();
    for &e in list {
        let Some(m) = table.mobys.get((e & 0x7fff) as usize) else { continue };
        if m.state == 1 { continue; }
        let p = m.position;
        let yaw = atan(cam[0] - p[0], cam[1] - p[1]);
        let d = ((p[0] - cam[0]).powi(2) + (p[1] - cam[1]).powi(2)).sqrt();
        let pitch = -atan(d, cam[2] - p[2]);
        let r = crate::follow_camera::script::euler_rows([0.0, pitch, yaw]);
        let o = [p[0], p[1], p[2] + GLOW_Z];
        let world = |v: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| v[0] * r[0][k] + v[1] * r[1][k] + v[2] * r[2][k] + o[k]) };
        for (h, rgba) in HALF.iter().zip(GLOW_RGBA) {
            let c = [[0.0, *h, -h], [0.0, -h, -h], [0.0, *h, *h], [0.0, -h, *h]].map(world);
            quads.push(FxQuad { corners: c, st: ST, rgba: [rgba; 4] });
        }
    }
    Some(FxQuads { fx: FX, additive: false, subtract: false, quads })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::scheduler::Groups;

    fn drone(state: u8) -> Moby {
        let mut m = Moby { o_class: 77, group: 0, state, pvars: vec![0; 0x70], ..Moby::default() };
        m.position = [5.0, 0.0, 1.0, 1.0];
        m
    }

    #[test]
    fn released_drone_rises_then_homes_and_explodes_on_ratchet() {
        let mut t = MobyTable::new(vec![drone(1), drone(1)], 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = Services::new();
        svc.groups = Groups { lists: vec![Some(vec![0, 0x8001])] };
        svc.pvar_shared = vec![0; 4];
        svc.units.set_word(ALARM_WORD, 1);
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 7);
        assert!(release(&mut w, 0, [4.0, 0.0, 2.0, 1.0]));
        assert_eq!((w.m(0).state, w.m(1).state), (2, 1), "the first resting member");
        assert_eq!(w.m(0).position[..3], [4.0, 0.0, 1.0]);
        assert_eq!(pi32(&w, 0, pv::TIMER), 60);
        assert!(pf(&w, 0, pv::HEADING).abs() <= 15f32.to_radians() + 1e-6);
        // Rises one second, then homes.
        for _ in 0..60 { update(&mut w, 0); }
        assert!((w.m(0).position[2] - 3.0).abs() < 1e-3, "{:?}", w.m(0).position);
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 3);
        let x0 = w.m(0).position[0];
        update(&mut w, 0);
        assert!(w.m(0).position[0] < x0, "toward Ratchet at the origin");
        // One registration per tick for the group.
        assert_eq!(w.svc.draw_callbacks.list1.len(), 1);
        assert_eq!(w.svc.shared_i32(0), 7);
        // Next to Ratchet's body point: the explosion and the reset.
        w.mm(0).position = [0.2, 0.0, 0.7, 1.0];
        update(&mut w, 0);
        assert_eq!(w.m(0).state, 4);
        update(&mut w, 0);
        assert_eq!((w.m(0).state, w.m(0).has_collision, w.m(0).mode & mode::HIDDEN), (1, false, mode::HIDDEN));
        // No alarm: a live drone goes back to rest at once.
        w.mm(1).state = 3;
        w.svc.units.set_word(ALARM_WORD, 0);
        update(&mut w, 1);
        assert_eq!(w.m(1).state, 1);
    }

    #[test]
    fn glow_has_four_quads_per_live_member() {
        let mut t = MobyTable::new(vec![drone(3), drone(1), drone(2)], 4);
        t.mobys[2].position = [0.0, 5.0, 1.0, 1.0];
        let mut svc = Services::new();
        svc.groups = Groups { lists: vec![Some(vec![0, 1, 0x8002])] };
        svc.units.set_word(CAMERA_WORD + 8, 1f32.to_bits());
        let g = fx_quads(&t, &svc, 0).unwrap();
        assert_eq!((g.quads.len(), g.fx, g.additive), (8, FX, false));
        // Camera at (0, 0, 1), level with it: the first drone's quads lie in the plane x = 5 (they face the camera).
        for c in g.quads[0].corners { assert!((c[0] - 5.0).abs() < 1e-5, "{c:?}"); }
        assert_eq!(g.quads[1].rgba, [GLOW_RGBA[1]; 4]);
    }
}
