//! **The fleet's ship, class 1379** (level17 `0x2ed018`, census U568, one instance): Ratchet's flown ship over Drek's
//! fleet. Standing by it he gets the "Enter" prompt; △ fades to black and he flies it (hero state 0x32, the
//! ridden-vehicle record [`crate::vehicle`]): the stick steers, ✕ boosts, □ / L1 fire the lasers 1009
//! ([`super::ship_laser`]), ○ / R1 the missiles 295 ([`super::ship_missile`]) at the HUD's lock. It is the level's copy
//! of Gemlik's ship ([`super::gemlik_ship`]): the same flight, steering pull and camera, with the play area's edge bend
//! of Pokitaru's jet (the stick bent back inside cuboid +0x12c, its floor the cuboid's bottom), one group and one class
//! (805) toggled instead of the scenery lists, no snapping of the camera at the mount and no script-camera check. The
//! HUD ([`super::fleet_ship_hud`], the draw callback `0x2eb9a8`) counts the turrets 347 left ([`super::fleet_turret`]);
//! none left: 3 s of flight, the fade, the landing: the mission done, the checkpoint. The level and the turrets 347
//! wreck it at once. Read from the level17 decomp (`0x2ed018` and its helpers `0x2ec360` / `0x2edf10` / `0x2ec000` /
//! `0x2ec150` / `0x2ec270` / `0x2ea688`) and disassembly; the level data from the overlay (gp 0x166c00). Native `f32`.
//!
//! **System or not.** Per-class code: the level's copy of Gemlik's ship, sharing the code that is the same (the
//! springs, the steering pull `0x2ec270` = `0x2ba088`, the exhaust `0x2ea688` = `0x2b8590` with the same descriptors,
//! the screen point). On the shared mechanisms: the vehicle record, the mounted hero state 0x32, the script camera,
//! the view tangent, the occlusion fallback, the group / class setters (`story::{group_set, class_list_set}`).
//!
//! **Pvar block** (0x134; the cuboid / group words come with the instance): as Gemlik's ship's to +0x10c (+0x00 base
//! velocity, +0x10 heading, +0x20 camera Euler, +0x30 / +0x34 / +0x38 roll / pitch / yaw, +0x40 / +0x50 the start,
//! +0x60 view, +0x61 gun side, +0x62 (s16) wreck ticks, +0x64 speed, +0x68 start-up, +0x6a missile joint, +0x6c / +0x70
//! the stick, +0x80 / +0x84 the cooldowns, +0x88 / +0x8c the lock and its timer, +0x90.. spring rates, +0xac / +0xb0
//! the field of view, +0xb4 the camera offset, +0xc4 boost, +0xdc scrape timer, +0xe0 / +0xe4 crosshair, +0xe8 the
//! targets left, +0xec missile target, +0xf0 / +0xf4 gauge, +0xf8 the landing timer, +0xfc exit, +0x100 landing, +0x104
//! the HUD set, +0x108 / +0x10c the engine / boost voices), then +0x110 the steering cuboid, +0x114 the arrow's pulse,
//! +0x118 the lock-lost count, +0x11c the group, +0x120 the mount cuboid, +0x124 −1, +0x128 the lock's class, +0x12c
//! the play area's cuboid, +0x130 the fade.
//!
//! ## Coverage (`0x2ed018`)
//! | address | what | port |
//! |---|---|---|
//! | entry | `FastDecTimer(+0xdc)`; state not 7 / 1 / 2: `MobyGetHitMessage(m, 0x30001, 0)`; in state 4: damage below the health → health −= it; the template's +0x30 & 1 → the up shake (1, `ticks(4)`), the push clamped to 3·dt added to the position, the velocity reflected off it, yaw / pitch turned by 0.1·damage·0.05 | [`hits`] |
//! | entry | the fatal hit: the up shake (0.2, `ticks(5)`), health 0, → 7, +0x62 = `ticks(30)`, Ratchet 0.2 up, his moby +0x98 = 0, `SetState(0, 1)`, the record's class −1 / moby none, `SpawnBeamExplosion(0, 0, 4, 2, 9, 1, 15, m, v, 0, 10, 3, 16, −1, 1)`, mode \|= 0x41 | [`hits`] |
//! | entry | hit slot 0xff; the record names this ship → 0x140958 = `trunc(health / 256 · 200)` (the edge bit 2 is never cleared here) | [`update`] |
//! | 0 | → 1; +0x60 = 0; scale; +0x20 / +0x24 = +0x30 / +0x34; FOV 2·atan(1, tan_x) (+0xb0 / +0xac), +0xc0 = 0; +0x30.., +0x50.. = rotation, +0x40.. = position; the gauge's CLUT saved (n/a); +0xe8 = +0xf8 = 1; the mission done → 9; group +0x11c (> 0) off (`0x2539c8(g, 0, 0, 0)`); class 805 (`0x253a90(0x325, 1, 1, −1)`); +0x124 = −1 | [`init`] (`story::{group_set, class_list_set}`) |
//! | 1 | the sphere gap below 2 and `try_set_help_message(7, 0x53e4)` held, △ pressed → `PromptRelease(7)`, → 2, +0x124 = −1; else the fade back; the blob shadow (`0x26eec8(1.5, m)`) | [`wait`] (`crate::shadows::blob`) |
//! | 2 | `Approach(1, 4·dt, +0x130)`, the fade; at 1: the record (class, missiles 10 / 20, 20, 1, health 255 / 256, 100, 3, 3, 3, bits 0), +0xf0 = 0, +0xf4 = 0xff; `MusicRequestTrack(2, 4)`; the mount cuboid +0x120 → `HeroTeleport(it, 0x32, 1)`, the ship there | [`mount`] ([`crate::vehicle::Record::take`]) |
//! | 2 | `force_help_message(6, 0)`; `SetState(0x32, 1)`; `CameraScript(position, rotation, 1, 0, 0)`; +0x68 = 0, speed 0.1, the stick / cooldowns 0; distances 0xff; +0x38 = +0x28 = rot.z; the offset (−15/4, 0, 1); Ratchet's moby +0x98 = −1; +0x00 = polar(0.1, yaw, −pitch); group on (1, 1, 1), class 805 (0, 0, −1); z + 3; scale / 4; Ratchet hidden; the engine voice; the help box suspended; 0x15f608 = 2; the flight; → 4 | [`mount`] |
//! | 4 | `force_help_message(6, 0)`; 0x15f608 = 2; the engine voice restarted when gone; the flight; +0xe8 < 1 → +0xf8 = 180, → 8; the fade back; the record's bit 0 → 5 | [`update`] |
//! | 5 | Ratchet shown, +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`, `FUN_0024b090(0x14095c, 0)`; the start; scale; `fun_0020e098`; `HeroTeleport` to the exit +0xfc; `MusicRequestTrack(0, 5)`; the voices released; → 1; fade 0.99 | [`quit`] (`FUN_0024b090`: n/a [L]) |
//! | 7 | `force_help_message(6, 0)`; 0x15f608 = 2; `FastDecTimer(+0x62)` out: the help box resumed (`0x225a88`), +0x98 = 0, the death sequence, `CameraScript2(0)`, `MusicRequestTrack(0, 5)`, the start, `fun_0020e098`, the voices released, → 0 | [`update`] (`HeroCall::Death`) |
//! | 8 | `force_help_message(6, 0)`; 0x15f608 = 2; the flight; `FastDecTimer(+0xf8)` out → `Approach(1, 4·dt)`, the fade; at 1: Ratchet shown, +0x98 = 0, `SetState(0, 1)`, `CameraScript2(0)`, `MusicRequestTrack(0, 5)`; at the landing cuboid +0x100 (0.57318 up) or the start; scale; `fun_0020e098`; with the exit: group off, class 805 (1, 1, −1), `HeroTeleport` there; `SetMissionDone(+0xb0)`; the voices released; the checkpoint at Ratchet (`0x29ac10`); → 9 | [`land`] (`checkpoint::record`) |
//! | 9 | +0x130 = 0 → `DeleteMoby`; else the fade back | [`update`] |
//! | tail | the position clamped to [15, 1008]; in state 4 Ratchet's position / rotation = the ship's; states 3 / 4: the exhaust (`0x2ea688`) | [`update`], [`super::gemlik_ship::exhaust`] |
//!
//! ## Coverage (the helpers)
//! | address | what | port |
//! |---|---|---|
//! | 0x2ec360 | Gemlik's flight `0x2ba178` with the level's constants (the same values), and: the edge bend (`0x2edf10`) before the springs; the camera offset and Euler always sprung (Gemlik's state-2 set dropped); no `CameraScript` check; a crash into class 0x15b (the turrets) as into the level; the wreck: `SpawnBeamExplosion(…, −1, 1)`, +0x62 = 30, → 7 (the world also mode \|= 0x41, the scrape's fatal case not); the scrape timer 10; the HUD callback `0x2eb9a8` | [`fly`] |
//! | 0x2edf10 | as the jet's `0x313f60` with the area cuboid +0x12c: the wall (\|row 0\|xy); the top (centre z + row 2's z, the absolute height ahead); the floor the cuboid's bottom (centre z − row 2's z): below it + 10 the stick pulls up by clamp((bottom − z + 10)/10)·clamp((pitch + 30°)/30°); the record's bit 2 | [`edge_bend`] |
//! | 0x2ec000 | Gemlik's guns `0x2b9df8` (the class sound first; the laser `0x2e2128` = `0x3025f8`; the cooldown 4) | [`guns`] (`ship_laser::spawn`) |
//! | 0x2ec150 | Gemlik's missiles `0x2b9f58` (the missile `0x2c9b38` = `0x2e6a58`; the target +0xec; 100·dt; the cooldown 30) | [`missiles`] (`ship_missile::spawn`) |
//! | 0x2ec270 | `0x2ba088` | [`super::gemlik_ship::steer`] |

