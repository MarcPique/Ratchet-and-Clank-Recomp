//! Novalis's small moving props (docs/plan/creatures.md §1, "World props"): the oscillating spinners 705
//! (`OscillatingSpinnerUpdate` level01 0x2f9bf0), the elevators 703 / 715 (`ElevatorUpdate` 0x2f95c0), the sliding
//! door halves 768 / 769 (`SlidingDoorUpdate` 0x2fed68) and the shootables 1042 (`ShootableUpdate` 0x307c48). Every
//! function is read from the disassembly; standard `f32`; the `rand` draws, sounds and order are the game's. The
//! level-table survey (`lvl.vtbl` × `tools/ghidra/names/clusters.tsv`) finds these functions on level 01 only, for
//! exactly these classes.
//!
//! **705** (pvar P: +0 f32 speed −1..1, +4 f32 direction ±1, +8 s32 reversal timer). State 0: Ratchet's light
//! word and ambient (+0x38..+0x3f, `FUN_00272078`), state 1, speed `randf(−1, 1)`, direction from `rand() & 1`
//! (2 draws). Every tick: `coll_sphere_mobys(1.2, pos, 0x10, self, {attacker self, flags 0x10001})` (what touches
//! the blades gets a wrench-kind hit: crates break); at full speed in its direction (`dir == speed`) a 90-tick
//! timer, then the direction flips; speed `+= 4·dt·dir` clamped to ±1; yaw `+= −4π·dt·speed`.
//!
//! **703 / 715** (P+0xa0 home z, +0xa4 home state byte, +0xa6 s16 timer, +0xa8 f32 travel, +0xac f32 step, +0xb0
//! f32 travel time in seconds, platform block P+0x60). Every tick the timer counts down. State 0: the collision
//! descriptor bytes (+0x28 = 4, +0x3e = 0xd, +0x20 = 0, +0x24 = 0), home z, `cmd` (+0xbc) = the home state, a
//! 120-tick wait, update and draw distance 0xff, +0xb4 = −1. `cmd` 0 (at the top) / 1 (at the bottom): when the wait
//! is out, `cmd` 2 with step `±travel / ticks(trunc(time·60))` (down / up) for that many ticks, class sound 0 / 1.
//! `cmd` 2: z −= step; when the timer is out `cmd` = step < 0 ? 0 : 1, snapped to the home z when that is the home
//! state, a 120-tick wait. Last, `CarryRiders(P+0x60, pos − old pos)` (the hero rides it: `hero::platform`).
//!
//! **768 / 769** (P+0x00 home position, +0x10 f32 open fraction, +0x14 f32 radius). State 0 records the home;
//! 1 closed: Ratchet within the radius (xy) and 2 in height → 2 (opening; 768 plays sound 0); 2: open fraction
//! `+= 1/(scale·20)` → 3 at 1; 3 open: Ratchet and the camera both beyond 1.1·radius → 4 (closing; sound 0 on 768);
//! 4: Ratchet back inside → 2, else fraction `−= 1/20` → 1 at 0. Position xy = home + (cos yaw, sin yaw)·fraction·(±2:
//! 768 +, 769 −).
//!
//! **1042**: a hit with flags 0x10000 and damage > 0 deletes it.
//!
//! **701** (collapsing platforms, `CollapsingPlatformUpdate` 0x2f9080; P+0x60 fall velocity, +0x70 home, +0x80
//! trigger cuboid, +0x84 s16 rubble timer, +0x86 s16 fall timer, +0x88 s16 wait (ticks), +0x8a s16 wait timer, +0x8c
//! camera hold). Outside state 1, while the rubble timer runs, a 5 % chance a tick of one rock bit (`randi(100) < 5`:
//! drift `randf(±dt/2)` in x and y, 2–3 from home at `rand_angle`, life 180–300; `SpawnDebrisMoby` =
//! `gunship::spawn_ember`). State 0: the collision descriptor bytes, home, state 1; a platform whose spawn id is
//! collected or dead starts fallen (state 4, 20 below home). 1: Ratchet above the trigger cuboid's centre, within 10
//! (xy) of it and in movement group (0x1413dc) 0, 1 or 9 → rubble for `rand_range(ticks(300), ticks(600))`, the wait `ticks(P+0x88)`,
//! state 2, a camera shake (up, 0.4, `ticks(30)`), class sound 1. 2: the wait, then state 3 with a 45-tick fall
//! timer. 3: falls (`vz −= 30·dt²`); once the timer is out and it is 20 below home: a shake (0.1, `ticks(20)`), sound 0,
//! state 4 at home − 20, the death bits. 4: when the rubble is over and the camera hold is set: `CameraScript2(0)`,
//! `SetState(0, 1)`, the letterbox off. States 2 and 3 first look at the platform (`FUN_002f9000`, [`collapse_look`]:
//! the follow camera turned along the trigger cuboid's x row at 8° a tick, distance 7, pivot 2.5, look 3.5, rates
//! 0.02; no-ops unless the follow camera is current).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{pv, pvar as p, HitTemplate, World};
use crate::moby_update::triggers;
use crate::ps2v::Pf;

