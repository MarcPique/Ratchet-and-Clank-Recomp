//! **The Drone Device's drones** (class 479 = 0x1df; one update on every level: level01 `0x2e92b8`), their launch
//! `0x2e8c20` (from `UpdateWrenchSelected` 0x2307e0 when 0x141345 is set), their creation `0x2e9150`, the target search
//! `0x2e8e88` and the trails `0x2e8dd8`. docs/plan/hero_gameplay.md §13.
//!
//! **The launch.** The Drone Device (item 24, hand class 483 with no update) is not a throw: picking it in the quick
//! select (`0x24d238`), buying it (`0x2af7e8`) or selecting it on the Gadgets page sets 0x141345; the next
//! `UpdateWrenchSelected` calls [`launch`]: 0x141345 cleared; with ammo and fewer than 6 drones, one ammo is used (the
//! statistics 0x141740) and the missing drones are made one above and ahead of Ratchet (his feet + his moby's rows 2
//! and 0), each thrown out at `(cos yaw, sin yaw, 1)·0.25` a tick. The drones live in six slots (0x141370) and count
//! themselves (0x141346).
//!
//! **Each drone** (+0x20):
//! * **2 orbiting** Ratchet: its slot angle (+0x40, the drones spread evenly) plus the orbit phase (0x141348, +0.05 a
//!   round), eased at 5 % a tick (+0x28), the radius → 1 (+0x20), a tilt (+0x24 → +0x34 at 1 %), the circle turned by
//!   the tilt around Ratchet's feet one up (his moby's row 2), the height eased at 5 %; moved at most 10 u/s by the
//!   shared `walker::move_collide` (`0x26d610`: radius 0.18); stuck while not drawn, it jumps behind the camera. Its
//!   0.2 sphere touching an enemy (class type 5 / 7 / 8) gives it a hit (damage 1, flags 0x10000, type 1 / 2) — a
//!   creature (type 5) also blows the drone up. It faces its motion (`SpringTurn` 2π·dt² / π·dt² / 2π·dt).
//! * **3 attacking** its target: collision on, it homes on the target's aim point led by 3.1 times the aim's motion,
//!   at 20 % of the gap a tick; its 0.25 sphere on the target: the hit, and on a creature `SpawnBeamExplosion` (flashes
//!   0.4 / 0.2 beyond 4, light 8, a spark pair, a puff, sound 1, the shake) and state 4; the target gone: back to 2.
//! * **4 fading** (alpha −4 a tick, following its target if any) → **5** deleted (its slot freed).
//! * Hit (mask 1) by an enemy (class type 5..8): state 4.
//!
//! **The target search** (once a round: the first drone of each tick rebuilds the list of taken targets 0x14134c):
//! the moby list's enemies (class type 5, 7 or 8; a creature must be targetable), drawn last frame (+0x31; classes
//! 0x350 / 0x31 always), not taken, within 4 (type 5), 5.3 (7) or 6 (8) of Ratchet's feet, not in the Suck Cannon, with
//! a clear line (flags 6) from his feet to their aim point; the nearest (3-D) goes to the nearest free drone (state 3,
//! class sound 0 / 2 / 3 at random).
//!
//! **Pvars** (0x60): +0x00 velocity, +0x10 the target's last aim point, +0x20 orbit radius, +0x24 tilt, +0x28 orbit
//! angle, +0x30 target (+1), +0x34 tilt goal, +0x3c the eased height, +0x40 slot angle, +0x44 the drone made before it,
//! +0x48 the target's class (−1 none), +0x4c / +0x50 the two type-55 trails, +0x54 s16 slot, +0x56 s16 100, +0x58 /
//! +0x5c the spring-turn speeds.
//!
//! Native `f32`; the rand draws are the game's (the spheres' centre is the drone's position: the disassembly's `lq`).
//! **Inferred [L]**: the moby list the search walks (0x15ffe4) is the table in index order; class 0x4d6's own rules
//! (its radius, a hit budget +0x56) are not ported (no ported level has one: G-WPN-002); the type-55 trails are
//! `particles::type19::spawn55` (the ribbons follow joint lists 0 / 1); the blob shadow `0x26eec8(0.17)` is `crate::shadows::blob`.