use super::gemlik_ship::{spring, spring_angle, steer, STEER_POINT};
use crate::moby_runtime::MobyId;
use crate::moby_update::classes::draw_callbacks::Callback;
use crate::moby_update::creature::fx::{self, polar};
use crate::moby_update::creature::{self as c, add, add_rot, atan, dist2, set_len3, sub_rot, DT, SPEED};
use crate::moby_update::services::{fl, pf as to_pf, HeroCall, HeroPose, World};
use crate::moby_update::story;

pub const REFERENCE_LEVEL: u32 = 17;
pub const UPDATE_FN: u32 = 0x2e_d018;
pub const CLASS: i16 = 0x563;
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
    pub const GROUP: usize = 0x11c;
    pub const MOUNT_CUBOID: usize = 0x120;
    pub const F124: usize = 0x124;
    pub const LOCK_CLASS: usize = 0x128;
    pub const AREA_CUBOID: usize = 0x12c;
    pub const FADE: usize = 0x130;
    pub const LEN: usize = 0x134;
}

/// The record's values at the mount (Gemlik's).
const MOUNT: crate::vehicle::Mount = crate::vehicle::Mount { missiles: 10, missiles_max: 20, b48: 20, b49: 3, b4a: 3, b4b: 3, health: 255.0, health_max: 256.0, f54: 100.0, b5e: 1 };
/// The class toggled with the flight (805).
const TOGGLED: [i16; 1] = [0x325];
/// The classes a scrape ignores (Gemlik's list).
const NO_SCRAPE: [i16; 6] = [0, 0x192, 0x4c2, 0x4c3, 0x52, 0x53];
/// The turrets: a crash into one wrecks the ship as the level does.
const WRECKS: i16 = 0x15b;
/// The "Enter" prompt and its owner.
const PROMPT: i32 = 0x53e4;
const PROMPT_OWNER: i32 = 7;

