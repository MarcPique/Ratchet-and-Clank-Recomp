//! Novalis's cave drips, class 787 (level 01): spawned by the ripple manager 751 (`0x2fd0e8`) while the camera is in
//! zone 0 or 6, one every `rand_range(300, 1200)` ticks from one of the five points of the drip table 0x1fa650
//! (`crate::water::RippleSim::drip_due`), through the spawner level01 `0x2ffcd0` ([`spawn`]); the update `0x2ffdc0`
//! ([`update`]) drops it under gravity until its line hits the world: a water face (surface 0) gives the drip's
//! splash and lets it sink on, any other face deletes it. Read from the level01 disassembly of both functions. Native
//! `f32`.
//!
//! **Pvar block**: +0x00 the velocity (quad), +0x10 the gravity (subtracted from vz every tick), +0x14 s32 the life
//! (ticks), +0x18 s32 hit the water, +0x1c the spin (added to rotation x every tick).
//!
//! | address | what | port |
//! |---|---|---|
//! | 751 `0x2fd0e8` | zone 0 or 6 and `--gp−0x4f7c < 1` → `randi(5)` point, x + `randf(±0.15)`, y + `randf(±0.15)`, `0x2ffcd0(0.2, point)`, timer = `rand_range(300, 0x4b0)` | `RippleSim::drip_due` / `crate::water::managers` (751) / [`spawn`] |
//! | 0x2ffcd0 | `CreateMoby(0x313)` (none: nothing, no draws) | [`spawn`] |
//! | | `FUN_00272078`: Ratchet's light word (+0x38) | [`spawn`] |
//! | | rotation x = 0, draw distance 0x40, update distance 0xff, scale × 0.2, rotation z = `rand_angle` | [`spawn`] |
//! | | position = the point (x, y, z, w); velocity = 0 then vz = −dt; gravity = 15·dt²; life 0x78; spin = `randf(0x3b4de32f, 0x3d80adfd)` (0.00314 .. 0.0628); `MobyBuildMatrix` | [`spawn`] |
//! | 0x2ffdc0 | vz −= gravity; rotation x += spin; to = position + velocity (`vec_add` 0x221188) | [`update`] |
//! | | `CollLine_Fix(position, to, 2, 0, 0)`: no hit → position = to | [`update`] |
//! | | hit, `coll_type` (0x2151d8) ≠ 0 → `DeleteMoby`, return (the position is not moved) | [`update`] |
//! | | hit water, not yet hit (+0x18 = 0): `PlayClassSound(0, 0, self)` (0x2a1618) | [`update`] |
//! | | `RippleDisturb(x, y, 0.5, −0.35, 0x1612d0, [0x1612d8] (= gp−0x5928), additive)` | [`update`] (`bomb_water::ripple`) |
//! | | life = 0x78, hit = 1, spin = 0, vz = −1.5·dt, gravity = 0.8·dt² | [`update`] |
//! | | the splash 775 (`0x2ff768(2, (x, y, hit z))`), alpha 0x70; 16 type-35 drops (`rand_angle`, `randf(0, 3dt)` out, `randf(3dt, 6.5dt)` up, `rand_range(0x5a, 0x78)` life, `randi(2)` kind) | [`update`] (`bomb_water::entry`: the same calls) |
//! | | position = to (after a water hit too: the drip sinks on) | [`update`] |
//! | | hit set → scale × 0.99 (0x3f7d70a4) | [`update`] |
//! | | `FastDecTimer(+0x14)` out → `DeleteMoby` | [`update`] |
//! | | no hit on Ratchet, no particle besides the splash's, no flag | n/a |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::bomb_water;
use crate::moby_update::creature::{self as c, DT, DT2};
use crate::moby_update::services::{pv, World};

/// The update in the level01 class table.
pub const UPDATE_FN: u32 = 0x2f_fdc0;
pub const REFERENCE_LEVEL: u32 = 1;
/// `CreateMoby(0x313)`.
pub const CLASS: i16 = 787;
pub const CLASSES: [i16; 1] = [CLASS];
/// The scale factor 751 passes (0x3e4ccccd).
pub const SIZE: f32 = 0.2;

pub mod pv {
    pub const VEL: usize = 0x00;
    pub const GRAVITY: usize = 0x10;
    pub const LIFE: usize = 0x14;
    pub const HIT: usize = 0x18;
    pub const SPIN: usize = 0x1c;
}

/// Level01 `0x2ffcd0(size, point)` (module doc).
pub fn spawn(w: &mut World, size: f32, at: [f32; 4]) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    if let Some(l) = w.hero_moby.map(|h| w.m(h).light) { w.mm(id).light = l; }
    let a = w.rng.rand_angle();
    {
        let m = w.mm(id);
        m.rotation[0] = 0.0;
        m.draw_dist = 0x40;
        m.update_dist = 0xff;
        m.scale *= size;
        m.rotation[2] = a;
        m.position = at;
    }
    c::set_pv4(w, id, pv::VEL, [0.0, 0.0, -DT, 0.0]);
    c::set_pf(w, id, pv::GRAVITY, DT2 * 15.0);
    c::set_pi32(w, id, pv::LIFE, 0x78);
    let spin = f32::from_bits(w.rng.randf_bits(0x3b4d_e32f, 0x3d80_adfd));
    c::set_pf(w, id, pv::SPIN, spin);
    w.build_matrix(id);
    Some(id)
}

/// Level01 `0x2ffdc0` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x20 { return; }
    let mut vel = c::pv4(w, id, pv::VEL);
    vel[2] -= c::pf(w, id, pv::GRAVITY);
    c::set_pv4(w, id, pv::VEL, vel);
    let spin = c::pf(w, id, pv::SPIN);
    w.mm(id).rotation[0] += spin;
    let p = w.m(id).position;
    let to: [f32; 4] = std::array::from_fn(|k| p[k] + vel[k]);
    if let Some(hit) = w.coll_line(pv(p), pv(to), 2, None) {
        if hit.surface_id() != 0 {
            w.delete_moby(id);
            return;
        }
        if c::pi32(w, id, pv::HIT) == 0 { w.play_sound(0, 0, id); }
        bomb_water::ripple(w, p[0], p[1], 0.5, f32::from_bits(0xbeb3_3333), true);
        c::set_pi32(w, id, pv::LIFE, 0x78);
        c::set_pi32(w, id, pv::HIT, 1);
        c::set_pf(w, id, pv::SPIN, 0.0);
        let mut v = c::pv4(w, id, pv::VEL);
        v[2] = DT * -1.5;
        c::set_pv4(w, id, pv::VEL, v);
        c::set_pf(w, id, pv::GRAVITY, DT2 * 0.8);
        bomb_water::splash_and_drops(w, 2.0, [p[0], p[1], hit.point[2], p[3]]);
    }
    w.mm(id).position = to;
    if c::pi32(w, id, pv::HIT) != 0 { w.mm(id).scale *= f32::from_bits(0x3f7d_70a4); }
    let mut life = c::pi32(w, id, pv::LIFE);
    let out = c::dec_timer_i32(&mut life);
    c::set_pi32(w, id, pv::LIFE, life);
    if out != 0 { w.delete_moby(id); }
}
