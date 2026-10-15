//! **Qwark's ship's pieces** (level 13, created by code only): what Qwark's ship 388 ([`super::qwark_ship`]) makes.
//! Read from the level13 decomp and disassembly (the second random bounds); native `f32`.
//!
//! * **The parts 389..400 and the hit shield 401** (spawner `0x2ecd10`, update `0x2ece00`): held at Qwark's joints
//!   while attached (state 1); the ones he loses tumble (state 3) and burst (state 4).
//! * **The shield 352** (spawner `0x2e8438`, update `0x2e84d8`): phase 4's bubble round Qwark, fading in and out.
//! * **The missile 82** (spawner `0x2c13b0`, update `0x2c1528`): homing, leading the player's ship; shootable.
//! * **The mine 83** (spawner `0x2c1f28`, update `0x2c2048`): drifts, drawn toward the ship within 60, bursts on
//!   contact; shootable (20 health).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x2ecd10 | `CreateMoby(class)`: update / draw 0xff, drawn, state 0; scale = class scale · (Qwark's scale / his class scale); +0x44 = the joint, at its point (`0x25b450` = L01 `0x2645a8`); +0x3c = Qwark; matrix; his rotation and rows; +0x40 = `ticks(600)`; matrix | [`spawn_part`] |
//! | 0x2ece00 | Qwark (+0x3c) gone or not 388 → `DeleteMoby`; mode bits 0..1 = his | [`part_update`] |
//! | 0x2ece00 | 0: → 1, +0x20..+0x2c = 0. 1: scale = class · his / his class; light word and ambient (+0x38..+0x3f) = his | [`part_update`] |
//! | 0x2ece00 | 3: position += velocity (+0x10), velocity ·= 0.995; `coll_sphere(2, pos, 0, m)`: the world or a moby not Qwark's own (0x191, 0x184..0x18c, 399, 400) → the velocity reflected (`0x20b148` = L01 `0x221570`), → 4; rotation += the spins (+0x30..), the spins ·= 0.999; out of [10, 1013] → deleted; `FastDecTimer(+0x40)` out and not drawn → deleted | [`part_update`] |
//! | 0x2ece00 | 4: `SpawnBeamExplosion(2, 4, 4, 2, 9, 1, 15, m, v, m, 10, 3, 16, −1, 1)`, `DeleteMoby` | [`part_update`] |
//! | 0x2e8438 | `CreateMoby(0x160)`: draw 0x3ff, update 0xff, drawn, state 0, +0xbc 0; at Qwark; +0x00 = Qwark, +0x04 = 0; mode \| 0x200; matrix; Ratchet's light word / ambient (`0x272078`); scale = class scale | [`spawn_shield`] |
//! | 0x2e84d8 | 0: collision on, → 1. 1: scale eases to 4 × class, +0x04 += 0.02·SPEED, at 1 → 2. 2: the same scale; Qwark gone → 3. 3: scale eases to 1 ×, +0x04 −= 0.02·SPEED, at 0 → 4. 4: `DeleteMoby`. Alpha = `trunc(+0x04·60)`; the class's texture scroll −0x40 a tick (wrapping 0..0x1000: `0x263d90`) | [`shield_update`] (the scroll: NOT drawn, the renderer has no moby texture scroll) |
//! | 0x2c13b0 | `CreateMoby(0x52)`: update / draw 0xff, drawn, state 0; +0x24 target, +0x20 owner; position, rotation; +0x2c speed, +0x38 top speed, +0x28 life, turn rates 0; velocity = the owner's pvars +0xc0..+0xcf (Qwark's: the part pointers, denormal floats: 0); sequence ≠ 1 → `MobyAnimBlend(1, 0, 10)`; position += velocity; +0x10 = the target's position; class scale; matrix; `PartType26Spawn(500000, m, 0x60808080, life, 0)` | [`spawn_missile`] |
//! | 0x2c1528 | not shot (`MobyGetHitMessage(0xa30000)` none, or by nothing, or by another 82): scale eases to the class's; the trail (95's, larger: types 44 80000 / 21 40000, without the spark's blend patch); the target gone or moved 3 → the turn rates brake (`0x270cc0`, 4π·dt², 8π·dt); else toward it (the player's ship led: `(d / (speed + 24))·its speed` along its heading, unless the owner is in state 5), within 60° the speed eases to the top speed (0.05), the turns (`0x270cc0`) | [`missile_update`] |
//! | 0x2c1528 | life > `ticks(120)`: velocity = polar(speed, yaw, −pitch); else blended in by (life / `ticks(120)`)²; position += it; out of [2, 1021] → deleted | [`missile_update`] |
//! | 0x2c1528 | `CollLine_Fix(old, joint 1, 0, m)` or `coll_sphere(1.5)`: the owner or Qwark's own classes (0x504, 0x534..0x53a, 0x52, 0x53, 0x191, 0x184..0x18c, 399, 400) pass; else `SpawnBeamExplosion(5, 10, 7, 4, 1, 1, 15, m, v, m, 20, 5, 16, 0, 1)`, deleted. Otherwise its life out → deleted; running → deleted only when it is not drawn while its owner is | [`missile_update`] |
//! | 0x2c1528 | shot: `SpawnBeamExplosion(0, 0, 0, 0, 0, 1, 0, m, v, m, 5, 2, 7, 1, 0)`, deleted | [`missile_update`] |
//! | 0x2c1f28 | `CreateMoby(0x53)`: update / draw 0xff, drawn, state 0, +0xbc 0; scale = class·0.1; position; +0x60 velocity, +0x74 owner, health +0x20 = 20 (+0x24 20, +0x28 0, +0x29 1, +0x30 0.5); +0x70 = `ticks(360)`; sequence ≠ 1 → blend 1; matrix; Ratchet's light word / ambient | [`spawn_mine`] |
//! | 0x2c2048 | scale eases to 2 × class (SPEED·0.1); anim flags & 2 and the sequences not 2 → blend 2; the ship within 60 and drawn: velocity += toward it at (1 − d/60)·1.5·dt; the first 9 ticks position += velocity; then position += velocity, velocity ·= 0.945; out of [2, 1021] → deleted | [`mine_update`] |
//! | 0x2c2048 | the owner alive and 8 away: the template (`0x26e808(16, m, 0x30000, velocity)`); `coll_sphere_mobys(1.7, pos + 0.75 up, 0x10, m, tmpl)` \| `coll_sphere(1.7, ·, 4, m)`; shot (any hit): health − damage, at 0 → \| 0x2000 and no moby; the last moby one of Qwark's own → nothing; a contact: drawn → `SpawnBeamExplosion(4, 16, 7, 4, 1, 1, 15, …, 20, 5, 16, 0, 1)` (shot: `(0, …, 1, 0, …, 5, 2, 7, 3, 1)`), deleted | [`mine_update`] |
//! | 0x2c2048 | `FastDecTimer(+0x70)` out and not drawn → deleted | [`mine_update`] |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::turn::turn_toward;
use crate::moby_update::creature::{self as c, add, add_rot, atan, diff_rots, dist2, dist3, set_len3, sub, DT, DT2, SPEED};
use crate::moby_update::services::{pv, HitTemplate, World};
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 13;
pub const PART_FN: u32 = 0x2e_ce00;
pub const SHIELD_FN: u32 = 0x2e_84d8;
pub const MISSILE_FN: u32 = 0x2c_1528;
pub const MINE_FN: u32 = 0x2c_2048;
pub const PART_CLASSES: [i16; 11] = [389, 390, 391, 392, 393, 394, 395, 396, 399, 400, 401];
pub const SHIELD_CLASSES: [i16; 1] = [0x160];
pub const MISSILE: i16 = 0x52;
pub const MINE: i16 = 0x53;
pub const MISSILE_CLASSES: [i16; 1] = [MISSILE];
pub const MINE_CLASSES: [i16; 1] = [MINE];