mod pad {
    pub const TRIANGLE: u32 = 0x10;
    pub const CROSS: u32 = 0x40;
    pub const MISSILE: u32 = 0x28;
    pub const GUN: u32 = 0x84;
    pub const L3: u32 = 0x200;
}

const DEG80: f32 = 1.396_263_4;
const DEG80_B: f32 = f32::from_bits(0x3fb2_b8c2);
const DEG30: f32 = std::f32::consts::FRAC_PI_6;
const STEP: f32 = 0.012_271_847;

fn pf(w: &World, id: MobyId, o: usize) -> f32 { c::pf(w, id, o) }
fn set(w: &mut World, id: MobyId, o: usize, v: f32) { c::set_pf(w, id, o, v) }
fn pi(w: &World, id: MobyId, o: usize) -> i32 { c::pi32(w, id, o) }
fn seti(w: &mut World, id: MobyId, o: usize, v: i32) { c::set_pi32(w, id, o, v) }
fn link(w: &World, id: MobyId, o: usize) -> Option<MobyId> { usize::try_from(pi(w, id, o) - 1).ok().filter(|&m| m < w.table.mobys.len()) }
fn row(name: u32) -> Option<u16> { super::row(REFERENCE_LEVEL, name) }
fn music(w: &mut World, track: i16, stinger: i16) { if let Some(s) = w.sound.as_deref_mut() { s.music_request(track, stinger); } }

fn release_voice(w: &mut World, id: MobyId, o: usize) {
    let s = pi(w, id, o);
    if s != -1 { w.release_sound(s, id); }
    seti(w, id, o, -1);
}

fn release_voices(w: &mut World, id: MobyId) {
    release_voice(w, id, pvo::ENGINE_VOICE);
    release_voice(w, id, pvo::BOOST_VOICE);
}

fn fade(w: &mut World, id: MobyId, to: f32) -> f32 {
    let mut f = pf(w, id, pvo::FADE);
    c::turn::approach(to, DT * 4.0, &mut f);
    set(w, id, pvo::FADE, f);
    crate::cinematic::set_fade(w, f);
    f
}

fn ratchet_collision(w: &mut World, bits: u32) { if let Some(h) = w.hero_moby { w.mm(h).coll_disable = bits; } }
fn class_scale(w: &World) -> f32 { fl(w.class_scale(CLASS)) }

fn restore_start(w: &mut World, id: MobyId) {
    let p = c::pv4(w, id, pvo::START_POS);
    let r = c::pv4(w, id, pvo::START_ROT);
    let m = w.mm(id);
    m.position = p;
    m.rotation = r;
}

