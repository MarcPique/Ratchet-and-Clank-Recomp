//! **The ships' homing missile, class 295** (created by code only): its spawner level13 `0x2e6a58` and its update
//! `0x2e6c08`, the same code on level 17 (`0x2c9ce8`): one shared unit through [`LevelPorts`] code identity. Gemlik's
//! ship 69 fires it at its locked target ([`super::gemlik_ship`]). No level data. Read from the level13 decomp and,
//! for the trail's second random bound, its disassembly. Native `f32`.
//!
//! **The earlier copy, class 1034** (level11 `0x3094f8`, its spawner `0x309378`; the same code on levels 13 and 17):
//! Pokitaru's jet 1242 fires it ([`super::pokitaru_jet`]). It is this missile without the target's class (+0x3c), the
//! spawn's glow and the update's check of a hidden target; [`spawn_early`] and [`update`] (by the class) carry the
//! differences.
//!
//! **Pvar block** (0x40): +0x00 the velocity, +0x10 the target's position when last seen, +0x20 the owner (index + 1),
//! +0x24 the target (index + 1), +0x28 its life, +0x2c the speed, +0x30 / +0x34 the yaw / pitch turn rates, +0x3c
//! the target's class.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x2e6a58 | `CreateMoby(0x127)` (none: nothing); update / draw distances 0xff, drawn, state 0; +0x20 owner, +0x24 target; position; rotation = the owner's | [`spawn`] |
//! | 0x2e6a58 | a target: +0x3c = its class; its target record (`0x268380` = L01 `0x2711f8`) +0x1e \|= 0x80 | [`spawn`] (`targeting::mark_missile`) |
//! | 0x2e6a58 | +0x28 = life, +0x2c = speed, +0x30 = +0x34 = 0; velocity = polar(speed, rot.z, −rot.y) (`0x26e8d8` = L01 `0x277b50`) | [`spawn`] |
//! | 0x2e6a58 | sequence (+0x53) ≠ 1 → `MobyAnimBlend(m, 1, 0, 10)` | [`spawn`] (`World::anim_blend`) |
//! | 0x2e6a58 | position += velocity; a target: +0x10 = its position; scale = class scale · (owner scale / owner class scale); `MobyBuildMatrix`; `PartType26Spawn(500000, m, 0x60808080, life, 0)` (the glow on joint list 0, 0.4 toward the camera) | [`spawn`] |
//! | 0x2e6c08 | old = position; scale += (class scale − scale) · 0.1 | [`update`] |
//! | 0x2e6c08 | the smoke: v = normalise(randf³)·randf(0.1, 0.2)·dt − normalise(velocity)·randf(0.1·dt, dt); `PartType44Spawn(40000, 1000, 1, −0.0004, 0, position, v, ticks(6), 0x7f, 0xb0b0b0, 3)`, blend 0x48 | [`trail`] |
//! | 0x2e6c08 | for t = 0, ⅓, ⅔: s = randf(t, t + ⅓); p = position + normalise(velocity)·s·speed; `PartType44Spawn(40000, 1000, 1, −0.0002, 0, p, v, ticks(60), 0x7f, 0x606060, 3)`, blend 0x48, def = the level's def 23 (`*0x1b23dc`); the spark `PartType21Spawn(20000, p, rot(rows · (0, 0, 0.02), rand_angle, row 0), 0x4f007fff, 0x1fffffff, ticks(5), 1)`, blend 0x48 | [`trail`] (`fx::part21`) |
//! | 0x2e6c08 | the target: targetable (0x1000) but mode & 1 with no collision (+0x94 = 0) → dropped | [`update`] |
//! | 0x2e6c08 | the target alive, still of its class (+0x3c), and within 3 of where it was seen → rot.z / rot.y point straight at it, +0x10 = its position | [`update`] |
//! | 0x2e6c08 | else the turn rates brake (`0x270cc0(angle, 80π·dt², 80π·dt², 800π·dt, &angle, &rate)` on rot.z / rot.y) and the target is dropped | [`update`] (`turn::turn_toward`) |
//! | 0x2e6c08 | velocity = polar(speed, rot.z, −rot.y); position += velocity | [`update`] |
//! | 0x2e6c08 | outside [2, 1021]³ → `DeleteMoby` | [`update`] |
//! | 0x309378 (1034) | `CreateMoby(0x40a)`; as 0x2e6a58 without +0x3c and the glow | [`spawn_early`] |
//! | 0x3094f8 (1034) | as 0x2e6c08 without the hidden-target drop; the blend bytes: the smoke and the spark left as spawned, the trail's 0x44 (def `*0x1b2adc`, def 23); the explosion's class sound −1 (none); the homing test: a target, alive, within 3 of where it was seen (no class test) | [`update`] |
//! | 0x2e6c08 | `CollLine_Fix(old, joint list 1's point, 0, owner, 0)`: no hit → `FastDecTimer(+0x28)` out → `DeleteMoby`; a hit → `SpawnBeamExplosion(2, 10, 4, 2, 9, 1, 20, m, velocity, hit point, 10, 3, 16, 0, 0)` (the debris / throttle / colour words not passed [L: 0, −1, 0]), `DeleteMoby` | [`update`] (`fx::beam_explosion`) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::turn::turn_toward;
use crate::moby_update::creature::{self as c, add, atan, dist2, dist3, set_len3, DT, DT2};
use crate::moby_update::services::{pv, World};

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2e_6c08;
pub const SPAWN_FN: u32 = 0x2e_6a58;
pub const CLASS: i16 = 0x127;
pub const CLASSES: [i16; 1] = [CLASS];
/// The earlier copy (module doc).
pub const EARLY_LEVEL: u32 = 11;
pub const EARLY_UPDATE_FN: u32 = 0x30_94f8;
pub const EARLY_SPAWN_FN: u32 = 0x30_9378;
pub const EARLY_CLASS: i16 = 0x40a;
pub const EARLY_CLASSES: [i16; 1] = [EARLY_CLASS];

