//! **Hoven's turret, class 1267** (level12 `0x303540`, census U405, one instance): the story mini-game that unlocks
//! Gemlik (planet 13). Ratchet walks onto the turret, it takes him (hero state 0x32, hidden, the script camera in
//! its seat) and he shoots down the attackers (the drones 326 in turret mode, the mines 1269) and the carrier ship
//! 1274 (pvar +0x94) with the shells 458 ([`super::turret_shell`]); the turret's health is 200, the HUD (draw callback
//! `0x304218`) shows it as a ring with the lock-on reticle; at 0 the screen goes red (`0x304d98`) under explosions and
//! the death sequence reloads the level; the carrier destroyed: the mission done, movie 13, `UnlockPlanet(13)`,
//! Ratchet placed off the turret, scene 1 and the save. Read from the level12 decomp and disassembly (`0x303540`, its
//! helpers `0x3032e8` / `0x303370`, the callbacks); the level data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code (no other copy: cluster of one), on the shared mechanisms: the hero's mounted
//! state 0x32 (`crate::hero::scripted`), the script camera (`cinematic::camera_script` / `camera_targets`), the
//! manipulator (`manip`), the shared shell [`super::turret_shell`] (also level 08's), the story layer
//! (`cinematic::{set_mission_done, start_movie, unlock_planet, hero_teleport, start_scene, save}`), the 2-D
//! primitives of the frame's callbacks (`DrawCallbacks::rings`, drawn by `rc-engine`'s scene_render). The game's
//! level words it shares with the other mini-game classes are unit words ([`lw`]).
//!
//! **Pvar block** (0xb0): +0x00 the barrel manipulator (joint list 0), +0x40..+0x64 ten aim-assist targets (moby
//! indices, −1 none), +0x70 (s16) the fire cooldown, +0x72 (s16), +0x74 the pitch, +0x78 / +0x7c the pitch / yaw
//! speeds, +0x80 the barrel toggle, +0x84 (s32) 200, +0x88 the lock-on count, +0x8c the health (f32), +0x90 the hit
//! timer, +0x94 the carrier (moby index, −1 none), +0x98 (s32) −1, +0xa0 the rest height, +0xa4 the turn-away spin,
//! +0xac (s16) the death timer, +0xae (s16) the screen explosions' timer.
//!
//! **Level words** ([`lw`], all 0 in the overlay data): 0x161f00 the game on (the drones 326 and the carrier 1274 read
//! it), 0x161f04 the turret destroyed, 0x161f20 the mounts (the attackers ease off with it), 0x161f24 the carrier's
//! last stand (1274's `0x3054b0` sets it: the turret can no longer be destroyed), 0x1619a0 the carrier's death camera
//! (1274). The vehicle "Quit?" 0x14095f is the shared record's ([`crate::vehicle::Record::quit`]).
//!
//! ## Coverage (`0x303540`)
//! | address | what | port |
//! |---|---|---|
//! | entry | 0x13e5bc = self (the sound occlusion line's ignored moby) | n/a: the port's occlusion line tests the world mesh only (`audio::voices`) |
//! | state 0 | 0x161f00 = 0x161f04 = 0; the mission (+0xb0) done → 6 | [`update`] (`hints::mission_done`) |
//! | state 0 | else `AttachManipulator(self, 0, +0x00)`, → 3; +0x84 = 200; rot.y = 0; +0x70 = `ticks(30)`; +0x72 = 0; +0x78 = +0x7c = 0; +0x98 = −1; +0xa0 = z; rot.z = π | [`update`] (`manip::attach`) |
//! | state 3 | `FUN_00268508(self, 3, tmp)` (joint point 3) | n/a: the result is not read |
//! | state 3 | Ratchet ≥ 7 away (xy) and not standing on it: z rises by dt to +0xa0; +0x98 ≠ −1 → `PlayClassSound(2, 0, self)` | [`update`] (the sound's guard is never true: +0x98 stays −1) |
//! | state 3 | Ratchet < 7 away or on it: z += ((+0xa0 − 1.2) − z)·0.11; +0xa4 = 0.9·add(+0xa4, 0.01·add(sub(atan(Ratchet − pos), rot.z), π/2)); rot.z = add(rot.z, +0xa4) | [`update`] |
//! | state 3 | Ratchet's moby ≥ 1.3 away (xy) → done | [`update`] |
//! | state 3 | grounded > `ticks(15)` (0x13f650): `CameraScript(pos, rot, 1, 0, 0)`; seq ≠ 5 → hard cut 5; `SetState(0x32, 1)`; the seat camera (`0x3032e8`); +0x8c = 200; +0xa4 (hit) = 0xff; 0x161f20 + 1; → 4 | [`update`] (`cinematic::camera_script`, `cinematic::hero_state`) |
//! | state 4 | 0x1413f5 = 1 (Ratchet hidden); 0x161f00 = 1; 0x13f51c = 0x13f510 = `ticks(120)` | [`update`] (`HeroFields::{hero_hidden, no_vel_clamp, invulnerable}`) |
//! | state 4 | game mode ≠ 2 and 0x1619a0 = 0 → `RegisterDrawCallback2(0x304218)` | [`update`] (`Callback::UnitFrame`) |
//! | state 4 | `0x303370`: the aim assist | [`aim_assist`] |
//! | state 4 | `FastDecTimer(+0x90)` out: `MobyGetHitMessage(self, 0x10000)` → +0x90 = `ticks(4)`, health −= damage; +0xa4 (hit) = 0xff | [`update`] |
//! | state 4 | 0x161f24 = 0: 0x14095f & 1 (Quit) → health > 100: 0x161f20 − 1; 0x14095f = 0; the death sequence (`0x2319b0`) | [`update`] (`HeroCall::Death`) |
//! | state 4 | 0x161f24 = 0 and health ≤ 0: 0x161f04 = 1, +0xac = `ticks(120)`, → 7 | [`update`] |
//! | state 4 | the carrier +0x94 deleted (state ≥ 0x80), game mode ≠ 2: `SetMissionDone(+0xb0)`, `DialogStreamUpdate(13)`, `UnlockPlanet(13)`, 0x161f00 = 0, `HeroTeleport((328.4, 338.7, 60.98), (0, 0, −1.7), 0, 1)`, → 5 | [`update`] |
//! | state 4 | the carrier deleted in game mode 2: manipulator attached → `DetachManipulator`; seq ≠ 0 → hard cut 0; z = +0xa0; rot.z = π | [`update`] |
//! | state 4, 0x1619a0 = 0 | the stick (0x141070 / 0x141074; y negated when 0x15eddc = 0): yaw speed +0x7c `Approach`es −150°·dt·x at 240°·dt² (speeding up) or 360°·dt² (slowing); rot.z += it; pitch speed +0x78 `Approach`es 70°·dt·y at 180° / 720°·dt²; +0x74 += it, clamped to [−45°, 35°]; the barrel joint turned by the pitch (`0x222e38(−pitch, +0x10, 1)`) | [`controls`] (`turn::approach`, `manip::set_axis`) |
//! | state 4, 0x1619a0 = 0 | anim flags & 2 → `MobyAnimBlend(self, 5, 0, 1)`; `FastDecTimer(+0x70)` | [`controls`] |
//! | state 4, 0x1619a0 = 0 | □ / ○ held (0x13cae0 & 0xa0) and +0x70 = 0: blend seq 4 (+0x80 = 1) / 3; vel = polar(50·dt, rot.z + π/2, pitch); the muzzle = joint point 2 (+0x80 = 1 → 0) / 1 (→ 1) + 3·vel; the shell (`0x2ef2e8(self, muzzle, vel, 0)`); `PlayClassSound(0, 0, self)`; the camera shakes 0.03 for `ticks(10)` (up) and 0.03 for `ticks(7)` (forward); +0x70 = `ticks(20)`; five sparks (`PartType27Spawn(50000, muzzle, normalise(normalise(randf_sym³)·0.65·\|vel\| + vel)·randf(3·dt, 6·dt), 0x5f2f4f6f, rand_range(ticks(5), ticks(10)))`), the flash (`PartType27Spawn(500000, muzzle, normalise(vel)·0.5·dt, 0x2f4f7f7f, rand_range(ticks(4), ticks(7)))`) | [`fire`] (`turret_shell::spawn`, `fx::part27`) |
//! | state 4, 0x1619a0 = 0 | the seat camera `0x3032e8` | [`seat_camera`] |
//! | state 5 | game mode ≠ 2: `DialogStreamStart(1)`, → 8 | [`update`] |
//! | state 6 | seq ≠ 0 → hard cut 0 | [`update`] |
//! | state 7 | `RegisterDrawCallback2(0x304d98)` (the red screen) | [`update`] (`Callback::UnitQuads`; its draw: [`tint`], G-REN-030) |
//! | state 7 | `FastDecTimer(+0xae)` out: an explosion before the camera: p = camera + polar(randf(0.7, 1.2), cam yaw + `randf_sym(0, 30°)`, cam pitch + `randf_sym(0, 30°)`) (`0x27b518`); f = `randf(0.5, 1.3)`; `SpawnBeamExplosion(0, 0, f, 0.57·f, 10, 0.7, 0, self, 0, p, 7, 15, 25, 3, 0, 0, −1, 0)`; +0xae = `ticks(rand_range(5, 13))` | [`update`] (`fx::beam_explosion`) |
//! | state 7 | `FastDecTimer(+0xac)` out: the death sequence | [`update`] (`HeroCall::Death`) |
//! | state 8 | game mode ≠ 2: `memcard_Save(0, −1)`, → 6 | [`update`] (`cinematic::save`) |
//!
//! ## Coverage (helpers and callbacks)
//! | address | what | port |
//! |---|---|---|
//! | 0x3032e8 | the current camera +0x88 = 1 (`0x312f58`) | n/a: the script camera is in mode 1 already (`CameraScript(…, 1, …)` at the mount); nothing else switches it |
//! | 0x3032e8 | target position = rows·(0, 0.45, 3) + pos (gp−0x4d10); target Euler (0, −pitch, rot.z + π/2) (`0x312bd8` / `0x312c30`) | [`seat_camera`] (`cinematic::camera_targets`) |
//! | 0x303370 | each of the ten targets (+0x40..): its point (z + 1.5 for classes 326 / 1278), tolerances 7° / 7° (1278: 2° yaw, 3° pitch); seen from the camera within them (`FastDiffRots` against the camera yaw 0x167258 and −pitch 0x167254) → locked | [`aim_assist`] |
//! | 0x303370 | locked: +0x88 + 2, at most `ticks(30)`; else `FastDecTimer(+0x88)` | [`aim_assist`] |
//! | 0x304218 | ALPHA 0x44; the backdrop: four quarter quads ±55 px around (64, 356), FX 6 + 0x28, ST 0..128, 0x60808080 | [`hud_frame`] |
//! | 0x304218 | the health ring: 32 slices of radius 40 from the top clockwise, FX 6, colour `FastTweenColor((h − 100)/100, 0x6000ffff, 0x6000ff00)` above 100 else `FastTweenColor(h/100, 0x600000ff, 0x6000ffff)`; the first `trunc((1 − h/200)·32)` slices at alpha 0x20 | [`hud_frame`] |
//! | 0x304218 | the reticle: colour `FastTweenColor((n − ticks(15))/ticks(15), 0x5000ffff, 0x500000ff)` above `ticks(15)` else `FastTweenColor(n/ticks(15), 0x5000ff00, 0x5000ffff)` (n = +0x88); two squares of radius 40 about the screen centre turned by 4·yaw (FX 0x11) and 8·pitch (FX 0x12), ST 0..64 | [`hud_frame`] |
//! | 0x304d98 | +0xac < `ticks(80)`: `emit_rgba_draw_packet(0xaa, 0, 0, (ticks(80) − t)·80 / ticks(80))` | [`tint`] (its draw: G-REN-030) |