use crate::moby_runtime::{mode, MobyId, MobyTable};
use crate::moby_update::creature::{self as c, fx, turn, walker, V};
use crate::moby_update::services::{pvar as p, HitTemplate, World};
use crate::ps2v::Pf;
use std::f32::consts::{PI, TAU};

pub const UPDATE_FN: u32 = 0x2e92b8;
pub const CLASS: i16 = 0x1df;
pub const CLASSES: [i16; 1] = [CLASS];
/// The Drone Device (item 24).
pub const DRONE_DEVICE: i32 = 24;

/// Pvar offsets.
pub mod pv {
    pub const VEL: usize = 0x00;
    pub const AIM: usize = 0x10;
    pub const RADIUS: usize = 0x20;
    pub const TILT: usize = 0x24;
    pub const ANGLE: usize = 0x28;
    pub const TARGET: usize = 0x30;
    pub const TILT_GOAL: usize = 0x34;
    pub const HEIGHT: usize = 0x3c;
    pub const SLOT_ANGLE: usize = 0x40;
    pub const PREV: usize = 0x44;
    pub const TARGET_CLASS: usize = 0x48;
    pub const TRAIL0: usize = 0x4c;
    pub const TRAIL1: usize = 0x50;
    pub const SLOT: usize = 0x54;
    pub const B56: usize = 0x56;
    pub const TURN_Y: usize = 0x58;
    pub const TURN_P: usize = 0x5c;
    pub const SIZE: usize = 0x60;
}

/// The drones' globals (0x141344..0x141388).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Globals {
    /// 0x141344: the drone of this round (1.. = count; the first of each round rebuilds the list).
    pub round: u8,
    /// 0x141346: drones alive.
    pub count: u8,
    /// 0x141347: drones not dying (state below 4) at the last round.
    pub live: u8,
    /// 0x141348: the orbit phase.
    pub phase: f32,
    /// 0x14134c: the targets taken (9).
    pub taken: [Option<MobyId>; 9],
    /// 0x141370: the six drone slots.
    pub slots: [Option<MobyId>; 6],
}

const DT: f32 = c::DT;
const DT2: f32 = c::DT2;

fn pvq(v: V) -> [Pf; 4] { v.map(Pf::f) }

/// Ratchet's feet (0x13f3d0).
fn feet(w: &World) -> V { w.hero.pos.map(|x| x.to_f32()) }

/// `0x2e8c20`: the launch (module doc). `yaw` = Ratchet's (0x13f3e8). Returns whether an ammo was used (the caller's
/// `0x249450(0x18, 1)` and statistics: the hero's ammo lives in the hero block).
pub fn launch(w: &mut World, yaw: f32) -> bool {
    let (co, si) = (yaw.cos() * 0.25, yaw.sin() * 0.25);
    if 6 <= w.svc.drones.count { return false; }
    let Some(h) = w.hero_moby else { return false };
    let rows = w.m(h).rows;
    let f = feet(w);
    let at = [f[0] + rows[2][0] + rows[0][0], f[1] + rows[2][1] + rows[0][1], f[2] + rows[2][2] + rows[0][2], 0.0];
    let mut prev: Option<MobyId> = None;
    for _ in w.svc.drones.count..6 {
        let d = create(w, prev, at);
        if let Some(d) = d { c::set_pv4(w, d, pv::VEL, [co, si, 0.25, 0.0]); }
        prev = d;
    }
    true
}