pub mod pvo {
    pub const VEL: usize = 0x00;
    pub const SEEN: usize = 0x10;
    pub const OWNER: usize = 0x20;
    pub const TARGET: usize = 0x24;
    pub const LIFE: usize = 0x28;
    pub const SPEED: usize = 0x2c;
    pub const YAW_V: usize = 0x30;
    pub const PITCH_V: usize = 0x34;
    pub const TARGET_CLASS: usize = 0x3c;
    pub const LEN: usize = 0x40;
}

/// The trail's particle def (`*0x1b23dc` on level 13: the level's def 23).
const TRAIL_DEF: u8 = 23;
/// The explosion (module doc).
const BLAST: fx::Beam = fx::Beam { damage_r: 2.0, damage: 10.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 20.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: 0, shake: false };

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> {
    let v = c::pi32(w, id, o);
    usize::try_from(v - 1).ok().filter(|&m| m < w.table.mobys.len())
}

fn set_link(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)); }

/// Level13 `0x2e6a58(speed, owner, pos, target, rotation, life)` (module doc).
pub fn spawn(w: &mut World, speed: f32, owner: MobyId, pos: [f32; 4], target: Option<MobyId>, rot: [f32; 4], life: i32) -> Option<MobyId> {
    make(w, CLASS, speed, owner, pos, target, rot, life)
}

/// Level11 `0x309378(speed, owner, pos, target, rotation, life)`: the class 1034 copy (module doc).
pub fn spawn_early(w: &mut World, speed: f32, owner: MobyId, pos: [f32; 4], target: Option<MobyId>, rot: [f32; 4], life: i32) -> Option<MobyId> {
    make(w, EARLY_CLASS, speed, owner, pos, target, rot, life)
}

#[allow(clippy::too_many_arguments)]
fn make(w: &mut World, class: i16, speed: f32, owner: MobyId, pos: [f32; 4], target: Option<MobyId>, rot: [f32; 4], life: i32) -> Option<MobyId> {
    let late = class == CLASS;
    let id = w.create_moby(class)?;
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.position = pos;
        m.rotation = rot;
    }
    set_link(w, id, pvo::OWNER, Some(owner));
    set_link(w, id, pvo::TARGET, target);
    if let Some(t) = target {
        if late {
            let k = w.m(t).o_class as i32;
            c::set_pi32(w, id, pvo::TARGET_CLASS, k);
        }
        crate::targeting::mark_missile(w.mm(t));
    }
    c::set_pi32(w, id, pvo::LIFE, life);
    c::set_pf(w, id, pvo::SPEED, speed);
    c::set_pf(w, id, pvo::YAW_V, 0.0);
    c::set_pf(w, id, pvo::PITCH_V, 0.0);
    let v = polar(speed, rot[2], -rot[1]);
    c::set_pv4(w, id, pvo::VEL, v);
    if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 10); }
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    if let Some(t) = link(w, id, pvo::TARGET) {
        let tp = w.m(t).position;
        c::set_pv4(w, id, pvo::SEEN, tp);
    }
    let s = super::ship_laser::owner_scale(w, class, owner);
    w.mm(id).scale = s;
    w.build_matrix(id);
    if late { glow_on_joint0(w, 500_000.0, id, 0x6080_8080, life); }
    Some(id)
}