/// The flight's group and class: `flying` their flight settings, else the walk's.
fn scenery(w: &mut World, id: MobyId, flying: bool) {
    let g = pi(w, id, pvo::GROUP);
    let v = flying as i32;
    if 0 < g { story::group_set(w, g, v, v, v); }
    story::class_list_set(w, &TOGGLED, 1 - v, 1 - v, -1);
}

fn dismount(w: &mut World) {
    w.hero_fields_mut().hero_hidden = Some(0);
    ratchet_collision(w, 0);
    crate::cinematic::hero_state(w, 0, true);
    crate::cinematic::camera_script2(w, 0);
}

/// Level17 `0x2ed018` (module doc).
pub fn update(w: &mut World, id: MobyId) {
    story::pvars(w, id, pvo::LEN);
    c::dec_timer_pvar_i32(w, id, pvo::SCRAPE_T);
    hits(w, id);
    w.mm(id).hit_slot = 0xff;
    if w.svc.vehicle.is(id) { w.svc.vehicle.hud = ((w.svc.vehicle.health / 256.0) * 200.0) as i32; }
    match w.m(id).state {
        0 => init(w, id),
        1 => wait(w, id),
        2 => mount(w, id),
        4 => {
            w.svc.interact.force_prompt(6, 0);
            w.svc.occlusion_fallback = Some((w.counter, 2));
            let v = pi(w, id, pvo::ENGINE_VOICE);
            if !w.sound_alive(v, id) {
                seti(w, id, pvo::ENGINE_VOICE, -1);
                let s = w.play_sound(0, 4, id);
                seti(w, id, pvo::ENGINE_VOICE, s);
            }
            fly(w, id);
            if pi(w, id, pvo::TARGETS) < 1 {
                seti(w, id, pvo::LAND_T, 180);
                w.mm(id).state = 8;
            }
            if pf(w, id, pvo::FADE) != 0.0 { fade(w, id, 0.0); }
            if w.svc.vehicle.quit & 1 != 0 { w.mm(id).state = 5; }
        }
        5 => quit(w, id),
        7 => {
            w.svc.interact.force_prompt(6, 0);
            w.svc.occlusion_fallback = Some((w.counter, 2));
            if c::dec_timer_pvar_s16(w, id, pvo::WRECK_T) != 0 {
                w.svc.help.resume();
                ratchet_collision(w, 0);
                w.hero_fields_mut().call(HeroCall::Death);
                crate::cinematic::camera_script2(w, 0);
                music(w, 0, 5);
                restore_start(w, id);
                crate::moby_update::creature::react::sphere_lerp(w, id);
                release_voices(w, id);
                w.mm(id).state = 0;
            }
        }
        8 => land(w, id),
        9 => {
            if pf(w, id, pvo::FADE) == 0.0 {
                w.delete_moby(id);
                return;
            }
            fade(w, id, 0.0);
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
    if st == 3 || st == 4 { super::gemlik_ship::exhaust(w, id); }
}

/// The hits at the top of `0x2ed018` (module doc).
fn hits(w: &mut World, id: MobyId) {
    if matches!(w.m(id).state, 7 | 1 | 2) { return; }
    let Some(h) = w.get_hit(id, 0x3_0001, false) else { return };
    if w.m(id).state != 4 { return; }
    let dmg = fl(h.damage);
    let (yaw, pitch, speed) = (pf(w, id, pvo::YAW), pf(w, id, pvo::PITCH), pf(w, id, pvo::SPEED));
    let a = polar(speed, yaw, -pitch);
    if dmg < w.svc.vehicle.health {
        w.svc.vehicle.health -= dmg;
        if h.w30 & 1 != 0 {
            let t = w.ticks(4);
            w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: 1.0, ticks: t });
            let k = dmg * 0.05;
            let push = c::clamp_len3(crate::moby_update::services::fv(h.dir), DT * 3.0);
            let p = add(w.m(id).position, push);
            w.mm(id).position = p;
            let r = crate::hero::guns::reflect([a[0], a[1], a[2]], [push[0], push[1], push[2]]);
            set(w, id, pvo::YAW, add_rot(yaw, sub_rot(atan(r[0], r[1]), yaw) * 0.1 * k));
            let xy = (r[0] * r[0] + r[1] * r[1]).sqrt();
            set(w, id, pvo::PITCH, add_rot(pitch, sub_rot(atan(xy, r[2]), pitch) * 0.1 * k));
        }
    } else {
        let t = w.ticks(5);
        w.shake_camera(crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: f32::from_bits(0x3e4c_cccd), ticks: t });
        w.svc.vehicle.health = 0.0;
        w.mm(id).state = 7;
        let t = w.ticks(0x1e) as i16;
        c::set_pi16(w, id, pvo::WRECK_T, t);
        let hp = w.hero_point();
        w.hero_fields_mut().pose = Some(HeroPose { pos: [hp[0], hp[1], hp[2] + 0.2], yaw, target_yaw: yaw });
        ratchet_collision(w, 0);
        crate::cinematic::hero_state(w, 0, true);
        w.svc.vehicle.class = -1;
        w.svc.vehicle.moby = None;
        let b = fx::Beam { damage_r: 0.0, damage: 0.0, flash: 4.0, flash2: 2.0, flash_dist: 9.0, scale: 1.0, light: 15.0, streaks: 10, sparks: 3, puffs: 16, debris: 0, sound: -1, shake: true };
        let p = w.m(id).position;
        fx::beam_explosion(w, &b, Some(id), p);
        w.mm(id).mode |= 0x41;
    }
}

