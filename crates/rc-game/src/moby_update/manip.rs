//! Manipulators on class mobys (docs/plan/moby_animation.md §9): the owner-side half of the runtime joint-modifier
//! list (moby `+0x64`, `Moby::joint_mods`). Native `f32`.
//!
//! A manipulator is a record in its owner's pvars (the owner may be the moby itself or another one, e.g. the vendor
//! 11 turning its hologram 1143's joints). Its first 0x40 bytes are the node the list links (`rc_formats::moby_anim::
//! JointModifier`); the NPC look-at `0x2777d8` uses the 0x80-byte record ([`rec`]). The port keeps the record's bytes
//! at the game's offsets and mirrors the node into the target moby's list, keyed by owner and offset ([`key`]):
//!
//! | address | what it does | port |
//! |---|---|---|
//! | `AttachManipulator` 0x264370 | once (+0x01 clear): +0x00 = list, +0x01 = 1, quat.w and scale xyz = 1.0, +0x04 = the list's target joint (`pb[pb[0] + 4]`), linked in front of the target's +0x64 | [`attach`] |
//! | `DetachManipulator` 0x2643e8 | unlinks the node (a node not in the list: nothing), then `FastMemSet(node, 0, 0x40)` | [`detach`] |
//! | `FUN_00221e38(a, out, axis)` | the rotation quaternion about axis 0 / 1 / 2 by −a into `out` (the node's +0x10) | [`set_axis`] (`hero::idle::axis_quat`) |
//! | `FUN_0026ee30(out, e)` | Euler x ⊗ y ⊗ z of +0x40 into +0x10 | [`look`] (`hero::idle::euler_quat`) |
//! | `FUN_002777d8(k, d, moby, rec, list)` | the NPC look-at record (below) | [`look`] |
//!
//! `FUN_002777d8` (L01; one copy per overlay, e.g. L04 0x2551c8), coverage:
//!
//! | address | what it does | port |
//! |---|---|---|
//! | 0x27780c..0x277848 | +0x78 (the moby the record was linked into) ≠ `moby`: when attached, `DetachManipulator(old, rec)` if the old moby's state < 0x7f (else the flag only), +0x01 = 0; +0x78 = `moby` | [`look`] |
//! | 0x277850..0x2778fc | active = a target (+0x60 / +0x64 / +0x68) ≠ 0, the scale +0x70 ≠ 1, or \|angle\| ≥ 0.005 (+0x40 / +0x44 / +0x48, `fabs` 0x221128) | [`look`] |
//! | 0x277920..0x277984 | active: `0x270b58(target, k, d, 0, &angle, &vel, 0)` on x, y, z (spring mode 0: no sticky side) | [`look`] (`hero::physics::turn_spring`) |
//! | 0x277988..0x2779a0 | not attached: `AttachManipulator(moby, list, rec)` | [`attach`] |
//! | 0x2779a8..0x2779d4 | `FUN_0026ee30(+0x10, +0x40)`; +0x20 / +0x24 / +0x28 = +0x70; +0x60 = 0 (`0x221170`, 16 bytes); +0x70 = 1.0 | [`look`] |
//! | 0x277904..0x277918 | inactive and attached: `DetachManipulator(moby, rec)` | [`detach`] |
//!
//! Side effects: only the record and the target's list (no sounds, particles or other mobys).
//!
//! The big-head cheats (`crate::cheats`): [`big_head`] (`0x278720` and its level copies, 0x15edb7) and
//! [`big_head_scale`] (`0x251d70`, a look-at record's scale).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pvar as p, World};
use crate::ps2v::Pf;
use rc_formats::moby_anim::JointModifier;