/// `0x2e9150(prev, pos)`: a drone (module doc's pvars).
fn create(w: &mut World, prev: Option<MobyId>, at: V) -> Option<MobyId> {
    let d = w.create_moby(CLASS)?;
    let hero_light = w.hero_moby.map(|h| (w.m(h).light, w.m(h).ambient));
    let f = feet(w);
    {
        let m = w.mm(d);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 2;
        m.cmd = 0;
        m.position = [at[0], at[1], at[2], m.position[3]];
        if m.pvars.len() < pv::SIZE { m.pvars.resize(pv::SIZE, 0); }
        p::set_i32(&mut m.pvars, pv::PREV, prev.map_or(0, |x| x as i32 + 1));
        if let Some((l, a)) = hero_light {
            m.light = l;
            m.ambient = a;
        }
        m.has_collision = false;
    }
    w.svc.drones.count += 1;
    c::set_pf(w, d, pv::ANGLE, c::atan(at[0] - f[0], at[1] - f[1]));
    c::set_pf(w, d, pv::TILT, c::atan(c::dist2(f, at), at[2] - f[2]));
    c::set_pf(w, d, pv::RADIUS, c::dist2(at, f));
    let slot = w.svc.drones.slots.iter().position(Option::is_none);
    if let Some(s) = slot { w.svc.drones.slots[s] = Some(d); }
    c::set_pi16(w, d, pv::SLOT, slot.map_or(6, |s| s as i16));
    c::set_pi16(w, d, pv::B56, 100);
    c::set_pf(w, d, pv::HEIGHT, f[2]);
    Some(d)
}

/// The class type byte (class header +0x46) of a moby.
fn class_type(w: &World, m: MobyId) -> u8 { w.classes.info(w.m(m).o_class).map_or(0, |i| i.ty) }

/// The target (+0x30 holds the moby + 1), still the class it was (+0x48) and alive.
fn target(w: &World, id: MobyId) -> Option<MobyId> {
    let t = c::pi32(w, id, pv::TARGET);
    let m = (t > 0).then(|| (t - 1) as MobyId).filter(|&m| m < w.table.mobys.len())?;
    (w.m(m).o_class as i32 == c::pi32(w, id, pv::TARGET_CLASS) && !w.m(m).is_deleted()).then_some(m)
}

/// A target's aim point (its position plus its record's height).
fn aim_point(w: &World, m: MobyId) -> V {
    let q = w.m(m).position;
    [q[0], q[1], q[2] + crate::targeting::aim_height(w.m(m)).unwrap_or(0.0), q[3]]
}

/// `0x2e8e88(feet, taken)`: the target search (module doc).
fn search(w: &World) -> Option<MobyId> {
    let f = feet(w);
    let mut best: Option<(MobyId, f32)> = None;
    for (i, m) in w.table.mobys.iter().enumerate() {
        if m.state == crate::moby_runtime::state::END { break; }
        if m.is_deleted() { continue; }
        let ty = class_type(w, i);
        if !matches!(ty, 5 | 7 | 8) { continue; }
        if w.svc.drones.taken.contains(&Some(i)) { continue; }
        if ty == 5 && m.mode & mode::TARGETABLE == 0 { continue; }
        if m.visible == 0 && m.o_class != 0x350 && m.o_class != 0x31 { continue; }
        let d = c::dist3(f, m.position);
        let range = match ty { 5 => 4.0, 7 => 5.3, _ => 6.0 };
        if range < d { continue; }
        if best.is_some_and(|(_, b)| b <= d) { continue; }
        // `0x304100`: not while the Suck Cannon holds it (its suck record's state +0x68 ≥ 1).
        if crate::moby_update::creature::react::rec_state(w, i).is_some_and(|s| 1 <= s) { continue; }
        let mut aim = m.position;
        if let Some(h) = crate::targeting::aim_height(m) { aim[2] += h + 0.05; }
        if w.coll_line(pvq(f), pvq(aim), 6, Some(i)).is_some() { continue; }
        best = Some((i, d));
    }
    best.map(|b| b.0)
}

