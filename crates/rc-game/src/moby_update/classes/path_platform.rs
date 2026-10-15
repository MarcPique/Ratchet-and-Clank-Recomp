//! Path platform / lift, class 726: `PathPlatformUpdate` level01 0x2b9eb0 (the whole function; the same code
//! runs on Kerwan 0x291dd0 and on levels 10 and 13, cluster 7e8874a8). docs/plan/triggers.md §6.
//!
//! The platform rides a path (spline pvar+0xb4) between its two ends, `t` = 0 (first point) and 1 (last).
//! It is **called** when Ratchet is nearer the other end than the one it waits at, or **ridden** when he
//! steps on it after having left it (pvar+0xb8). Two optional trigger cuboids gate it, through the shared
//! test [`World::in_cuboid`] (`PointInCuboid` 0x274820) on Ratchet's feet:
//! * pvar+0xcc **appear cuboid**: hidden (mode |= 0x41, no collision, update distance 0xff) and frozen until
//!   Ratchet is inside once; then shown, collision back, index cleared (Crocker level 13 uses it);
//! * pvar+0xc0 **activation cuboid**: the platform ignores calls until Ratchet has been inside once; the first
//!   time sets the spawn id's death bits (persistent, so it stays active on later visits) and clears the
//!   index. Also active when the spawn id's `0x1bbb04` byte or death bit is already set (Kerwan instance 3).
//!
//! Novalis has one (instance 1, spline 40 from the landing pad's cliff (161.7, 146.0, 60.0) down to
//! (174.5, 159.5, 41.0), 4 s, starting at the top) with neither cuboid.
//!
//! Pvar block (0xd0 bytes, offsets in the block):
//!
//! | off | type | meaning |
//! |---|---|---|
//! | 0x08 | s32 | platform block offset (0x60; mode 0x20 carrier, [`triggers::platform_block`]) |
//! | 0x20 / 0x24 / 0x28 / 0x3e | f32 / s16 / u8 / s16 | shared sub-vars, set to 0 / 0 / 4 / 5 at init |
//! | 0x60..0x9f | | platform block: Euler Δ +0x60, displacement +0x70 ([`triggers::carry_riders`]) |
//! | 0xa0 | u8 | start end: 0 = at t = 1 (state 0), else at t = 0 (state 1) |
//! | 0xa4 | f32 | t along the path |
//! | 0xa8 | f32 | current step of t per tick (ramps toward 0xac by `0xac·dt` per tick) |
//! | 0xac | f32 | full step `±1 / scale(0xb0·60)` |
//! | 0xb0 | f32 | travel time, seconds |
//! | 0xb4 | s32 | path index (`0x1b0930`), −1 = inert |
//! | 0xb8 | s16 | re-armed: Ratchet is off it and > 2 away in XY (riding it again sends it) |
//! | 0xba | s16 | wait timer (`FastDecTimer`): 15 ticks after arriving, 30 when it pauses for Ratchet below |
//! | 0xbc | f32 | last tick's z change (< 0: descending) |
//! | 0xc0 | s32 | activation cuboid (−1 = none / done) |
//! | 0xc4 | s32 | loop voice (−1 = none) |
//! | 0xc8 | s32 | 0: Ratchet riding it gets the edge brake 0x13f544 and jump lockout 0x13f542 = 4 (hero hook) |
//! | 0xcc | s32 | appear cuboid (−1 = none / done) |
//!
//! Moby +0xbc (`cmd`) is the state: 0 at t = 1, 1 at t = 0, 2 moving.
//!
//! Standard `f32`; `dt` is `0x15ed6c`. The loop voice: while moving, `SoundIsAlive(m, voice)` else
//! `PlayClassSound(0, 4, m)`; on arriving and while paused `release_voice_slot(voice)` when the slot still plays
//! this moby's sound, then −1 ([`World::sound_alive`] / [`World::release_sound`], the audio layer's
//! `SoundSink`; with no sink a voice is alive while ≠ −1, and `PlayClassSound` returns −1, so the loop sound is
//! requested every moving tick). The hero writes of pvar+0xc8 = 0 go through `World::hero_fields_mut` (0xc8 = 1 on every disc
//! instance, so none happen in RAC1). Carrying
//! Ratchet is the hero's side (`HeroPlatformUpdate` 0x249618, triggers.md §5).

