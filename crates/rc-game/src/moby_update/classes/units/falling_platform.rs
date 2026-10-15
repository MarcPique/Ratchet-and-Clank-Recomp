//! U565 (census 2026-09-29): class 1381, the falling platforms of Veldin's last arena (level18 `0x2f16f0`: 8 created).
//! A platform that carries the Veldin carriers 1584 of its moby group as children placed by its matrix (the child
//! placement pair `triggers::record_children` / `triggers::place_children`, G-CLS-024), swings toward a target pose
//! (a cuboid) when told to, and falls away once Ratchet has stood on it and left, deleting its children below z 20.
//! Read from the level18 decomp. The name is descriptive [L]. Native `f32`.
//!
//! **Pvars** (0x350): +0x04 the target cuboid, +0x08 (≠ 0: fall once past t 1), +0x0c (the boss's handler reads it),
//! +0x10 / +0x20 the start pose, +0x30 16 child records (0x30 each: offset, Euler, moby index), +0x330 t and +0x334
//! its speed, +0x338 the child count, +0x340 the fall speed, +0x344 the debris rate, +0x348 Ratchet stood on it,
//! +0x34c the leave timer.
//!
//! | address | what it does | port |
//! |---|---|---|
//! | state 0 | start pose +0x10 / +0x20; the group's members of class 1584 (0x630) into the records (at most 16; no group list → stay), +0x348 = 0, +0x34c = `ticks(30)`, +0x338 = count, `0x265250` (the records in its frame), state 1 | [`update`] (`triggers::record_children`) |
//! | state 2 | (gp 0x162288 ≠ 0: always) t springs (`0x270830`) toward 1.5 (3·dt², 3·dt², 2·dt) below 1.5, else toward 2 (0.1·dt², 0.1·dt², 0.2·dt) | [`update`] (`turn::spring`) |
//! | | t > 0.1525 and +0x344 = 0 → +0x344 = 10; t > 1 and +0x08 ≠ 0 → state 4 | [`update`] |
//! | | Ratchet's ground moby is a child → +0x348 = 1; a class-1892 moby counts as on it; stood on it, now grounded elsewhere: `FastDecTimer(+0x34c)` → state 4 | [`update`] (`Hero::ground_moby`, `air_ticks`) |
//! | state 4 | z += +0x340, +0x340 −= 5·dt²; below z 20: each child `0x2fa8e0` (class 1584: its attachment +0x84 and itself deleted), itself deleted | [`update`] |
//! | states 1–2 | pose = `lerp(start, cuboid centre / Euler, t/2)` (`0x2211e8` ×2) | [`update`] |
//! | every tick | `0x265358`: the children placed | [`update`] (`triggers::place_children`) |
//! | | +0x344 ≠ 0, state ≠ 4: +0x344 += 0.25; `randi(trunc(+0x344)) == 0` → a burning bit (`SpawnDebrisMoby(0.1, 2, 1, 0.75, p, 0, class 696, ticks(240), 0)`) at Ratchet's feet + (15 up, `randf(0, 20)` out at the camera yaw `randf(±60°)`) | [`update`] (`gunship::spawn_ember`, `World::camera_yaw`) |
//! | 0x2f1c38 | (called by the boss 1422) a 1381 in state 1: +0x0c ≠ −1 → the grind-path cuts `0x2f1d08(0x802 / 0x908 / 0x954, cuboid +0x0c)`; state 2; the group command `0x2fa888(group, 1)` | [`start`] (the group command ported; the grind cuts NOT ported: G-HERO-039) |
//! | 0x2fa888(group, v) | every member of class 0x630 (the carriers 1584): +0xbc = v | [`start`] |
//! | 0x2f1d08(k, cuboid) | the grind path `0x1c6200[byte 0x1c6500 + k]`: each of its points (0x20 apart from +0x28, count +0x26) inside the cuboid (\|x\|, \|y\| < 1 in its frame) gets +0x0c = 0 and +0x14 = 1 (the rail cut) | NOT ported (G-HERO-039: the loader's grind table 0x1c6500 / 0x1c6200 and the 0x20-byte run-time rail points are not modelled; the hero's rails are `svc.volumes.grind_paths`) |
//! | 0x2f1cc8 | (called by the boss) state 1, t 0, rate 0; the children placed | [`reset`] |
//! | | no sound, particle, light, hit, save flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::gunship;
use crate::moby_update::creature::{self as c, add_rot, turn, DT, DT2};
use crate::moby_update::services::World;
use crate::moby_update::triggers;