use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::fx::{beam_explosion, part27, polar, Beam};
use crate::moby_update::creature::{self as c, add, add_rot, atan, diff_rots, dist2, len3, set_len3, sub_rot, DT, DT2, SPEED};
use crate::moby_update::manip;
use crate::moby_update::services::{HeroCall, World};
use super::trespasser_lock::RingPrim;

pub const REFERENCE_LEVEL: u32 = 12;
pub const UPDATE_FN: u32 = 0x30_3540;
/// The HUD draw callback and the red screen (list 2).
pub const HUD_FN: u32 = 0x30_4218;
pub const TINT_FN: u32 = 0x30_4d98;
pub const CLASSES: [i16; 1] = [1267];

/// The classes of the aim-assist targets with their own aim point / tolerances.
const DRONE: i16 = 0x146;
const SPECIAL: i16 = 0x4fe;

/// The level words (module doc).
pub mod lw {
    pub const ACTIVE: u32 = 0x16_1f00;
    pub const DEAD: u32 = 0x16_1f04;
    pub const MOUNTS: u32 = 0x16_1f20;
    pub const LAST_STAND: u32 = 0x16_1f24;
    pub const CARRIER_CAM: u32 = 0x16_19a0;
}

pub mod pvo {
    pub const MANIP: usize = 0x00;
    pub const TARGETS: usize = 0x40;
    pub const COOLDOWN: usize = 0x70;
    pub const F72: usize = 0x72;
    pub const PITCH: usize = 0x74;
    pub const PITCH_V: usize = 0x78;
    pub const YAW_V: usize = 0x7c;
    pub const BARREL: usize = 0x80;
    pub const F84: usize = 0x84;
    pub const LOCK: usize = 0x88;
    pub const HEALTH: usize = 0x8c;
    pub const HIT_T: usize = 0x90;
    pub const CARRIER: usize = 0x94;
    pub const SOUND: usize = 0x98;
    pub const REST_Z: usize = 0xa0;
    pub const SPIN: usize = 0xa4;
    pub const DEATH_T: usize = 0xac;
    pub const BOOM_T: usize = 0xae;
    pub const LEN: usize = 0xb0;
}