/// Record offsets: the node (0x00..0x40), then the look-at record's fields.
pub mod rec {
    /// +0x00 the owner's joint list; +0x01 attached; +0x03 mode (0 composes).
    pub const LIST: usize = 0x00;
    pub const ATTACHED: usize = 0x01;
    pub const MODE: usize = 0x03;
    /// +0x04: the target joint (the game: its joint record `joint·0x40 + 0x70000000`; the port: the joint index, 0xff
    /// when the target class has no such list loaded).
    pub const JOINT: usize = 0x04;
    /// +0x0c weight (mode ≠ 0), +0x10 quaternion, +0x20 scale, +0x30 translation.
    pub const WEIGHT: usize = 0x0c;
    pub const QUAT: usize = 0x10;
    pub const SCALE: usize = 0x20;
    pub const TRANS: usize = 0x30;
    /// The node's size (`DetachManipulator` clears it).
    pub const NODE: usize = 0x40;
    /// `0x2777d8`'s record: +0x40 Euler angles, +0x50 their velocities, +0x60 this tick's targets, +0x70 the scale,
    /// +0x78 the moby it is linked into (the port: index + 1, 0 none).
    pub const ANGLES: usize = 0x40;
    pub const VEL: usize = 0x50;
    pub const TARGET: usize = 0x60;
    pub const REC_SCALE: usize = 0x70;
    pub const MOBY: usize = 0x78;
    pub const SIZE: usize = 0x80;
}

/// The key of the node at pvar offset `ofs` of `owner` in a list ([`crate::moby_runtime::Moby::joint_mod_keys`]).
pub fn key(owner: MobyId, ofs: usize) -> u32 { ((owner as u32) << 16) | (ofs as u32 & 0xffff) }

/// The target joint of `target`'s joint list `list` (the loader's `Services::joint_targets`).
pub fn target_joint(w: &World, target: MobyId, list: u8) -> Option<u8> {
    let oc = w.m(target).o_class;
    w.svc.joint_targets.get(&oc).and_then(|t| t.get(list as usize)).copied().filter(|&j| j != 0xff)
}

fn ensure(w: &mut World, owner: MobyId, ofs: usize, size: usize) {
    let pv = &mut w.mm(owner).pvars;
    if pv.len() < ofs + size { pv.resize(ofs + size, 0); }
}

/// The node at `ofs` of `owner`'s pvars as the evaluator reads it.
pub fn node(w: &World, owner: MobyId, ofs: usize) -> JointModifier {
    let pv = &w.m(owner).pvars;
    let v3 = |o: usize| [p::ff(pv, o), p::ff(pv, o + 4), p::ff(pv, o + 8)];
    JointModifier {
        joint: p::u8(pv, ofs + rec::JOINT),
        mode: p::u8(pv, ofs + rec::MODE),
        weight: p::ff(pv, ofs + rec::WEIGHT),
        quat: p::v4f(pv, ofs + rec::QUAT),
        scale: v3(ofs + rec::SCALE),
        trans: v3(ofs + rec::TRANS),
    }
}

/// Whether the node at `ofs` of `owner` is attached (+0x01).
pub fn attached(w: &World, owner: MobyId, ofs: usize) -> bool { w.m(owner).pvars.get(ofs + rec::ATTACHED).is_some_and(|&b| b != 0) }

/// `AttachManipulator(target, list, rec)` for the record at `ofs` of `owner` (module doc). A target list the loader has
/// no joint for leaves the node attached but out of the list (nothing to turn).
pub fn attach(w: &mut World, target: MobyId, list: u8, owner: MobyId, ofs: usize) {
    ensure(w, owner, ofs, rec::NODE);
    if attached(w, owner, ofs) { return; }
    let joint = target_joint(w, target, list);
    {
        let pv = &mut w.mm(owner).pvars;
        p::set_u8(pv, ofs + rec::LIST, list);
        p::set_u8(pv, ofs + rec::ATTACHED, 1);
        p::set_ff(pv, ofs + rec::QUAT + 12, 1.0);
        for k in 0..3 { p::set_ff(pv, ofs + rec::SCALE + 4 * k, 1.0); }
        p::set_u8(pv, ofs + rec::JOINT, joint.unwrap_or(0xff));
    }
    if joint.is_none() {
        w.svc.unported("manipulator: target joint list not loaded");
        return;
    }
    let n = node(w, owner, ofs);
    let k = key(owner, ofs);
    let m = w.mm(target);
    if m.joint_mod_keys.len() != m.joint_mods.len() { m.joint_mod_keys.resize(m.joint_mods.len(), u32::MAX); }
    m.joint_mods.insert(0, n);
    m.joint_mod_keys.insert(0, k);
}