pub const SPINNER_FN: u32 = 0x2f9bf0;
pub const SPINNER_CLASSES: [i16; 1] = [705];
pub const ELEVATOR_FN: u32 = 0x2f95c0;
pub const ELEVATOR_CLASSES: [i16; 2] = [703, 715];
pub const DOOR_FN: u32 = 0x2fed68;
pub const DOOR_CLASSES: [i16; 2] = [768, 769];
pub const SHOOTABLE_FN: u32 = 0x307c48;
pub const SHOOTABLE_CLASSES: [i16; 1] = [1042];
pub const COLLAPSE_FN: u32 = 0x2f9080;
pub const COLLAPSE_CLASSES: [i16; 1] = [701];

/// `0x15ed6c`: dt.
const DT: f32 = 1.0 / 60.0;
const PI: f32 = std::f32::consts::PI;

/// `FastDecTimer__FRi` 0x220e78 (the int variant): 1 if it is 0, else `t = max(t, 1) − 1` and 2 when that is ≤ 0.
fn fast_dec_timer_i32(t: &mut i32) -> i32 {
    if *t == 0 { return 1; }
    *t = (*t).max(1) - 1;
    if *t < 1 { 2 } else { 0 }
}

fn add_rot(a: f32, b: f32) -> f32 {
    let s = a + b;
    if s >= PI || s.is_nan() { (s - PI) - PI } else if s < -PI { (s + PI) + PI } else { s }
}

// ---------------------------------------------------------------------------------------------------
// 705

/// `OscillatingSpinnerUpdate` (0x2f9bf0).
pub fn spinner_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xc { return; }
    if w.m(id).state == 0 {
        // FUN_00272078: the hero moby's light word and ambient.
        if let Some(h) = w.hero_moby {
            let (l, a) = (w.m(h).light, w.m(h).ambient);
            let m = w.mm(id);
            m.light = l;
            m.ambient = a;
        }
        w.mm(id).state = 1;
        let s = w.rng.randf(-1.0, 1.0);
        let dir = if w.rng.rand() & 1 != 0 { 1.0 } else { -1.0 };
        let pv = &mut w.mm(id).pvars;
        p::set_ff(pv, 0, s);
        p::set_ff(pv, 4, dir);
    }
    let tmpl = HitTemplate { attacker: Some(id), flags: 0x1_0001, ..Default::default() };
    let pos = pv(w.m(id).position);
    w.sphere_mobys(Pf::b(0x3f99_999a), pos, 0x10, Some(id), Some(&tmpl));
    let (speed, dir) = (p::ff(&w.m(id).pvars, 0), p::ff(&w.m(id).pvars, 4));
    if dir == speed {
        let mut t = p::i32(&w.m(id).pvars, 8);
        let fired = fast_dec_timer_i32(&mut t) != 0;
        p::set_i32(&mut w.mm(id).pvars, 8, t);
        if fired {
            let n = w.ticks(0x5a);
            let pv = &mut w.mm(id).pvars;
            p::set_ff(pv, 4, -dir);
            p::set_i32(pv, 8, n);
        }
    }
    let dir = p::ff(&w.m(id).pvars, 4);
    let s = (speed + DT * 4.0 * dir).clamp(-1.0, 1.0);
    let m = w.mm(id);
    p::set_ff(&mut m.pvars, 0, s);
    m.rotation[2] = add_rot(m.rotation[2], DT * -12.566_371 * s);
}