/// The level data (gp−0x4dec..−0x4cd0).
mod k {
    pub const PITCH_MIN: f32 = f32::from_bits(0xbf49_0fdb);
    pub const PITCH_MAX: f32 = f32::from_bits(0x3f1c_61aa);
    /// Degrees a second and their accelerations (degrees a second², ×dt²): yaw 150 (240 up / 360 down), pitch 70
    /// (180 / 720).
    pub const YAW_SPEED: f32 = 150.0;
    pub const PITCH_SPEED: f32 = 70.0;
    pub const YAW_ACC: f32 = 240.0;
    pub const PITCH_ACC: f32 = 180.0;
    pub const YAW_DEC: f32 = 360.0;
    pub const PITCH_DEC: f32 = 720.0;
    /// The shell's speed (·dt).
    pub const SHELL_SPEED: f32 = 50.0;
    /// The seat camera's offset in the turret's frame (gp−0x4d10).
    pub const SEAT: [f32; 3] = [0.0, 0.45, 3.0];
    /// The HUD (gp−0x4dec..): the panel centre (x −192, y 148 from the screen centre), its half size, the ring radius;
    /// the FX textures (gp−0x4dc4 / −0x4dc0 / −0x4da4).
    pub const PANEL: [f32; 2] = [-192.0, 148.0];
    pub const PANEL_HALF: [f32; 2] = [55.0, 55.0];
    pub const RING_R: [f32; 2] = [40.0, 40.0];
    pub const RETICLE_R: [f32; 2] = [40.0, 40.0];
    pub const FX_RETICLE_YAW: usize = 0x11;
    pub const FX_RETICLE_PITCH: usize = 0x12;
    pub const FX_BASE: usize = 6;
    pub const PANEL_RGBA: u32 = 0x6080_8080;
    /// The mount's teleport when the carrier is down.
    pub const EXIT_POS: [f32; 3] = [328.4, 338.7, f32::from_bits(0x4273_eb85)];
    pub const EXIT_YAW: f32 = f32::from_bits(0xbfd9_999a);
}

