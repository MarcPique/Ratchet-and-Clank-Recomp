//! **Gemlik's ship, class 69** (level13 `0x2bb068`, census U424, one instance): the flown ship of the Gemlik base
//! battle. Ratchet presses △ beside it ("Enter"), the screen fades, he is placed in it (hero state 0x32, hidden) and
//! flies it with the chase camera: the stick steers, ✕ boosts, □ / L1 fire the lasers 1009 ([`super::ship_laser`]),
//! ○ / R1 the homing missiles 295 ([`super::ship_missile`]) at the HUD's lock. Walls destroy it; other mobys scrape it;
//! the level's attackers' shots hit it (the ridden-vehicle record's health, [`crate::vehicle`]). The HUD
//! ([`super::gemlik_ship_hud`], the draw callback `0x2b97f8`) counts the targets left; when none are left the ship
//! lands: the mission done, Ratchet placed at the exit, the checkpoint. "Quit?" (the freeze menu's kind 1) puts him
//! back at the start. Read from the level13 decomp (`0x2bb068` and its helpers `0x2ba178` / `0x2ba088` / `0x2b9f58` /
//! `0x2b9df8` / `0x2b84e8` / `0x2b8590`, the level's names mapped to level 01 with `rc-trace overlay-diff`), the level
//! data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (no other copy), on the shared mechanisms: the vehicle record
//! ([`crate::vehicle::Record`]: the HUD, the pause triggers, the water managers' pause, Qwark's ship 388 read it), the
//! mounted hero state 0x32, the script camera (`cinematic::{camera_script, camera_targets}`), the view tangent
//! ([`crate::moby_update::Services::view_tan_x`]), the occlusion fallback ([`crate::moby_update::Services::occlusion_fallback`]),
//! the group / class-list setters (`story::{group_set, class_list_set, class_state_set}`), the shared shots.
//!
//! **Pvar block** (0x140; the cuboid / group words come with the instance): +0x00 the mount's base velocity, +0x10 the
//! heading (unit), +0x20..+0x28 the camera Euler (sprung), +0x30 / +0x34 / +0x38 roll / pitch / yaw, +0x40 the start
//! position, +0x50 the start rotation, +0x60 (u8) the camera view, +0x61 (u8) the gun side, +0x62 (s16) the wreck
//! timer, +0x64 the speed, +0x68 (s16) the start-up count, +0x6a (u16) the missile joint, +0x6c / +0x70 the stick's
//! sprung x / y, +0x7c the fade, +0x80 / +0x84 the gun / missile cooldowns, +0x88 the lock target, +0x8c the lock
//! timer, +0x90 / +0x94 / +0x98 spring rates, +0xa0..+0xa8 the camera Euler's rates, +0xac / +0xb0 the field of view
//! (radians), +0xb4..+0xbc the camera offset, +0xc0 the FOV rate, +0xc4 the boost fraction, +0xc8..+0xd0 the offset's
//! rates, +0xdc the scrape timer, +0xe0 / +0xe4 the crosshair (screen pixels), +0xe8 the targets left, +0xec the missile
//! target, +0xf0 / +0xf4 the HUD gauge, +0xf8 the landing timer, +0xfc the exit cuboid, +0x100 the landing cuboid,
//! +0x104 the HUD texture set, +0x108 / +0x10c the engine / boost voices, +0x110 the steering cuboid, +0x114 the off-screen
//! arrow's pulse, +0x118 the lock-lost count, +0x11c / +0x13c the groups hidden while flying, +0x120 the last attacker's
//! class, +0x124 the lock's class, +0x128 −1, +0x12c the quit cuboid, +0x130 the mount cuboid, +0x134 the mount area
//! cuboid, +0x138 the lock time.
//!
//! **Level data** (the overlay): the default steering point 0x161430 (512, 512, 320); the class lists 0x1cbde8
//! ([`SCENERY`], 65 classes) and 0x161440 ([`SHOWN`]); the exhaust descriptors 0x1613d8.. ([`EXHAUST`]).
//!
//! ## Coverage (`0x2bb068`)
//! | address | what | port |
//! |---|---|---|
//! | entry | Ratchet in 0x72 → nothing | [`update`] |
//! | entry | +0x120 = 0; `FastDecTimer(+0xdc)` | [`update`] |
//! | entry | state not 1 / 2 / 6 / 7: `MobyGetHitMessage(m, 0x30001, 0)`; in state 4: damage below the health → health −= it, the attacker of class 0x52 / 0x53 → +0x120 = its class; the template's +0x20 & 1 → the up shake (1.0, `ticks(4)`), the hit's push clamped to 3·dt added to the position, the velocity reflected off it and yaw / pitch turned toward it by SPEED·0.1·damage·0.05 (pitch the game's +atan) | [`hits`] (`World::shake_camera`) |
//! | entry | the fatal hit: +0x120 = class + 10000 (0x52 / 0x53), the up shake (0.2, `ticks(5)`), health 0, mode \|= 0x41, `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, v, m, 10, 3, 16, 6, 1)`, collision off (+0x94 = 0), +0x62 = `ticks(240)`, → 6 | [`hits`] (`fx::beam_explosion`) |
//! | entry | hit slot 0xff; the record names this ship → 0x140958 = `trunc(health / 256 · 200)` | [`update`] |
//! | state 0 | → 1; +0x60 = 0; scale = class scale; +0x20 / +0x24 = +0x30 / +0x34; +0xb0 = +0xac = 2·atan(tan_x); +0xc0 = +0x120 = 0; +0x30.. and +0x50.. = rotation; +0x40.. = position; +0x7c = 0 | [`init`] |
//! | state 0 | the first time, the gauge's CLUT saved to 0x1cbe70 (gp−0x583c) | n/a: the port's gauge draws from the bank's own CLUT ([`super::gemlik_ship_hud`]) |
//! | state 0 | +0xe8 = +0xf8 = 1; the mission done → 9; groups +0x11c / +0x13c (> 0) off (0, 0, 0); [`SCENERY`] (1, 1, −1), [`SHOWN`] (1, −1, −1); class 0x102's mobys → state 0; +0x128 = −1; +0x138 = `ticks(575)` | [`init`] (`story::{group_set, class_list_set, class_state_set}`) |
//! | state 1 | the mission done → 9; Ratchet in the mount area +0x134 → +0x7c = fade = 0.99, mount; else within 2 (`FUN_00265210`) → `try_set_help_message(7, 0x53e4)`, △ pressed (0x13cae4 & 0x10) → mount when the prompt was taken | [`wait`] (`interact::try_prompt`, `pickup::sphere_gap`) |
//! | state 1 | mount: the reset of state 0's camera / FOV / heading words, +0xe8 = +0xf8 = 1, → 2, +0x128 = −1; else +0x7c ≠ 0 → `Approach(0, 4·dt)`, the fade | [`wait`] (`cinematic::set_fade`) |
//! | state 1 | the blob shadow (`0x266070(1.5, m)` = L01 `0x26eec8`) | [`wait`] (`crate::shadows::blob`) |
//! | state 2 | `Approach(1, 4·dt, +0x7c)`, the fade; at 1: the mount cuboid +0x130 → `HeroTeleport(its centre, Euler, 0x32, 1)`, position / rotation = it; `SetState(0x32, 1)`; `CameraScript(pos, rot, 0, 0, 0)` | [`mount`] |
//! | state 2 | +0x68 = +0x6c = +0x70 = +0x84 = +0x80 = 0, speed = SPEED·0.1; distances 0xff; Ratchet's moby +0x98 = −1; +0x20.. = rotation; the rates 0 | [`mount`] |
//! | state 2 | the record: class, health 255, missiles 10 / 20, 0x140948 = 20, 0x14095e = 1, the moby, 0x14094b / 49 / 4a = 3, quit 0, 256, 100; +0xf4 = 0xff, +0xf0 = 0; +0x00 = polar(SPEED·0.1, yaw, −pitch) | [`mount`] ([`crate::vehicle::Record::take`]) |
//! | state 2 | groups on (1, 1, 1); [`SCENERY`] (0, 0, −1), [`SHOWN`] (0, −1, −1); class 0x102 → 0; z + 3; scale = class scale / 4; Ratchet hidden (0x1413f5); the engine voice (`PlayClassSound(0, 4)`) unless held; 0x15f608 = 2; the flight; the help box suspended (`0x225a28`); `MusicRequestTrack(2, 4)`; → 4 | [`mount`] |
//! | state 4 | +0x7c ≠ 0 → `Approach(0, 4·dt)`, the fade; `force_help_message(6, 0)`; 0x15f608 = 2; the engine voice restarted when gone; the flight; quit (0x14095f & 1) → 5; +0xe8 ≤ 0 → +0xf8 = `ticks(1200)`, → 8 | [`update`] |
//! | state 5 | Ratchet shown, his moby's +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`; `FUN_0024b090(0x14095c, 0)`; position / rotation = the start; scale; `fun_0020e098`; groups off, the lists back (1, 1, −1) / (1, −1, −1), class 0x102 → 0; `HeroTeleport` to the quit cuboid +0x12c, or the start position with the exit cuboid's Euler; the voices released; `MusicRequestTrack(0, 5)`; fade 0.99; → 1 | [`quit`] (`FUN_0024b090` on the record's handle: n/a, nothing writes 0x14095c on Gemlik [L]) |
//! | state 6 | 0x15f608 = 2; the voices released; → 7 | [`update`] |
//! | state 7 | 0x15f608 = 2; `FastDecTimer(+0x62)` out with the dialogue player idle (0x151720 = 0, 0x1516ec = −1): the death sequence (`0x2319b0`), `CameraScript2(0)`, the start position / rotation, `fun_0020e098`, the voices released, → 0 | [`update`] (`HeroCall::Death`) |
//! | state 8 | Ratchet shown, moby +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`; at the landing cuboid +0x100 (0.57318 up) or the start; scale; `fun_0020e098`; with the exit cuboid +0xfc: groups off, the lists back, class 0x102 → 0, `HeroTeleport` there; `SetMissionDone(+0xb0)`; the voices released; the checkpoint at Ratchet (`0x29ac10(0x13f3d0, 0x13f3e0)`); → 9 | [`land`] (`checkpoint::record`; the `STUB_printf`s n/a) |
//! | state 9 | `DeleteMoby` | [`update`] |
//! | tail | the position clamped to [15, 1008]; in state 4 Ratchet's position / rotation (0x13f3d0 / 0x13f3e0) = the ship's position and +0x30..; states 3, 4 and 8: the exhaust | [`update`] (`HeroFields::pose`: the yaw only, the hero block has no pitch / roll), [`exhaust`] |
//!
//! ## Coverage (the helpers)
//! | address | what | port |
//! |---|---|---|
//! | 0x2ba178 | mode &= ~0x100; +0xbc ≠ 0 (held by Qwark's ship) → speed 0.0001 | [`fly`] |
//! | 0x2ba178 | +0x68 > `ticks(120)`: ✕ pressed (raw 0x13caf4 & 0x40) → the boost voice (`PlayClassSound(1, 4)`) after releasing the last, ✕ released → released; ✕ held → speed + SPEED·0.035 up to 30·dt; else toward 24·dt (+0.025·SPEED, or 0.1 of the gap above); else +0x68 + 1, speed + SPEED·0.003 | [`fly`] |
//! | 0x2ba178 | +0x68 > `ticks(5)`: the stick springs (`Spring(stick, 0.03, 1, k)`, k = SPEED·0.1 / 0.25, L3 held: 0.08 with the stick rescaled by 0.0157/0.0123), the roll `SpringAngle(+0x6c, 0.05, 1, SPEED)`, pitch / yaw −= the springs · SPEED·0.01227; the pitch and rot.y clamped to ±80° | [`fly`] (`hero::physics::{spring, turn_spring}`) |
//! | 0x2ba178 | the steering limit `0x2ba088` toward the steering cuboid +0x110's centre (default 0x161430): the yaw and rot.z by the xy distance from the default point (50..\|rows 0 + 1\|), the pitch and rot.y by the height (50..\|row 2\|), power 5 | [`fly`], [`steer`] |
//! | 0x2ba178 | v = polar(speed, yaw, −pitch); +0x10 = unit(v); +0xbc = 0 → position += v | [`fly`] |
//! | 0x2ba178 | the camera: the boost fraction (+0xc4); the view table (chase: (−15 − y − 4f, −3x, 2y + 4)/4, cockpit (−1.275, 0, 0.65)) at +0x60; the offset sprung (`Spring(·, 0.15, 0.8, SPEED)`, set outright in state 2); through the Euler (+0x30) rows; `0x316dd0`; the Euler (roll/2 + x/32, pitch + 5.625° − y·0.0123, yaw) sprung (`SpringAngle(·, 0.15, 0.8)`, set in state 2); `0x314e98` (= L01 `0x316e28`); the current camera not 5 → `CameraScript(eye, Euler, 0, 0, 0)` | [`fly`] (`cinematic::{camera_targets, camera_script_unless_script}`) |
//! | 0x2ba178 | the FOV: `Spring(f·20° + 65°, 0.05, 1, 1, +0xb0)`; 0x16cdf0 = tan(+0xb0 / 2); `UpdateViewContext` | [`fly`] (`Services::view_tan_x`) |
//! | 0x2ba178 | rotation = (roll + x/2, pitch − y/2, yaw) | [`fly`] |
//! | 0x2ba178 | `coll_sphere(1.2, pos, 0, m)` touching the world or a moby not of class 0, 0x192, 0x4c2, 0x4c3, 0x52, 0x53: position = the push; v reflected; yaw / pitch turned 0.1 toward it; speed ·= 0.7; the world → health 0, `SpawnBeamExplosion(8, 100, 16, 8, 9, 1, 50, m, …, m, 30, 10, 24, 6, 1)`, mode \|= 0x41, +0x62 = `ticks(240)`, → 6; a moby with the scrape timer out → health − 5, below 0 the same explosion (no mode store), +0x62, → 6; the timer `ticks(10)` | [`fly`] (the explosion's direction words `0x1f9a28` / normalisations: n/a, the port's explosion takes none) |
//! | 0x2ba178 | `RegisterDrawCallback(0x2b97f8)`; the missiles `0x2b9f58`; the guns `0x2b9df8(cam yaw − x/7, −(cam pitch − y/7 − 0.1))` | [`fly`] (`Callback::UnitFrame`) |
//! | 0x2ba088 | `(v, cur, target, lo, hi, angle, n)`: lo < v → f = clamp((v − lo)/(hi − lo), 0, 1) squared n − 1 times; cur moves f of the way to target (wrapped for an angle) | [`steer`] |
//! | 0x2b9f58 | `FastDecTimer(+0x84)` out, ○ / R1 held (raw & 0x28), missiles left: +0x6a = (+0x6a + 1) & 3; joint list +0x6a + 7's point + unit(+0x00)·0.3; `0x2e6a58(100·dt, m, p, +0xec, rotation, ticks(300))`; made → `PlayClassSound(2, 0)`, one missile spent, +0x84 = `ticks(30)` | [`missiles`] (`ship_missile::spawn`) |
//! | 0x2b9df8 | aim = polar(400·dt, yaw, pitch) + +0x00; `FastDecTimer(+0x80)` out with □ / L1 held (raw & 0x84): `PlayClassSound(3, 0)`; the muzzle = joint list (+0x61 & 1) + 3's point + unit(aim)·2.2; `0x3025f8(200, −1, m, aim, muzzle)`; +0x80 = `ticks(4)`; +0x61 ^= 1 | [`guns`] (`ship_laser::spawn`) |
//! | 0x2b9df8 | the crosshair: unit(aim)·23 + position through `0x2b84e8` → +0xe0 / +0xe4 | [`guns`], [`screen_point`] |
//! | 0x2b84e8 | `FUN_00202620` (the world → screen transform), (x − 0x13e510)/16, (y − 0x13e514)/16 + k | [`screen_point`] (the camera of the tick: `World::camera`, its Euler, `view_tan_x`) |
//! | 0x2b8590 | joint lists 1 and 2: f = clamp(speed / 24·dt, 0.35, 1); three copies of the point jittered by (0.02 + 0.02·b, 0.02 + 0.02·b, 0.02 + 0.01·b) (b = +0xc4); for each `PartType74Spawn(m, p, (−(randf(6, 7) + k·b)·dt·f, 0, 0), desc, 1)` with k = 2, 3, 4 and the descriptors 0x1613d8 / e8 / f8; b > 0.5: again jittered (0.01 + …) and three more, then 0x161408 (k 2) and 0x161418 at the joint point itself (k 3) | [`exhaust`] (`particles::type74`) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, set_len3, sub_rot, DT, SPEED};
use crate::moby_update::services::{fl, pf as to_pf, HeroCall, HeroPose, World};
use crate::moby_update::story;
use crate::ps2v::Pf;