/// `DetachManipulator(target, rec)`: unlink the node of `owner`'s record at `ofs` from `target`'s list, then clear the
/// node's 0x40 bytes.
pub fn detach(w: &mut World, target: MobyId, owner: MobyId, ofs: usize) {
    let k = key(owner, ofs);
    if let Some(m) = w.table.mobys.get_mut(target) {
        if let Some(i) = m.joint_mod_keys.iter().position(|&x| x == k) {
            m.joint_mod_keys.remove(i);
            if i < m.joint_mods.len() { m.joint_mods.remove(i); }
        }
    }
    ensure(w, owner, ofs, rec::NODE);
    w.mm(owner).pvars[ofs..ofs + rec::NODE].fill(0);
}

/// Copy the record's node into `target`'s list (after the owner rewrote it; the game's list points at the record).
pub fn sync(w: &mut World, target: MobyId, owner: MobyId, ofs: usize) {
    let k = key(owner, ofs);
    let Some(i) = w.m(target).joint_mod_keys.iter().position(|&x| x == k) else { return };
    let n = node(w, owner, ofs);
    if let Some(x) = w.mm(target).joint_mods.get_mut(i) { *x = n; }
}

/// `FUN_00221e38(a, rec + 0x10, axis)` on the record at `ofs` of `owner` (linked into `target`).
pub fn set_axis(w: &mut World, target: MobyId, owner: MobyId, ofs: usize, a: f32, axis: usize) {
    ensure(w, owner, ofs, rec::NODE);
    let q = crate::hero::idle::axis_quat(a, axis);
    p::set_v4f(&mut w.mm(owner).pvars, ofs + rec::QUAT, q);
    sync(w, target, owner, ofs);
}

/// The record's rotation +0x10 written whole, `q` (x, y, z, w): two `FUN_00221e38` axes combined by `fun_001fa3c0`
/// (the turret of level 18's tank 1454, `classes::units::veldin_tank`).
pub fn set_quat(w: &mut World, target: MobyId, owner: MobyId, ofs: usize, q: [f32; 4]) {
    ensure(w, owner, ofs, rec::NODE);
    p::set_v4f(&mut w.mm(owner).pvars, ofs + rec::QUAT, q);
    sync(w, target, owner, ofs);
}

/// `FUN_002777d8(k, d, moby, rec, list)`: the NPC look-at record at `ofs` of `owner`'s pvars, linked into `moby`'s list
/// `list` (module doc). The owner writes this tick's target angles (+0x60..) and scale (+0x70) before the call.
pub fn look(w: &mut World, moby: MobyId, owner: MobyId, ofs: usize, list: u8, k: f32, d: f32) {
    ensure(w, owner, ofs, rec::SIZE);
    let old = p::i32(&w.m(owner).pvars, ofs + rec::MOBY);
    if old != moby as i32 + 1 {
        if old != 0 {
            if attached(w, owner, ofs) {
                let o = (old - 1) as usize;
                if w.table.mobys.get(o).is_some_and(|m| m.state < 0x7f) { detach(w, o, owner, ofs); }
            }
            p::set_u8(&mut w.mm(owner).pvars, ofs + rec::ATTACHED, 0);
        }
        p::set_i32(&mut w.mm(owner).pvars, ofs + rec::MOBY, moby as i32 + 1);
    }
    let pv = &w.m(owner).pvars;
    let tgt = [0, 1, 2].map(|i| p::ff(pv, ofs + rec::TARGET + 4 * i));
    let scale = p::ff(pv, ofs + rec::REC_SCALE);
    let ang = [0, 1, 2].map(|i| p::ff(pv, ofs + rec::ANGLES + 4 * i));
    // 0x3ba3d70a = 0.005.
    let small = |a: f32| a.abs() < f32::from_bits(0x3ba3_d70a);
    let active = tgt.iter().any(|&t| t != 0.0) || scale != 1.0 || !ang.iter().all(|&a| small(a));
    if !active {
        if attached(w, owner, ofs) { detach(w, moby, owner, ofs); }
        return;
    }
    let vel = [0, 1, 2].map(|i| p::ff(pv, ofs + rec::VEL + 4 * i));
    let mut new = [[0.0f32; 2]; 3];
    for i in 0..3 {
        let (mut a, mut v) = (Pf::f(ang[i]), Pf::f(vel[i]));
        crate::hero::physics::turn_spring(Pf::f(tgt[i]), Pf::f(k), Pf::f(d), Pf::ZERO, &mut a, &mut v, 0);
        new[i] = [a.to_f32(), v.to_f32()];
    }
    {
        let pv = &mut w.mm(owner).pvars;
        for (i, [a, v]) in new.iter().enumerate() {
            p::set_ff(pv, ofs + rec::ANGLES + 4 * i, *a);
            p::set_ff(pv, ofs + rec::VEL + 4 * i, *v);
        }
    }
    if !attached(w, owner, ofs) { attach(w, moby, list, owner, ofs); }
    let pv = &mut w.mm(owner).pvars;
    let q = crate::hero::idle::euler_quat([new[0][0], new[1][0], new[2][0]]);
    p::set_v4f(pv, ofs + rec::QUAT, q);
    for i in 0..3 { p::set_ff(pv, ofs + rec::SCALE + 4 * i, scale); }
    pv[ofs + rec::TARGET..ofs + rec::TARGET + 16].fill(0);
    p::set_ff(pv, ofs + rec::REC_SCALE, 1.0);
    sync(w, moby, owner, ofs);
}