/// A type-44 smoke puff with its blend byte (and def) patched (`blend` None: left as spawned).
fn puff(w: &mut World, a: &crate::particles::type44::Spawn, blend: Option<u8>, def: Option<u8>) {
    if let Some(sys) = w.particles.as_deref_mut() {
        *w.svc.fx.part_spawns.entry(44).or_default() += 1;
        if let Some(i) = crate::particles::type44::spawn_rng(sys, w.rng, a) {
            let d = def.map(|n| sys.def_first(n));
            let r = &mut sys.pool.recs[i];
            if let Some(b) = blend { r[3] = b; }
            if let Some(d) = d { r[2] = d; }
        } else {
            w.svc.fx.part_failed += 1;
        }
    } else {
        fx::part44(w, a);
    }
}

/// `PartType21Spawn(20000, p, v, 0x4f007fff, 0x1fffffff, life, 1)` with the record's blend byte set to 0x48 (`patch`;
/// the class 1034 copy leaves it).
fn spark(w: &mut World, p: [f32; 4], v: [f32; 4], life: i32, patch: bool) {
    if life == 0 || w.particles.is_none() || !patch {
        fx::part21(w, 20000.0, p, v, 0x4f00_7fff, 0x1fff_ffff, life, 1);
        return;
    }
    *w.svc.fx.part_spawns.entry(21).or_default() += 1;
    let Some(sys) = w.particles.as_deref_mut() else { return };
    match crate::particles::type21::spawn_rng(sys, w.rng, 20000.0, p, v, 0x4f00_7fff, 0x1fff_ffff, life, 1) {
        Some(i) => sys.pool.recs[i][3] = 0x48,
        None => w.svc.fx.part_failed += 1,
    }
}

/// `PartType26Spawn(size, m, rgba, life, 0)`: the glow on joint list 0's point, 0.4 toward the camera (0x167240).
fn glow_on_joint0(w: &mut World, size: f32, m: MobyId, rgba: u32, life: i32) {
    *w.svc.fx.part_spawns.entry(26).or_default() += 1;
    let j = w.joint_point(m, 0);
    let cam = w.camera_point();
    let d = [cam[0] - j[0], cam[1] - j[1], cam[2] - j[2]];
    let at = set_len3([d[0], d[1], d[2], 0.0], 0.4);
    let at = [j[0] + at[0], j[1] + at[1], j[2] + at[2]];
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type26::spawn(sys, w.rng, size, m, rgba, life, 0, at).is_none() { w.svc.fx.part_failed += 1; }
}

/// `v` through the rows (`fun_001f9cf8`).
fn by_rows(rows: &[[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] { std::array::from_fn(|k| rows[0][k] * v[0] + rows[1][k] * v[1] + rows[2][k] * v[2]) }

/// The smoke and sparks behind it (module doc; `late`: the blend bytes of class 295, else 1034's).
fn trail(w: &mut World, id: MobyId, late: bool) {
    let x = w.rng.randf(-1.0, 1.0);
    let y = w.rng.randf(-1.0, 1.0);
    let z = w.rng.randf(-1.0, 1.0);
    let f = w.rng.randf(0.1, 0.2);
    let e = set_len3([x, y, z, 0.0], f * DT);
    let g = w.rng.randf(DT * 0.1, DT);
    let vel = c::pv4(w, id, pvo::VEL);
    let b = set_len3(vel, -g);
    let e = add(e, b);
    let pos = w.m(id).position;
    let n6 = w.ticks(6);
    let a = crate::particles::type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos: [pos[0], pos[1], pos[2]], vel: [e[0], e[1], e[2]], life: n6, alpha: 0x7f, rgb: 0xb0_b0b0, spin: 3 };
    puff(w, &a, late.then_some(0x48), None);
    let speed = c::pf(w, id, pvo::SPEED);
    let mut t = 0.0f32;
    loop {
        let t1 = t + 0.333_333_34;
        let s = w.rng.randf(t, t1);
        let p = add(set_len3(vel, s * speed), pos);
        let n60 = w.ticks(0x3c);
        let a = crate::particles::type44::Spawn { size: 40000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb951_b717), w: 0.0, pos: [p[0], p[1], p[2]], vel: [e[0], e[1], e[2]], life: n60, alpha: 0x7f, rgb: 0x60_6060, spin: 3 };
        puff(w, &a, Some(if late { 0x48 } else { 0x44 }), Some(TRAIL_DEF));
        let rows = w.m(id).rows;
        let up = by_rows(&rows, [0.0, 0.0, f32::from_bits(0x3ca3_d70a)]);
        let ang = w.rng.rand_angle();
        let r0 = [rows[0][0], rows[0][1], rows[0][2]];
        let up = crate::moby_update::classes::blaster_shot::rotate(up, ang, r0);
        let n5 = w.ticks(5);
        spark(w, p, [up[0], up[1], up[2], 0.0], n5, late);
        t = t1;
        if 1.0 <= t1 { break; }
    }
}