pub const REFERENCE_LEVEL: u32 = 13;
pub const UPDATE_FN: u32 = 0x2b_b068;
pub const CLASS: i16 = 0x45;
pub const CLASSES: [i16; 1] = [CLASS];

pub mod pvo {
    pub const BASE_VEL: usize = 0x00;
    pub const HEADING: usize = 0x10;
    pub const CAM_EULER: usize = 0x20;
    pub const ROLL: usize = 0x30;
    pub const PITCH: usize = 0x34;
    pub const YAW: usize = 0x38;
    pub const START_POS: usize = 0x40;
    pub const START_ROT: usize = 0x50;
    pub const VIEW: usize = 0x60;
    pub const GUN_SIDE: usize = 0x61;
    pub const WRECK_T: usize = 0x62;
    pub const SPEED: usize = 0x64;
    pub const STARTUP: usize = 0x68;
    pub const MISSILE_JOINT: usize = 0x6a;
    pub const STICK_X: usize = 0x6c;
    pub const STICK_Y: usize = 0x70;
    pub const FADE: usize = 0x7c;
    pub const GUN_T: usize = 0x80;
    pub const MISSILE_T: usize = 0x84;
    pub const LOCK: usize = 0x88;
    pub const LOCK_T: usize = 0x8c;
    pub const STICK_XV: usize = 0x90;
    pub const STICK_YV: usize = 0x94;
    pub const ROLL_V: usize = 0x98;
    pub const CAM_EULER_V: usize = 0xa0;
    pub const FOV0: usize = 0xac;
    pub const FOV: usize = 0xb0;
    pub const CAM_OFF: usize = 0xb4;
    pub const FOV_V: usize = 0xc0;
    pub const BOOST: usize = 0xc4;
    pub const CAM_OFF_V: usize = 0xc8;
    pub const SCRAPE_T: usize = 0xdc;
    pub const CROSS_X: usize = 0xe0;
    pub const CROSS_Y: usize = 0xe4;
    pub const TARGETS: usize = 0xe8;
    pub const MISSILE_TARGET: usize = 0xec;
    pub const GAUGE: usize = 0xf0;
    pub const GAUGE_CUT: usize = 0xf4;
    pub const LAND_T: usize = 0xf8;
    pub const EXIT_CUBOID: usize = 0xfc;
    pub const LAND_CUBOID: usize = 0x100;
    pub const HUD_SET: usize = 0x104;
    pub const ENGINE_VOICE: usize = 0x108;
    pub const BOOST_VOICE: usize = 0x10c;
    pub const STEER_CUBOID: usize = 0x110;
    pub const ARROW_PHASE: usize = 0x114;
    pub const LOCK_LOST: usize = 0x118;
    pub const GROUP_A: usize = 0x11c;
    pub const ATTACKER: usize = 0x120;
    pub const LOCK_CLASS: usize = 0x124;
    pub const F128: usize = 0x128;
    pub const QUIT_CUBOID: usize = 0x12c;
    pub const MOUNT_CUBOID: usize = 0x130;
    pub const MOUNT_AREA: usize = 0x134;
    pub const LOCK_TIME: usize = 0x138;
    pub const GROUP_B: usize = 0x13c;
    pub const LEN: usize = 0x140;
}