/// `FUN_00278720(scale, moby, list, rec)` (L01; one copy per overlay with the same code, e.g. L00 0x263ac8, L02
/// 0x264dd0, L03 0x251d00, L04 0x2563b8, L05 0x28dcd8, L07 0x28c2f0, L08 0x26dae0, L09 0x256da0, L11 0x288f40, L12
/// 0x27c220, L13 0x282a90, L15 0x2531c0, L16 0x25e9f0, L17 0x25d928, L18 0x266098 — the names the class ports cite): the
/// enemies' big-head cheat 0x15edb7 ("Enemies have massive domes", `crate::cheats`) on the node at `ofs` of `owner`'s
/// pvars, linked into `moby`'s list `list`. Cheat off: an attached node is detached (`DetachManipulator`). Cheat on: the
/// node attached when it is not (`AttachManipulator`), then its scale +0x20 / +0x24 / +0x28 = `scale`.
pub fn big_head(w: &mut World, scale: f32, moby: MobyId, list: u8, owner: MobyId, ofs: usize) {
    let on = w.svc.cheats.on(crate::cheats::slot::ENEMIES);
    head_node(w, on, scale, moby, list, owner, ofs);
}

/// The same node code on the actors' cheat 0x15edb0 (the mouse 0x1b1's head, `MouseCritterUpdate` 0x30df40 in states
/// ≥ 3: list 5, 2.1, its record at P+0x90).
pub fn actor_big_head(w: &mut World, scale: f32, moby: MobyId, list: u8, owner: MobyId, ofs: usize) {
    let on = w.svc.cheats.on(crate::cheats::slot::ACTORS);
    head_node(w, on, scale, moby, list, owner, ofs);
}

/// [`big_head`]'s body: off → detach an attached node; on → attach once, scale xyz = `scale`.
fn head_node(w: &mut World, on: bool, scale: f32, moby: MobyId, list: u8, owner: MobyId, ofs: usize) {
    if !on {
        if attached(w, owner, ofs) { detach(w, moby, owner, ofs); }
        return;
    }
    if !attached(w, owner, ofs) { attach(w, moby, list, owner, ofs); }
    let pv = &mut w.mm(owner).pvars;
    for k in 0..3 { p::set_ff(pv, ofs + rec::SCALE + 4 * k, scale); }
    sync(w, moby, owner, ofs);
}

/// The key of the scene head record 0x17c8c0's node in an actor's list ([`scene_big_head`]).
pub const SCENE_HEAD_KEY: u32 = 0xffff_fff0;