pub mod part_pvo {
    pub const VEL: usize = 0x10;
    pub const F20: usize = 0x20;
    pub const SPIN: usize = 0x30;
    pub const OWNER: usize = 0x3c;
    pub const TIMER: usize = 0x40;
    pub const JOINT: usize = 0x44;
    pub const LEN: usize = 0x48;
}

mod shield_pvo {
    pub const OWNER: usize = 0x00;
    pub const FADE: usize = 0x04;
    pub const SCROLL: usize = 0x08;
    pub const LEN: usize = 0x10;
}

mod missile_pvo {
    pub const VEL: usize = 0x00;
    pub const SEEN: usize = 0x10;
    pub const OWNER: usize = 0x20;
    pub const TARGET: usize = 0x24;
    pub const LIFE: usize = 0x28;
    pub const SPEED: usize = 0x2c;
    pub const YAW_V: usize = 0x30;
    pub const PITCH_V: usize = 0x34;
    pub const TOP: usize = 0x38;
    pub const LEN: usize = 0x40;
}

mod mine_pvo {
    pub const HEALTH: usize = 0x20;
    pub const F24: usize = 0x24;
    pub const F28: usize = 0x28;
    pub const F29: usize = 0x29;
    pub const F30: usize = 0x30;
    pub const VEL: usize = 0x60;
    pub const LIFE: usize = 0x70;
    pub const OWNER: usize = 0x74;
    pub const LEN: usize = 0x80;
}

