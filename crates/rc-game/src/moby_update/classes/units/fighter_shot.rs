//! **The fighters' laser shot, class 1017** (created by code only): its spawner level11 `0x308f48` and its update
//! `0x309098`, the same code on levels 13 and 17 (`0x302e48` / `0x2e2978` the updates): one shared unit through
//! [`LevelPorts`](super::LevelPorts) code identity. Pokitaru's fighters 1319 fire it at the jet
//! ([`super::ship_fighter`]). No level data but the trail's colours. Read from the level11 decomp. Native `f32`.
//!
//! **Pvar block** (0x24): +0x00 the velocity, +0x10 the brightness (1.0, ×0.74 a tick once its life is out), +0x14 its
//! life (ticks), +0x18 the life timer, +0x1c the owner (index + 1), +0x20 the damage.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x308f48 | `CreateMoby(0x3f9)` (none: nothing); collision off (+0x94 = 0); +0x1c = owner; update distance 0xff, draw distance `trunc(range)`, rot.x = 0, drawn | [`spawn`] |
//! | 0x308f48 | rot.y = −atan(\|dir\|xy, dir.z), rot.z = atan(dir.x, dir.y); +0x10 = 1; speed > 0: velocity = dir at that speed, else velocity = dir, speed = \|dir\|; position | [`spawn`] |
//! | 0x308f48 | +0x14 = `trunc(range / speed)`, +0x20 = damage, +0x18 = +0x14 − 13; mode = 0x200; `MobyBuildMatrix` | [`spawn`] |
//! | 0x309098 | no pvars → nothing; old = position | [`update`] |
//! | 0x309098 | the trail: `PartType27Spawn(21000, position + rand_vec(0, 0.1), velocity·0.5, FastTweenColor(randf(0, 1), 0x80802000, 0x80808020), ticks(10))` | [`update`] (`fx::part27`) |
//! | 0x309098 | position += velocity; the template (`0x26e808`: damage +0x20, flags 0x10000, self, the velocity); `CollLine_Fix(old, position, 0, owner, tmpl)` | [`update`] (`services::line_hit_in`) |
//! | 0x309098 | no hit: `FastDecTimer(+0x18)` out → +0x10 ·= 0.74, below 0.02 → `DeleteMoby` | [`update`] |
//! | 0x309098 | a hit: five sparks as the ships' laser's (`PartType27Spawn(90000, …)`), `DeleteMoby` | [`update`] ([`super::ship_laser::hit_sparks`]) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::part27;
use crate::moby_update::creature::{self as c, add, atan, len3, set_len3};
use crate::moby_update::services::{pf as to_pf, pv, HitTemplate, World};

pub const REFERENCE_LEVEL: u32 = 11;
pub const UPDATE_FN: u32 = 0x30_9098;
pub const SPAWN_FN: u32 = 0x30_8f48;
pub const CLASS: i16 = 0x3f9;
pub const CLASSES: [i16; 1] = [CLASS];

pub mod pvo {
    pub const VEL: usize = 0x00;
    pub const GLOW: usize = 0x10;
    pub const LIFE: usize = 0x14;
    pub const TIMER: usize = 0x18;
    pub const OWNER: usize = 0x1c;
    pub const DAMAGE: usize = 0x20;
    pub const LEN: usize = 0x24;
}

/// gp−0x4e40 / −0x4e3c / −0x4e38 / −0x4e34 / −0x4e30 (level 11): the trail's colours, life, size and the velocity
/// share.
const TRAIL_A: u32 = 0x8080_2000;
const TRAIL_B: u32 = 0x8080_8020;
const TRAIL_LIFE: i32 = 10;
const TRAIL_SIZE: f32 = 21000.0;
const TRAIL_VEL: f32 = 0.5;

fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(c::pi32(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }

/// Level11 `0x308f48(range, speed, damage, owner, dir, pos)` (module doc).
pub fn spawn(w: &mut World, range: f32, speed: f32, damage: f32, owner: MobyId, dir: [f32; 4], pos: [f32; 4]) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    w.mm(id).has_collision = false;
    c::set_pi32(w, id, pvo::OWNER, owner as i32 + 1);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = range as i16;
        m.rotation[0] = 0.0;
        m.visible = 1;
        let xy = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
        m.rotation[1] = -atan(xy, dir[2]);
        m.rotation[2] = atan(dir[0], dir[1]);
    }
    c::set_pf(w, id, pvo::GLOW, 1.0);
    let speed = if 0.0 < speed {
        c::set_pv4(w, id, pvo::VEL, set_len3(dir, speed));
        speed
    } else {
        c::set_pv4(w, id, pvo::VEL, dir);
        len3(dir)
    };
    w.mm(id).position = pos;
    let life = (range / speed) as i32;
    c::set_pi32(w, id, pvo::LIFE, life);
    c::set_pf(w, id, pvo::DAMAGE, damage);
    c::set_pi32(w, id, pvo::TIMER, life - 13);
    w.mm(id).mode = 0x200;
    w.build_matrix(id);
    Some(id)
}

/// Level11 `0x309098` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { return; }
    let old = w.m(id).position;
    let f = w.rng.randf(0.0, 1.0);
    let rgba = crate::particles::tween_color(f.to_bits(), TRAIL_A, TRAIL_B);
    let j = w.rng.rand_vec(0.0, 0.1);
    let p = add(old, [j[0], j[1], j[2], 0.0]);
    let vel = c::pv4(w, id, pvo::VEL);
    let v = [vel[0] * TRAIL_VEL, vel[1] * TRAIL_VEL, vel[2] * TRAIL_VEL, vel[3] * TRAIL_VEL];
    let life = w.ticks(TRAIL_LIFE);
    part27(w, TRAIL_SIZE, p, v, rgba, life);
    let p = add(old, vel);
    w.mm(id).position = p;
    let dmg = to_pf(c::pf(w, id, pvo::DAMAGE));
    let tmpl = HitTemplate { dir: pv(vel), attacker: Some(id), flags: 0x1_0000, b18: 0, b19: 0, h1a: w.m(id).o_class as u16, damage: dmg, w20: 0 };
    let ign = link(w, id, pvo::OWNER);
    let Some(h) = crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), 0, ign, &tmpl) else {
        if c::dec_timer_pvar_i32(w, id, pvo::TIMER) == 0 { return; }
        let g = c::pf(w, id, pvo::GLOW) * 0.74;
        c::set_pf(w, id, pvo::GLOW, g);
        if 0.02 <= g { return; }
        w.delete_moby(id);
        return;
    };
    super::ship_laser::hit_sparks(w, p, vel, h.normal);
    w.delete_moby(id);
}