/// 0x1cbde8: the classes shown while Ratchet is on foot and hidden while he flies (update and drawing).
pub const SCENERY: [i16; 65] = [
    11, 13, 14, 15, 16, 21, 29, 36, 63, 66, 70, 91, 101, 129, 130, 170, 186, 213, 214, 222, 223, 225, 226, 229, 230, 244, 258, 270, 315, 360, 479, 500,
    501, 505, 511, 558, 615, 667, 726, 750, 758, 803, 605, 805, 806, 832, 1134, 1135, 1137, 1143, 1185, 1268, 1261, 1262, 1270, 1438, 1447, 1449, 1456,
    1457, 1458, 1459, 1464, 1558, 1632,
];
/// gp−0x57c0 (0x161440): the classes updated only while Ratchet is on foot.
pub const SHOWN: [i16; 5] = [127, 128, 182, 183, 304];
/// The class whose mobys are set to state 0 at every mount and dismount.
pub const RESET_CLASS: i16 = 0x102;
/// gp−0x57d0 (0x161430): the default steering point.
pub const STEER_POINT: [f32; 3] = [512.0, 512.0, 320.0];
/// The "Enter" prompt.
const PROMPT: i32 = 0x53e4;

/// The exhaust descriptors 0x1613d8, 0x1613e8, 0x1613f8, 0x161408, 0x161418 (type 74).
pub const EXHAUST: [crate::particles::type74::Desc; 5] = {
    use crate::particles::type74::Desc;
    [
        Desc { rgba0: 0xcf00_00ff, rgba1: 0x0000_00cf, size_range: -10, size_base: 30, life: 10 },
        Desc { rgba0: 0x6000_ffff, rgba1: 0x0000_0080, size_range: -10, size_base: 50, life: 12 },
        Desc { rgba0: 0xefff_7f4f, rgba1: 0x00ff_0000, size_range: -20, size_base: 20, life: 8 },
        Desc { rgba0: 0x6000_80ff, rgba1: 0x0000_0080, size_range: -5, size_base: 60, life: 12 },
        Desc { rgba0: 0xefff_7f00, rgba1: 0x00ff_0000, size_range: -20, size_base: 30, life: 10 },
    ]
};