const PI: f32 = std::f32::consts::PI;
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
const DEG: f32 = 0.017_453_292;

/// The screen explosions of the destroyed turret (`SpawnBeamExplosion(0, 0, f, 0.57f, 10, 0.7, 0, …, 7, 15, 25, 3, 0, 0,
/// −1, 0)`; the flash sizes are set per call).
const SCREEN_BLAST: Beam = Beam { damage_r: 0.0, damage: 0.0, flash: 0.0, flash2: 0.0, flash_dist: 10.0, scale: f32::from_bits(0x3f33_3333), light: 0.0, streaks: 7, sparks: 15, puffs: 25, debris: 0, sound: 3, shake: false };

fn word(w: &World, a: u32) -> u32 { w.svc.units.word(a) }
fn set_word(w: &mut World, a: u32, v: u32) { w.svc.units.set_word(a, v) }

fn row(name: u32) -> Option<u16> { super::row(REFERENCE_LEVEL, name) }

/// Level12 `0x303540` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    crate::moby_update::story::pvars(w, id, pvo::LEN);
    let st = w.m(id).state;
    match st {
        0 => init(w, id),
        3 => wait(w, id),
        4 => ride(w, id),
        5 => {
            if w.svc.game_mode == 2 { return; }
            crate::cinematic::start_scene(w, 1, false);
            w.mm(id).state = 8;
        }
        6 => if w.m(id).anim.seq_b != 0 { c::hard_cut(w, id, 0, 0); },
        7 => destroyed(w, id),
        8 if w.svc.game_mode != 2 => {
            crate::cinematic::save(w);
            w.mm(id).state = 6;
        }
        _ => {}
    }
}

fn init(w: &mut World, id: MobyId) {
    set_word(w, lw::ACTIVE, 0);
    set_word(w, lw::DEAD, 0);
    if super::hints::mission_done(w, w.m(id).mission as i32) {
        w.mm(id).state = 6;
        return;
    }
    manip::attach(w, id, 0, id, pvo::MANIP);
    w.mm(id).state = 3;
    c::set_pi32(w, id, pvo::F84, 200);
    w.mm(id).rotation[1] = 0.0;
    let t = w.ticks(30) as i16;
    c::set_pi16(w, id, pvo::COOLDOWN, t);
    c::set_pi16(w, id, pvo::F72, 0);
    c::set_pf(w, id, pvo::PITCH_V, 0.0);
    c::set_pf(w, id, pvo::YAW_V, 0.0);
    c::set_pi32(w, id, pvo::SOUND, -1);
    let z = w.m(id).position[2];
    c::set_pf(w, id, pvo::REST_Z, z);
    w.mm(id).rotation[2] = PI;
}