/// Qwark's own classes (his ship, its parts, his shots and the level's 0x504 / 0x534..0x53a) his shots pass through.
fn own(k: i16) -> bool { matches!(k, 0x504 | 0x534..=0x53a | 0x52 | 0x53 | 0x191 | 0x184..=0x18c | 399 | 400) }
/// The classes a falling part passes through.
fn own_part(k: i16) -> bool { matches!(k, 0x191 | 0x184..=0x18c | 399 | 400) }

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn set_link(w: &mut World, id: MobyId, o: usize, m: Option<MobyId>) { c::set_pi32(w, id, o, m.map_or(0, |m| m as i32 + 1)) }
fn gone(w: &World, m: MobyId) -> bool { let s = w.m(m).state; s == 0xfe || s == 0xfd }
fn class_scale(w: &World, k: i16) -> f32 { super::class_scale(w, k) }
fn in_box(p: [f32; 4], lo: f32, hi: f32) -> bool { (0..3).all(|k| lo <= p[k] && p[k] <= hi) }

/// `0x272078(m)`: Ratchet's light word and ambient.
fn hero_light(w: &mut World, id: MobyId) {
    if let Some(h) = w.hero_moby {
        let (l, a) = (w.m(h).light, w.m(h).ambient);
        let m = w.mm(id);
        m.light = l;
        m.ambient = a;
    }
}

/// Level13 `0x2ecd10(owner, joint, class)` (module doc).
pub fn spawn_part(w: &mut World, owner: MobyId, joint: usize, class: i16) -> Option<MobyId> {
    let id = w.create_moby(class)?;
    crate::moby_update::story::pvars(w, id, part_pvo::LEN);
    let s = class_scale(w, class) * w.m(owner).scale / class_scale(w, w.m(owner).o_class);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.scale = s;
    }
    c::set_pi32(w, id, part_pvo::JOINT, joint as i32);
    let p = w.joint_point(owner, joint);
    w.mm(id).position = p;
    set_link(w, id, part_pvo::OWNER, Some(owner));
    w.build_matrix(id);
    let (rot, rows) = (w.m(owner).rotation, w.m(owner).rows);
    {
        let m = w.mm(id);
        m.rotation = rot;
        m.rows[0] = rows[0];
        m.rows[1] = rows[1];
        m.rows[2] = rows[2];
    }
    let t = w.ticks(600);
    c::set_pi32(w, id, part_pvo::TIMER, t);
    w.build_matrix(id);
    Some(id)
}