use crate::moby_runtime::MobyId;
use crate::moby_update::services::{fast_dec_timer_s16, fl, pvar as p, World, DT};
use crate::moby_update::triggers;

/// The loop voice +0xc4 stopped (`release_voice_slot` when the slot still plays this moby's sound: owner
/// 0x13e5d8 and active 0x13e5c4 checked first, [`World::release_sound`]) and forgotten (−1).
fn release_loop(w: &mut World, id: MobyId) {
    let v = p::i32(&w.m(id).pvars, 0xc4);
    if v != -1 { w.release_sound(v, id); }
    p::set_i32(&mut w.mm(id).pvars, 0xc4, -1);
}

/// The path-platform update address in the level01 class table.
pub const UPDATE_FN: u32 = 0x2b9eb0;

/// Classes that run [`update`].
pub const CLASSES: [i16; 1] = [726];

/// Offset of the platform block in the pvars (pvar+0x08 on the disc).
const BLOCK: usize = 0x60;

fn dist3(a: [f32; 3], b: [f32; 4]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }
fn dist_xy(a: [f32; 4], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }

/// Gaspar's copy (level09 `0x3033a0`, classes 1150 / 1151, 5 placed): the same platform without the appear
/// cuboid; ridden away (pvar+0xc8 = 0) it puts Ratchet in the riding state (`SetState(0x72, 1)`, back to 0 on
/// arrival) instead of the edge brake; leaving t = 0 plays sound 1 and every arrival sound 2; it pauses for Ratchet
/// below within 2 (xy) and more than 1 under it.
pub const GASPAR_FN: u32 = 0x30_33a0;
pub const GASPAR_LEVEL: u32 = 9;
pub const GASPAR_CLASSES: [i16; 2] = [1150, 1151];

/// Oltanis's copy (level14 `0x2aba80`, class 903, census U487, 2 placed): the level01 platform with Gaspar's pause
/// (within 2, more than 1 above Ratchet), but it springs onto its path point (0.005 / 0.2, velocity +0xd0) instead of
/// snapping to it, and at rest springs onto the end it waits at (`0x2ac2b8`).
pub const OLTANIS_FN: u32 = 0x2a_ba80;
pub const OLTANIS_LEVEL: u32 = 14;
pub const OLTANIS_CLASSES: [i16; 1] = [903];

/// Which level's copy runs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Copy {
    Novalis,
    Gaspar,
    Oltanis,
}

/// `PathPlatformUpdate` (0x2b9eb0).
pub fn update(w: &mut World, id: MobyId) { run(w, id, Copy::Novalis) }

/// Level09 `0x3033a0` ([`GASPAR_FN`]).
pub fn gaspar_update(w: &mut World, id: MobyId) { run(w, id, Copy::Gaspar) }

/// Level14 `0x2aba80` ([`OLTANIS_FN`]).
pub fn oltanis_update(w: &mut World, id: MobyId) {
    if w.m(id).pvars.len() < 0xe0 { w.mm(id).pvars.resize(0xe0, 0); }
    run(w, id, Copy::Oltanis)
}