/// State 3: waiting for Ratchet.
fn wait(w: &mut World, id: MobyId) {
    let pos = w.m(id).position;
    let hero = w.hero_point();
    let h4 = [hero[0], hero[1], hero[2], 0.0];
    let on = w.hero.ground_moby == Some(id);
    let rest = c::pf(w, id, pvo::REST_Z);
    if 7.0 <= dist2(pos, h4) && !on {
        let z = w.m(id).position[2];
        w.mm(id).position[2] = if z < rest { z + DT } else { rest };
        if c::pi32(w, id, pvo::SOUND) != -1 { w.play_sound(2, 0, id); }
    } else {
        let z = w.m(id).position[2];
        w.mm(id).position[2] = z + ((rest - 1.2) - z) * f32::from_bits(0x3de1_47ae);
        let a = atan(hero[0] - pos[0], hero[1] - pos[1]);
        let d = add_rot(sub_rot(a, w.m(id).rotation[2]), HALF_PI);
        let s = add_rot(c::pf(w, id, pvo::SPIN), d * f32::from_bits(0x3c23_d70a)) * f32::from_bits(0x3f66_6666);
        c::set_pf(w, id, pvo::SPIN, s);
        let rz = w.m(id).rotation[2];
        w.mm(id).rotation[2] = add_rot(rz, s);
    }
    let hm = w.hero_moby.map(|h| w.m(h).position).unwrap_or(h4);
    if 1.3 <= dist2(w.m(id).position, hm) { return; }
    if w.ticks(15) < w.hero.grounded_ticks {
        let (p, r) = { let m = w.m(id); ([m.position[0], m.position[1], m.position[2]], [m.rotation[0], m.rotation[1], m.rotation[2]]) };
        crate::cinematic::camera_script(w, p, r, 1, 0, false);
        if w.m(id).anim.seq_b != 5 { c::hard_cut(w, id, 5, 0); }
        crate::cinematic::hero_state(w, 0x32, true);
        seat_camera(w, id);
        c::set_pf(w, id, pvo::HEALTH, 200.0);
        w.mm(id).hit_slot = 0xff;
        let n = word(w, lw::MOUNTS).wrapping_add(1);
        set_word(w, lw::MOUNTS, n);
        w.mm(id).state = 4;
    }
}

/// State 4: Ratchet in the turret.
fn ride(w: &mut World, id: MobyId) {
    let t120 = w.ticks(120);
    {
        let f = w.hero_fields_mut();
        f.hero_hidden = Some(1);
        f.no_vel_clamp = Some(t120);
        f.invulnerable = Some(t120);
    }
    set_word(w, lw::ACTIVE, 1);
    if w.svc.game_mode != 2 && word(w, lw::CARRIER_CAM) == 0 {
        if let Some(i) = row(HUD_FN) { w.svc.draw_callbacks.register2(Callback::UnitFrame(i), id); }
    }
    aim_assist(w, id);
    if c::dec_timer_pvar_i32(w, id, pvo::HIT_T) != 0 {
        if let Some(h) = w.get_hit(id, 0x1_0000, false) {
            let t = w.ticks(4);
            c::set_pi32(w, id, pvo::HIT_T, t);
            let hp = c::pf(w, id, pvo::HEALTH) - crate::moby_update::services::fl(h.damage);
            c::set_pf(w, id, pvo::HEALTH, hp);
        }
        w.mm(id).hit_slot = 0xff;
    }
    if word(w, lw::LAST_STAND) == 0 {
        let mut hp = c::pf(w, id, pvo::HEALTH);
        if w.svc.vehicle.quit & 1 != 0 {
            if 100.0 < hp {
                let n = word(w, lw::MOUNTS).wrapping_sub(1);
                set_word(w, lw::MOUNTS, n);
            }
            w.svc.vehicle.quit = 0;
            w.hero_fields_mut().call(HeroCall::Death);
            hp = c::pf(w, id, pvo::HEALTH);
        }
        if hp <= 0.0 {
            set_word(w, lw::DEAD, 1);
            c::set_pi16(w, id, pvo::DEATH_T, t120 as i16);
            w.mm(id).state = 7;
            return;
        }
    }
    let carrier = c::pi32(w, id, pvo::CARRIER);
    let down = carrier != -1 && crate::moby_update::story::link(w, carrier).is_some_and(|m| w.m(m).state >= 0x80);
    if down {
        if w.svc.game_mode != 2 {
            crate::cinematic::set_mission_done(w, w.m(id).mission);
            crate::cinematic::start_movie(w, 0xd);
            crate::cinematic::unlock_planet(w, 0xd);
            set_word(w, lw::ACTIVE, 0);
            crate::cinematic::hero_teleport(w, k::EXIT_POS, [0.0, 0.0, k::EXIT_YAW], 0, true);
            w.mm(id).state = 5;
            return;
        }
        if manip::attached(w, id, pvo::MANIP) { manip::detach(w, id, id, pvo::MANIP); }
        if w.m(id).anim.seq_b != 0 { c::hard_cut(w, id, 0, 0); }
        let z = c::pf(w, id, pvo::REST_Z);
        w.mm(id).position[2] = z;
        w.mm(id).rotation[2] = PI;
    }
    if word(w, lw::CARRIER_CAM) == 0 {
        controls(w, id);
        seat_camera(w, id);
    }
}