/// Level13 `0x2ece00` (module doc).
pub fn part_update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, part_pvo::LEN);
    let Some(o) = link(w, id, part_pvo::OWNER).filter(|&o| w.m(o).o_class == super::qwark_ship::CLASS && !gone(w, o)) else {
        w.delete_moby(id);
        return;
    };
    let om = w.m(o).mode & 3;
    w.mm(id).mode = (w.m(id).mode & !3) | om;
    match w.m(id).state {
        0 => {
            w.mm(id).state = 1;
            c::set_pv4(w, id, part_pvo::F20, [0.0; 4]);
        }
        1 => {
            let s = class_scale(w, w.m(id).o_class) * w.m(o).scale / class_scale(w, w.m(o).o_class);
            let (l, a) = (w.m(o).light, w.m(o).ambient);
            let m = w.mm(id);
            m.scale = s;
            m.light = l;
            m.ambient = a;
        }
        3 => {
            let v = c::pv4(w, id, part_pvo::VEL);
            let p = add(w.m(id).position, v);
            w.mm(id).position = p;
            let mut v = c::scale(v, f32::from_bits(0x3f7e_b852));
            c::set_pv4(w, id, part_pvo::VEL, v);
            if let Some(h) = w.coll_sphere(pv(p), Pf::f(2.0), 0, Some(id)) {
                if h.moby.is_none_or(|m| !own_part(w.m(m).o_class)) {
                    let r = crate::hero::guns::reflect([v[0], v[1], v[2]], h.normal);
                    v = [r[0], r[1], r[2], v[3]];
                    c::set_pv4(w, id, part_pvo::VEL, v);
                    w.mm(id).state = 4;
                }
            }
            for k in 0..3 {
                let s = c::pf(w, id, part_pvo::SPIN + 4 * k);
                let r = add_rot(w.m(id).rotation[k], s);
                w.mm(id).rotation[k] = r;
                c::set_pf(w, id, part_pvo::SPIN + 4 * k, s * 0.999);
            }
            let p = w.m(id).position;
            if in_box(p, 10.0, 1013.0) && (c::dec_timer_pvar_i32(w, id, part_pvo::TIMER) == 0 || w.m(id).visible != 0) { return; }
            w.delete_moby(id);
        }
        4 => {
            let b = fx::Beam { damage_r: 2.0, damage: 4.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: -1, shake: true };
            let p = w.m(id).position;
            fx::beam_explosion(w, &b, Some(id), p);
            w.delete_moby(id);
        }
        _ => {}
    }
}

/// Level13 `0x2e8438(owner)` (module doc).
pub fn spawn_shield(w: &mut World, owner: MobyId) -> Option<MobyId> {
    let id = w.create_moby(super::qwark_ship::SHIELD)?;
    crate::moby_update::story::pvars(w, id, shield_pvo::LEN);
    let p = w.m(owner).position;
    {
        let m = w.mm(id);
        m.draw_dist = 0x3ff;
        m.update_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.cmd = 0;
        m.position = p;
        m.mode |= 0x200;
    }
    set_link(w, id, shield_pvo::OWNER, Some(owner));
    c::set_pf(w, id, shield_pvo::FADE, 0.0);
    w.build_matrix(id);
    hero_light(w, id);
    let s = class_scale(w, super::qwark_ship::SHIELD);
    w.mm(id).scale = s;
    Some(id)
}