/// State 0 (module doc).
fn init(w: &mut World, id: MobyId) {
    w.mm(id).state = 1;
    c::set_pu8(w, id, pvo::VIEW, 0);
    let s = class_scale(w);
    w.mm(id).scale = s;
    let (r0, r1) = (pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH));
    set(w, id, pvo::CAM_EULER, r0);
    set(w, id, pvo::CAM_EULER + 4, r1);
    let f = atan(1.0, w.svc.view_tan_x);
    set(w, id, pvo::FOV_V, 0.0);
    set(w, id, pvo::FOV, f + f);
    set(w, id, pvo::FOV0, f + f);
    let r = w.m(id).rotation;
    c::set_pv4(w, id, pvo::ROLL, r);
    c::set_pv4(w, id, pvo::START_ROT, r);
    let p = w.m(id).position;
    c::set_pv4(w, id, pvo::START_POS, p);
    seti(w, id, pvo::TARGETS, 1);
    seti(w, id, pvo::LAND_T, 1);
    let mission = w.m(id).mission;
    if mission != 0xff && story::mission_done(w, mission as i32) { w.mm(id).state = 9; }
    let g = pi(w, id, pvo::GROUP);
    if 0 < g { story::group_set(w, g, 0, 0, 0); }
    story::class_list_set(w, &TOGGLED, 1, 1, -1);
    seti(w, id, pvo::F124, -1);
}

/// State 1 (module doc).
fn wait(w: &mut World, id: MobyId) {
    let mut go = false;
    if let Some(h) = w.hero_moby {
        if crate::moby_update::classes::pickup::sphere_gap(w, id, h) < 2.0 && w.svc.interact.try_prompt(PROMPT_OWNER, PROMPT) != 0 {
            go = w.hero.loop_in.pad.pressed & pad::TRIANGLE != 0;
        }
    }
    if go {
        let up = w.svc.interact.prompt_hud;
        w.svc.interact.prompt.release(PROMPT_OWNER, up);
        w.mm(id).state = 2;
        seti(w, id, pvo::F124, -1);
    } else if pf(w, id, pvo::FADE) != 0.0 {
        fade(w, id, 0.0);
    }
    // `0x26eec8(1.5, m)`: the blob shadow.
    crate::shadows::blob(w, 1.5, id);
}

/// State 2 (module doc).
fn mount(w: &mut World, id: MobyId) {
    if fade(w, id, 1.0) != 1.0 { return; }
    w.svc.vehicle.take(id, CLASS, &MOUNT);
    set(w, id, pvo::GAUGE, 0.0);
    seti(w, id, pvo::GAUGE_CUT, 0xff);
    music(w, 2, 4);
    if let Some((pos, rot)) = story::cuboid(w, pi(w, id, pvo::MOUNT_CUBOID)) {
        crate::cinematic::hero_teleport(w, pos, rot, 0x32, true);
        let m = w.mm(id);
        m.position = [pos[0], pos[1], pos[2], m.position[3]];
        m.rotation = [rot[0], rot[1], rot[2], m.rotation[3]];
    }
    w.svc.interact.force_prompt(6, 0);
    crate::cinematic::hero_state(w, 0x32, true);
    let (p, r) = { let m = w.m(id); ([m.position[0], m.position[1], m.position[2]], [m.rotation[0], m.rotation[1], m.rotation[2]]) };
    crate::cinematic::camera_script(w, p, r, 1, 0, false);
    c::set_pi16(w, id, pvo::STARTUP, 0);
    set(w, id, pvo::SPEED, SPEED * 0.1);
    for o in [pvo::STICK_X, pvo::STICK_Y, pvo::MISSILE_T, pvo::GUN_T] { seti(w, id, o, 0); }
    {
        let m = w.mm(id);
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
    }
    let rz = w.m(id).rotation[2];
    set(w, id, pvo::YAW, rz);
    set(w, id, pvo::CAM_EULER + 8, rz);
    set(w, id, pvo::CAM_OFF, -15.0 * 0.25);
    set(w, id, pvo::CAM_OFF + 8, 4.0 * 0.25);
    set(w, id, pvo::CAM_OFF + 4, 0.0);
    ratchet_collision(w, 0xffff_ffff);
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
    w.svc.help.suspend();
    w.svc.occlusion_fallback = Some((w.counter, 2));
    fly(w, id);
    w.mm(id).state = 4;
}