pub const UPDATE_FN: u32 = 0x2f_16f0;
pub const REFERENCE_LEVEL: u32 = 18;
pub const CLASSES: [i16; 1] = [1381];
/// The children it gathers (the Veldin carriers) and the class that counts as standing on it (their attachment).
pub const CHILD_CLASS: i16 = 0x630;
pub const ATTACHMENT_CLASS: i16 = 0x764;
pub const RECORDS: usize = 0x30;
pub const SIZE: usize = 0x350;
/// Level18 gp words (0x162290.., read from the overlay's data; the switch 0x162288 before them is 1 there and no code
/// writes it, so the spring always runs).
const NEAR: (f32, f32, f32) = (3.0, 3.0, 2.0);
const FAR: (f32, f32, f32) = (0.1, 0.1, 0.2);
const RATE_AT: f32 = f32::from_bits(0x3e1c_28f6);
const FALL_AT: f32 = 1.0;
const RATE0: f32 = 10.0;
const DEBRIS_SIZE: f32 = 0.1;
const DEBRIS_CLASS: i16 = 0x2b8;

fn count(w: &World, id: MobyId) -> usize { c::pi32(w, id, 0x338).clamp(0, 16) as usize }

/// Level18 0x2f16f0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    match w.m(id).state {
        0 => {
            let (p, r) = (c::pos(w, id), w.m(id).rotation);
            c::set_pv4(w, id, 0x10, p);
            c::set_pv4(w, id, 0x20, r);
            let g = w.m(id).group;
            let Some(Some(list)) = usize::try_from(g).ok().and_then(|g| w.svc.groups.lists.get(g)).cloned() else { return };
            let mut n = 0usize;
            for e in list {
                if n > 0xf { break; }
                let i = (e & 0x7fff) as usize;
                if w.table.mobys.get(i).is_some_and(|m| m.o_class == CHILD_CLASS) {
                    c::set_pi32(w, id, RECORDS + 0x20 + n * triggers::CHILD_RECORD, i as i32);
                    n += 1;
                }
            }
            c::set_pi32(w, id, 0x348, 0);
            let t = w.ticks(0x1e);
            c::set_pi32(w, id, 0x338, n as i32);
            c::set_pi32(w, id, 0x34c, t);
            triggers::record_children(w, id, RECORDS, n);
            w.mm(id).state = 1;
        }
        2 => {
            let (tgt, k) = if c::pf(w, id, 0x330) < 1.5 { (1.5, NEAR) } else { (2.0, FAR) };
            let (mut t, mut v) = (c::pf(w, id, 0x330), c::pf(w, id, 0x334));
            turn::spring(tgt, k.0 * DT2, k.1 * DT2, k.2 * DT, &mut t, &mut v);
            c::set_pf(w, id, 0x330, t);
            c::set_pf(w, id, 0x334, v);
            let t = c::pf(w, id, 0x330);
            if RATE_AT < t && c::pf(w, id, 0x344) == 0.0 { c::set_pf(w, id, 0x344, RATE0); }
            if FALL_AT < t && c::pi32(w, id, 8) != 0 { w.mm(id).state = 4; }
            let ground = w.hero.ground_moby;
            let mut on = false;
            for k in 0..count(w, id) {
                let i = c::pi32(w, id, RECORDS + 0x20 + k * triggers::CHILD_RECORD);
                if ground.is_some_and(|g| g as i32 == i) {
                    on = true;
                    c::set_pi32(w, id, 0x348, 1);
                    break;
                }
            }
            if ground.and_then(|g| w.table.mobys.get(g)).is_some_and(|m| m.o_class == ATTACHMENT_CLASS) { on = true; }
            if c::pi32(w, id, 0x348) != 0 && !on && w.hero.air_ticks == 0 && c::dec_timer_pvar_i32(w, id, 0x34c) != 0 {
                w.mm(id).state = 4;
            }
        }
        4 => {
            let v = c::pf(w, id, 0x340);
            w.mm(id).position[2] += v;
            c::set_pf(w, id, 0x340, v - 5.0 * DT2);
            if w.m(id).position[2] < 20.0 {
                for k in 0..count(w, id) {
                    let i = c::pi32(w, id, RECORDS + 0x20 + k * triggers::CHILD_RECORD);
                    if let Some(ch) = usize::try_from(i).ok().filter(|&i| i < w.table.mobys.len()) { delete_child(w, ch); }
                }
                w.delete_moby(id);
                return;
            }
        }
        _ => {}
    }
    let st = w.m(id).state;
    if st == 1 || st == 2 {
        let t = c::pf(w, id, 0x330) * 0.5;
        if let Some(cub) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, c::pi32(w, id, 4)) {
            let (centre, euler) = (cub.centre(), cub.euler);
            let (a, b) = (c::pv4(w, id, 0x10), c::pv4(w, id, 0x20));
            let m = w.mm(id);
            for k in 0..3 {
                m.position[k] = a[k] + (centre[k] - a[k]) * t;
                m.rotation[k] = b[k] + (euler[k] - b[k]) * t;
            }
        }
    }
    let n = count(w, id);
    triggers::place_children(w, id, RECORDS, n);
    let rate = c::pf(w, id, 0x344);
    if rate != 0.0 && w.m(id).state != 4 {
        let rate = rate + 0.25;
        c::set_pf(w, id, 0x344, rate);
        if w.rng.randi(rate as i32) == 0 {
            let a = add_rot(w.rng.randf(f32::from_bits(0xbf86_0a92), f32::from_bits(0x3f86_0a92)), w.camera_yaw);
            let r = w.rng.randf(0.0, 20.0);
            let h = w.hero.pos;
            let p = [a.cos() * r + h[0].to_f32(), a.sin() * r + h[1].to_f32(), 15.0 + h[2].to_f32(), 0.0];
            let life = w.ticks(0xf0);
            gunship::spawn_ember(w, DEBRIS_SIZE, 2.0, 1.0, 0.75, p, [0.0; 4], DEBRIS_CLASS, life, 0);
        }
    }
}