/// The round (the first drone of each tick): the taken targets rebuilt, a new target given to the nearest free drone.
fn round(w: &mut World, id: MobyId) {
    let g = &mut w.svc.drones;
    g.round += 1;
    if g.round <= g.count { return; }
    g.round = 1;
    g.live = 0;
    g.phase = c::add_rot(g.phase, 0.05);
    let mut n = 0;
    let slots = w.svc.drones.slots;
    let mut taken = [None; 9];
    for (k, s) in slots.iter().enumerate() {
        let Some(d) = *s else { continue };
        if w.m(d).o_class != CLASS || w.m(d).is_deleted() {
            w.svc.drones.slots[k] = None;
            continue;
        }
        let t = c::pi32(w, d, pv::TARGET);
        taken[n] = (t > 0).then(|| (t - 1) as MobyId);
        n += 1;
        if w.m(d).state < 4 { w.svc.drones.live += 1; }
    }
    w.svc.drones.taken = taken;
    let _ = id;
    let Some(t) = search(w) else { return };
    let aim = aim_point(w, t);
    let mut nearest: Option<(MobyId, f32)> = None;
    for d in w.svc.drones.slots.iter().flatten().copied() {
        let busy = c::pi32(w, d, pv::TARGET) > 0 && w.table.mobys.get((c::pi32(w, d, pv::TARGET) - 1) as usize).is_some_and(|m| !m.is_deleted());
        if busy { continue; }
        let dd = c::dist2(w.m(d).position, aim);
        if nearest.is_none_or(|(_, b)| dd < b) { nearest = Some((d, dd)); }
    }
    let Some((d, _)) = nearest else { return };
    c::set_pi32(w, d, pv::TARGET, t as i32 + 1);
    c::set_pi32(w, d, pv::TARGET_CLASS, w.m(t).o_class as i32);
    c::set_pv4(w, d, pv::AIM, aim);
    w.mm(d).state = 3;
    // The sound is the updating drone's (`fun_0022da68(…, 0, param_1)`), not the chosen one's.
    let r = w.rng.rand() % 3;
    w.play_sound((0 < r) as i32 + r, 0, id);
    if n < 9 { w.svc.drones.taken[n] = Some(t); }
}

/// The two trails (`0x2e8dd8`: `PartType55Spawn(0.05, 0.01, drone, list 0 / 1, 0x80808080, 0x10808080)` once each).
fn trails(w: &mut World, id: MobyId) {
    for (list, o) in [(0i16, pv::TRAIL0), (1, pv::TRAIL1)] {
        if c::pi32(w, id, o) == 0 {
            let at = w.joint_point(id, list as usize);
            let class = w.m(id).o_class;
            *w.svc.fx.part_spawns.entry(55).or_default() += 1;
            if let Some(sys) = w.particles.as_deref_mut() {
                if crate::particles::type19::spawn55(sys, 0.05, 0.01, id, list, class, 0x8080_8080, 0x1080_8080, [at[0], at[1], at[2]]).is_none() { w.svc.fx.part_failed += 1; }
            }
            c::set_pi32(w, id, o, 1);
        }
    }
}

/// The drone's hit on `m` (template `0x26e7d8`: damage 1, flags 0x10000, the drone's direction with z 1, type 1 / 2);
/// on a creature (type 5) the drone blows up (state 4).
fn strike(w: &mut World, id: MobyId, m: MobyId, ty: u8) {
    let v = c::set_len3(c::pv4(w, id, pv::VEL), 1.0);
    let tmpl = HitTemplate { dir: [Pf::f(v[0]), Pf::f(v[1]), Pf::ONE, Pf::b(0x45af_df66)], attacker: Some(id), flags: 0x1_0000, b18: 1, b19: 2, h1a: CLASS as u16, damage: Pf::ONE, w20: 1 };
    w.deliver_hit(m, &tmpl);
    if ty == 5 {
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 0.4, flash2: 0.2, flash_dist: 4.0, scale: 1.0, light: 8.0, streaks: 0, sparks: 1, puffs: 1, debris: 1, sound: 1, shake: true };
        let pos = c::pos(w, id);
        fx::beam_explosion(w, &b, Some(id), pos);
        w.mm(id).state = 4;
    }
}

