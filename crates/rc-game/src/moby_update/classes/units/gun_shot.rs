//! **The gun shot, class 184** (created by code only): its spawner level12 `0x2d4150` and its update `0x2d4378`, the
//! same code on level04 (`0x2aee30` / `0x2af058`, clusters of two): **one shared unit** (code identity through
//! [`LevelPorts`]). On Hoven the carrier 1274's five guns fire it ([`super::hoven_carrier`]); on Eudora its class-255
//! shooter (the 0xff branches) is not ported yet. A new consumer calls [`spawn`] with its own moby, muzzle point,
//! velocity and life. No level data (every constant is inline). Read from the level12 decomp. Native `f32`.
//!
//! **Pvar block** (0x2c): +0x00 the velocity, +0x10 `randf(−dt·π/2, …)` (unread), +0x14 0, +0x18 the owner (index + 1),
//! +0x1c (s16) the timer, +0x1e (s16) its start, +0x20 the last distance to Ratchet, +0x24 coming closer, +0x28 the
//! whizz played.
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x2d4150 | `CreateMoby(184)` (none: nothing); +0x18 = owner; update / draw distances 0xff, +0x31 = 1 | [`spawn`] |
//! | 0x2d4150 | owner of class 0xff: rot.x = 0; else scale ·= 0.5, rot.x = 0; rot.y = 0; rot.z = atan(vel.x, vel.y); position; +0x00 = vel | [`spawn`] |
//! | 0x2d4150 | +0x10 = `randf(−dt·π/2, f13)` (the second bound is a register the decomp lost [L]: the draw kept, the value unread); +0x14 = 0; +0x1c = +0x1e = life; +0x20 = distance to Ratchet; +0x24 = +0x28 = 0 | [`spawn`] (the draw with the bound −dt·π/2..dt·π/2 [L]) |
//! | 0x2d4150 | two glows on the shot (`PartType26Spawn(100000, shot, c1, life·5/4, −1)`, `(40000, shot, c2, …)`): class 0xff 0x2f7f4f4f / 0x4f7f7f7f, else 0x2f4f7f7f / 0x4f6f7f7f | [`spawn`] (`projectile::part26`) |
//! | 0x2d4150 | `MobyBuildMatrix` | [`spawn`] |
//! | 0x2d4378 | the far limit: 64, 128 for an owner of class 0xff, 100 on level 12 | [`update`] |
//! | 0x2d4378 | d = distance to Ratchet; old = position; position += velocity | [`update`] |
//! | 0x2d4378 | the whizz: not played, came closer before and now moving away (last < d) within 3 → `PlayClassSound(0, 0, self)`, played; d < last → coming closer; last = d | [`update`] |
//! | 0x2d4378 | x, y or z < 0, or farther than the limit from the camera (xy) → `DeleteMoby` | [`update`] |
//! | 0x2d4378 | +0xbc = 0: the template (dir = vel with its xy length 1, z = 1, w = 5627.98; self; flags 0x50001 and damage 3 for an owner of class 0xff or 0x4b1, else 0x10001 and 1; type 1 / 1, the class; +0x20 = 1) | [`update`] |
//! | 0x2d4378 | `FastDecTimer(+0x1c)` out with +0xbc = 0: +0xbc = 1, +0x1c = +0x1e / 4; out or ≤ 0 with +0xbc set → `DeleteMoby` | [`update`] |
//! | 0x2d4378 | `CollLine_Fix(old, pos, 0, owner, template or none)`: a hit → position = the hit point; +0xbc = 0: five sparks (`PartType27Spawn(30000, pos, normalise(reflect(vel, n) + normalise(randf³)·0.5·\|reflect\|)·randf(3·dt, 6·dt), 0x7f2f4f6f, rand_range(ticks(10), ticks(15)))`); `DeleteMoby` | [`update`] (`hero::guns::reflect`, `fx::part27`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::fx::part27;
use crate::moby_update::creature::projectile::part26;
use crate::moby_update::creature::{self as c, add, atan, dist2, dist3, set_len3, DT};
use crate::moby_update::services::{pf as to_pf, pv, HitTemplate, World};

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x2d_4378;
pub const SPAWN_FN: u32 = 0x2d_4150;
pub const CLASS: i16 = 184;
pub const CLASSES: [i16; 1] = [CLASS];

pub mod pvo {
    pub const VEL: usize = 0x00;
    pub const F10: usize = 0x10;
    pub const F14: usize = 0x14;
    pub const OWNER: usize = 0x18;
    pub const TIMER: usize = 0x1c;
    pub const TIMER0: usize = 0x1e;
    pub const LAST: usize = 0x20;
    pub const CLOSER: usize = 0x24;
    pub const WHIZZ: usize = 0x28;
    pub const LEN: usize = 0x2c;
}

/// The owner classes with the heavy shot.
const HEAVY: [i16; 2] = [0xff, 0x4b1];

fn owner(w: &World, id: MobyId) -> Option<MobyId> {
    let o = c::pi32(w, id, pvo::OWNER);
    usize::try_from(o - 1).ok().filter(|&m| m < w.table.mobys.len())
}

fn owner_class(w: &World, id: MobyId) -> i16 { owner(w, id).map_or(-1, |o| w.m(o).o_class) }