/// The record's values at the mount.
const MOUNT: crate::vehicle::Mount = crate::vehicle::Mount { missiles: 10, missiles_max: 20, b48: 20, b49: 3, b4a: 3, b4b: 3, health: 255.0, health_max: 256.0, f54: 100.0, b5e: 1 };

/// The classes a scrape ignores (Ratchet, the level's own pieces and the fighters' shots).
const NO_SCRAPE: [i16; 6] = [0, 0x192, 0x4c2, 0x4c3, 0x52, 0x53];
/// The attackers whose class the ship remembers (+0x120).
const FIGHTER_SHOTS: [i16; 2] = [0x52, 0x53];

/// The pad bits (the game's order): △ pressed, ✕, ○ | R1, □ | L1, L3.
mod pad {
    pub const TRIANGLE: u32 = 0x10;
    pub const CROSS: u32 = 0x40;
    pub const MISSILE: u32 = 0x28;
    pub const GUN: u32 = 0x84;
    pub const L3: u32 = 0x200;
}

const DEG80: f32 = 1.396_263_4;
const DEG80_B: f32 = f32::from_bits(0x3fb2_b8c2);
/// SPEED·0.012271847 (0.703°) a unit of stick a tick.
const STEP: f32 = 0.012_271_847;

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(pi(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }

fn row(name: u32) -> Option<u16> { super::row(REFERENCE_LEVEL, name) }

fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }

/// `Spring(t, k, d, max, &x, &v)` 0x270780 on two pvar floats.
#[allow(clippy::too_many_arguments)]
pub(super) fn spring(w: &mut World, id: MobyId, t: f32, k: f32, d: f32, max: f32, x: usize, v: usize) {
    let (mut a, mut b) = (Pf::f(pf(w, id, x)), Pf::f(pf(w, id, v)));
    crate::hero::physics::spring(Pf::f(t), Pf::f(k), Pf::f(d), Pf::f(max), &mut a, &mut b);
    set(w, id, x, a.to_f32());
    set(w, id, v, b.to_f32());
}

/// `SpringAngle(t, k, d, max, &a, &v, 0)` 0x270b58 on two pvar floats.
#[allow(clippy::too_many_arguments)]
pub(super) fn spring_angle(w: &mut World, id: MobyId, t: f32, k: f32, d: f32, max: f32, x: usize, v: usize) {
    let (mut a, mut b) = (Pf::f(pf(w, id, x)), Pf::f(pf(w, id, v)));
    crate::hero::physics::turn_spring(Pf::f(t), Pf::f(k), Pf::f(d), Pf::f(max), &mut a, &mut b, 0);
    set(w, id, x, a.to_f32());
    set(w, id, v, b.to_f32());
}

fn release_voice(w: &mut World, id: MobyId, o: usize) {
    let s = pi(w, id, o);
    if s != -1 { w.release_sound(s, id); }
    seti(w, id, o, -1);
}

fn release_voices(w: &mut World, id: MobyId) {
    release_voice(w, id, pvo::ENGINE_VOICE);
    release_voice(w, id, pvo::BOOST_VOICE);
}

fn fade(w: &mut World, id: MobyId, to: f32) {
    let mut f = pf(w, id, pvo::FADE);
    c::turn::approach(to, DT * 4.0, &mut f);
    set(w, id, pvo::FADE, f);
    crate::cinematic::set_fade(w, f);
}

fn ratchet_collision(w: &mut World, bits: u32) { if let Some(h) = w.hero_moby { w.mm(h).coll_disable = bits; } }

/// Groups +0x11c / +0x13c and the class lists: `on` the flight's settings, else the walk's.
fn scenery(w: &mut World, id: MobyId, flying: bool) {
    let v = flying as i32;
    for o in [pvo::GROUP_A, pvo::GROUP_B] {
        let g = pi(w, id, o);
        if 0 < g { story::group_set(w, g, v, v, v); }
    }
    story::class_list_set(w, &SCENERY, 1 - v, 1 - v, -1);
    story::class_list_set(w, &SHOWN, 1 - v, -1, -1);
    story::class_state_set(w, RESET_CLASS, 0);
}

fn class_scale(w: &World) -> f32 { fl(w.class_scale(CLASS)) }

fn restore_start(w: &mut World, id: MobyId) {
    let p = c::pv4(w, id, pvo::START_POS);
    let r = c::pv4(w, id, pvo::START_ROT);
    let m = w.mm(id);
    m.position = p;
    m.rotation = r;
}

/// The camera / FOV / heading words state 0 and the mount reset (module doc).
fn reset_view(w: &mut World, id: MobyId) {
    let s = class_scale(w);
    w.mm(id).scale = s;
    let (r0, r1) = (pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH));
    set(w, id, pvo::CAM_EULER, r0);
    set(w, id, pvo::CAM_EULER + 4, r1);
    let f = atan(1.0, w.svc.view_tan_x);
    set(w, id, pvo::FOV_V, 0.0);
    seti(w, id, pvo::ATTACKER, 0);
    set(w, id, pvo::FOV, f + f);
    set(w, id, pvo::FOV0, f + f);
    let r = w.m(id).rotation;
    c::set_pv4(w, id, pvo::ROLL, r);
}

/// Level13 `0x2bb068` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    if w.hero.state == 0x72 { return; }
    story::pvars(w, id, pvo::LEN);
    seti(w, id, pvo::ATTACKER, 0);
    c::dec_timer_pvar_i32(w, id, pvo::SCRAPE_T);
    hits(w, id);
    w.mm(id).hit_slot = 0xff;
    if w.svc.vehicle.is(id) { w.svc.vehicle.hud = (w.svc.vehicle.health * 0.003_906_25 * 200.0) as i32; }
    match w.m(id).state {
        0 => init(w, id),
        1 => wait(w, id),
        2 => mount(w, id),
        4 => {
            if pf(w, id, pvo::FADE) != 0.0 { fade(w, id, 0.0); }
            w.svc.interact.force_prompt(6, 0);
            w.svc.occlusion_fallback = Some((w.counter, 2));
            let v = pi(w, id, pvo::ENGINE_VOICE);
            if !w.sound_alive(v, id) {
                seti(w, id, pvo::ENGINE_VOICE, -1);
                let s = w.play_sound(0, 4, id);
                seti(w, id, pvo::ENGINE_VOICE, s);
            }
            fly(w, id);
            if w.svc.vehicle.quit & 1 != 0 { w.mm(id).state = 5; }
            if pi(w, id, pvo::TARGETS) <= 0 {
                let t = w.ticks(1200);
                seti(w, id, pvo::LAND_T, t);
                w.mm(id).state = 8;
            }
        }
        5 => quit(w, id),
        6 => {
            w.svc.occlusion_fallback = Some((w.counter, 2));
            release_voices(w, id);
            w.mm(id).state = 7;
        }
        7 => {
            w.svc.occlusion_fallback = Some((w.counter, 2));
            if c::dec_timer_pvar_s16(w, id, pvo::WRECK_T) != 0 && !w.svc.help.voice.busy && w.svc.help.voice.request == -1 {
                w.hero_fields_mut().call(HeroCall::Death);
                crate::cinematic::camera_script2(w, 0);
                restore_start(w, id);
                crate::moby_update::creature::react::sphere_lerp(w, id);
                release_voices(w, id);
                w.mm(id).state = 0;
            }
        }
        8 => land(w, id),
        9 => {
            w.delete_moby(id);
            return;
        }
        _ => {}
    }
    let p = w.m(id).position;
    let cl = |x: f32| x.clamp(15.0, 1008.0);
    w.mm(id).position = [cl(p[0]), cl(p[1]), cl(p[2]), p[3]];
    let st = w.m(id).state;
    if st == 4 {
        let p = w.m(id).position;
        let yaw = pf(w, id, pvo::YAW);
        w.hero_fields_mut().pose = Some(HeroPose { pos: [p[0], p[1], p[2]], yaw, target_yaw: yaw });
    }
    if st == 3 || st == 4 || st == 8 { exhaust(w, id); }
}