/// `0x2e92b8`.
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { w.mm(id).pvars.resize(pv::SIZE, 0); }
    if matches!(w.svc.game_mode, 2 | 5 | 6) || w.hero.state == 0x32 {
        w.mm(id).mode |= 0x41;
        return;
    }
    w.mm(id).mode &= !0x41;
    if w.m(id).state < 5 {
        round(w, id);
        let g = &w.svc.drones;
        let (r, n) = (g.round as u32, g.count as u32);
        let div = n.max(1) as f32;
        c::set_pf(w, id, pv::SLOT_ANGLE, (r as f32 / div) * TAU - PI);
        let m1 = (n + 1).max(1);
        let tilt = (((r * ((n + 1) / 5)) % m1) as f32 / div * TAU - PI) * 0.25;
        c::set_pf(w, id, pv::TILT_GOAL, tilt);
        if let Some(h) = w.get_hit(id, 1, false) {
            let ty = h.attacker.map_or(0, |a| class_type(w, a));
            if (5..=8).contains(&ty) { w.mm(id).state = 4; }
        }
    }
    match w.m(id).state {
        3 => attack(w, id),
        2 => orbit(w, id),
        4 => fade(w, id),
        5 => {
            w.svc.drones.count = w.svc.drones.count.saturating_sub(1);
            let s = c::pi16(w, id, pv::SLOT);
            if let Some(slot) = w.svc.drones.slots.get_mut(s as usize) { *slot = None; }
            w.delete_moby(id);
        }
        _ => {}
    }
    // `FUN_0026eec8(0.17)`: the blob shadow.
    if w.m(id).state < 0x80 { crate::shadows::blob(w, f32::from_bits(0x3e2e_147b), id); }
}

/// Class scale eased at 5 % a tick.
fn ease_scale(w: &mut World, id: MobyId) {
    let full = w.class_scale(CLASS).to_f32();
    let m = w.mm(id);
    m.scale += (full - m.scale) * 0.05;
}

/// State 3.
fn attack(w: &mut World, id: MobyId) {
    let coll = w.classes.info(CLASS).is_some_and(|i| i.has_collision);
    w.mm(id).has_collision = coll;
    let start = c::pos(w, id);
    ease_scale(w, id);
    trails(w, id);
    let Some(t) = target(w, id) else {
        w.mm(id).state = 2;
        c::set_pi32(w, id, pv::TARGET_CLASS, -1);
        c::set_pi32(w, id, pv::TARGET, 0);
        face(w, id, start, false);
        return;
    };
    let aim = aim_point(w, t);
    let lead = c::scale(c::sub(aim, c::pv4(w, id, pv::AIM)), f32::from_bits(0x4046_6666));
    let goal = c::add(aim, lead);
    let vel = c::scale(c::sub(goal, start), 1.0 - 0.8);
    c::set_pv4(w, id, pv::VEL, vel);
    let pos = c::add(start, vel);
    c::set_pos(w, id, pos);
    c::set_pv4(w, id, pv::AIM, aim);
    if let Some(o) = w.coll_sphere(pvq(pos), Pf::f(0.25), 0, Some(id)) {
        let hit = o.moby.filter(|&m| !w.m(m).is_deleted() && m == t && w.m(m).o_class != 0);
        match hit {
            None => {
                if let Some(pc) = o.pushed_centre { c::set_pos(w, id, [pc[0], pc[1], pc[2], pos[3]]); }
            }
            Some(m) => {
                let ty = class_type(w, m);
                strike(w, id, m, ty);
            }
        }
    }
    face(w, id, start, false);
}