/// Level13 `0x2e84d8` (module doc).
pub fn shield_update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, shield_pvo::LEN);
    let cs = class_scale(w, w.m(id).o_class);
    let ease = |w: &mut World, to: f32| { let m = w.mm(id); m.scale += (to - m.scale) * 0.1; };
    match w.m(id).state {
        0 => {
            let col = super::class_collision(w, w.m(id).o_class);
            let m = w.mm(id);
            m.state = 1;
            m.has_collision = col;
        }
        1 => {
            ease(w, cs * 4.0);
            let f = c::pf(w, id, shield_pvo::FADE) + SPEED * 0.02;
            if 1.0 <= f {
                c::set_pf(w, id, shield_pvo::FADE, 1.0);
                w.mm(id).state = 2;
            } else {
                c::set_pf(w, id, shield_pvo::FADE, f);
            }
            let f = c::pf(w, id, shield_pvo::FADE);
            w.mm(id).alpha = (f * 60.0) as i32 as u8;
        }
        2 => {
            ease(w, cs * 4.0);
            if link(w, id, shield_pvo::OWNER).is_none_or(|o| gone(w, o)) {
                w.mm(id).state = 3;
            }
        }
        3 => {
            ease(w, cs);
            let f = c::pf(w, id, shield_pvo::FADE) - SPEED * 0.02;
            if f <= 0.0 {
                c::set_pf(w, id, shield_pvo::FADE, 0.0);
                w.mm(id).state = 4;
            } else {
                c::set_pf(w, id, shield_pvo::FADE, f);
            }
            let f = c::pf(w, id, shield_pvo::FADE);
            w.mm(id).alpha = (f * 60.0) as i32 as u8;
        }
        4 => {
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    // `0x263d90(class, 0, d)`: the class's texture scroll, kept for the renderer (not drawn yet).
    let s = c::pi32(w, id, shield_pvo::SCROLL) - 0x40;
    let s = if 0x1000 < s { s - 0x1000 } else if s < 0 { s + 0x1000 } else { s };
    c::set_pi32(w, id, shield_pvo::SCROLL, s);
}

/// The PS2 FPU's view of a word: denormals are 0.
fn flushed(bits: u32) -> f32 { if (bits >> 23) & 0xff == 0 { 0.0 } else { f32::from_bits(bits) } }

/// Level13 `0x2c13b0(speed, top, owner, pos, target, rot, life)` (module doc).
#[allow(clippy::too_many_arguments)]
pub fn spawn_missile(w: &mut World, speed: f32, top: f32, owner: MobyId, pos: [f32; 4], target: Option<MobyId>, rot: [f32; 4], life: i32) -> Option<MobyId> {
    use missile_pvo as p;
    let id = w.create_moby(MISSILE)?;
    crate::moby_update::story::pvars(w, id, p::LEN);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.position = pos;
        m.rotation = rot;
    }
    set_link(w, id, p::TARGET, target);
    set_link(w, id, p::OWNER, Some(owner));
    c::set_pf(w, id, p::SPEED, speed);
    c::set_pf(w, id, p::TOP, top);
    c::set_pi32(w, id, p::LIFE, life);
    c::set_pf(w, id, p::YAW_V, 0.0);
    c::set_pf(w, id, p::PITCH_V, 0.0);
    let op = &w.m(owner).pvars;
    let word = |k: usize| op.get(0xc0 + 4 * k..0xc4 + 4 * k).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
    let v = [flushed(word(0)), flushed(word(1)), flushed(word(2)), flushed(word(3))];
    c::set_pv4(w, id, p::VEL, v);
    if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 10); }
    let np = add(w.m(id).position, v);
    w.mm(id).position = np;
    if let Some(t) = link(w, id, p::TARGET) {
        let tp = w.m(t).position;
        c::set_pv4(w, id, p::SEEN, tp);
    }
    let s = class_scale(w, MISSILE);
    w.mm(id).scale = s;
    w.build_matrix(id);
    glow_on_joint0(w, id, life);
    Some(id)
}

/// `PartType26Spawn(500000, m, 0x60808080, life, 0)`: the glow on joint list 0's point, 0.4 toward the camera.
fn glow_on_joint0(w: &mut World, m: MobyId, life: i32) {
    *w.svc.fx.part_spawns.entry(26).or_default() += 1;
    let j = w.joint_point(m, 0);
    let cam = w.camera_point();
    let d = set_len3([cam[0] - j[0], cam[1] - j[1], cam[2] - j[2], 0.0], 0.4);
    let at = [j[0] + d[0], j[1] + d[1], j[2] + d[2]];
    let Some(sys) = w.particles.as_deref_mut() else {
        w.rng.rand();
        return;
    };
    if crate::particles::type26::spawn(sys, w.rng, 500_000.0, m, 0x6080_8080, life, 0, at).is_none() { w.svc.fx.part_failed += 1; }
}