/// `Approach` of a speed toward `target`: faster while it speeds up in the same direction (`acc`), else `dec`.
fn speed_toward(v: &mut f32, target: f32, acc: f32, dec: f32) {
    let x = *v;
    let rate = if (0.0 <= x && x < target) || (x <= 0.0 && target < x) { acc } else { dec };
    c::turn::approach(target, rate * DEG * DT2, v);
}

/// State 4's turning, the barrel and the trigger (0x1619a0 = 0).
fn controls(w: &mut World, id: MobyId) {
    let [sx, sy] = w.hero.stick.map(|x| f32::from_bits(x.0));
    let sy = if w.svc.help.cam_pitch_word == 0 { -sy } else { sy };
    let mut yv = c::pf(w, id, pvo::YAW_V);
    speed_toward(&mut yv, k::YAW_SPEED * DEG * DT * -sx, k::YAW_ACC, k::YAW_DEC);
    c::set_pf(w, id, pvo::YAW_V, yv);
    let rz = w.m(id).rotation[2];
    w.mm(id).rotation[2] = add_rot(rz, yv);
    let mut pv_ = c::pf(w, id, pvo::PITCH_V);
    speed_toward(&mut pv_, -(k::PITCH_SPEED * DEG * DT) * -sy, k::PITCH_ACC, k::PITCH_DEC);
    c::set_pf(w, id, pvo::PITCH_V, pv_);
    let mut pitch = add_rot(c::pf(w, id, pvo::PITCH), pv_);
    pitch = pitch.clamp(k::PITCH_MIN, k::PITCH_MAX);
    c::set_pf(w, id, pvo::PITCH, pitch);
    manip::set_axis(w, id, id, pvo::MANIP, -pitch, 1);
    if w.m(id).anim.flags & 2 != 0 { w.anim_blend(id, 5, 0, 1); }
    c::dec_timer_pvar_s16(w, id, pvo::COOLDOWN);
    if w.hero.loop_in.pad.held & 0xa0 != 0 && c::pi16(w, id, pvo::COOLDOWN) == 0 { fire(w, id); }
}

/// A shot (module doc).
fn fire(w: &mut World, id: MobyId) {
    let left = c::pi32(w, id, pvo::BARREL) == 1;
    w.anim_blend(id, if left { 4 } else { 3 }, 0, 1);
    let yaw = add_rot(w.m(id).rotation[2], HALF_PI);
    let pitch = c::pf(w, id, pvo::PITCH);
    let vel = polar(k::SHELL_SPEED * DT, yaw, pitch);
    let mut p = if left {
        c::set_pi32(w, id, pvo::BARREL, 0);
        w.joint_point(id, 2)
    } else {
        let p = w.joint_point(id, 1);
        c::set_pi32(w, id, pvo::BARREL, 1);
        p
    };
    for _ in 0..3 { p = add(p, vel); }
    super::turret_shell::spawn(w, Some(id), p, vel, None);
    w.play_sound(0, 0, id);
    let (t10, t7) = (w.ticks(10), w.ticks(7));
    use crate::follow_camera::{ShakeAxis, ShakeRequest};
    w.shake_camera(ShakeRequest { axis: ShakeAxis::Up, amp: f32::from_bits(0x3cf5_c28f), ticks: t10 });
    w.shake_camera(ShakeRequest { axis: ShakeAxis::Forward, amp: f32::from_bits(0x3cf5_c28f), ticks: t7 });
    let t20 = w.ticks(20) as i16;
    c::set_pi16(w, id, pvo::COOLDOWN, t20);
    let speed = len3(vel);
    for _ in 0..5 {
        let x = w.rng.randf(-1.0, 1.0);
        let y = w.rng.randf(-1.0, 1.0);
        let z = w.rng.randf(-1.0, 1.0);
        let r = set_len3([x, y, z, 0.0], speed * (SPEED * -0.35 + 1.0));
        let r = add([r[0], r[1], r[2], 0.0], vel);
        let s = w.rng.randf(DT * 3.0, DT * 6.0);
        let r = set_len3(r, s);
        let (t5, t10) = (w.ticks(5), w.ticks(10));
        let life = w.rng.rand_range(t5, t10);
        part27(w, 50000.0, p, r, 0x5f2f_4f6f, life);
    }
    let v = set_len3(vel, DT * 0.5);
    let (t4, t7) = (w.ticks(4), w.ticks(7));
    let life = w.rng.rand_range(t4, t7);
    part27(w, 500000.0, p, v, 0x2f4f_7f7f, life);
}