/// Level18 `0x2fa8e0`: a Veldin carrier child is deleted with its attachment (+0x84, the port's `id + 1`).
fn delete_child(w: &mut World, ch: MobyId) {
    if w.m(ch).o_class != CHILD_CLASS { return; }
    let a = if w.m(ch).pvars.len() >= 0x88 { c::pi32(w, ch, 0x84) } else { 0 };
    if a > 0 && ((a - 1) as usize) < w.table.mobys.len() { w.delete_moby((a - 1) as usize); }
    w.delete_moby(ch);
}

/// Level18 `0x2f1c38` (the boss's call): a 1381 in state 1 goes to 2 and its group's carriers get +0xbc = 1
/// (`0x2fa888`). The grind-path cuts `0x2f1d08` (with +0x0c ≠ −1) are not ported (G-HERO-039).
pub fn start(w: &mut World, id: MobyId) {
    if w.m(id).o_class != 1381 || w.m(id).state != 1 { return; }
    if w.m(id).pvars.len() >= 0x10 && c::pi32(w, id, 0x0c) != -1 {
        w.svc.unported("1381: the grind-path cuts 0x2f1d08 (G-HERO-039)");
    }
    w.mm(id).state = 2;
    let g = w.m(id).group;
    group_command(w, g, 1);
}

/// Level18 `0x2fa888(group, v)`: every member of class 0x630 gets +0xbc = v.
pub fn group_command(w: &mut World, group: i8, v: u8) {
    for m in crate::moby_update::scheduler::group_ids(w, group) {
        if w.m(m).o_class == CHILD_CLASS { w.mm(m).cmd = v; }
    }
}

