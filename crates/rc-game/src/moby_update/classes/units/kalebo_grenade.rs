//! **The grenades of Kalebo's arena troopers, class 281** (level16 `0x2c5eb0`; created by the troopers 541 of kind 0,
//! `0x2c6268`: [`spawn`]; [`super::kalebo_trooper`]). A lobbed grenade: it flies under its gravity, bounces off what it
//! meets (losing three quarters of its speed), comes to rest on the ground, blinks faster as its fuse runs down and
//! blows up (a beam explosion that hurts everything within 1.5). Read from the level16 decomp and disassembly.
//!
//! **Pvars**: +0x00 the flash record (+0x07 the flash, +0x0c / +0x0e s16 its fade in / out), +0x10 the velocity,
//! +0x20 the thrower (a moby index + 1 here; a pointer in the game), +0x24 the gravity, +0x28 the fuse.
//!
//! | address | what | port |
//! |---|---|---|
//! | `0x2c6268(g, thrower, at, vel, fuse)` | `CreateMoby(0x119)`: draw and update distance 0xff, state 1, drawn; +0x20 / +0x24 / +0x28; +0x0c `ticks(5)`, +0x0e `ticks(15)`; position `at`, +0x10 `vel`; pitch `randf(−180°, 180°)`, yaw `atan(vel) + 90°`; `MobyBuildMatrix` | [`spawn`] |
//! | state 1 | position += velocity; `CollLine_Fix(old, new, 5, thrower)`: n = its normal; v·n < 0 → v = reflect(v, n)·0.25 | [`update`] |
//! | | `coll_sphere(0.333, position, 2)`: n = the normal at 0.333; v·n < 0 → v = reflect(v, n)·0.25; position = the pushed centre, z + n.z; n.z ≥ 0.3 and \|v\| < 0.25·dt → 2, fuse `ticks(90)` | [`update`] |
//! | | the fuse running: below `ticks(45)` on each `ticks(10)` → flash 0xfa (`0x258d50`); out → 3; v.z −= gravity | [`update`] (`flash::start`) |
//! | state 2 | the fuse: below `ticks(61)` on each `ticks(20)` → the flash; out → 3 | [`update`] |
//! | state 3 | `PlayClassSound(0, 0)`; `SpawnBeamExplosion(1.5, 1, 3, 1.7, 4, 1, 7, m, 0, at the moby, 7, 10, 20, −1, 0, 7, −1, 0)`; `DeleteMoby` | [`update`] (`fx::beam_explosion`) |
//! | every tick | the flash (`0x258e30`) | [`update`] (`flash::update`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::creature::{self as c, flash, fx};
use crate::moby_update::services::{pf, pv as v4, World};

pub const REFERENCE_LEVEL: u32 = 16;
pub const UPDATE_FN: u32 = 0x2c_5eb0;
pub const CLASS: i16 = 0x119;
pub const CLASSES: [i16; 1] = [CLASS];

/// Pvar offsets (module doc).
pub mod pv {
    pub const FLASH: usize = 0x00;
    pub const VEL: usize = 0x10;
    pub const THROWER: usize = 0x20;
    pub const GRAVITY: usize = 0x24;
    pub const FUSE: usize = 0x28;
    pub const SIZE: usize = 0x2c;
}

/// The bounce's sphere (0x3eaa7efa) and what is left of the speed.
const RADIUS: f32 = f32::from_bits(0x3eaa_7efa);
const BOUNCE: f32 = 0.25;
/// The blast (`SpawnBeamExplosion`'s arguments; the stack's debris 7).
pub const BLAST: fx::Beam = fx::Beam { damage_r: 1.5, damage: 1.0, flash: 3.0, flash2: f32::from_bits(0x3fd9_999a), flash_dist: 4.0, scale: 1.0, light: 7.0, streaks: 7, sparks: 10, puffs: 20, debris: 7, sound: -1, shake: false };