/// `0x3032e8`: the seat camera's targets (module doc).
pub fn seat_camera(w: &mut World, id: MobyId) {
    let m = w.m(id);
    let r = &m.rows;
    let o = k::SEAT;
    let p: [f32; 3] = std::array::from_fn(|l| r[0][l] * o[0] + r[1][l] * o[1] + r[2][l] * o[2] + m.position[l]);
    let pitch = c::pf(w, id, pvo::PITCH);
    let e = [0.0, -pitch, add_rot(HALF_PI, m.rotation[2])];
    crate::cinematic::camera_targets(w, Some(p), Some(e));
}

/// `0x303370`: the aim assist's lock-on count (module doc).
pub fn aim_assist(w: &mut World, id: MobyId) {
    let cam = w.camera_point();
    let (cam_yaw, cam_pitch) = (w.camera_yaw, w.hero.loop_in.cam_euler[1]);
    let mut locked = false;
    for i in 0..10 {
        let t = c::pi32(w, id, pvo::TARGETS + 4 * i);
        if t == -1 { continue; }
        let Some(m) = crate::moby_update::story::link(w, t) else { continue };
        let mut q = w.m(m).position;
        let class = w.m(m).o_class;
        let (mut tol_yaw, mut tol_pitch) = (f32::from_bits(0x3dfa_35dd), f32::from_bits(0x3dfa_35dd));
        if class == DRONE { q[2] += 1.5; }
        if class == SPECIAL {
            tol_yaw = f32::from_bits(0x3d0e_fa35);
            q[2] += 1.5;
            tol_pitch = f32::from_bits(0x3d56_7750);
        }
        let yaw = atan(q[0] - cam[0], q[1] - cam[1]);
        let d = ((q[0] - cam[0]).powi(2) + (q[1] - cam[1]).powi(2)).sqrt();
        let pitch = atan(d, q[2] - cam[2]);
        if diff_rots(yaw, cam_yaw) < tol_yaw && diff_rots(pitch, -cam_pitch) < tol_pitch { locked = true; }
    }
    if locked {
        let n = c::pi32(w, id, pvo::LOCK) + 2;
        let max = w.ticks(30);
        c::set_pi32(w, id, pvo::LOCK, if max < n { max } else { n });
    } else {
        c::dec_timer_pvar_i32(w, id, pvo::LOCK);
    }
}

/// State 7: the turret destroyed.
fn destroyed(w: &mut World, id: MobyId) {
    if let Some(i) = row(TINT_FN) { w.svc.draw_callbacks.register2(Callback::UnitQuads(i), id); }
    if c::dec_timer_pvar_s16(w, id, pvo::BOOM_T) != 0 {
        let r = w.rng.randf(0.7, 1.2);
        let ry = w.rng.randf_sym(0.0, f32::from_bits(0x3f06_0a92));
        let yaw = add_rot(w.camera_yaw, ry);
        let rp = w.rng.randf_sym(0.0, f32::from_bits(0x3f06_0a92));
        let pitch = add_rot(w.hero.loop_in.cam_euler[1], rp);
        let cam = w.camera_point();
        let p = add(polar(r, yaw, pitch), [cam[0], cam[1], cam[2], crate::moby_update::services::fl(w.camera[3])]);
        let f = w.rng.randf(0.5, f32::from_bits(0x3fa6_6666));
        beam_explosion(w, &Beam { flash: f, flash2: f * 0.57, ..SCREEN_BLAST }, Some(id), p);
        let n = w.rng.rand_range(5, 13);
        let t = w.ticks(n) as i16;
        c::set_pi16(w, id, pvo::BOOM_T, t);
    }
    if c::dec_timer_pvar_s16(w, id, pvo::DEATH_T) != 0 {
        w.hero_fields_mut().call(HeroCall::Death);
    }
}

// ------------------------------------------------------------------------------------------------
// The callbacks.

/// A screen point from the panel-relative GS offsets: x truncated to whole pixels, y to 1/16 pixel (the game's packing).
fn at(x: f32, y: f32) -> [f32; 2] {
    let c0 = super::trespasser_lock::CENTRE;
    [c0[0] + (x as i32) as f32, c0[1] + ((y * 16.0) as i32) as f32 / 16.0]
}

