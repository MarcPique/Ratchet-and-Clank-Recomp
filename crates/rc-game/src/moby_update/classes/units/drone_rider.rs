//! **The drone's rider, class 1371** (created by code only): its spawner level12 `0x308670`, its knock-off
//! `0x308708` and its update `0x308918`; the same three functions on level15 (`0x2ea338` / `0x2ea3d0` / `0x2ea5e0`,
//! clusters of two): **one shared unit**. Hoven's drones 326 ([`super::hoven_drone`]) carry one each (posed on them every
//! tick while they live, `mode |= 6`); when a drone dies (or the carrier's death camera starts) the rider is knocked off:
//! it flies off backwards, spinning, and blows up where it lands (or vanishes below z 5). A new consumer (Quartu's
//! carrier class) calls [`spawn`] / [`knock_off`]. Read from the level12 decomp and disassembly. Native `f32`.
//!
//! **Pvar block** (0x80): +0x00 the creature header (+0x10 → the knock record), +0x20 the knock record
//! (`creature::knock`).
//!
//! ## Coverage
//! | address | what | port |
//! |---|---|---|
//! | 0x308670 | `CreateMoby(1371)` (none: nothing); update distance 0xff, +0x31 = 1, draw distance 0x7f, state 0; the drone's position and Euler; +0xb8 = the drone; the header zeroed, +0x10 → +0x20; `MobyBuildMatrix` | [`spawn`] |
//! | 0x308708 | `randi(3)` = 0 → `PlayClassSound(1, 0x20, rider)` | [`knock_off`] |
//! | 0x308708 | K: height 0.7, radius 717 (×1024), +0x6c = 0; update distance 0xff; mode &= ~6; angle = yaw + π; the push (cos, sin, 1, 5627.98); gravity 40·dt², drag 100·dt², up = √(160·dt²), speed = 3 / (2·up / (40·dt²)) + 100·dt²·½·(2·up / (40·dt²)), +0x5d = 0, flags 1; `0x273a38` (aim) and `0x275448(angle, rider, K, 1, 5, 2)` (the flight); air speed 2·dt, keys −1 / −1; mode &= ~0x1000; `MobyAnimBlend(rider, 1, 0, 0)` | [`knock_off`] (`knock::aim`, `knock::start`) |
//! | 0x308918 | sequence 1 only: `0x275588` (the flight's tick), rot.x / rot.y += 250°·dt, mode &= ~0x1000 | [`update`] (`knock::update`) |
//! | 0x308918 | the flight ended (bits 0x61): `SpawnBeamExplosion(0, 0, 2, 1, 9, 1, 15, rider, K, pos + (0, 0, 1), 5, 2, 4, 0, 1, 1, −1, 0)`, `DeleteMoby`; else below z 5 → `DeleteMoby` | [`update`] (`fx::beam_explosion`) |

use crate::moby_runtime::{mode, MobyId};
use crate::moby_update::creature::fx::{beam_explosion, Beam};
use crate::moby_update::creature::knock;
use crate::moby_update::creature::{self as c, add_rot, DT, DT2};

use crate::moby_update::services::World;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_8918;
pub const SPAWN_FN: u32 = 0x30_8670;
pub const KNOCK_FN: u32 = 0x30_8708;
pub const CLASS: i16 = 0x55b;
pub const CLASSES: [i16; 1] = [CLASS];

/// The knock record.
const K: usize = 0x20;
const LEN: usize = 0x80;

/// The landing blast.
const BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 2.0, flash2: 1.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 5, sparks: 2, puffs: 4, debris: 1, sound: 0, shake: true };

/// Level12 `0x308670(drone)` (module doc).
pub fn spawn(w: &mut World, drone: MobyId) -> Option<MobyId> {
    let id = w.create_moby(CLASS)?;
    crate::moby_update::story::pvars(w, id, LEN);
    let (pos, rot) = (w.m(drone).position, w.m(drone).rotation);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.visible = 1;
        m.draw_dist = 0x7f;
        m.state = 0;
        m.position = pos;
        m.parent = Some(drone);
        m.rotation = rot;
        m.pvars[..K].fill(0);
    }
    c::set_pi32(w, id, 0x10, K as i32);
    w.build_matrix(id);
    Some(id)
}

/// Level12 `0x308708(drone, rider)` (module doc).
pub fn knock_off(w: &mut World, _drone: MobyId, id: MobyId) {
    crate::moby_update::story::pvars(w, id, LEN);
    if w.rng.randi(3) == 0 { w.play_sound(1, 0x20, id); }
    use knock::k;
    c::set_pf(w, id, K + k::ZOFF, f32::from_bits(0x3f33_3333));
    c::set_pi32(w, id, K + k::RADIUS, f32::from_bits(0x4433_3333) as i32);
    c::set_pf(w, id, K + k::AIR_SPEED, 0.0);
    w.mm(id).update_dist = 0xff;
    w.mm(id).mode &= !(mode::NO_UPDATE | mode::KEEP_MATRIX);
    let a = add_rot(w.m(id).rotation[2], std::f32::consts::PI);
    let dir = [a.cos(), a.sin(), 1.0, f32::from_bits(0x45af_df66)];
    let g = DT2 * 40.0;
    let drag = DT2 * 100.0;
    c::set_pf(w, id, K + k::GRAVITY, g);
    c::set_pf(w, id, K + k::DRAG, drag);
    let r = (g * 4.0).abs().sqrt();
    let r2 = (DT2 * 40.0 * 4.0).abs().sqrt();
    let speed = 3.0 / ((r + r) / g) + DT2 * 100.0 * 0.5 * ((r2 + r2) / g);
    c::set_pf(w, id, K + k::SPEED, speed);
    c::set_pu8(w, id, K + 0x3d, 0);
    c::set_pf(w, id, K + k::UP, (g * 4.0).abs().sqrt());
    c::set_pi32(w, id, K + k::FLAGS, 1);
    let mut sp = c::pf(w, id, K + k::SPEED);
    let mut up = c::pf(w, id, K + k::UP);
    let angle = knock::aim(dir, &mut sp, &mut up);
    c::set_pf(w, id, K + k::SPEED, sp);
    c::set_pf(w, id, K + k::UP, up);
    knock::start(w, id, K, angle, 1, 5, 2);
    c::set_pf(w, id, K + k::KEY_LAND, -1.0);
    c::set_pf(w, id, K + k::KEY_APEX, -1.0);
    c::set_pf(w, id, K + k::AIR_SPEED, DT + DT);
    w.mm(id).mode &= !mode::TARGETABLE;
    w.anim_blend(id, 1, 0, 0);
}

/// Level12 `0x308918` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.m(id).anim.seq_b != 1 || w.m(id).pvars.len() < LEN { return; }
    let r = knock::update(w, id, K);
    let m = w.mm(id);
    m.rotation[0] = add_rot(m.rotation[0], DT * f32::from_bits(0x408b_a058));
    m.rotation[1] = add_rot(m.rotation[1], DT * f32::from_bits(0x408b_a058));
    m.mode &= !mode::TARGETABLE;
    if r & 0x61 == 0 {
        if w.m(id).position[2] < 5.0 { w.delete_moby(id); }
        return;
    }
    let mut p = w.m(id).position;
    p[2] += 1.0;
    beam_explosion(w, &BLAST, Some(id), p);
    w.delete_moby(id);
}