/// State 5 (module doc).
fn quit(w: &mut World, id: MobyId) {
    dismount(w);
    restore_start(w, id);
    let s = class_scale(w);
    w.mm(id).scale = s;
    crate::moby_update::creature::react::sphere_lerp(w, id);
    if let Some((pos, rot)) = story::cuboid(w, pi(w, id, pvo::EXIT_CUBOID)) { crate::cinematic::hero_teleport(w, pos, rot, 0, true); }
    music(w, 0, 5);
    release_voices(w, id);
    w.mm(id).state = 1;
    set(w, id, pvo::FADE, f32::from_bits(0x3f7d_70a4));
    crate::cinematic::set_fade(w, 0.99);
}

/// State 8 (module doc).
fn land(w: &mut World, id: MobyId) {
    w.svc.interact.force_prompt(6, 0);
    w.svc.occlusion_fallback = Some((w.counter, 2));
    fly(w, id);
    if c::dec_timer_pvar_i32(w, id, pvo::LAND_T) != 0 { fade(w, id, 1.0); }
    if pf(w, id, pvo::FADE) < 1.0 { return; }
    dismount(w);
    music(w, 0, 5);
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

/// `0x2edf10(m, &x, &y)` (module doc).
fn edge_bend(w: &mut World, id: MobyId, x: &mut f32, y: &mut f32) {
    let Some(s) = w.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, pi(w, id, pvo::AREA_CUBOID)) else { return };
    let (m, centre) = (s.matrix, s.centre());
    let k = pf(w, id, pvo::SPEED) * w.ticks(0x3c) as f32;
    let ahead = add(c::pv4(w, id, pvo::HEADING).map(|q| q * k), w.m(id).position);
    let d = [ahead[0] - centre[0], ahead[1] - centre[1]];
    let r = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let big_r = (m[0][0] * m[0][0] + m[0][1] * m[0][1]).sqrt();
    let (top, bottom) = (m[2][2] + centre[2], -m[2][2] + centre[2]);
    let a = add_rot(atan(d[0], d[1]), f32::from_bits(0x4049_0fd0));
    if big_r - 10.0 < r {
        let k = (((r - big_r) + 10.0) / 10.0).clamp(0.0, 1.0);
        let side = if 0.0 < sub_rot(a, pf(w, id, pvo::YAW)) { -1.0 } else { 1.0 };
        *x += (side - *x) * k;
        w.svc.vehicle.quit |= 2;
    }
    let z = ahead[2];
    let pitch = pf(w, id, pvo::PITCH);
    let (target, f) = if top - 10.0 < z {
        let k = (((z - top) + 10.0) / 10.0).clamp(0.0, 1.0);
        (-1.0, k * ((DEG30 - pitch) / DEG30).clamp(0.0, 1.0))
    } else {
        if bottom + 10.0 <= z { return; }
        let k = (((bottom - z) + 10.0) / 10.0).clamp(0.0, 1.0);
        (1.0, k * ((pitch + DEG30) / DEG30).clamp(0.0, 1.0))
    };
    *y += (target - *y) * f;
    w.svc.vehicle.quit |= 2;
}

