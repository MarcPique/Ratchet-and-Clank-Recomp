//! Orxon's spark fountains, class 1240 (level 10, 4 created instances): level10 0x2e5be0 (census U341). Every tick
//! `count` type-64 sparks fly out along the moby's heading and pitch from points scattered across it, at the best of
//! three `randf(0.5, 2)` speeds, fall with 12.5 units/s² and burst on the floor height of the pvar; 30 in 100 are
//! small additive ones with a scattered speed. Read from the level10 decomp and disassembly of 0x2e5be0 (the scatter
//! ranges are `randf(−s, s)`) and the overlay's data (gp−0x4d10 .. −0x4cfc). Native `f32`; the `rand` draws in the
//! game's order.
//!
//! **Pvar block**: +0x00 the floor z, +0x04 the size, +0x08 the life (seconds), +0x0c s32 the count, +0x10 the
//! sideways scatter, +0x14 the vertical scatter.
//!
//! | address | what | port |
//! |---|---|---|
//! | every tick | update distance +0x30 = 0x80 | [`update`] |
//! | per spark | small = `randi(100)` < 30 (gp−0x4cfc; 0x24a5c8 = L01 0x26c930); a = yaw + π/2; p = position + (cos a·`randf(−s, s)`, sin a·`randf(−s, s)`, `randf(−sz, sz)`) | [`update`] |
//! | | speed = the largest of three `randf(0.5, 2)` (gp−0x4d04 / −0x4d08); v = `0x255ed0(speed·dt, yaw, −pitch)` (L01 0x277b50) | [`update`] (`fx::polar`) |
//! | | small: v ·= `randf(0.5, 1.5)`, k = `randf(0.24, 0.36)`; else k = `randf(0.8, 1.2)` | [`update`] |
//! | | colour 1 = `FastTweenColor(randf(0.5, 1), 0x8020ff40, 0x6020ffc0)`, colour 2 = `FastTweenColor(randf(0.25, 0.5), 0x6020ffc0, 0x60000000)` (gp−0x4d10 / −0x4d0c); life = trunc(`multiply_global_scale(life·60)`) | [`update`] |
//! | | `PartType64Spawn(size·k, floor, 12.5·dt² (gp−0x4d00), p, v, life, colour 1, colour 2, small)` (0x266930 = L01 0x288d90) | [`update`] (`bomb_water::part64`) |
//! | | no sound, hit, flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb_water::part64;
use crate::moby_update::creature::{self as c, fx, DT, DT2};
use crate::moby_update::services::World;
use crate::particles::type64;

/// The update in the level10 class table.
pub const UPDATE_FN: u32 = 0x2e_5be0;
pub const REFERENCE_LEVEL: u32 = 10;
pub const CLASSES: [i16; 1] = [1240];
pub const COLOURS: [u32; 2] = [0x8020_ff40, 0x6020_ffc0];
pub const SMALL_ODDS: i32 = 30;
pub const GRAVITY: f32 = 12.5;

/// Level10 0x2e5be0 (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    w.mm(id).update_dist = 0x80;
    let (floor, size, life_s, n, s, sz) = (c::pf(w, id, 0), c::pf(w, id, 4), c::pf(w, id, 8), c::pi32(w, id, 0xc), c::pf(w, id, 0x10), c::pf(w, id, 0x14));
    for _ in 0..n.max(0) {
        let small = w.rng.randi(100) < SMALL_ODDS;
        let (pos, rot) = (w.m(id).position, w.m(id).rotation);
        let a = c::add_rot(rot[2], std::f32::consts::FRAC_PI_2);
        let ox = a.cos() * w.rng.randf(-s, s);
        let oy = a.sin() * w.rng.randf(-s, s);
        let oz = w.rng.randf(-sz, sz);
        let p = [pos[0] + ox, pos[1] + oy, pos[2] + oz, pos[3]];
        let mut sp = w.rng.randf(0.5, 2.0);
        let b = w.rng.randf(0.5, 2.0);
        if sp < b { sp = b; }
        let b = w.rng.randf(0.5, 2.0);
        if sp < b { sp = b; }
        let mut v = fx::polar(sp * DT, rot[2], -rot[1]);
        let (lo, hi) = if small {
            let k = w.rng.randf(0.5, 1.5);
            v = c::scale(v, k);
            (f32::from_bits(0x3e75_c28f), f32::from_bits(0x3eb8_51ec))
        } else {
            (f32::from_bits(0x3f4c_cccd), f32::from_bits(0x3f99_999a))
        };
        let k = w.rng.randf(lo, hi);
        let t1 = w.rng.randf(0.5, 1.0);
        let c1 = crate::hud::tween_color(t1, COLOURS[0], COLOURS[1]);
        let t2 = w.rng.randf(0.25, 0.5);
        let c2 = crate::hud::tween_color(t2, COLOURS[1], COLOURS[1] & 0xff00_0000);
        let life = (life_s * 60.0 * w.svc.timing.timer_scale.to_f32()) as i32;
        part64(w, &type64::Spawn { size: size * k, floor, g: GRAVITY * DT2, pos: p, vel: v, life: life as i16, rgba: c1, rgba2: c2, additive: small });
    }
}