// ---------------------------------------------------------------------------------------------------
// 703 / 715

/// `ElevatorUpdate` (0x2f95c0).
pub fn elevator_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xb8 { return; }
    let old = w.m(id).position;
    {
        let mut t = p::i16(&w.m(id).pvars, 0xa6);
        crate::moby_update::services::fast_dec_timer_s16(&mut t);
        p::set_i16(&mut w.mm(id).pvars, 0xa6, t);
    }
    if w.m(id).state == 0 {
        let wait = w.ticks(0x78);
        let m = w.mm(id);
        let z = m.position[2];
        let pv = &mut m.pvars;
        pv[0x28] = 4;
        p::set_i16(pv, 0x3e, 0xd);
        p::set_i32(pv, 0x20, 0);
        p::set_i16(pv, 0x24, 0);
        p::set_ff(pv, 0xa0, z);
        let home = pv[0xa4];
        p::set_i16(pv, 0xa6, wait as i16);
        p::set_i32(pv, 0xb4, -1);
        m.cmd = home;
        m.state = 1;
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
    }
    let cmd = w.m(id).cmd;
    match cmd {
        0 | 1 if p::i16(&w.m(id).pvars, 0xa6) == 0 => {
            let time = p::ff(&w.m(id).pvars, 0xb0) * 60.0;
            let n = w.ticks(time as i32);
            let travel = p::ff(&w.m(id).pvars, 0xa8);
            let step = travel / n as f32;
            let n2 = w.ticks(time as i32);
            let m = w.mm(id);
            m.cmd = 2;
            p::set_ff(&mut m.pvars, 0xac, if cmd == 1 { -step } else { step });
            p::set_i16(&mut m.pvars, 0xa6, n2 as i16);
            w.play_sound(cmd as i32, 0, id);
        }
        2 => {
            let wait = w.ticks(0x78);
            let m = w.mm(id);
            m.position[2] -= p::ff(&m.pvars, 0xac);
            if p::i16(&m.pvars, 0xa6) == 0 {
                m.cmd = if p::ff(&m.pvars, 0xac) < 0.0 { 0 } else { 1 };
                if m.cmd == m.pvars[0xa4] { m.position[2] = p::ff(&m.pvars, 0xa0); }
                p::set_i16(&mut m.pvars, 0xa6, wait as i16);
            }
        }
        _ => {}
    }
    let m = w.mm(id);
    let d = [m.position[0] - old[0], m.position[1] - old[1], m.position[2] - old[2], m.position[3] - old[3]];
    let r = m.rotation;
    triggers::carry_riders(&mut m.pvars, 0x60, d, r, r);
}

// ---------------------------------------------------------------------------------------------------
// 768 / 769

/// `SlidingDoorUpdate` (0x2fed68).
pub fn door_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0x18 { return; }
    let hero = crate::hero::physics::to_f32x3(w.hero.pos);
    let cam = [w.camera[0].to_f32(), w.camera[1].to_f32(), w.camera[2].to_f32()];
    let home = p::v4f(&w.m(id).pvars, 0);
    let r = p::ff(&w.m(id).pvars, 0x14);
    let dxy = |a: [f32; 4], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
    let near = |w: &World| dxy(home, hero) < r && (w.m(id).position[2] - hero[2]).abs() < 2.0;
    let big = w.m(id).o_class == 0x300;
    let step = 1.0 / w.svc.timing.scale(Pf::f(20.0)).to_f32();
    let st = w.m(id).state;
    let mut sound = false;
    let new = match st {
        0 => {
            let m = w.mm(id);
            let pos = m.position;
            p::set_v4f(&mut m.pvars, 0, pos);
            p::set_ff(&mut m.pvars, 0x10, 0.0);
            Some(1)
        }
        1 if dxy(home, hero) < r && (w.m(id).position[2] - hero[2]).abs() < 2.0 => { sound = true; Some(2) }
        2 => {
            let f = p::ff(&w.m(id).pvars, 0x10) + step;
            if f < 1.0 { p::set_ff(&mut w.mm(id).pvars, 0x10, f); None } else { p::set_ff(&mut w.mm(id).pvars, 0x10, 1.0); Some(3) }
        }
        3 if dxy(home, hero) > r * 1.1 && dxy(home, cam) > r * 1.1 => { sound = true; Some(4) }
        4 if near(w) => Some(2),
        4 => {
            let f = p::ff(&w.m(id).pvars, 0x10) - step;
            if 0.0 < f { p::set_ff(&mut w.mm(id).pvars, 0x10, f); None } else { p::set_ff(&mut w.mm(id).pvars, 0x10, 0.0); Some(1) }
        }
        _ => None,
    };
    if let Some(s) = new { w.mm(id).state = s; }
    if sound && big { w.play_sound(0, 0, id); }
    let m = w.mm(id);
    let (home, f) = (p::v4f(&m.pvars, 0), p::ff(&m.pvars, 0x10));
    let (s, c) = m.rotation[2].sin_cos();
    let k = if big { 2.0 } else { -2.0 };
    m.position[0] = home[0] + c * f * k;
    m.position[1] = home[1] + s * f * k;
}