/// `0x2ec360`: one tick of flight (module doc).
fn fly(w: &mut World, id: MobyId) {
    w.mm(id).mode &= !0x100;
    let mut speed = pf(w, id, pvo::SPEED);
    let t = c::pi16(w, id, pvo::STARTUP) as i32;
    if 120 < t {
        let (pressed, released, raw) = { let p = &w.hero.loop_in.pad; (p.raw_pressed, p.raw_released, p.raw) };
        if pressed & pad::CROSS != 0 {
            release_voice(w, id, pvo::BOOST_VOICE);
            let s = w.play_sound(1, 4, id);
            seti(w, id, pvo::BOOST_VOICE, s);
        } else if released & pad::CROSS != 0 {
            release_voice(w, id, pvo::BOOST_VOICE);
        }
        if raw & pad::CROSS == 0 {
            let target = 0.4;
            if speed < target - 0.025 {
                speed += 0.025;
            } else if speed <= target {
                speed = target;
            } else {
                speed += (target - speed) * 0.1;
            }
        } else {
            speed += 0.035;
            if 0.5 < speed { speed = 0.5; }
        }
    } else {
        c::set_pi16(w, id, pvo::STARTUP, (t + 1) as i16);
        speed += 0.003;
    }
    set(w, id, pvo::SPEED, speed);
    if 5 < c::pi16(w, id, pvo::STARTUP) as i32 {
        let [sx, sy] = w.hero.stick.map(|x| f32::from_bits(x.0));
        let (mut kx, mut ky, mut x, mut y) = (0.1f32, 0.25f32, sx, sy);
        if w.hero.loop_in.pad.held & pad::L3 != 0 {
            kx = 0.08;
            ky = 0.08;
            y = (sy / STEP) * 0.015_707_964;
            x = (sx / STEP) * 0.015_707_964;
        }
        edge_bend(w, id, &mut x, &mut y);
        spring(w, id, x, 0.03, 1.0, kx, pvo::STICK_X, pvo::STICK_XV);
        spring(w, id, y, 0.03, 1.0, ky, pvo::STICK_Y, pvo::STICK_YV);
        let s = pf(w, id, pvo::STICK_X);
        spring_angle(w, id, s, 0.05, 1.0, 1.0, pvo::ROLL, pvo::ROLL_V);
        let p = sub_rot(pf(w, id, pvo::PITCH), pf(w, id, pvo::STICK_Y) * STEP);
        set(w, id, pvo::PITCH, p);
        let yw = sub_rot(pf(w, id, pvo::YAW), pf(w, id, pvo::STICK_X) * STEP);
        set(w, id, pvo::YAW, yw);
        let p = pf(w, id, pvo::PITCH);
        if DEG80 < p { set(w, id, pvo::PITCH, DEG80_B) } else if p < -DEG80 { set(w, id, pvo::PITCH, -DEG80_B) }
        let ry = w.m(id).rotation[1];
        if DEG80 < ry { w.mm(id).rotation[1] = DEG80_B } else if ry < -DEG80 { w.mm(id).rotation[1] = -DEG80_B }
    }
    // 0x2ec270: the steering pull (Gemlik's `0x2ba088`).
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
    let yv = steer(d0, pf(w, id, pvo::YAW), to, 50.0, hi_xy, true, 5);
    set(w, id, pvo::YAW, yv);
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
    let f = speed - 0.4;
    let f = if f < 0.0 { 0.0 / (0.5 - 0.4) } else { f / (0.5 - 0.4) };
    set(w, id, pvo::BOOST, f);
    let (sx, sy) = (pf(w, id, pvo::STICK_X), pf(w, id, pvo::STICK_Y));
    let chase = [((-15.0 - sy) - f * 4.0) * 0.25, sx * -3.0 * 0.25, (4.0 + sy * 2.0) * 0.25];
    let cockpit = [-5.1 * 0.25, 0.0, f32::from_bits(0x4026_6666) * 0.25];
    let off = if c::pu8(w, id, pvo::VIEW) & 1 == 0 { chase } else { cockpit };
    for (k, &o) in off.iter().enumerate() { spring(w, id, o, 0.15, 0.8, 1.0, pvo::CAM_OFF + 4 * k, pvo::CAM_OFF_V + 4 * k); }
    let o = [pf(w, id, pvo::CAM_OFF), pf(w, id, pvo::CAM_OFF + 4), pf(w, id, pvo::CAM_OFF + 8)];
    let rows = super::ship_fighter::rows_of([pf(w, id, pvo::ROLL), pf(w, id, pvo::PITCH), pf(w, id, pvo::YAW)]);
    let pos = w.m(id).position;
    let eye: [f32; 3] = std::array::from_fn(|k| o[0] * rows[0][k] + o[1] * rows[1][k] + o[2] * rows[2][k] + pos[k]);
    crate::cinematic::camera_targets(w, Some(eye), None);
    let ex = add_rot(pf(w, id, pvo::ROLL) * 0.5, sx * 0.031_25);
    let ey = sub_rot(add_rot(pf(w, id, pvo::PITCH), f32::from_bits(0x3dc9_0fdb)), sy * STEP);
    let ez = pf(w, id, pvo::YAW);
    spring_angle(w, id, ex, 0.15, 0.8, 1.0, pvo::CAM_EULER, pvo::CAM_EULER_V);
    spring_angle(w, id, ey, 0.15, 0.8, 1.0, pvo::CAM_EULER + 4, pvo::CAM_EULER_V + 4);
    spring_angle(w, id, ez, 0.15, 0.8, 1.0, pvo::CAM_EULER + 8, pvo::CAM_EULER_V + 8);
    let euler = [pf(w, id, pvo::CAM_EULER), pf(w, id, pvo::CAM_EULER + 4), pf(w, id, pvo::CAM_EULER + 8)];
    crate::cinematic::camera_targets(w, None, Some(euler));
    spring(w, id, f32::from_bits(0x3f91_361e) + (f32::from_bits(0x3fbd_e44e) - f32::from_bits(0x3f91_361e)) * f, 0.05, 1.0, 1.0, pvo::FOV, pvo::FOV_V);
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
            set(w, id, pvo::YAW, add_rot(yw, sub_rot(atan(v[0], v[1]), yw) * 0.1));
            let xy = (v[0] * v[0] + v[1] * v[1]).sqrt();
            let p = pf(w, id, pvo::PITCH);
            set(w, id, pvo::PITCH, add_rot(p, sub_rot(-atan(xy, v[2]), p) * 0.1));
            set(w, id, pvo::SPEED, pf(w, id, pvo::SPEED) * 0.7);
            let blast = fx::Beam { damage_r: 8.0, damage: 100.0, flash: 16.0, flash2: 8.0, flash_dist: 9.0, scale: 1.0, light: 50.0, streaks: 30, sparks: 10, puffs: 24, debris: 0, sound: -1, shake: true };
            if class.is_none_or(|k| k == WRECKS) {
                w.svc.vehicle.health = 0.0;
                let p = w.m(id).position;
                fx::beam_explosion(w, &blast, Some(id), p);
                c::set_pi16(w, id, pvo::WRECK_T, 30);
                w.mm(id).state = 7;
                w.mm(id).mode |= 0x41;
            } else if pi(w, id, pvo::SCRAPE_T) == 0 {
                w.svc.vehicle.health -= 5.0;
                if w.svc.vehicle.health < 0.0 {
                    let p = w.m(id).position;
                    fx::beam_explosion(w, &blast, Some(id), p);
                    w.mm(id).state = 7;
                }
                seti(w, id, pvo::SCRAPE_T, 10);
            }
        }
    }
    if let Some(i) = row(super::fleet_ship_hud::HUD_FN) { w.svc.draw_callbacks.register(Callback::UnitFrame(i), id); }
    missiles(w, id);
    let yaw = add_rot(pf(w, id, pvo::CAM_EULER + 8), -pf(w, id, pvo::STICK_X) / 7.0);
    let pitch = add_rot(pf(w, id, pvo::CAM_EULER + 4), -(pf(w, id, pvo::STICK_Y) / 7.0) - 0.1);
    guns(w, id, yaw, -pitch);
}