/// The hits at the top of `0x2bb068` (module doc).
fn hits(w: &mut World, id: MobyId) {
    let st = w.m(id).state;
    if matches!(st, 1 | 2 | 6 | 7) { return; }
    let Some(h) = w.get_hit(id, 0x3_0001, false) else { return };
    if st != 4 { return; }
    let dmg = fl(h.damage);
    let attacker = h.attacker.map(|a| w.m(a).o_class);
    let (yaw, pitch, speed) = (pf(w, id, pvo::YAW), pf(w, id, pvo::PITCH), pf(w, id, pvo::SPEED));
    if dmg < w.svc.vehicle.health {
        w.svc.vehicle.health -= dmg;
        if let Some(k) = attacker.filter(|k| FIGHTER_SHOTS.contains(k)) { seti(w, id, pvo::ATTACKER, k as i32); }
        if h.w30 & 1 != 0 {
            let t = w.ticks(4);
            w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: 1.0, ticks: t });
            let a = polar(speed, yaw, -pitch);
            let push = c::clamp_len3(crate::moby_update::services::fv(h.dir), DT * 3.0);
            let p = add(w.m(id).position, push);
            w.mm(id).position = p;
            let r = crate::hero::guns::reflect([a[0], a[1], a[2]], [push[0], push[1], push[2]]);
            let k = SPEED * 0.1 * (dmg * 0.05);
            let ny = add_rot(yaw, sub_rot(atan(r[0], r[1]), yaw) * k);
            set(w, id, pvo::YAW, ny);
            let xy = (r[0] * r[0] + r[1] * r[1]).sqrt();
            let np = add_rot(pitch, sub_rot(atan(xy, r[2]), pitch) * k);
            set(w, id, pvo::PITCH, np);
        }
    } else {
        if let Some(k) = attacker.filter(|k| FIGHTER_SHOTS.contains(k)) { seti(w, id, pvo::ATTACKER, k as i32 + 10000); }
        let t = w.ticks(5);
        w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: f32::from_bits(0x3e4c_cccd), ticks: t });
        w.svc.vehicle.health = 0.0;
        w.mm(id).mode |= 0x41;
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: 6, shake: true };
        let p = w.m(id).position;
        fx::beam_explosion(w, &b, Some(id), p);
        w.mm(id).has_collision = false;
        let t = w.ticks(240) as i16;
        c::set_pi16(w, id, pvo::WRECK_T, t);
        w.mm(id).state = 6;
    }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).state = 1;
    c::set_pu8(w, id, pvo::VIEW, 0);
    reset_view(w, id);
    let r = w.m(id).rotation;
    c::set_pv4(w, id, pvo::START_ROT, r);
    let p = w.m(id).position;
    c::set_pv4(w, id, pvo::START_POS, p);
    set(w, id, pvo::FADE, 0.0);
    seti(w, id, pvo::TARGETS, 1);
    seti(w, id, pvo::LAND_T, 1);
    if story::mission_done(w, w.m(id).mission as i32) { w.mm(id).state = 9; }
    scenery(w, id, false);
    seti(w, id, pvo::F128, -1);
    let t = w.ticks(0x23f);
    seti(w, id, pvo::LOCK_TIME, t);
}

/// State 1: waiting beside the ship (module doc).
fn wait(w: &mut World, id: MobyId) {
    let mission = w.m(id).mission;
    if mission != 0xff && story::mission_done(w, mission as i32) {
        w.mm(id).state = 9;
        return;
    }
    let mut go = false;
    let area = pi(w, id, pvo::MOUNT_AREA);
    if area != -1 && story::hero_in(w, area) {
        set(w, id, pvo::FADE, 0.99);
        crate::cinematic::set_fade(w, 0.99);
        go = true;
    } else if let Some(h) = w.hero_moby {
        if crate::moby_update::classes::pickup::sphere_gap(w, id, h) < 2.0 {
            let taken = w.svc.interact.try_prompt(7, PROMPT);
            if w.hero.loop_in.pad.pressed & pad::TRIANGLE != 0 { go = taken != 0; }
        }
    }
    if go {
        reset_view(w, id);
        seti(w, id, pvo::TARGETS, 1);
        seti(w, id, pvo::LAND_T, 1);
        w.mm(id).state = 2;
        seti(w, id, pvo::F128, -1);
    } else if pf(w, id, pvo::FADE) != 0.0 {
        fade(w, id, 0.0);
    }
    // `0x266070(1.5, m)` (= L01 0x26eec8): the blob shadow.
    crate::shadows::blob(w, 1.5, id);
}

/// State 2: the fade out and the mount (module doc).
fn mount(w: &mut World, id: MobyId) {
    fade(w, id, 1.0);
    if pf(w, id, pvo::FADE) != 1.0 { return; }
    if let Some((pos, rot)) = story::cuboid(w, pi(w, id, pvo::MOUNT_CUBOID)) {
        crate::cinematic::hero_teleport(w, pos, rot, 0x32, true);
        let m = w.mm(id);
        m.position = [pos[0], pos[1], pos[2], m.position[3]];
        m.rotation = [rot[0], rot[1], rot[2], m.rotation[3]];
    }
    crate::cinematic::hero_state(w, 0x32, true);
    let (p, r) = { let m = w.m(id); ([m.position[0], m.position[1], m.position[2]], [m.rotation[0], m.rotation[1], m.rotation[2]]) };
    crate::cinematic::camera_script(w, p, r, 0, 0, false);
    c::set_pi16(w, id, pvo::STARTUP, 0);
    set(w, id, pvo::STICK_X, 0.0);
    set(w, id, pvo::STICK_Y, 0.0);
    seti(w, id, pvo::MISSILE_T, 0);
    set(w, id, pvo::SPEED, SPEED * 0.1);
    seti(w, id, pvo::GUN_T, 0);
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
    }
    ratchet_collision(w, 0xffff_ffff);
    let r = w.m(id).rotation;
    c::set_pv4(w, id, pvo::CAM_EULER, r);
    for o in [pvo::CAM_EULER_V, pvo::CAM_EULER_V + 4, pvo::CAM_EULER_V + 8, pvo::CAM_OFF_V, pvo::CAM_OFF_V + 4, pvo::CAM_OFF_V + 8] { set(w, id, o, 0.0); }
    w.svc.vehicle.take(id, CLASS, &MOUNT);
    seti(w, id, pvo::GAUGE_CUT, 0xff);
    set(w, id, pvo::GAUGE, 0.0);
    let v = polar(SPEED * 0.1, pf(w, id, pvo::YAW), -pf(w, id, pvo::PITCH));
    c::set_pv4(w, id, pvo::BASE_VEL, v);
    scenery(w, id, true);
    w.mm(id).position[2] += 3.0;
    let s = class_scale(w) * 0.25;
    w.mm(id).scale = s;
    w.hero_fields_mut().hero_hidden = Some(1);
    if pi(w, id, pvo::ENGINE_VOICE) == -1 {
        let s = w.play_sound(0, 4, id);
        seti(w, id, pvo::ENGINE_VOICE, s);
    }
    w.svc.occlusion_fallback = Some((w.counter, 2));
    fly(w, id);
    w.svc.help.suspend();
    music(w, 2, 4);
    w.mm(id).state = 4;
}