// ---------------------------------------------------------------------------------------------------
// 1042

/// `ShootableUpdate` (0x307c48).
pub fn shootable_update(w: &mut World, id: MobyId) {
    if let Some(h) = w.get_hit(id, 0x1_0000, false) {
        if Pf::ZERO < h.damage { w.delete_moby(id); }
    }
}

// ---------------------------------------------------------------------------------------------------
// 701

/// `FUN_002f9000` (states 2 and 3, before the rest): the follow camera turned along the trigger cuboid's x row (rotated
/// about its z row by the level's gp word 0x161c28 = 0: the row itself) at 8° a tick (`0x313af0(0x3e0efa35, 0, dir)`),
/// distance 7 (`0x313628(7, 0.02, 0)`), pivot height 2.5 (`0x313690(2.5, 0.02)`), look height 3.5
/// (`0x3136c8(3.5, 0.02, 0)`) — the follow camera's one-tick setters, through the moby → camera channel.
fn collapse_look(w: &mut World, id: MobyId) {
    let c = p::i32(&w.m(id).pvars, 0x80);
    let Some(m) = usize::try_from(c).ok().and_then(|c| w.svc.volumes.cuboids.get(c)).map(|s| s.matrix) else { return };
    let dir = [m[0][0], m[0][1], m[0][2]];
    crate::cinematic::follow_turn_toward(w, f32::from_bits(0x3e0e_fa35), 0.0, dir);
    crate::cinematic::follow_distance(w, 7.0, f32::from_bits(0x3ca3_d70a), false);
    crate::cinematic::follow_pivot_height(w, 2.5, f32::from_bits(0x3ca3_d70a));
    crate::cinematic::follow_look_height(w, 3.5, f32::from_bits(0x3ca3_d70a), false);
}