/// The missile's trail (`0x2c1528`'s first part).
fn missile_trail(w: &mut World, id: MobyId) {
    use missile_pvo as p;
    let x = w.rng.randf(-1.0, 1.0);
    let y = w.rng.randf(-1.0, 1.0);
    let z = w.rng.randf(-1.0, 1.0);
    let f = w.rng.randf(0.1, 0.2);
    let e = set_len3([x, y, z, 0.0], f * DT);
    let g = w.rng.randf(DT * 0.1, DT);
    let vel = c::pv4(w, id, p::VEL);
    let e = add(e, set_len3(vel, -g));
    let pos = w.m(id).position;
    let n6 = w.ticks(6);
    fx::part44(w, &crate::particles::type44::Spawn { size: 80000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb9d1_b717), w: 0.0, pos: [pos[0], pos[1], pos[2]], vel: [e[0], e[1], e[2]], life: n6, alpha: 0x7f, rgb: 0xb0_b0b0, spin: 3 });
    let speed = c::pf(w, id, p::SPEED);
    let mut t = 0.0f32;
    loop {
        let t1 = t + 0.333_333_34;
        let s = w.rng.randf(t, t1);
        let q = add(set_len3(vel, s * speed), pos);
        let n60 = w.ticks(0x3c);
        let a = crate::particles::type44::Spawn { size: 80000.0, growth: 1000.0, damp: 1.0, fall: f32::from_bits(0xb951_b717), w: 0.0, pos: [q[0], q[1], q[2]], vel: [e[0], e[1], e[2]], life: n60, alpha: 0x7f, rgb: 0x60_6060, spin: 3 };
        if let Some(sys) = w.particles.as_deref_mut() {
            *w.svc.fx.part_spawns.entry(44).or_default() += 1;
            match crate::particles::type44::spawn_rng(sys, w.rng, &a) {
                Some(i) => {
                    let d = sys.def_first(23);
                    let r = &mut sys.pool.recs[i];
                    r[3] = 0x44;
                    r[2] = d;
                }
                None => w.svc.fx.part_failed += 1,
            }
        } else {
            fx::part44(w, &a);
        }
        let rows = w.m(id).rows;
        let up: [f32; 3] = std::array::from_fn(|k| rows[2][k] * f32::from_bits(0x3ca3_d70a));
        let ang = w.rng.rand_angle();
        let up = crate::moby_update::classes::blaster_shot::rotate(up, ang, [rows[0][0], rows[0][1], rows[0][2]]);
        let n5 = w.ticks(5);
        fx::part21(w, 40000.0, q, [up[0], up[1], up[2], 0.0], 0x4f00_7fff, 0x1fff_ffff, n5, 1);
        t = t1;
        if 1.0 <= t1 { break; }
    }
}