/// State 5: the quit back to the start (module doc).
fn quit(w: &mut World, id: MobyId) {
    w.hero_fields_mut().hero_hidden = Some(0);
    ratchet_collision(w, 0);
    crate::cinematic::hero_state(w, 0, true);
    crate::cinematic::camera_script2(w, 0);
    restore_start(w, id);
    let s = class_scale(w);
    w.mm(id).scale = s;
    crate::moby_update::creature::react::sphere_lerp(w, id);
    scenery(w, id, false);
    let q = pi(w, id, pvo::QUIT_CUBOID);
    if q == -1 {
        let p = c::pv4(w, id, pvo::START_POS);
        let rot = story::cuboid(w, pi(w, id, pvo::EXIT_CUBOID)).map_or([0.0; 3], |c| c.1);
        crate::cinematic::hero_teleport(w, [p[0], p[1], p[2]], rot, 0, true);
    } else if let Some((pos, rot)) = story::cuboid(w, q) {
        crate::cinematic::hero_teleport(w, pos, rot, 0, true);
    }
    release_voices(w, id);
    music(w, 0, 5);
    set(w, id, pvo::FADE, 0.99);
    crate::cinematic::set_fade(w, 0.99);
    w.mm(id).state = 1;
}

/// State 8: the landing (module doc).
fn land(w: &mut World, id: MobyId) {
    w.hero_fields_mut().hero_hidden = Some(0);
    ratchet_collision(w, 0);
    crate::cinematic::hero_state(w, 0, true);
    crate::cinematic::camera_script2(w, 0);
    match story::cuboid(w, pi(w, id, pvo::LAND_CUBOID)) {
        None => restore_start(w, id),
        Some((pos, rot)) => {
            let m = w.mm(id);
            m.position = [pos[0], pos[1], pos[2] + 0.57318, m.position[3]];
            m.rotation = [rot[0], rot[1], rot[2], m.rotation[3]];
        }
    }
    let s = class_scale(w);
    w.mm(id).scale = s;
    crate::moby_update::creature::react::sphere_lerp(w, id);
    // The checkpoint takes Ratchet's position / Euler as `HeroTeleport` leaves them (the game's stores are immediate);
    // without the exit cuboid, the ship's copy of the last flying tick (0x13f3d0 / 0x13f3e0 = position / +0x30..).
    let mut at = (w.hero_point(), [pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH), pf(w, id, pvo::YAW)]);
    if let Some(e) = story::cuboid(w, pi(w, id, pvo::EXIT_CUBOID)) {
        scenery(w, id, false);
        crate::cinematic::hero_teleport(w, e.0, e.1, 0, true);
        at = e;
    }
    let mission = w.m(id).mission;
    if mission != 0xff { crate::cinematic::set_mission_done(w, mission); }
    release_voices(w, id);
    crate::moby_update::classes::checkpoint::record(w, crate::moby_update::classes::checkpoint::Record { pos: at.0, rot: at.1 });
    w.mm(id).state = 9;
}

/// `0x2ba088(v, cur, target, lo, hi, angle, n)` (module doc).
pub fn steer(v: f32, cur: f32, target: f32, lo: f32, hi: f32, angle: bool, n: i32) -> f32 {
    if v <= lo { return cur; }
    let mut f = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    for _ in 1..n.max(1) { f *= f; }
    if angle { add_rot(cur, sub_rot(target, cur) * f) } else { cur * (1.0 - f) + target * f }
}

/// The view's rows (forward, left, up) from the camera Euler of the tick.
fn camera_rows(w: &World) -> [[f32; 3]; 3] {
    let e = w.hero.loop_in.cam_euler;
    let r = crate::moby_update::services::euler_rows([to_pf(e[0]), to_pf(e[1]), to_pf(e[2]), Pf::ZERO]);
    std::array::from_fn(|k| [fl(r[k][0]), fl(r[k][1]), fl(r[k][2])])
}