/// `0x2ec150` (module doc).
fn missiles(w: &mut World, id: MobyId) {
    if c::dec_timer_pvar_i32(w, id, pvo::MISSILE_T) == 0 { return; }
    if w.hero.loop_in.pad.raw & pad::MISSILE == 0 || w.svc.vehicle.missiles == 0 { return; }
    let j = (c::pi16(w, id, pvo::MISSILE_JOINT) as u16).wrapping_add(1) & 3;
    c::set_pi16(w, id, pvo::MISSILE_JOINT, j as i16);
    let at = add(w.joint_point(id, j as usize + 7), set_len3(c::pv4(w, id, pvo::BASE_VEL), 0.3));
    let life = w.ticks(300);
    let target = link(w, id, pvo::MISSILE_TARGET);
    let rot = w.m(id).rotation;
    if super::ship_missile::spawn(w, f32::from_bits(0x3fd5_5555), id, at, target, rot, life).is_some() {
        w.play_sound(2, 0, id);
        w.svc.vehicle.missiles -= 1;
        seti(w, id, pvo::MISSILE_T, 30);
    }
}

/// `0x2ec000(yaw, pitch, m, pvars)` (module doc).
fn guns(w: &mut World, id: MobyId, yaw: f32, pitch: f32) {
    let aim = add(polar(f32::from_bits(0x40d5_5555), yaw, pitch), c::pv4(w, id, pvo::BASE_VEL));
    if c::dec_timer_pvar_i32(w, id, pvo::GUN_T) != 0 && w.hero.loop_in.pad.raw & pad::GUN != 0 {
        w.play_sound(3, 0, id);
        let d = set_len3(aim, f32::from_bits(0x400c_cccd));
        let side = c::pu8(w, id, pvo::GUN_SIDE) & 1;
        let muzzle = add(w.joint_point(id, side as usize + 3), d);
        super::ship_laser::spawn(w, 200.0, -1.0, id, aim, muzzle);
        seti(w, id, pvo::GUN_T, 4);
        let s = c::pu8(w, id, pvo::GUN_SIDE) ^ 1;
        c::set_pu8(w, id, pvo::GUN_SIDE, s);
    }
    let a = set_len3(aim, 23.0);
    let p = w.m(id).position;
    if let Some([x, y]) = super::gemlik_ship::screen_point(w, [a[0] + p[0], a[1] + p[1], a[2] + p[2]], 0) {
        seti(w, id, pvo::CROSS_X, x);
        seti(w, id, pvo::CROSS_Y, y);
    }
}