/// Faces the motion since `start`: at once (attacking) or by the spring turns (orbiting).
fn face(w: &mut World, id: MobyId, start: V, spring: bool) {
    let pos = c::pos(w, id);
    let yaw = c::atan(pos[0] - start[0], pos[1] - start[1]);
    let pitch = c::atan(c::dist2(start, pos), pos[2] - start[2]);
    if !spring {
        let m = w.mm(id);
        m.rotation[2] = yaw;
        m.rotation[1] = -pitch;
        return;
    }
    let mut vy = c::pf(w, id, pv::TURN_Y);
    let y = turn::spring_turn(w.m(id).rotation[2], yaw, DT2 * TAU, DT2 * PI, DT * TAU, &mut vy);
    c::set_pf(w, id, pv::TURN_Y, vy);
    let mut vp = c::pf(w, id, pv::TURN_P);
    let pt = turn::spring_turn(w.m(id).rotation[1], -pitch, DT2 * TAU, DT2 * PI, DT * TAU, &mut vp);
    c::set_pf(w, id, pv::TURN_P, vp);
    let m = w.mm(id);
    m.rotation[2] = y;
    m.rotation[1] = pt;
}

/// State 2.
fn orbit(w: &mut World, id: MobyId) {
    w.mm(id).has_collision = false;
    let start = c::pos(w, id);
    trails(w, id);
    ease_scale(w, id);
    let r = c::pf(w, id, pv::RADIUS);
    c::set_pf(w, id, pv::RADIUS, r + (1.0 - r) * 0.05);
    let goal_a = c::add_rot(c::pf(w, id, pv::SLOT_ANGLE), w.svc.drones.phase);
    let a = c::pf(w, id, pv::ANGLE);
    let a = c::add_rot(a, c::sub_rot(goal_a, a) * 0.05);
    c::set_pf(w, id, pv::ANGLE, a);
    let tl = c::pf(w, id, pv::TILT);
    let tl = c::add_rot(tl, c::sub_rot(c::pf(w, id, pv::TILT_GOAL), tl) * 0.01);
    c::set_pf(w, id, pv::TILT, tl);
    let r = c::pf(w, id, pv::RADIUS);
    // The circle turned by the tilt about y (Euler (0, tilt, 0)).
    let (x, y) = (a.cos() * r, a.sin() * r);
    let e = rc_formats::moby_light::rotation_rows([0.0, tl, 0.0]).map(|row| row.map(f32::from_bits));
    let off: V = [x * e[0][0] + y * e[1][0], x * e[0][1] + y * e[1][1], x * e[0][2] + y * e[1][2], 0.0];
    let f = feet(w);
    let up = w.hero_moby.map_or([0.0, 0.0, 1.0, 0.0], |h| w.m(h).rows[2]);
    let mut goal = [f[0] + up[0] + off[0], f[1] + up[1] + off[1], f[2] + up[2], 0.0];
    let hz = c::pf(w, id, pv::HEIGHT);
    let hz = hz + (goal[2] - hz) * 0.05;
    c::set_pf(w, id, pv::HEIGHT, hz);
    goal[2] = hz + off[2];
    let mut mv = c::clamp_len3(c::sub(goal, start), DT * 10.0);
    let res = walker::move_collide(w, id, 0.0, f32::from_bits(0x3e38_51ec), 0.0, &mut mv, 0);
    if res & 1 != 0 && w.m(id).visible == 0 {
        // Stuck out of view: behind the camera (its position + up − forward).
        if let Some(v) = w.view {
            // The view's columns: camera y (down) and z (forward) in world space (0x167470 up = −y, 0x167450 forward).
            let col = |k: usize| [0, 1, 2].map(|j| f32::from_bits(v.rows[j][k]));
            let (down, fwd) = (col(1), col(2));
            let cam = w.camera.map(|x| x.to_f32());
            let q = [cam[0] - down[0] - fwd[0], cam[1] - down[1] - fwd[1], cam[2] - down[2] - fwd[2], 0.0];
            c::set_pos(w, id, q);
        }
    }
    let pos = c::pos(w, id);
    if let Some(o) = w.coll_sphere(pvq(pos), Pf::f(0.2), 0, Some(id)) {
        if let Some(m) = o.moby.filter(|&m| !w.m(m).is_deleted()) {
            let ty = class_type(w, m);
            if w.m(m).o_class != 0 && matches!(ty, 5 | 7 | 8) { strike(w, id, m, ty); }
        }
    }
    let pos = c::pos(w, id);
    c::set_pv4(w, id, pv::VEL, c::sub(pos, start));
    face(w, id, start, true);
}