/// `0x2c6268(g, thrower, at, vel, fuse)` (module doc).
pub fn spawn(w: &mut World, g: f32, thrower: MobyId, at: c::V, vel: c::V, fuse: i32) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    let (t5, t15) = (w.ticks(5), w.ticks(15));
    {
        let m = w.mm(id);
        m.draw_dist = 0xff;
        m.state = 1;
        m.visible = 1;
        m.update_dist = 0xff;
        if m.pvars.len() < pv::SIZE { m.pvars.resize(pv::SIZE, 0); }
        m.position = at;
    }
    c::set_pi32(w, id, pv::THROWER, thrower as i32 + 1);
    c::set_pf(w, id, pv::GRAVITY, g);
    c::set_pi32(w, id, pv::FUSE, fuse);
    c::set_pi16(w, id, pv::FLASH + 0xc, t5 as i16);
    c::set_pi16(w, id, pv::FLASH + 0xe, t15 as i16);
    c::set_pv4(w, id, pv::VEL, vel);
    let pitch = w.rng.randf(-180.0, 180.0) * 0.017_453_292;
    let m = w.mm(id);
    m.rotation[0] = pitch;
    m.rotation[2] = c::atan(vel[0], vel[1]) + 1.570_796_4;
    w.build_matrix(id);
    Some(id)
}

fn bounce(v: c::V, n: [f32; 3]) -> c::V {
    let r = crate::hero::guns::reflect([v[0], v[1], v[2]], n);
    [r[0] * BOUNCE, r[1] * BOUNCE, r[2] * BOUNCE, v[3] * BOUNCE]
}

fn blink(w: &mut World, id: MobyId, below: i32, every: i32) {
    let fuse = c::pi32(w, id, pv::FUSE);
    let (b, e) = (w.ticks(below), w.ticks(every).max(1));
    if fuse < b && fuse % e == 0 {
        c::set_pu8(w, id, pv::FLASH + 7, 0xfa);
        flash::start(w, id, pv::FLASH);
    }
}

/// Level16 `0x2c5eb0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pv::SIZE { return; }
    match w.m(id).state {
        1 => {
            let old = w.m(id).position;
            let mut v = c::pv4(w, id, pv::VEL);
            let new = c::add(old, v);
            w.mm(id).position = new;
            let thrower = usize::try_from(c::pi32(w, id, pv::THROWER) - 1).ok();
            if let Some(h) = w.coll_line(v4(old), v4(new), 5, thrower) {
                let n = c::set_len3([h.normal[0], h.normal[1], h.normal[2], 0.0], 1.0);
                if c::dot3(n, v) < 0.0 { v = bounce(v, [n[0], n[1], n[2]]); }
            }
            if let Some(h) = w.coll_sphere(v4(new), pf(RADIUS), 2, None) {
                let n = c::set_len3([h.normal[0], h.normal[1], h.normal[2], 0.0], RADIUS);
                if c::dot3(n, v) < 0.0 { v = bounce(v, [n[0], n[1], n[2]]); }
                if let Some(pc) = h.pushed_centre {
                    let m = w.mm(id);
                    m.position = [pc[0], pc[1], pc[2] + n[2], new[3]];
                }
                if 0.3 <= n[2] && c::len3(v) < c::DT * 0.25 {
                    w.mm(id).state = 2;
                    let t = w.ticks(90);
                    c::set_pi32(w, id, pv::FUSE, t);
                }
            }
            if c::dec_timer_pvar_i32(w, id, pv::FUSE) == 0 {
                blink(w, id, 45, 10);
            } else {
                w.mm(id).state = 3;
            }
            v[2] -= c::pf(w, id, pv::GRAVITY);
            c::set_pv4(w, id, pv::VEL, v);
        }
        2 => {
            if c::dec_timer_pvar_i32(w, id, pv::FUSE) == 0 {
                blink(w, id, 61, 20);
            } else {
                w.mm(id).state = 3;
            }
        }
        3 => {
            w.play_sound(0, 0, id);
            let p = w.m(id).position;
            fx::beam_explosion(w, &BLAST, Some(id), p);
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    flash::update(w, id, pv::FLASH);
}