/// `0x2b84e8(p, &x, &y, k)`: the screen pixel of a world point (`FUN_00202620` with the view of the tick, the GS
/// offsets removed), y + k; None behind the camera.
pub fn screen_point(w: &World, p: [f32; 3], k: i32) -> Option<[i32; 2]> {
    let eye = w.camera_point();
    let rows = camera_rows(w);
    let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
    let dot = |a: [f32; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
    let (z, x, y) = (dot(rows[0]), -dot(rows[1]), -dot(rows[2]));
    if z <= 0.0 { return None; }
    let tx = w.svc.view_tan_x;
    let ty = tx * 0.775;
    let sx = 256.0 + x / z * (256.0 / tx);
    let sy = 208.0 + y / z * (208.0 / ty);
    Some([sx as i32, sy as i32 + k])
}

/// `0x2ba178`: one tick of flight (module doc).
fn fly(w: &mut World, id: MobyId) {
    w.mm(id).mode &= !0x100;
    let mut speed = pf(w, id, pvo::SPEED);
    if w.m(id).cmd != 0 {
        speed = 0.0001;
    } else {
        let t = c::pi16(w, id, pvo::STARTUP) as i32;
        if w.ticks(0x78) < t {
            let pad = &w.hero.loop_in.pad;
            let (pressed, released, raw) = (pad.raw_pressed, pad.raw_released, pad.raw);
            if pressed & pad::CROSS != 0 {
                release_voice(w, id, pvo::BOOST_VOICE);
                let s = w.play_sound(1, 4, id);
                seti(w, id, pvo::BOOST_VOICE, s);
            } else if released & pad::CROSS != 0 {
                release_voice(w, id, pvo::BOOST_VOICE);
            }
            if raw & pad::CROSS == 0 {
                let target = DT * 24.0;
                if speed < target - SPEED * 0.025 {
                    speed += SPEED * 0.025;
                } else if speed <= target {
                    speed = target;
                } else {
                    speed += (target - speed) * SPEED * 0.1;
                }
            } else {
                speed += SPEED * 0.035;
                if DT * 30.0 < speed { speed = DT * 30.0; }
            }
        } else {
            c::set_pi16(w, id, pvo::STARTUP, (t + 1) as i16);
            speed += SPEED * 0.003;
        }
    }
    set(w, id, pvo::SPEED, speed);
    if w.ticks(5) < c::pi16(w, id, pvo::STARTUP) as i32 {
        let [sx, sy] = w.hero.stick.map(|x| f32::from_bits(x.0));
        let (mut kx, mut ky, mut x, mut y) = (SPEED * 0.1, SPEED * 0.25, sx, sy);
        if w.hero.loop_in.pad.held & pad::L3 != 0 {
            kx = SPEED * 0.08;
            y = (sy / (SPEED * STEP)) * SPEED * 0.015_707_964;
            x = (sx / (SPEED * STEP)) * SPEED * 0.015_707_964;
            ky = kx;
        }
        spring(w, id, x, 0.03, 1.0, kx, pvo::STICK_X, pvo::STICK_XV);
        spring(w, id, y, 0.03, 1.0, ky, pvo::STICK_Y, pvo::STICK_YV);
        let s = pf(w, id, pvo::STICK_X);
        spring_angle(w, id, s, 0.05, 1.0, SPEED, pvo::ROLL, pvo::ROLL_V);
        let p = sub_rot(pf(w, id, pvo::PITCH), pf(w, id, pvo::STICK_Y) * SPEED * STEP);
        set(w, id, pvo::PITCH, p);
        let yw = sub_rot(pf(w, id, pvo::YAW), pf(w, id, pvo::STICK_X) * SPEED * STEP);
        set(w, id, pvo::YAW, yw);
        let p = pf(w, id, pvo::PITCH);
        if DEG80 < p { set(w, id, pvo::PITCH, DEG80_B) } else if p < -DEG80 { set(w, id, pvo::PITCH, -DEG80_B) }
        let ry = w.m(id).rotation[1];
        if DEG80 < ry { w.mm(id).rotation[1] = DEG80_B } else if ry < -DEG80 { w.mm(id).rotation[1] = -DEG80_B }
    }
    // 0x2ba088: the steering limit toward the steering cuboid (or the default point).
    let (point, hi_z, hi_xy) = match w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, pi(w, id, pvo::STEER_CUBOID)) {
        Some(s) if pi(w, id, pvo::STEER_CUBOID) != -1 => {
            let m = s.matrix;
            let l = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            (s.centre(), l([m[2][0], m[2][1], m[2][2]]), l([m[0][0] + m[1][0], m[0][1] + m[1][1], m[0][2] + m[1][2]]))
        }
        _ => (STEER_POINT, 310.0, 500.0),
    };
    let pos = w.m(id).position;
    let d0 = dist2(pos, [STEER_POINT[0], STEER_POINT[1], STEER_POINT[2], 0.0]);
    let to = atan(point[0] - pos[0], point[1] - pos[1]);
    let y = steer(d0, pf(w, id, pvo::YAW), to, 50.0, hi_xy, true, 5);
    set(w, id, pvo::YAW, y);
    let rz = steer(d0, w.m(id).rotation[2], to, 50.0, hi_xy, true, 5);
    w.mm(id).rotation[2] = rz;
    let dz = (point[2] - pos[2]).abs();
    let pt = [point[0], point[1], point[2], 0.0];
    let el = -atan(dist2(pos, pt), point[2] - pos[2]);
    let p = steer(dz, pf(w, id, pvo::PITCH), el, 50.0, hi_z, true, 5);
    set(w, id, pvo::PITCH, p);
    let ry = steer(dz, w.m(id).rotation[1], el, 50.0, hi_z, true, 5);
    w.mm(id).rotation[1] = ry;
    let mut v = polar(speed, pf(w, id, pvo::YAW), -pf(w, id, pvo::PITCH));
    c::set_pv4(w, id, pvo::HEADING, set_len3(v, 1.0));
    if w.m(id).cmd == 0 {
        let p = add(w.m(id).position, v);
        w.mm(id).position = p;
    }
    // The camera.
    let mut f = speed - DT * 24.0;
    let span = DT * 30.0 - DT * 24.0;
    f = if f < 0.0 { 0.0 / span } else { f / span };
    set(w, id, pvo::BOOST, f);
    let (sx, sy) = (pf(w, id, pvo::STICK_X), pf(w, id, pvo::STICK_Y));
    let chase = [((-15.0 - sy) - f * 4.0) * 0.25, sx * -3.0 * 0.25, (sy + sy + 4.0) * 0.25];
    let cockpit = [-1.275, 0.0, 0.65];
    let view = c::pu8(w, id, pvo::VIEW) as usize;
    let off = if view & 1 == 0 { chase } else { cockpit };
    if w.m(id).state == 2 {
        for (k, &v) in off.iter().enumerate() { set(w, id, pvo::CAM_OFF + 4 * k, v); }
    } else {
        for (k, &v) in off.iter().enumerate() { spring(w, id, v, 0.15, 0.8, SPEED, pvo::CAM_OFF + 4 * k, pvo::CAM_OFF_V + 4 * k); }
    }
    let o = [pf(w, id, pvo::CAM_OFF), pf(w, id, pvo::CAM_OFF + 4), pf(w, id, pvo::CAM_OFF + 8)];
    let e = c::pv4(w, id, pvo::ROLL);
    let rows = crate::moby_update::services::euler_rows([to_pf(e[0]), to_pf(e[1]), to_pf(e[2]), Pf::ZERO]);
    let r = |k: usize| [fl(rows[k][0]), fl(rows[k][1]), fl(rows[k][2])];
    let (r0, r1, r2) = (r(0), r(1), r(2));
    let pos = w.m(id).position;
    let eye: [f32; 3] = std::array::from_fn(|k| o[0] * r0[k] + o[1] * r1[k] + o[2] * r2[k] + pos[k]);
    crate::cinematic::camera_targets(w, Some(eye), None);
    let ex = add_rot(pf(w, id, pvo::ROLL) * 0.5, sx * 0.031_25);
    let ey = sub_rot(add_rot(pf(w, id, pvo::PITCH), f32::from_bits(0x3dc9_0fdb)), sy * SPEED * STEP);
    let ez = pf(w, id, pvo::YAW);
    if w.m(id).state == 2 {
        c::set_pv4(w, id, pvo::CAM_EULER, [ex, ey, ez, 0.0]);
    } else {
        spring_angle(w, id, ex, 0.15, 0.8, SPEED, pvo::CAM_EULER, pvo::CAM_EULER_V);
        spring_angle(w, id, ey, 0.15, 0.8, SPEED, pvo::CAM_EULER + 4, pvo::CAM_EULER_V + 4);
        spring_angle(w, id, ez, 0.15, 0.8, SPEED, pvo::CAM_EULER + 8, pvo::CAM_EULER_V + 8);
    }
    let euler = [pf(w, id, pvo::CAM_EULER), pf(w, id, pvo::CAM_EULER + 4), pf(w, id, pvo::CAM_EULER + 8)];
    crate::cinematic::camera_targets(w, None, Some(euler));
    crate::cinematic::camera_script_unless_script(w, eye, euler, 0, 0, false);
    spring(w, id, f * 0.349_065_78 + 1.134_464, f32::from_bits(0x3d4c_cccd), 1.0, 1.0, pvo::FOV, pvo::FOV_V);
    let half = pf(w, id, pvo::FOV) * 0.5;
    w.svc.view_tan_x = half.sin() / half.cos();
    {
        let (roll, pitch, yaw) = (pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH), pf(w, id, pvo::YAW));
        let m = w.mm(id);
        m.rotation[0] = add_rot(roll, sx * 0.5);
        m.rotation[1] = add_rot(pitch, -sy * 0.5);
        m.rotation[2] = add_rot(yaw, -sx * 0.0);
    }
    // The ship against the world and the mobys.
    let centre = crate::moby_update::services::pv(w.m(id).position);
    if let Some(h) = w.coll_sphere(centre, to_pf(1.2), 0, Some(id)) {
        let class = h.moby.map(|m| w.m(m).o_class);
        if class.is_none_or(|k| !NO_SCRAPE.contains(&k)) {
            if let Some(q) = h.pushed_centre {
                let m = w.mm(id);
                m.position = [q[0], q[1], q[2], m.position[3]];
            }
            let r = crate::hero::guns::reflect([v[0], v[1], v[2]], h.normal);
            v = [r[0], r[1], r[2], 0.0];
            let yw = pf(w, id, pvo::YAW);
            set(w, id, pvo::YAW, add_rot(yw, sub_rot(atan(v[0], v[1]), yw) * SPEED * 0.1));
            let xy = (v[0] * v[0] + v[1] * v[1]).sqrt();
            let p = pf(w, id, pvo::PITCH);
            set(w, id, pvo::PITCH, add_rot(p, sub_rot(-atan(xy, v[2]), p) * SPEED * 0.1));
            set(w, id, pvo::SPEED, pf(w, id, pvo::SPEED) * 0.7);
            let blast = fx::Beam { damage_r: 8.0, damage: 100.0, flash: 16.0, flash2: 8.0, flash_dist: 9.0, scale: 1.0, light: 50.0, streaks: 30, sparks: 10, puffs: 24, debris: 0, sound: 6, shake: true };
            if class.is_none() {
                w.svc.vehicle.health = 0.0;
                let p = w.m(id).position;
                fx::beam_explosion(w, &blast, Some(id), p);
                w.mm(id).mode |= 0x41;
                let t = w.ticks(240) as i16;
                c::set_pi16(w, id, pvo::WRECK_T, t);
                w.mm(id).state = 6;
            } else if pi(w, id, pvo::SCRAPE_T) == 0 {
                w.svc.vehicle.health -= 5.0;
                if w.svc.vehicle.health < 0.0 {
                    let p = w.m(id).position;
                    fx::beam_explosion(w, &blast, Some(id), p);
                    let t = w.ticks(240) as i16;
                    c::set_pi16(w, id, pvo::WRECK_T, t);
                    w.mm(id).state = 6;
                }
                let t = w.ticks(10);
                seti(w, id, pvo::SCRAPE_T, t);
            }
        }
    }
    if let Some(i) = row(super::gemlik_ship_hud::HUD_FN) { w.svc.draw_callbacks.register(Callback::UnitFrame(i), id); }
    missiles(w, id);
    let yaw = add_rot(pf(w, id, pvo::CAM_EULER + 8), -pf(w, id, pvo::STICK_X) / 7.0);
    let pitch = add_rot(pf(w, id, pvo::CAM_EULER + 4), -(pf(w, id, pvo::STICK_Y) / 7.0) - 0.1);
    guns(w, id, yaw, -pitch);
}