/// `Spring(t, 0.005, 0.2, 0, &pos.k, &vel.k)` toward `q` (level14: `0x2ac2b8` and the travel's placement).
fn spring_to(w: &mut World, id: MobyId, q: [f32; 4]) {
    for (k, &t) in q.iter().take(3).enumerate() {
        let (mut x, mut v) = (crate::ps2v::Pf::f(w.m(id).position[k]), crate::ps2v::Pf::f(p::ff(&w.m(id).pvars, 0xd0 + 4 * k)));
        crate::hero::physics::spring(crate::ps2v::Pf::f(t), crate::ps2v::Pf::b(0x3ba3_d70a), crate::ps2v::Pf::b(0x3e4c_cccd), crate::ps2v::Pf::ZERO, &mut x, &mut v);
        w.mm(id).position[k] = x.to_f32();
        p::set_ff(&mut w.mm(id).pvars, 0xd0 + 4 * k, v.to_f32());
    }
}

fn run(w: &mut World, id: MobyId, v: Copy) {
    if w.m(id).pvars.len() < 0xd0 { return; }
    let gaspar = v == Copy::Gaspar;
    let path = p::i32(&w.m(id).pvars, 0xb4);
    if path == -1 { return; }
    // 0x1b0930[path]: the live spline (other classes may edit the shared table).
    let Some(pts) = usize::try_from(path).ok().and_then(|i| w.svc.splines.get(i)) else { return };
    let pts: Vec<[f32; 4]> = pts.iter().map(|q| q.map(f32::from_bits)).collect();
    if pts.is_empty() { return; }
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let hero = w.hero_point();

    // Appear cuboid (pvar+0xcc): hidden until Ratchet is inside. The game tests twice; the second test has the
    // same inputs and always agrees.
    let appear = if gaspar { -1 } else { p::i32(&w.m(id).pvars, 0xcc) };
    if appear != -1 {
        if !w.in_cuboid(hero, appear) {
            let m = w.mm(id);
            m.update_dist = 0xff;
            m.has_collision = false;
            m.mode |= 0x41;
            return;
        }
        let coll = w.classes.info(w.m(id).o_class).is_some_and(|c| c.has_collision);
        let m = w.mm(id);
        m.mode &= !0x41;
        m.has_collision = coll;
        p::set_i32(&mut m.pvars, 0xcc, -1);
    }

    // Activation cuboid (pvar+0xc0): a one-shot latch kept in the save (death bits of the spawn id).
    let activation = p::i32(&w.m(id).pvars, 0xc0);
    let active = if activation == -1 {
        true
    } else {
        let spawn_id = w.m(id).spawn_id;
        let level = w.svc.level;
        let latched = w.svc.save.collected.get(&spawn_id).is_some_and(|&b| b != 0) || w.svc.save.death.contains(&(level, spawn_id));
        if w.in_cuboid(hero, activation) || latched {
            w.svc.save.death.insert((level, spawn_id));
            w.svc.save.death_level.insert(spawn_id);
            p::set_i32(&mut w.mm(id).pvars, 0xc0, -1);
            true
        } else {
            false
        }
    };

    let old = w.m(id).position;
    {
        let pv = &mut w.mm(id).pvars;
        let mut t = p::i16(pv, 0xba);
        fast_dec_timer_s16(&mut t);
        p::set_i16(pv, 0xba, t);
    }
    if w.m(id).state == 0 {
        let m = w.mm(id);
        let start = p::u8(&m.pvars, 0xa0);
        p::set_u8(&mut m.pvars, 0x28, 4);
        p::set_i16(&mut m.pvars, 0x3e, 5);
        p::set_i32(&mut m.pvars, 0x20, 0);
        p::set_i16(&mut m.pvars, 0x24, 0);
        m.cmd = start;
        let t: f32 = if start == 0 { 1.0 } else { 0.0 };
        p::set_ff(&mut m.pvars, 0xa4, t);
        m.position = if t as i32 != 0 { last } else { first };
        p::set_i16(&mut m.pvars, 0xba, 0);
        p::set_ff(&mut m.pvars, 0xa8, 0.0);
        p::set_i32(&mut m.pvars, 0xbc, 0);
        m.state = 1;
        m.update_dist = 0xff;
        m.draw_dist = 0xff;
        p::set_i32(&mut m.pvars, 0xc4, -1);
    }

    match w.m(id).cmd {
        // Waiting at t = 1: go when called from the first end or ridden.
        0 if active => {
            let ridden = p::i16(&w.m(id).pvars, 0xb8) != 0 && w.hero_on_moby(id);
            if ridden || dist3(hero, first) < dist3(hero, last) { depart(w, id, -1.0, gaspar); }
        }
        // Waiting at t = 0.
        1 if active => {
            let ridden = p::i16(&w.m(id).pvars, 0xb8) != 0 && w.hero_on_moby(id);
            if ridden || dist3(hero, last) < dist3(hero, first) { depart(w, id, 1.0, gaspar); }
        }
        2 => travel(w, id, &pts, hero, old, v),
        _ => {}
    }
    if v == Copy::Oltanis && w.m(id).cmd != 2 {
        let end = if 0.5 < p::ff(&w.m(id).pvars, 0xa4) { last } else { first };
        spring_to(w, id, end);
    }

    // CarryRiders(pvar+0x60, pos − old, +0x40, +0x40).
    let m = w.mm(id);
    let pos = m.position;
    let delta = [pos[0] - old[0], pos[1] - old[1], pos[2] - old[2], pos[3]];
    let rot = m.rotation;
    triggers::carry_riders(&mut m.pvars, BLOCK, delta, rot, rot);
    if !w.hero_on_moby(id) && dist_xy(pos, hero) > 2.0 && p::i16(&w.m(id).pvars, 0xba) == 0 {
        p::set_i16(&mut w.mm(id).pvars, 0xb8, 1);
    }
}