/// Level13 `0x2c1528` (module doc).
pub fn missile_update(w: &mut World, id: MobyId) {
    use missile_pvo as p;
    if w.m(id).pvars.len() < p::LEN { w.delete_moby(id); return; }
    let cs = class_scale(w, MISSILE);
    { let m = w.mm(id); m.scale += (cs - m.scale) * 0.1; }
    let old = w.m(id).position;
    let hit = w.get_hit(id, 0xa3_0000, false);
    let shot = hit.is_some_and(|h| h.attacker.is_some_and(|a| w.m(a).o_class != MISSILE));
    if shot {
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 0.0, flash2: 0.0, flash_dist: 0.0, scale: 1.0, light: 0.0, streaks: 5, sparks: 2, puffs: 7, debris: 0, sound: 1, shake: false };
        let pos = w.m(id).position;
        fx::beam_explosion(w, &b, Some(id), pos);
        w.delete_moby(id);
        return;
    }
    missile_trail(w, id);
    let (k4, k8) = (DT2 * 12.566_371, DT * 25.132_742);
    let brake = |w: &mut World| {
        let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, p::YAW_V));
        turn_toward(a, k4, k4, k8, &mut a, &mut v);
        w.mm(id).rotation[2] = a;
        c::set_pf(w, id, p::YAW_V, v);
        let (mut a, mut v) = (w.m(id).rotation[1], c::pf(w, id, p::PITCH_V));
        turn_toward(a, k4, k4, k8, &mut a, &mut v);
        w.mm(id).rotation[1] = a;
        c::set_pf(w, id, p::PITCH_V, v);
        set_link(w, id, p::TARGET, None);
    };
    let seen = c::pv4(w, id, p::SEEN);
    match link(w, id, p::TARGET).filter(|&t| !gone(w, t)) {
        Some(t) if dist3(seen, w.m(t).position) < 3.0 => {
            let pos = w.m(id).position;
            let owner_tractor = link(w, id, p::OWNER).is_some_and(|o| w.m(o).state == 5);
            let ship = w.svc.vehicle.moby.filter(|&v| v < w.table.mobys.len() && w.m(v).o_class == 0x45);
            let aim = match ship {
                Some(v) if !owner_tractor && w.m(v).pvars.len() >= super::gemlik_ship::pvo::LEN => {
                    use super::gemlik_ship::pvo as sp;
                    let vm = w.m(v);
                    let yaw = add_rot(vm.rotation[2], c::pf(w, v, sp::STICK_XV));
                    let pitch = add_rot(vm.rotation[1], c::pf(w, v, sp::STICK_YV));
                    let d = dist3(pos, vm.position);
                    let k = (d / (c::pf(w, id, p::SPEED) + 24.0)) * c::pf(w, v, sp::SPEED);
                    add(polar(k, yaw, -pitch), vm.position)
                }
                _ => w.m(t).position,
            };
            let yaw = atan(aim[0] - pos[0], aim[1] - pos[1]);
            let pitch = atan(dist2(pos, aim), aim[2] - pos[2]);
            if diff_rots(yaw, w.m(id).rotation[2]) < std::f32::consts::FRAC_PI_3 {
                let s = c::pf(w, id, p::SPEED);
                c::set_pf(w, id, p::SPEED, s + (c::pf(w, id, p::TOP) - s) * 0.05);
            }
            let (mut a, mut v) = (w.m(id).rotation[2], c::pf(w, id, p::YAW_V));
            turn_toward(yaw, k4, k4, k8, &mut a, &mut v);
            w.mm(id).rotation[2] = a;
            c::set_pf(w, id, p::YAW_V, v);
            let (mut a, mut v) = (w.m(id).rotation[1], c::pf(w, id, p::PITCH_V));
            turn_toward(-pitch, k4, k4, k8, &mut a, &mut v);
            w.mm(id).rotation[1] = a;
            c::set_pf(w, id, p::PITCH_V, v);
            let tp = w.m(t).position;
            c::set_pv4(w, id, p::SEEN, tp);
        }
        _ => brake(w),
    }
    let life = c::pi32(w, id, p::LIFE);
    let t120 = w.ticks(0x78);
    let (speed, yaw, pitch) = (c::pf(w, id, p::SPEED), w.m(id).rotation[2], w.m(id).rotation[1]);
    let v = if t120 < life {
        polar(speed, yaw, -pitch)
    } else {
        let f = (life as f32 / t120 as f32).clamp(0.0, 1.0);
        add(c::scale(c::pv4(w, id, p::VEL), 1.0 - f * f), polar(speed * f * f, yaw, -pitch))
    };
    c::set_pv4(w, id, p::VEL, v);
    let pos = add(w.m(id).position, v);
    w.mm(id).position = pos;
    if !in_box(pos, 2.0, 1021.0) {
        w.delete_moby(id);
        return;
    }
    let owner = link(w, id, p::OWNER);
    let nose = w.joint_point(id, 1);
    let h = w.coll_line(pv(old), pv(nose), 0, Some(id)).or_else(|| w.coll_sphere(pv(pos), Pf::f(1.5), 0, Some(id)));
    let pass = h.as_ref().is_none_or(|h| h.moby.is_some_and(|m| Some(m) == owner || own(w.m(m).o_class)));
    if pass {
        if c::dec_timer_pvar_i32(w, id, p::LIFE) == 0 {
            if w.m(id).visible != 0 { return; }
            let Some(o) = owner else { return };
            if w.m(o).visible == 0 { return; }
        }
        w.delete_moby(id);
        return;
    }
    let b = fx::Beam { damage_r: 5.0, damage: 10.0, flash: 7.0, flash2: 4.0, flash_dist: 1.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 5, puffs: 16, debris: 0, sound: 0, shake: true };
    fx::beam_explosion(w, &b, Some(id), pos);
    w.delete_moby(id);
}

/// Level13 `0x2c1f28(owner, pos, vel)` (module doc).
pub fn spawn_mine(w: &mut World, owner: MobyId, pos: [f32; 4], vel: [f32; 4]) -> Option<MobyId> {
    use mine_pvo as p;
    let id = w.create_moby(MINE)?;
    crate::moby_update::story::pvars(w, id, p::LEN);
    let s = class_scale(w, MINE) * 0.1;
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        m.state = 0;
        m.cmd = 0;
        m.scale = s;
        m.position = pos;
    }
    c::set_pv4(w, id, p::VEL, vel);
    set_link(w, id, p::OWNER, Some(owner));
    c::set_pu8(w, id, p::F28, 0);
    c::set_pf(w, id, p::HEALTH, 20.0);
    c::set_pi16(w, id, p::F24, 0x14);
    c::set_pf(w, id, p::F30, 0.5);
    c::set_pu8(w, id, p::F29, 1);
    let t = w.ticks(0x168);
    c::set_pi32(w, id, p::LIFE, t);
    if w.m(id).anim.seq_b != 1 { w.anim_blend(id, 1, 0, 10); }
    w.build_matrix(id);
    hero_light(w, id);
    Some(id)
}