/// The scene actors' big head ("Actors have oversized craniums", the cheat 0x15edb0): L01 `FUN_002ff028` (the talking
/// NPC's: its own class or 0x32b), `FUN_002fac80` (the mission NPC's: 0x32b, 2.75), `FUN_002fb4b8` (the camera trigger's:
/// 0x398 / 0x1bf, 2.1), level11 `0x2d0ec0`, level18 `0x2f2b18`, … — one per class family, the same code. In game mode
/// 2 with the cheat, each scene actor (0x16ce58[0..0x16cd24]) whose class is in `classes`: when the record 0x17c8c0 is
/// not attached (+0x01), `AttachManipulator(actor, list, 0x17c8c0)` and its scale +0x20 / +0x24 / +0x28 = `scale` (so
/// only the first matching actor of a scene gets it; `DialogStreamStart` clears the record: `Services::scene_head`).
/// `list` is 0 but for level15's `0x2ec678` (Quartu's Giant Clank mission NPC: 1).
pub fn scene_big_head(w: &mut World, classes: &[i16], list: u8, scale: f32) {
    if w.svc.game_mode != 2 || !w.svc.cheats.on(crate::cheats::slot::ACTORS) || w.svc.scene_head.is_some() { return; }
    let actors: Vec<(Option<MobyId>, i16)> = w.svc.cinematic.scene.as_ref().map(|s| s.actors.iter().map(|a| (a.moby, a.o_class)).collect()).unwrap_or_default();
    for (moby, oc) in actors {
        let Some(id) = moby.filter(|_| classes.contains(&oc)) else { continue };
        if w.svc.scene_head.is_some() { break; }
        w.svc.scene_head = Some(id);
        let Some(joint) = target_joint(w, id, list) else {
            w.svc.unported("scene big head: the actor's joint list 0 not loaded");
            continue;
        };
        let node = JointModifier { joint, mode: 0, weight: 0.0, quat: [0.0, 0.0, 0.0, 1.0], scale: [scale; 3], trans: [0.0; 3] };
        let m = w.mm(id);
        if m.joint_mod_keys.len() != m.joint_mods.len() { m.joint_mod_keys.resize(m.joint_mods.len(), u32::MAX); }
        m.joint_mods.insert(0, node);
        m.joint_mod_keys.insert(0, SCENE_HEAD_KEY);
    }
}