/// Start moving: state 2, full step `dir / scale(travel·60)`, re-arm cleared. Gaspar: the riding state when ridden
/// away, and sound 1 leaving t = 0.
fn depart(w: &mut World, id: MobyId, dir: f32, gaspar: bool) {
    if gaspar {
        if p::i32(&w.m(id).pvars, 0xc8) == 0 && w.hero_on_moby(id) { crate::cinematic::hero_state(w, 0x72, true); }
        if 0.0 < dir { w.play_sound(1, 0, id); }
    }
    let scale = fl(w.svc.timing.timer_scale);
    let m = w.mm(id);
    m.cmd = 2;
    let ticks = p::ff(&m.pvars, 0xb0) * 60.0 * scale;
    p::set_i16(&mut m.pvars, 0xb8, 0);
    p::set_ff(&mut m.pvars, 0xac, dir / ticks);
}

/// State 2: pause for Ratchet below a descending platform, else ramp the step, advance `t`, arrive, and place
/// the platform on the path (piecewise linear between the path points).
fn travel(w: &mut World, id: MobyId, pts: &[[f32; 4]], hero: [f32; 3], old: [f32; 4], v: Copy) {
    let gaspar = v == Copy::Gaspar;
    if !gaspar && p::i32(&w.m(id).pvars, 0xc8) == 0 && w.hero_on_moby(id) {
        // 0x13f544 = 4 (edge brake), 0x13f542 = 4 (jump lockout), through the hero-block writes.
        let f = w.hero_fields_mut();
        f.edge_brake = 4;
        f.jump_lockout = 4;
    }
    let pos = w.m(id).position;
    let mut pause = p::i16(&w.m(id).pvars, 0xba) != 0;
    let (reach, under) = if v == Copy::Novalis { (2.83, 0.9) } else { (2.0, 1.0) };
    if !pause && p::ff(&w.m(id).pvars, 0xbc) < 0.0 && dist_xy(pos, hero) < reach && (hero[2] - pos[2]).abs() < 4.0 && under < pos[2] - hero[2] {
        // Descending onto Ratchet: wait 30 ticks.
        let t = w.ticks(30);
        p::set_i16(&mut w.mm(id).pvars, 0xba, t as i16);
        pause = true;
    }
    if pause {
        p::set_ff(&mut w.mm(id).pvars, 0xa8, 0.0);
        release_loop(w, id);
        return;
    }
    // SoundIsAlive(m, voice) ‖ PlayClassSound(0, 4, m): the loop sound.
    let voice = p::i32(&w.m(id).pvars, 0xc4);
    if !w.sound_alive(voice, id) {
        let v = w.play_sound(0, 4, id);
        p::set_i32(&mut w.mm(id).pvars, 0xc4, v);
    }
    let arrive_ticks = w.ticks(15);
    let mut arrived = false;
    let ridden = gaspar && p::i32(&w.m(id).pvars, 0xc8) == 0 && w.hero_on_moby(id);
    let m = w.mm(id);
    let full = p::ff(&m.pvars, 0xac);
    let mut step = p::ff(&m.pvars, 0xa8) + full * fl(DT);
    if full.abs() < step.abs() { step = full; }
    p::set_ff(&mut m.pvars, 0xa8, step);
    let mut t = p::ff(&m.pvars, 0xa4) + step;
    p::set_ff(&mut m.pvars, 0xa4, t);
    if (t - 0.5).abs() > 0.5 {
        // Gaspar: the riding state ends, the arrival sound 2.
        if ridden { crate::cinematic::hero_state(w, 0, true); }
        let m = w.mm(id);
        if 0.0 < full {
            m.cmd = 0;
            t = 1.0;
        } else {
            m.cmd = 1;
            t = 0.0;
        }
        p::set_ff(&mut m.pvars, 0xa4, t);
        if gaspar { w.play_sound(2, 0, id); }
        let m = w.mm(id);
        p::set_ff(&mut m.pvars, 0xa8, 0.0);
        p::set_i16(&mut m.pvars, 0xba, arrive_ticks as i16);
        arrived = true;
    }
    let n1 = pts.len() as i32 - 1;
    let i = (n1 as f32 * t) as i32;
    let f = n1 as f32 * t - i as f32;
    let (iu, a) = (i as usize, pts[i.clamp(0, n1) as usize]);
    let q = if i == n1 {
        a
    } else {
        let b = pts[iu + 1];
        [(b[0] - a[0]) * f + a[0], (b[1] - a[1]) * f + a[1], (b[2] - a[2]) * f + a[2], b[3]]
    };
    if v == Copy::Oltanis { spring_to(w, id, q); } else { w.mm(id).position = q; }
    let m = w.mm(id);
    p::set_ff(&mut m.pvars, 0xbc, m.position[2] - old[2]);
    // Arrived at a path end: the loop sound stops.
    if arrived { release_loop(w, id); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::physics as ph;
    use crate::hero::Hero;
    use crate::moby_runtime::{mode, ClassInfo, Moby, MobyTable};
    use crate::moby_update::services::{ClassTable, Services};
    use crate::rng::Rng;
    use rc_formats::volumes::{Shape, Volumes};

    /// A straight 11-point path from (0, 0, 10) to (10, 0, 0), a 726 at its start (pvar+0xa0 = 1), travel 1 s.
    fn setup(pvar_edit: impl Fn(&mut [u8])) -> (MobyTable, Services, ClassTable) {
        let info = ClassInfo { update_fn: Some(UPDATE_FN), has_collision: true, scale: 1.0, ..Default::default() };
        let mut classes = ClassTable::default();
        classes.classes.insert(726, (info, None));
        let mut m = Moby::init_instance(1, 726, Some(&info));
        m.mode |= 0x20;
        m.spawn_id = 98;
        m.position = [0.0, 0.0, 10.0, 0.0];
        let mut pv = vec![0u8; 0xd0];
        p::set_i32(&mut pv, 0, 0x20);
        p::set_i32(&mut pv, 8, 0x60);
        p::set_u8(&mut pv, 0xa0, 1);
        p::set_ff(&mut pv, 0xb0, 1.0);
        p::set_i32(&mut pv, 0xb4, 0);
        for o in [0xc0, 0xcc] { p::set_i32(&mut pv, o, -1); }
        p::set_i32(&mut pv, 0xc8, 1);
        pvar_edit(&mut pv);
        m.pvars = pv;
        let hero = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        let table = MobyTable::new(vec![hero, m], 0);
        let mut svc = Services::new();
        let path: Vec<[f32; 4]> = (0..11).map(|k| [k as f32, 0.0, 10.0 - k as f32, -1.0]).collect();
        svc.set_splines(&[path]);
        (table, svc, classes)
    }

    fn hero_at(p: [f32; 3]) -> Hero {
        let mut h = Hero::new();
        h.pos = ph::v4(p[0], p[1], p[2]);
        h
    }

    fn step(t: &mut MobyTable, svc: &mut Services, classes: &ClassTable, hero: &Hero, n: usize) {
        let mut rng = Rng::new();
        for _ in 0..n {
            let mut w = World::new(t, hero, &mut rng, classes, svc, 0);
            update(&mut w, 1);
        }
    }

    fn t_of(t: &MobyTable) -> f32 { p::ff(&t.mobys[1].pvars, 0xa4) }

    #[test]
    fn waits_until_called_then_travels_and_arrives() {
        let (mut t, mut svc, classes) = setup(|_| {});
        // Ratchet near the start: init (t = 0, state 1) and stays.
        let near = hero_at([-3.0, 0.0, 10.0]);
        step(&mut t, &mut svc, &classes, &near, 5);
        assert_eq!((t.mobys[1].state, t.mobys[1].cmd, t_of(&t)), (1, 1, 0.0));
        assert_eq!(t.mobys[1].position, [0.0, 0.0, 10.0, -1.0]);
        assert_eq!(t.mobys[1].update_dist, 0xff);
        // Called from the far end: departs, the step ramps by full·dt per tick.
        let far = hero_at([13.0, 0.0, 0.0]);
        step(&mut t, &mut svc, &classes, &far, 1);
        assert_eq!(t.mobys[1].cmd, 2);
        let full = p::ff(&t.mobys[1].pvars, 0xac);
        assert!((full - 1.0 / 60.0).abs() < 1e-7);
        step(&mut t, &mut svc, &classes, &far, 1);
        assert!((p::ff(&t.mobys[1].pvars, 0xa8) - full * fl(DT)).abs() < 1e-9);
        assert_eq!(svc.sounds.len(), 1, "loop sound 0 requested (no sink: every moving tick)");
        assert_eq!((svc.sounds[0].index, svc.sounds[0].flags), (0, 4));
        // The carry block holds this tick's displacement.
        let d = triggers::platform_delta(&t.mobys[1]).unwrap();
        let pos = t.mobys[1].position;
        assert!(d.displacement[0] > 0.0 && (d.displacement[0] + d.displacement[2]).abs() < 1e-6 && pos[0] == d.displacement[0]);
        // Arrives at t = 1 (state 0), 15-tick wait, on the last point.
        let mut ticks = 2;
        while t.mobys[1].cmd == 2 {
            step(&mut t, &mut svc, &classes, &far, 1);
            ticks += 1;
            assert!(ticks < 400);
        }
        assert_eq!((t.mobys[1].cmd, t_of(&t)), (0, 1.0));
        assert_eq!(t.mobys[1].position, [10.0, 0.0, 0.0, -1.0]);
        assert_eq!(p::i16(&t.mobys[1].pvars, 0xba), 15);
        // Ramp 60 ticks to full speed (covering 0.5 of t), then 30 ticks at full speed: ~91 ticks.
        assert!((85..=95).contains(&ticks), "{ticks} ticks");
        // Called back from the start end.
        step(&mut t, &mut svc, &classes, &near, 1);
        assert_eq!(t.mobys[1].cmd, 2);
        assert!(p::ff(&t.mobys[1].pvars, 0xac) < 0.0);
    }

    #[test]
    fn ridden_after_rearm() {
        let (mut t, mut svc, classes) = setup(|_| {});
        // Ratchet 1 unit from the start (nearer it): not called; within 2 in XY: not re-armed.
        let mut h = hero_at([1.0, 0.0, 10.0]);
        step(&mut t, &mut svc, &classes, &h, 2);
        assert_eq!((t.mobys[1].cmd, p::i16(&t.mobys[1].pvars, 0xb8)), (1, 0));
        // Standing on it without having left: nothing.
        h.ground_moby = Some(1);
        step(&mut t, &mut svc, &classes, &h, 2);
        assert_eq!(t.mobys[1].cmd, 1);
        // Walks off 3 units (re-arms), then steps on: it departs.
        let off = hero_at([-3.0, 0.0, 10.0]);
        step(&mut t, &mut svc, &classes, &off, 1);
        assert_eq!(p::i16(&t.mobys[1].pvars, 0xb8), 1);
        step(&mut t, &mut svc, &classes, &h, 1);
        assert_eq!(t.mobys[1].cmd, 2);
        assert_eq!(p::i16(&t.mobys[1].pvars, 0xb8), 0);
        // In the air (air ticks > 0) he is not on it.
        let mut air = h.clone();
        air.air_ticks = 3;
        let w_on = { let mut rng = Rng::new(); let w = World::new(&mut t, &air, &mut rng, &classes, &mut svc, 0); w.hero_on_moby(1) };
        assert!(!w_on);
    }

    #[test]
    fn pauses_above_ratchet_while_descending() {
        let (mut t, mut svc, classes) = setup(|_| {});
        let far = hero_at([13.0, 0.0, 0.0]);
        step(&mut t, &mut svc, &classes, &far, 40);
        assert_eq!(t.mobys[1].cmd, 2);
        let pos = t.mobys[1].position;
        // Ratchet 1.5 below it: the platform stops for 30 ticks.
        let below = hero_at([pos[0] + 0.5, 0.0, pos[2] - 1.5]);
        step(&mut t, &mut svc, &classes, &below, 1);
        assert_eq!(p::i16(&t.mobys[1].pvars, 0xba), 30);
        assert_eq!(p::ff(&t.mobys[1].pvars, 0xa8), 0.0);
        let held = t.mobys[1].position;
        step(&mut t, &mut svc, &classes, &far, 29);
        assert_eq!(t.mobys[1].position, held);
        step(&mut t, &mut svc, &classes, &far, 2);
        assert!(t.mobys[1].position[0] > held[0]);
    }

    /// A cuboid around (20, 20, 0), half size 2.
    fn cuboid_volumes() -> Volumes {
        let mut s = Shape::default();
        for k in 0..3 { s.matrix[k][k] = 2.0; s.inverse[k][k] = 0.5; }
        s.matrix[3] = [20.0, 20.0, 0.0, 1.0];
        Volumes { cuboids: vec![s], ..Default::default() }
    }

    #[test]
    fn activation_cuboid_latches_into_the_death_bits() {
        let (mut t, mut svc, classes) = setup(|pv| p::set_i32(pv, 0xc0, 0));
        svc.set_volumes(cuboid_volumes());
        // Called from the far end, but not yet activated.
        let far = hero_at([13.0, 0.0, 0.0]);
        step(&mut t, &mut svc, &classes, &far, 3);
        assert_eq!(t.mobys[1].cmd, 1);
        assert!(svc.save.death.is_empty());
        // Ratchet enters the cuboid: latched (death bits, index cleared); from there it answers calls.
        let inside = hero_at([21.0, 19.0, 1.0]);
        step(&mut t, &mut svc, &classes, &inside, 1);
        assert_eq!(p::i32(&t.mobys[1].pvars, 0xc0), -1);
        assert!(svc.save.death.contains(&(1, 98)) && svc.save.death_level.contains(&98));
        step(&mut t, &mut svc, &classes, &far, 1);
        assert_eq!(t.mobys[1].cmd, 2);
        // A later visit (fresh pvars, death bit kept): active at once without the cuboid.
        let (mut t2, _, _) = setup(|pv| p::set_i32(pv, 0xc0, 0));
        step(&mut t2, &mut svc, &classes, &far, 2);
        assert_eq!(t2.mobys[1].cmd, 2);
    }

    /// Novalis (from `extracted/`): instance 1's pvars and spline 40. At Ratchet's spawn it waits at the top;
    /// called from the bottom end it descends in 4 s of travel plus the 1 s ramp and stops on the last point.
    #[test]
    fn novalis_lift() {
        let Some(g) = rc_formats::test_data::gameplay(1) else { eprintln!("skipped: no extracted/levels/01"); return };
        let inst = rc_formats::gameplay::parse_moby_instances(&g).unwrap();
        let pvars = rc_formats::gameplay::parse_pvars(&g).unwrap();
        let splines = rc_formats::gameplay::parse_splines(&g).unwrap();
        let lifts: Vec<usize> = inst.iter().enumerate().filter(|(_, m)| m.o_class == 726).map(|(i, _)| i).collect();
        assert_eq!(lifts, [1]);
        let li = &inst[1];
        assert_eq!(li.mode_bits, 0x20, "a carrier");
        let pv = li.pvar(&pvars).unwrap().to_vec();
        assert_eq!((p::i32(&pv, 8), p::u8(&pv, 0xa0), p::ff(&pv, 0xb0), p::i32(&pv, 0xb4)), (0x60, 1, 4.0, 40));
        assert_eq!((p::i32(&pv, 0xc0), p::i32(&pv, 0xc8), p::i32(&pv, 0xcc)), (-1, 1, -1), "no cuboids on Novalis");
        let (mut t, mut svc, classes) = setup(|_| {});
        let m = &mut t.mobys[1];
        m.pvars = pv;
        m.position = [li.position[0], li.position[1], li.position[2], 0.0];
        svc.set_splines(&splines);
        let s = &splines[40];
        let (top, bottom) = (s[0], s[s.len() - 1]);
        // Ratchet at his spawn: nearer the top, it stays.
        let spawn = hero_at([162.53032, 136.39348, 60.5]);
        step(&mut t, &mut svc, &classes, &spawn, 10);
        assert_eq!((t.mobys[1].cmd, t.mobys[1].position), (1, top));
        // Called from the bottom.
        let below = hero_at([bottom[0] + 2.0, bottom[1] + 2.0, bottom[2]]);
        let mut n = 0;
        while t.mobys[1].cmd != 0 {
            step(&mut t, &mut svc, &classes, &below, 1);
            n += 1;
            assert!(n < 1000);
        }
        assert_eq!(t.mobys[1].position, bottom);
        // 60 ramp ticks cover 1830/(60·240) of t, the rest at 1/240 per tick, + the departing tick.
        assert!((268..=274).contains(&n), "{n} ticks");
    }

    #[test]
    fn appear_cuboid_hides_until_entered() {
        let (mut t, mut svc, classes) = setup(|pv| p::set_i32(pv, 0xcc, 0));
        svc.set_volumes(cuboid_volumes());
        let far = hero_at([13.0, 0.0, 0.0]);
        step(&mut t, &mut svc, &classes, &far, 3);
        let m = &t.mobys[1];
        assert_eq!((m.state, m.mode & 0x41, m.has_collision), (0, 0x41, false), "hidden, not even initialised");
        let inside = hero_at([20.0, 20.0, 0.0]);
        step(&mut t, &mut svc, &classes, &inside, 1);
        let m = &t.mobys[1];
        assert_eq!((m.state, m.mode & 0x41, m.has_collision), (1, 0, true));
        assert_eq!(p::i32(&m.pvars, 0xcc), -1);
        assert_eq!(m.mode & mode::HIDDEN, 0);
    }
}