/// Level13 `0x2c2048` (module doc).
pub fn mine_update(w: &mut World, id: MobyId) {
    use mine_pvo as p;
    if w.m(id).pvars.len() < p::LEN { w.delete_moby(id); return; }
    let cs = class_scale(w, MINE);
    { let m = w.mm(id); m.scale += ((cs + cs) - m.scale) * SPEED * 0.1; }
    if w.m(id).anim.flags & 2 != 0 && w.m(id).anim.seq_a != 2 && w.m(id).anim.seq_b != 2 { w.anim_blend(id, 2, 0, 10); }
    let pos = w.m(id).position;
    let mut v = c::pv4(w, id, p::VEL);
    if let Some(s) = w.svc.vehicle.moby.filter(|&s| s < w.table.mobys.len()) {
        let sp = w.m(s).position;
        let d = dist3(pos, sp);
        if d < 60.0 && w.m(id).visible != 0 {
            let f = (1.0 - d / 60.0).min(1.0);
            v = add(v, set_len3(sub(sp, pos), f * (DT * 1.5)));
        }
    }
    let first = c::pi32(w, id, p::LIFE) >= w.ticks(0x168) - 9;
    let np = add(pos, v);
    w.mm(id).position = np;
    if !first { v = c::scale(v, f32::from_bits(0x3f71_eb85)); }
    c::set_pv4(w, id, p::VEL, v);
    if !in_box(np, 2.0, 1021.0) {
        w.delete_moby(id);
        return;
    }
    if let Some(o) = link(w, id, p::OWNER).filter(|&o| !gone(w, o)) {
        if 8.0 < dist3(np, w.m(o).position) {
            let tmpl = HitTemplate { dir: pv(v), attacker: Some(id), flags: 0x3_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::f(16.0), w20: 1 };
            let centre = [np[0], np[1], np[2] + 0.75, np[3]];
            let r = Pf::b(0x3fd9_999a);
            let listed = crate::moby_update::services::sphere_mobys_in(w.table, w.svc, w.classes, r, pv(centre), 0x10, Some(id), Some(&tmpl));
            let world = w.coll_sphere(pv(centre), r, 4, Some(id));
            let mut last = listed.last().copied();
            if let Some(h) = &world { if h.moby.is_some() { last = h.moby; } }
            let mut n: u32 = (!listed.is_empty() || world.is_some()) as u32;
            if let Some(h) = w.get_hit(id, u32::MAX, false) {
                let hp = c::pf(w, id, p::HEALTH) - crate::moby_update::services::fl(h.damage);
                c::set_pf(w, id, p::HEALTH, hp);
                if hp <= 0.0 {
                    n |= 0x2000;
                    last = None;
                }
            }
            if last.is_some_and(|m| own(w.m(m).o_class)) { n = 0; }
            if n != 0 {
                if w.m(id).visible != 0 {
                    let b = if n & 0x2000 == 0 {
                        fx::Beam { damage_r: 4.0, damage: 16.0, flash: 7.0, flash2: 4.0, flash_dist: 1.0, scale: 1.0, light: 15.0, streaks: 20, sparks: 5, puffs: 16, debris: 0, sound: 0, shake: true }
                    } else {
                        fx::Beam { damage_r: 0.0, damage: 0.0, flash: 0.0, flash2: 0.0, flash_dist: 0.0, scale: 1.0, light: 0.0, streaks: 5, sparks: 2, puffs: 7, debris: 0, sound: 3, shake: true }
                    };
                    fx::beam_explosion(w, &b, Some(id), np);
                }
                w.delete_moby(id);
                return;
            }
        }
    }
    if c::dec_timer_pvar_i32(w, id, p::LIFE) != 0 && w.m(id).visible == 0 { w.delete_moby(id); }
}