/// State 4: fading, following its target.
fn fade(w: &mut World, id: MobyId) {
    if let Some(t) = target(w, id) {
        let aim = aim_point(w, t);
        let pos = c::pos(w, id);
        let v = c::clamp_len3(c::scale(c::sub(aim, pos), 1.0 - 0.8), 0.9);
        c::set_pv4(w, id, pv::VEL, v);
        c::set_pos(w, id, c::add(pos, v));
    } else {
        c::set_pi32(w, id, pv::TARGET, 0);
    }
    let m = w.mm(id);
    m.alpha = m.alpha.wrapping_sub(4);
    if m.alpha < 6 { m.state = 5; }
}

/// The drones' slots and count on a table (tests).
pub fn alive(table: &MobyTable) -> Vec<(MobyId, u8)> {
    table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == CLASS && !m.is_deleted()).map(|(i, m)| (i, m.state)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moby_update::classes::bomb_water::tests::{pool, Bench};

    /// State 2 stuck (the move's wall bit: the floor 0.1 under it) while not drawn (+0x31 = 0): it jumps behind the
    /// camera, to its position plus its up row less its forward row (0x167240 + 0x167470 − 0x167450).
    #[test]
    fn stuck_out_of_view_jumps_behind_the_camera() {
        let mut b = Bench::new(pool(8.0, 4.0), &[CLASS], [30.0, 30.0, 8.0]);
        b.camera = [Pf::f(25.0), Pf::f(15.0), Pf::f(12.0), Pf::ONE];
        let view = crate::particles::BSphereView::from_camera([25.0, 15.0, 12.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775);
        let d = b.create(CLASS);
        b.table.mobys[d].position = [20.0, 20.0, 8.1, 1.0];
        b.table.mobys[d].state = 2;
        b.table.mobys[d].visible = 0;
        b.table.mobys[d].update_dist = 0xff;
        b.table.mobys[d].pvars.resize(pv::SIZE, 0);
        b.tick(Some(&view));
        let q = b.table.mobys[d].position;
        assert!((q[0] - 25.0).abs() < 1e-4 && (q[1] - 14.0).abs() < 1e-4 && (q[2] - 13.0).abs() < 1e-4, "behind the camera: {q:?}");
        // Drawn: it stays where the move left it.
        let mut b2 = Bench::new(pool(8.0, 4.0), &[CLASS], [30.0, 30.0, 8.0]);
        let d2 = b2.create(CLASS);
        b2.table.mobys[d2].position = [20.0, 20.0, 8.1, 1.0];
        b2.table.mobys[d2].state = 2;
        b2.table.mobys[d2].visible = 1;
        b2.table.mobys[d2].update_dist = 0xff;
        b2.table.mobys[d2].pvars.resize(pv::SIZE, 0);
        b2.tick(Some(&view));
        let q = b2.table.mobys[d2].position;
        assert!((q[0] - 20.0).abs() < 0.2 && (q[1] - 20.0).abs() < 0.2, "drawn: stays {q:?}");
    }
}