/// Level13 `0x2e6c08` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { w.delete_moby(id); return; }
    let old = w.m(id).position;
    let class = w.m(id).o_class;
    let late = class == CLASS;
    let cs = crate::moby_update::services::fl(w.class_scale(class));
    {
        let m = w.mm(id);
        m.scale += (cs - m.scale) * 0.1;
    }
    trail(w, id, late);
    if let Some(t) = link(w, id, pvo::TARGET).filter(|_| late) {
        let m = w.m(t);
        if m.mode & mode::TARGETABLE != 0 && m.mode & 1 != 0 && !m.has_collision { set_link(w, id, pvo::TARGET, None); }
    }
    let seen = c::pv4(w, id, pvo::SEEN);
    let homing = link(w, id, pvo::TARGET).filter(|&t| {
        let m = w.m(t);
        (!late || m.o_class as i32 == c::pi32(w, id, pvo::TARGET_CLASS)) && m.state != 0xfe && m.state != 0xfd && dist3(seen, m.position) < 3.0
    });
    let pitch = if let Some(t) = homing {
        let tp = w.m(t).position;
        let p = w.m(id).position;
        let yaw = atan(tp[0] - p[0], tp[1] - p[1]);
        let pitch = -atan(dist2(p, tp), tp[2] - p[2]);
        {
            let m = w.mm(id);
            m.rotation[2] = yaw;
            m.rotation[1] = pitch;
        }
        c::set_pv4(w, id, pvo::SEEN, tp);
        pitch
    } else {
        let k = 251.327_41f32;
        let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, pvo::YAW_V));
        turn_toward(a, DT2 * k, DT2 * k, DT * 2_513.274_2, &mut a, &mut v);
        w.mm(id).rotation[2] = a;
        c::set_pf(w, id, pvo::YAW_V, v);
        let (mut a, mut v) = (w.m(id).rotation[1], c::pf(w, id, pvo::PITCH_V));
        turn_toward(a, DT2 * k, DT2 * k, DT * 2_513.274_2, &mut a, &mut v);
        w.mm(id).rotation[1] = a;
        c::set_pf(w, id, pvo::PITCH_V, v);
        set_link(w, id, pvo::TARGET, None);
        a
    };
    let speed = c::pf(w, id, pvo::SPEED);
    let v = polar(speed, w.m(id).rotation[2], -pitch);
    c::set_pv4(w, id, pvo::VEL, v);
    let p = add(w.m(id).position, v);
    w.mm(id).position = p;
    if !crate::moby_update::creature::projectile::in_world(p) {
        w.delete_moby(id);
        return;
    }
    let nose = w.joint_point(id, 1);
    let ign = link(w, id, pvo::OWNER);
    match w.coll_line(pv(old), pv(nose), 0, ign) {
        None => {
            if c::dec_timer_pvar_i32(w, id, pvo::LIFE) == 0 { return; }
        }
        Some(h) => {
            let at = [h.point[0], h.point[1], h.point[2], 0.0];
            let b = if late { BLAST } else { fx::Beam { sound: -1, ..BLAST } };
            fx::beam_explosion(w, &b, Some(id), at);
        }
    }
    w.delete_moby(id);
}