/// `CollapsingPlatformUpdate` (0x2f9080).
pub fn collapse_update(w: &mut World, id: MobyId) {
    use crate::follow_camera::{ShakeAxis, ShakeRequest};
    use crate::moby_update::services::fast_dec_timer_s16;
    if w.m(id).pvars.len() < 0x90 { return; }
    let dec = |w: &mut World, o: usize| {
        let mut t = p::i16(&w.m(id).pvars, o);
        let r = fast_dec_timer_s16(&mut t);
        p::set_i16(&mut w.mm(id).pvars, o, t);
        r
    };
    if w.m(id).state != 1 && dec(w, 0x84) == 0 && w.rng.randi(100) < 5 {
        let vx = w.rng.randf(DT * -0.5, DT * 0.5);
        let vy = w.rng.randf(DT * -0.5, DT * 0.5);
        let r = w.rng.randf(2.0, 3.0);
        let a = w.rng.rand_angle();
        let home = p::v4f(&w.m(id).pvars, 0x70);
        let at = [a.cos() * r + home[0], a.sin() * r + home[1], home[2], home[3]];
        let class = [696, 697, 698][w.rng.randi(3) as usize];
        let life = w.rng.rand_range(0xb4, 300);
        crate::moby_update::classes::gunship::spawn_ember(w, 0.05, 1.0, 1.0, 0.75, at, [vx, vy, 0.0, 0.0], class, life, 0);
    }
    let dead = |w: &World| {
        let sid = w.m(id).spawn_id;
        w.svc.save.collected.get(&sid).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(w.svc.level, sid))
    };
    match w.m(id).state {
        0 => {
            let m = w.mm(id);
            let pos = m.position;
            let pv = &mut m.pvars;
            p::set_i16(pv, 0x3e, 5);
            p::set_i32(pv, 0x20, 0);
            p::set_i16(pv, 0x24, 0);
            pv[0x28] = 4;
            p::set_v4f(pv, 0x70, pos);
            m.state = 1;
            if dead(w) {
                let m = w.mm(id);
                p::set_v4f(&mut m.pvars, 0x60, [0.0; 4]);
                m.state = 4;
                m.position[2] = p::ff(&m.pvars, 0x78) - 20.0;
                p::set_i32(&mut m.pvars, 0x8c, -1);
            }
        }
        1 => {
            let c = p::i32(&w.m(id).pvars, 0x80);
            let hero = crate::hero::physics::to_f32x3(w.hero.pos);
            let centre = usize::try_from(c).ok().and_then(|c| w.svc.volumes.cuboids.get(c)).map(|s| s.matrix[3]);
            let Some(ctr) = centre.filter(|_| c != -1) else { return };
            let near = ctr[2] <= hero[2] && ((hero[0] - ctr[0]).powi(2) + (hero[1] - ctr[1]).powi(2)).sqrt() < 10.0;
            if near && ((w.hero.group as u32) < 2 || w.hero.group == 9) {
                let (lo, hi) = (w.ticks(300), w.ticks(600));
                let rubble = w.rng.rand_range(lo, hi);
                let wait = w.ticks(p::i16(&w.m(id).pvars, 0x88) as i32);
                let shake = w.ticks(0x1e);
                {
                    let m = w.mm(id);
                    p::set_i16(&mut m.pvars, 0x84, rubble as i16);
                    p::set_i16(&mut m.pvars, 0x8a, wait as i16);
                    m.state = 2;
                }
                w.shake_camera(ShakeRequest { axis: ShakeAxis::Up, amp: f32::from_bits(0x3ecc_cccd), ticks: shake });
                w.play_sound(1, 0, id);
            }
        }
        2 => {
            collapse_look(w, id);
            if dec(w, 0x8a) != 0 {
                let t = w.ticks(0x2d);
                let m = w.mm(id);
                p::set_v4f(&mut m.pvars, 0x60, [0.0; 4]);
                p::set_i16(&mut m.pvars, 0x86, t as i16);
                m.state = 3;
            }
        }
        3 => {
            collapse_look(w, id);
            {
                let m = w.mm(id);
                let vz = p::ff(&m.pvars, 0x68) - DT * DT * 30.0;
                p::set_ff(&mut m.pvars, 0x68, vz);
                let v = p::v4f(&m.pvars, 0x60);
                for (p, d) in m.position.iter_mut().zip(v) { *p += d; }
            }
            if dec(w, 0x86) != 0 && 20.0 < p::ff(&w.m(id).pvars, 0x78) - w.m(id).position[2] {
                let t = w.ticks(0x14);
                w.shake_camera(ShakeRequest { axis: ShakeAxis::Up, amp: f32::from_bits(0x3dcc_cccd), ticks: t });
                {
                    let m = w.mm(id);
                    p::set_v4f(&mut m.pvars, 0x60, [0.0; 4]);
                    m.state = 4;
                    m.position[2] = p::ff(&m.pvars, 0x78) - 20.0;
                }
                w.play_sound(0, 0, id);
                let (sid, lvl) = (w.m(id).spawn_id, w.svc.level);
                w.svc.save.death.insert((lvl, sid));
                w.svc.save.death_level.insert(sid);
            }
        }
        4 if p::i16(&w.m(id).pvars, 0x84) == 0 && p::i32(&w.m(id).pvars, 0x8c) != -1 => {
            crate::cinematic::camera_script2(w, 0);
            crate::cinematic::hero_state(w, 0, true);
            crate::cinematic::letterbox(w, false);
            p::set_i32(&mut w.mm(id).pvars, 0x8c, -1);
        }
        _ => {}
    }
}