/// `FUN_00251d70(scale, rec)` (L03; its copy per overlay, e.g. L16 0x25ea60): the big-head cheat 0x15edb7 on a look-at
/// record ([`look`]): its scale request +0x70 = `scale` with the cheat, else 1.0.
pub fn big_head_scale(w: &mut World, scale: f32, owner: MobyId, ofs: usize) {
    ensure(w, owner, ofs, rec::SIZE);
    let s = if w.svc.cheats.on(crate::cheats::slot::ENEMIES) { scale } else { 1.0 };
    p::set_ff(&mut w.mm(owner).pvars, ofs + rec::REC_SCALE, s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_runtime::{Moby, MobyTable};

    /// A table of `(o_class, pvar size)` mobys, run with `f` in a world whose `joint_targets` are `targets`.
    fn with(mobys: &[(i16, usize)], targets: &[(i16, Vec<u8>)], f: impl FnOnce(&mut World)) {
        let ms = mobys.iter().map(|&(o_class, n)| Moby { o_class, state: 1, pvars: vec![0; n], ..Moby::default() }).collect();
        let mut t = MobyTable::new(ms, 4);
        let hero = crate::hero::Hero::new();
        let mut rng = crate::rng::Rng::new();
        let classes = crate::moby_update::ClassTable::default();
        let mut svc = crate::moby_update::Services::new();
        for (c, v) in targets { svc.joint_targets.insert(*c, v.clone()); }
        let mut w = World::new(&mut t, &hero, &mut rng, &classes, &mut svc, 0);
        f(&mut w);
    }

    #[test]
    fn attach_links_in_front_and_detach_clears() {
        with(&[(1143, 0x10), (11, 0xa0)], &[(1143, vec![7, 9])], |w| {
            let (t, o) = (0, 1);
            attach(w, t, 0, o, 0x10);
            attach(w, t, 1, o, 0x50);
            // The second node heads the list; a second attach of a linked record does nothing.
            attach(w, t, 1, o, 0x50);
            assert_eq!(w.m(t).joint_mods.iter().map(|m| m.joint).collect::<Vec<_>>(), vec![9, 7]);
            assert_eq!(w.m(t).joint_mod_keys, vec![key(o, 0x50), key(o, 0x10)]);
            assert_eq!(w.m(t).joint_mods[0], JointModifier::compose(9));
            assert_eq!(p::u8(&w.m(o).pvars, 0x10 + rec::LIST), 0);
            assert_eq!(p::u8(&w.m(o).pvars, 0x50 + rec::LIST), 1);
            // FUN_00221e38 on the record reaches the list.
            set_axis(w, t, o, 0x50, 0.3, 2);
            assert_eq!(w.m(t).joint_mods[0].quat, crate::hero::idle::axis_quat(0.3, 2));
            detach(w, t, o, 0x50);
            assert_eq!(w.m(t).joint_mods.iter().map(|m| m.joint).collect::<Vec<_>>(), vec![7]);
            assert!(w.m(o).pvars[0x50..0x90].iter().all(|&b| b == 0));
            // Without a loaded target joint: attached, not linked.
            attach(w, o, 0, o, 0x60);
            assert!(attached(w, o, 0x60));
            assert!(w.m(o).joint_mods.is_empty());
        });
    }

    #[test]
    fn look_springs_attaches_and_detaches() {
        with(&[(774, 0x160)], &[(774, vec![3, 5])], |w| {
            let n = 0;
            // (+0x70 = 1 as the placed data has it; 0 would be a scale request.) Nothing set, angles settled: stays
            // detached.
            p::set_ff(&mut w.mm(n).pvars, 0x50 + rec::REC_SCALE, 1.0);
            look(w, n, n, 0x50, 0, 0.02, 0.3);
            assert!(!attached(w, n, 0x50) && w.m(n).joint_mods.is_empty());
            assert_eq!(p::i32(&w.m(n).pvars, 0x50 + rec::MOBY), n as i32 + 1);
            // A yaw target: one spring step (mode 0), attached, the Euler quaternion, targets cleared, scale 1.
            p::set_ff(&mut w.mm(n).pvars, 0x50 + rec::TARGET + 8, 1.0);
            look(w, n, n, 0x50, 0, 0.02, 0.3);
            let (mut a, mut v) = (Pf::ZERO, Pf::ZERO);
            crate::hero::physics::turn_spring(Pf::ONE, Pf::f(0.02), Pf::f(0.3), Pf::ZERO, &mut a, &mut v, 0);
            let pv = &w.m(n).pvars;
            assert_eq!(p::ff(pv, 0x50 + rec::ANGLES + 8), a.to_f32());
            assert_eq!(p::ff(pv, 0x50 + rec::VEL + 8), v.to_f32());
            assert_eq!(p::ff(pv, 0x50 + rec::TARGET + 8), 0.0);
            assert!(attached(w, n, 0x50));
            let m = w.m(n).joint_mods[0];
            assert_eq!((m.joint, m.quat, m.scale), (3, crate::hero::idle::euler_quat([0.0, 0.0, a.to_f32()]), [1.0; 3]));
            // A scale request (the big-head cheat writes +0x70) goes to the node's scale, then +0x70 is back to 1.
            p::set_ff(&mut w.mm(n).pvars, 0x50 + rec::REC_SCALE, 2.75);
            look(w, n, n, 0x50, 0, 0.02, 0.3);
            assert_eq!(w.m(n).joint_mods[0].scale, [2.75; 3]);
            assert_eq!(p::ff(&w.m(n).pvars, 0x50 + rec::REC_SCALE), 1.0);
            // Settled back under 0.005 with no target: detached (the node cleared, the angles kept).
            for _ in 0..4000 { look(w, n, n, 0x50, 0, 0.02, 0.3); }
            assert!(!attached(w, n, 0x50) && w.m(n).joint_mods.is_empty());
            assert!(p::ff(&w.m(n).pvars, 0x50 + rec::ANGLES + 8).abs() < 0.005);
        });
    }

    #[test]
    fn look_moves_to_another_moby() {
        with(&[(774, 0x160), (774, 0x160)], &[(774, vec![3])], |w| {
            let (a, c) = (0, 1);
            p::set_ff(&mut w.mm(a).pvars, 0x50 + rec::REC_SCALE, 1.0);
            p::set_ff(&mut w.mm(a).pvars, 0x50 + rec::TARGET + 8, 1.0);
            look(w, a, a, 0x50, 0, 0.02, 0.3);
            assert_eq!(w.m(a).joint_mods.len(), 1);
            // The same record now for moby c: unlinked from a (a live moby), linked into c.
            p::set_ff(&mut w.mm(a).pvars, 0x50 + rec::TARGET + 8, 1.0);
            look(w, c, a, 0x50, 0, 0.02, 0.3);
            assert!(w.m(a).joint_mods.is_empty());
            assert_eq!(w.m(c).joint_mod_keys, vec![key(a, 0x50)]);
        });
    }
}