/// Level12 `0x2d4150(owner, pos, vel, life)` (module doc).
pub fn spawn(w: &mut World, own: MobyId, pos: [f32; 4], vel: [f32; 4], life: i32) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    c::set_pi32(w, id, pvo::OWNER, own as i32 + 1);
    let big = w.m(own).o_class == 0xff;
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        m.visible = 1;
        if !big { m.scale *= 0.5; }
        m.rotation[0] = 0.0;
        m.rotation[1] = 0.0;
        m.rotation[2] = atan(vel[0], vel[1]);
        m.position = pos;
    }
    c::set_pv4(w, id, pvo::VEL, vel);
    let r = w.rng.randf(-(DT * std::f32::consts::FRAC_PI_2), DT * std::f32::consts::FRAC_PI_2);
    c::set_pi32(w, id, pvo::F14, 0);
    c::set_pi16(w, id, pvo::TIMER, life as i16);
    c::set_pi16(w, id, pvo::TIMER0, life as i16);
    c::set_pf(w, id, pvo::F10, r);
    let h = crate::moby_update::story::hero4(w);
    c::set_pf(w, id, pvo::LAST, dist3(pos, h));
    c::set_pi32(w, id, pvo::WHIZZ, 0);
    c::set_pi32(w, id, pvo::CLOSER, 0);
    let t = (life * 5) / 4;
    let (c1, c2) = if big { (0x2f7f_4f4f, 0x4f7f_7f7f) } else { (0x2f4f_7f7f, 0x4f6f_7f7f) };
    part26(w, 100_000.0, id, c1, t);
    part26(w, 40_000.0, id, c2, t);
    w.build_matrix(id);
    Some(id)
}

/// Level12 `0x2d4378` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { w.delete_moby(id); return; }
    let oc = owner_class(w, id);
    let mut far = if oc == 0xff { 128.0 } else { 64.0 };
    if w.svc.level == 0xc { far = 100.0; }
    let h = crate::moby_update::story::hero4(w);
    let old = w.m(id).position;
    let d = dist3(old, h);
    let vel = c::pv4(w, id, pvo::VEL);
    let p = add(old, vel);
    w.mm(id).position = p;
    let last = c::pf(w, id, pvo::LAST);
    if c::pi32(w, id, pvo::WHIZZ) == 0 && c::pi32(w, id, pvo::CLOSER) != 0 && last < d && d < 3.0 {
        w.play_sound(0, 0, id);
        c::set_pi32(w, id, pvo::WHIZZ, 1);
    }
    if d < c::pf(w, id, pvo::LAST) { c::set_pi32(w, id, pvo::CLOSER, 1); }
    c::set_pf(w, id, pvo::LAST, d);
    let cam = w.camera_point();
    if p[0] < 0.0 || p[1] < 0.0 || p[2] < 0.0 || far < dist2(p, [cam[0], cam[1], cam[2], 0.0]) {
        w.delete_moby(id);
        return;
    }
    let fresh = w.m(id).cmd == 0;
    let tmpl = fresh.then(|| {
        let (flags, damage) = if HEAVY.contains(&oc) { (0x5_0001, 3.0) } else { (0x1_0001, 1.0) };
        let l = (vel[0] * vel[0] + vel[1] * vel[1]).sqrt();
        let s = if l == 0.0 { 0.0 } else { 1.0 / l };
        let dir = [vel[0] * s, vel[1] * s, 1.0, f32::from_bits(0x45af_df66)];
        HitTemplate { dir: pv(dir), attacker: Some(id), flags, b18: 1, b19: 1, h1a: w.m(id).o_class as u16, damage: to_pf(damage), w20: 1 }
    });
    if c::dec_timer_pvar_s16(w, id, pvo::TIMER) == 0 {
        if c::pi16(w, id, pvo::TIMER) < 1 && !fresh {
            w.delete_moby(id);
            return;
        }
    } else if !fresh {
        if c::pi16(w, id, pvo::TIMER) < 1 {
            w.delete_moby(id);
            return;
        }
    } else {
        w.mm(id).cmd = 1;
        let t0 = c::pi16(w, id, pvo::TIMER0);
        c::set_pi16(w, id, pvo::TIMER, t0 / 4);
    }
    let ign = owner(w, id);
    let (point, normal) = match &tmpl {
        Some(t) => match crate::moby_update::services::line_hit_in(w.table, w.svc, w.classes, w.coll, pv(old), pv(p), 0, ign, t) {
            Some(h) => (h.point, h.normal),
            None => return,
        },
        None => match w.coll_line(pv(old), pv(p), 0, ign) {
            Some(h) => (h.point, h.normal),
            None => return,
        },
    };
    w.mm(id).position = [point[0], point[1], point[2], 0.0];
    if w.m(id).cmd == 0 {
        let v3 = [vel[0], vel[1], vel[2]];
        for _ in 0..5 {
            let x = w.rng.randf(-1.0, 1.0);
            let y = w.rng.randf(-1.0, 1.0);
            let z = w.rng.randf(-1.0, 1.0);
            let r = crate::hero::guns::reflect(v3, normal);
            let l = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
            let q = set_len3([x, y, z, 0.0], l * 0.5);
            let s = w.rng.randf(DT * 3.0, DT * 6.0);
            let q = set_len3([r[0] + q[0], r[1] + q[1], r[2] + q[2], 0.0], s);
            let (t10, t15) = (w.ticks(10), w.ticks(15));
            let life = w.rng.rand_range(t10, t15);
            let at = w.m(id).position;
            part27(w, 30000.0, at, q, 0x7f2f_4f6f, life);
        }
    }
    w.delete_moby(id);
}