/// The reticle's corner: x and y to 1/16 pixel about the screen centre.
fn at16(x: f32, y: f32) -> [f32; 2] {
    let c0 = super::trespasser_lock::CENTRE;
    [c0[0] + ((x * 16.0) as i32) as f32 / 16.0, c0[1] + ((y * 16.0) as i32) as f32 / 16.0]
}

fn tween(f: f32, a: u32, b: u32) -> u32 { crate::particles::tween_color(f.to_bits(), a, b) }

/// `0x304218` (list 2), run by the frame's callbacks: the HUD's primitives into `DrawCallbacks::rings` (module doc).
pub fn hud_frame(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < pvo::LEN { return; }
    let mut out = Vec::new();
    let [px, py] = k::PANEL;
    let [hx, hy] = k::PANEL_HALF;
    let uv_panel = [[0.0, 0.0], [0.0, 128.0], [128.0, 0.0], [128.0, 128.0]];
    for q in 0..4 {
        let a = if q & 1 == 0 { 1.0 } else { -1.0 };
        let b = if q < 2 { 1.0 } else { -1.0 };
        let pos = [at(px + hx * a, py + hy * b), at(px + hx * a, py), at(px, py + hy * b), at(px, py)];
        out.push(RingPrim { fx: k::FX_BASE + 0x28, pos, uv: uv_panel, rgba: k::PANEL_RGBA, repeat: false });
    }
    let hp = c::pf(w, id, pvo::HEALTH);
    let (f, ca, cb) = if 100.0 < hp { (hp - 100.0, 0x6000_ffff, 0x6000_ff00) } else { (hp, 0x6000_00ff, 0x6000_ffff) };
    let colour = tween(f / 100.0, ca, cb);
    let dim = ((1.0 - hp / 200.0) * 32.0) as i32;
    let slice = f32::from_bits(0x3e49_0fdb);
    let [rx, ry] = k::RING_R;
    let uv_ring = [[22.0, 22.0], [10.0, 10.0], [22.0, 22.0], [0.0, 0.0]];
    for i in 0..32 {
        let rgba = if i < dim { (colour & 0xff_ffff).wrapping_add(0x2000_0000) } else { colour };
        let a0 = add_rot(i as f32 * slice, 0.0);
        let a1 = add_rot(a0, slice);
        let (x0, y0) = (((rx * a0.sin()) as i32) as f32, ((ry * -a0.cos()) as i32) as f32);
        let (x1, y1) = (((rx * a1.sin()) as i32) as f32, ((ry * -a1.cos()) as i32) as f32);
        let pos = [at(px + x1, py + y1), at(px, py), at(px + x0, py + y0), at(px, py)];
        out.push(RingPrim { fx: k::FX_BASE, pos, uv: uv_ring, rgba, repeat: false });
    }
    let n = c::pi32(w, id, pvo::LOCK);
    let t15 = w.ticks(15);
    let rgba = if t15 < n { tween((n - t15) as f32 / t15 as f32, 0x5000_ffff, 0x5000_00ff) } else { tween(n as f32 / t15 as f32, 0x5000_ff00, 0x5000_ffff) };
    let uv_ret = [[0.0, 0.0], [0.0, 64.0], [64.0, 0.0], [64.0, 64.0]];
    let [qx, qy] = k::RETICLE_R;
    let corners = |base: f32| -> [[f32; 2]; 4] {
        [0.0, HALF_PI, -HALF_PI, PI].map(|o| {
            let a = add_rot(add_rot(base, o), 0.0);
            at16(a.cos() * qx, a.sin() * qy)
        })
    };
    let yaw4 = w.m(id).rotation[2] * 4.0;
    out.push(RingPrim { fx: k::FX_RETICLE_YAW, pos: corners(yaw4), uv: uv_ret, rgba, repeat: false });
    let pitch8 = c::pf(w, id, pvo::PITCH) * 8.0;
    out.push(RingPrim { fx: k::FX_RETICLE_PITCH, pos: corners(pitch8), uv: uv_ret, rgba, repeat: false });
    w.svc.draw_callbacks.rings.extend(out);
}

/// `0x304d98` (list 2): the red screen's GS RGBA while the death timer is under `ticks(80)` (draw only; its draw is
/// G-REN-030: no class tint path in `rc-engine`).
pub fn tint(table: &crate::moby_runtime::MobyTable, svc: &crate::moby_update::Services, id: MobyId) -> Option<u32> {
    let m = table.mobys.get(id)?;
    if m.pvars.len() < pvo::LEN { return None; }
    let n = svc.ticks(80);
    let t = crate::moby_update::services::pvar::i16(&m.pvars, pvo::DEATH_T) as i32;
    if n <= t || n == 0 { return None; }
    let a = ((n - t) * 0x50) / n;
    Some(0xaa | (a as u32) << 24)
}