/// Level18 `0x2f1cc8` (the boss's call): back to state 1 (t 0, no debris), the children placed.
pub fn reset(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < SIZE { return; }
    if w.m(id).o_class == 1381 {
        w.mm(id).state = 1;
        c::set_pf(w, id, 0x330, 0.0);
        c::set_pf(w, id, 0x344, 0.0);
    }
    let n = count(w, id);
    triggers::place_children(w, id, RECORDS, n);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};
    use crate::moby_update::scheduler::Groups;
    use crate::moby_update::services::{pvar as p, Services};

    fn scene() -> (MobyTable, Services) {
        let mut plat = Moby { o_class: 1381, group: 0, pvars: vec![0; SIZE], position: [10.0, 0.0, 50.0, 1.0], ..Moby::default() };
        p::set_i32(&mut plat.pvars, 4, 0);
        let kid = |x: f32| Moby { o_class: CHILD_CLASS, pvars: vec![0; 0x88], position: [x, 0.0, 51.0, 1.0], ..Moby::default() };
        let other = Moby { o_class: 5, position: [0.0; 4], ..Moby::default() };
        let t = MobyTable::new(vec![plat, kid(12.0), other, kid(8.0)], 8);
        let mut svc = Services::new();
        svc.groups = Groups { lists: vec![Some(vec![0, 1, 2, 0x8003])] };
        let mut cub = rc_formats::volumes::Shape::default();
        cub.matrix[3] = [10.0, 10.0, 50.0, 1.0];
        cub.euler = [0.0, 0.0, std::f32::consts::PI];
        svc.volumes = std::sync::Arc::new(rc_formats::volumes::Volumes { cuboids: vec![cub], ..Default::default() });
        (t, svc)
    }

    fn tick(t: &mut MobyTable, svc: &mut Services, hero: &crate::hero::Hero, n: u32) {
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        for k in 0..n {
            let mut w = World::new(t, hero, &mut rng, &classes, svc, k as u64);
            update(&mut w, 0);
        }
    }

    #[test]
    fn records_its_carriers_and_swings_them_with_it() {
        let (mut t, mut svc) = scene();
        let hero = crate::hero::Hero::new();
        tick(&mut t, &mut svc, &hero, 1);
        let m = &t.mobys[0];
        assert_eq!((m.state, p::i32(&m.pvars, 0x338)), (1, 2));
        assert_eq!((p::i32(&m.pvars, 0x50), p::i32(&m.pvars, 0x80)), (1, 3));
        assert_eq!(p::v4f(&m.pvars, 0x30)[..3], [2.0, 0.0, 1.0]);
        // Told to swing: t springs toward 1.5; the platform goes toward the cuboid, the children with it.
        t.mobys[0].state = 2;
        tick(&mut t, &mut svc, &hero, 30);
        let m = &t.mobys[0];
        let tt = p::ff(&m.pvars, 0x330);
        assert!(tt > 0.0);
        assert!((m.position[1] - 10.0 * tt * 0.5).abs() < 1e-4, "{:?} t {tt}", m.position);
        let yaw = std::f32::consts::PI * tt * 0.5;
        let kid = &t.mobys[1];
        assert!((kid.position[0] - (m.position[0] + 2.0 * yaw.cos())).abs() < 1e-4 && (kid.position[1] - (m.position[1] + 2.0 * yaw.sin())).abs() < 1e-4, "{:?}", kid.position);
        assert!((kid.rotation[2] - yaw).abs() < 1e-4);
        // Past t 0.1525 the debris rate starts at 10 (+0.25 a tick).
        assert!(p::ff(&m.pvars, 0x344) >= 10.0);
    }

    #[test]
    fn falls_after_ratchet_left_it_and_deletes_its_carriers() {
        let (mut t, mut svc) = scene();
        let mut hero = crate::hero::Hero::new();
        tick(&mut t, &mut svc, &hero, 1);
        t.mobys[0].state = 2;
        p::set_i32(&mut t.mobys[1].pvars, 0x84, 3); // child 1's attachment: moby 2
        hero.ground_moby = Some(1);
        tick(&mut t, &mut svc, &hero, 1);
        assert_eq!(p::i32(&t.mobys[0].pvars, 0x348), 1);
        hero.ground_moby = None;
        tick(&mut t, &mut svc, &hero, 29);
        assert_eq!(t.mobys[0].state, 2, "30 ticks off it");
        tick(&mut t, &mut svc, &hero, 1);
        assert_eq!(t.mobys[0].state, 4);
        // Falls from z ≈ 50 with 5·dt² a tick: well below 20 within 300 ticks; then everything is gone.
        tick(&mut t, &mut svc, &hero, 300);
        assert!(t.mobys[0].is_deleted() && t.mobys[1].is_deleted() && t.mobys[2].is_deleted() && t.mobys[3].is_deleted());
    }
}