/// `0x2b9f58` (module doc).
fn missiles(w: &mut World, id: MobyId) {
    if c::dec_timer_pvar_i32(w, id, pvo::MISSILE_T) == 0 { return; }
    if w.hero.loop_in.pad.raw & pad::MISSILE == 0 || w.svc.vehicle.missiles == 0 { return; }
    let j = (c::pi16(w, id, pvo::MISSILE_JOINT) as u16).wrapping_add(1) & 3;
    c::set_pi16(w, id, pvo::MISSILE_JOINT, j as i16);
    let at = w.joint_point(id, j as usize + 7);
    let b = set_len3(c::pv4(w, id, pvo::BASE_VEL), 0.3);
    let at = add(at, b);
    let life = w.ticks(300);
    let target = link(w, id, pvo::MISSILE_TARGET);
    let rot = w.m(id).rotation;
    if super::ship_missile::spawn(w, DT * 100.0, id, at, target, rot, life).is_some() {
        w.play_sound(2, 0, id);
        w.svc.vehicle.missiles -= 1;
        let t = w.ticks(0x1e);
        seti(w, id, pvo::MISSILE_T, t);
    }
}

/// `0x2b9df8(yaw, pitch, m, pvars)` (module doc).
fn guns(w: &mut World, id: MobyId, yaw: f32, pitch: f32) {
    let aim = add(polar(DT * 400.0, yaw, pitch), c::pv4(w, id, pvo::BASE_VEL));
    if c::dec_timer_pvar_i32(w, id, pvo::GUN_T) != 0 && w.hero.loop_in.pad.raw & pad::GUN != 0 {
        w.play_sound(3, 0, id);
        let d = set_len3(aim, f32::from_bits(0x400c_cccd));
        let side = c::pu8(w, id, pvo::GUN_SIDE) & 1;
        let muzzle = add(w.joint_point(id, side as usize + 3), d);
        super::ship_laser::spawn(w, 200.0, -1.0, id, aim, muzzle);
        let t = w.ticks(4);
        seti(w, id, pvo::GUN_T, t);
        let s = c::pu8(w, id, pvo::GUN_SIDE) ^ 1;
        c::set_pu8(w, id, pvo::GUN_SIDE, s);
    }
    let a = set_len3(aim, 23.0);
    let p = w.m(id).position;
    if let Some([x, y]) = screen_point(w, [a[0] + p[0], a[1] + p[1], a[2] + p[2]], 0) {
        seti(w, id, pvo::CROSS_X, x);
        seti(w, id, pvo::CROSS_Y, y);
    }
}

/// `PartType74Spawn(m, p, v, desc, 1)` ([`crate::moby_update::creature::fx::part74`]).
fn part74(w: &mut World, id: MobyId, p: [f32; 4], v: [f32; 3], d: crate::particles::type74::Desc) { crate::moby_update::creature::fx::part74(w, id, p, v, d, true) }

/// `0x2b8590` (module doc); Pokitaru's jet 1242 runs the same code (`0x3112b8`, the same descriptors, its pvars at the
/// same offsets).
pub(super) fn exhaust(w: &mut World, id: MobyId) {
    for j in 1..3 {
        let f = (pf(w, id, pvo::SPEED) / (DT * 24.0)).clamp(0.35, 1.0);
        let jp = w.joint_point(id, j);
        let jp = [jp[0], jp[1], jp[2], 0.0];
        let b = pf(w, id, pvo::BOOST);
        let (mut p0, mut p1, mut p2) = (jp, jp, jp);
        fx::jitter(w, b * 0.02 + 0.02, &mut p0);
        fx::jitter(w, b * 0.02 + 0.02, &mut p1);
        fx::jitter(w, b * 0.01 + 0.02, &mut p2);
        let puff = |w: &mut World, p: [f32; 4], k: f32, d: usize| {
            let r = w.rng.randf(6.0, 7.0);
            let b = pf(w, id, pvo::BOOST);
            let vx = -((r + b * k) * DT) * f;
            part74(w, id, p, [vx, 0.0, 0.0], EXHAUST[d]);
        };
        puff(w, p0, 2.0, 0);
        puff(w, p1, 3.0, 1);
        puff(w, p2, 4.0, 2);
        if 0.5 < b {
            fx::jitter(w, b * 0.02 + 0.01, &mut p0);
            fx::jitter(w, b * 0.02 + 0.01, &mut p1);
            fx::jitter(w, b * 0.01 + 0.01, &mut p2);
            puff(w, p0, 2.0, 0);
            puff(w, p1, 3.0, 1);
            puff(w, p2, 4.0, 2);
            puff(w, p0, 2.0, 3);
            puff(w, jp, 3.0, 4);
        }
    }
}
