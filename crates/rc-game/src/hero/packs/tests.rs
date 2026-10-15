//! Unit tests of the pack states on hand-built floors: entry conditions per pack, heights, speeds, timings and
//! the transitions out.

use super::*;
use crate::hero::testkit::{cam_x, cell, mesh};
use crate::hero::{hero_update, AnimView, HeroTick};
use crate::moby_runtime::Moby;
use crate::pad::{PadInput, PadState};
use rc_formats::collision::{Collision, CollisionCell};
use std::collections::BTreeMap;

/// A looping stand-in for Ratchet's animation: `set_anim` starts `seq` at `frame`, the key time advances by the
/// playback speed and wraps (flags bit 1) at a per-sequence length.
#[derive(Default)]
struct LoopAnim {
    v: AnimView,
    blend_left: f32,
}

/// Key-time length per sequence: the stomp 0x2a 60, the rebound 0x29 30, the jumps long enough not to wrap, the
/// rest 16.
fn len(seq: u8) -> f32 {
    match seq {
        0x2a => 60.0,
        0x29 => 30.0,
        7..=0x9 | 0x11 | 0x12 | 0x13 | 0x15 | 0x16 | 0x26 => 90.0,
        _ => 16.0,
    }
}

impl AnimCtl for LoopAnim {
    fn set_anim(&mut self, blend: Pf, seq: u8, frame: i32) {
        self.v.seq_a = 0xff;
        self.v.seq_b = seq;
        self.v.frame = frame as f32;
        self.v.frame_count_b = len(seq) as u8;
        self.v.rate = 1.0;
        self.v.frame_b_rate = 1.0;
        self.blend_left = if Pf::ZERO < blend { blend.to_f32() } else { 1.0 };
    }
    fn advance(&mut self, speed: Pf) {
        self.v.flags = 0;
        if self.v.seq_a != self.v.seq_b {
            self.blend_left -= 1.0;
            if self.blend_left <= 0.0 { self.v.seq_a = self.v.seq_b; }
        }
        self.v.frame += speed.to_f32();
        if len(self.v.seq_b) <= self.v.frame {
            self.v.frame -= len(self.v.seq_b);
            self.v.flags |= 2;
        }
    }
    fn view(&self) -> AnimView { self.v }
    fn frame_count(&self, seq: u8) -> u8 { len(seq) as u8 }
    fn set_loop(&mut self, _start: i32, _end: i32) {}
    fn clear_loop(&mut self) {}
}

struct Sim {
    hero: Hero,
    moby: Moby,
    pad: PadState,
    anim: LoopAnim,
    rng: Rng,
    states: Vec<i32>,
    zs: Vec<f32>,
}

impl Sim {
    /// Ratchet at `pos` facing +x (the camera looks along +x: stick (0, −1) is forward), owning `items`, with the
    /// back item `back` (the saved back item the slot is created with).
    fn new(pos: [f32; 3], items: &[usize], back: i32) -> Sim {
        let mut hero = Hero::spawn(pos, 0.0);
        hero.grant_items(items);
        hero.equip_back(back);
        let mut moby = Moby::zeroed();
        moby.position = hero.pos.map(Pf::to_f32);
        Sim { hero, moby, pad: PadState::default(), anim: LoopAnim::default(), rng: Rng::new(), states: Vec::new(), zs: Vec::new() }
    }
    fn tick(&mut self, coll: &Collision, input: PadInput) {
        self.pad.update(Some(&input.bytes()), false);
        let (cam_rows, cam_yaw) = cam_x();
        let env = Env { coll, pad: &self.pad, cam_yaw, cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
        let r = hero_update(&mut self.hero, &mut self.moby, &env, &mut self.anim, &mut self.rng);
        assert_eq!(r, HeroTick::Ran, "hero stopped in state {:#x}", self.hero.state);
        self.states.push(self.hero.state);
        self.zs.push(self.hero.position()[2]);
    }
    fn run(&mut self, coll: &Collision, input: PadInput, n: usize) { for _ in 0..n { self.tick(coll, input); } }
    /// Ticks `input` until `state` (at most `n`); the tick count, or None.
    fn until(&mut self, coll: &Collision, input: PadInput, n: usize, state: i32) -> Option<usize> {
        for i in 0..n {
            self.tick(coll, input);
            if self.hero.state == state { return Some(i); }
        }
        None
    }
    fn pos(&self) -> [f32; 3] { self.hero.position() }
}

fn dedup(v: &[i32]) -> Vec<i32> {
    let mut o: Vec<i32> = Vec::new();
    for &s in v { if o.last() != Some(&s) { o.push(s); } }
    o
}

/// Quads into the collision cells holding their centroids (split at the 4-unit grid by the callers). Type 0x2f:
/// surface 0xf, which no level's surface reaction handles.
fn cells_of(quads: &[[[f32; 3]; 4]]) -> Vec<CollisionCell> {
    let mut by: BTreeMap<[i16; 3], Vec<[[f32; 3]; 4]>> = BTreeMap::new();
    for q in quads {
        let c: [f32; 3] = std::array::from_fn(|k| (q[0][k] + q[1][k] + q[2][k] + q[3][k]) / 4.0);
        by.entry(c.map(|v| (v / 4.0).floor() as i16)).or_default().push(*q);
    }
    by.into_iter()
        .map(|(k, qs)| {
            let verts: Vec<[f32; 3]> = qs.iter().flatten().copied().collect();
            let idx: Vec<([u8; 4], u8)> = (0..qs.len()).map(|i| { let b = (4 * i) as u8; ([b, b + 1, b + 2, b + 3], 0x2f) }).collect();
            cell(k, &verts, &idx)
        })
        .collect()
}

fn spans(a: f32, b: f32) -> Vec<(f32, f32)> {
    let mut v = Vec::new();
    let mut x = a;
    while x < b {
        let e = (((x / 4.0).floor() + 1.0) * 4.0).min(b);
        v.push((x, e));
        x = e;
    }
    v
}

fn flat(z: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<[[f32; 3]; 4]> {
    let mut q = Vec::new();
    for (a, b) in spans(x0, x1) {
        for (c, d) in spans(y0, y1) { q.push([[a, c, z], [a, d, z], [b, d, z], [b, c, z]]); }
    }
    q
}

/// A wall facing −x at `x`, `[y0, y1] × [z0, z1]`.
fn wall(x: f32, y0: f32, y1: f32, z0: f32, z1: f32) -> Vec<[[f32; 3]; 4]> {
    let mut q = Vec::new();
    for (za, zb) in spans(z0, z1) {
        for (c, d) in spans(y0, y1) { q.push([[x, c, za], [x, d, za], [x, d, zb], [x, c, zb]]); }
    }
    q
}

/// A floor at z = 100 over x 340..500, y 380..460.
fn field() -> Collision { mesh(cells_of(&flat(100.0, 340.0, 500.0, 380.0, 460.0))) }

const START: [f32; 3] = [360.0, 420.0, 100.0];
const HELI: usize = 2;

fn n() -> PadInput { PadInput::neutral() }
fn fwd() -> PadInput { PadInput::neutral().stick(0.0, -1.0) }

/// The glide from a jump: ✕ (the jump), a second ✕ at tick 16 (the double jump 0xe), then ✕ held: 0x13f7f6 and
/// the 17-tick / 1.5 thresholds pass and the glide 8 starts (the jumps' glide test needs a ✕ press within the
/// buffer or the double jump; a ✕ held since the takeoff does not glide). The tick count to 8, or None.
fn double_jump_into_glide(s: &mut Sim, coll: &Collision) -> Option<usize> {
    s.run(coll, n(), 2);
    s.tick(coll, n().press(button::CROSS));
    s.run(coll, n(), 15);
    s.until(coll, n().press(button::CROSS), 80, 8)
}

/// A wrench in hand (the weapon check 0x240ed8, whose end runs the Thruster hover test, needs a ready hand item).
fn give_wrench(h: &mut Hero) {
    use rc_formats::moby_anim::AnimState;
    let anim = AnimState { seq_a: 1, frame_a: 0, seq_b: 1, frame_b: 0, t: 0.0, speed: 1.0, rate: 1.0, flags: 0, trigger_count: 0, skip_advance: false };
    h.items.slot.item = Some(super::super::items::HandItem { o_class: 0x47, mstate: 0, anim, snapshot: None, scale: 1.0, position: [0.0; 3], rows: [[0; 4]; 3], hit_timer: 0, flight: Default::default() });
    (h.items.slot.state, h.items.slot.id) = (2, 8);
}

/// Crouch standing (R1 for 10 ticks), then ✕ (with R1): the tick count to the next state.
fn crouch_jump_standing(s: &mut Sim, coll: &Collision) {
    s.run(coll, n(), 2);
    s.run(coll, n().press(button::R1), 10);
    assert_eq!(s.hero.state, 4, "crouching");
    s.tick(coll, n().press(button::R1 | button::CROSS));
}

/// Run forward 40 ticks, crouch (R1) 8 ticks while pushing forward, then ✕.
fn crouch_jump_running(s: &mut Sim, coll: &Collision) {
    s.run(coll, n(), 2);
    s.run(coll, fwd(), 40);
    s.run(coll, fwd().press(button::R1), 8);
    assert_eq!(s.hero.state, 4, "crouching: {:x?}", dedup(&s.states));
    s.tick(coll, fwd().press(button::R1 | button::CROSS));
}

#[test]
fn crouch_jumps_by_pack() {
    let coll = field();
    // No pack: the plain jump.
    let mut s = Sim::new(START, &[], 2);
    crouch_jump_standing(&mut s, &coll);
    assert_eq!(s.hero.state, 7);
    // The Heli-Pack on the back but not owned: still the plain jump.
    let mut s = Sim::new(START, &[], 2);
    crouch_jump_standing(&mut s, &coll);
    assert_eq!(s.hero.state, 7);
    // Owned: the high jump 0xf (anim 0x15); moving: the long jump 10 (anim 0x12).
    let mut s = Sim::new(START, &[HELI], 2);
    crouch_jump_standing(&mut s, &coll);
    assert_eq!((s.hero.state, s.anim.v.seq_b), (0xf, 0x15));
    let mut s = Sim::new(START, &[HELI], 2);
    crouch_jump_running(&mut s, &coll);
    assert_eq!((s.hero.state, s.anim.v.seq_b), (10, 0x12));
    // The Thruster-Pack on the back needs no ownership test: 0xd (anim 0x11) / 0x10 (anim 0x26).
    let mut s = Sim::new(START, &[], 3);
    crouch_jump_standing(&mut s, &coll);
    assert_eq!((s.hero.state, s.anim.v.seq_b), (0xd, 0x11));
    let mut s = Sim::new(START, &[], 3);
    crouch_jump_running(&mut s, &coll);
    assert_eq!((s.hero.state, s.anim.v.seq_b), (0x10, 0x26));
    // Clank hidden: no pack jump.
    let mut s = Sim::new(START, &[HELI], 3);
    s.hero.back_slot.clank_hidden = 1;
    crouch_jump_standing(&mut s, &coll);
    assert_eq!(s.hero.state, 7);
}

/// The Heli high jump: 1.9 after a 9-tick windup, then the rotor's lift (48·dt² against the 29.7·dt² gravity) over
/// ticks 35..71; far higher and longer than the plain jump.
#[test]
fn heli_high_jump_lifts() {
    let coll = field();
    let mut s = Sim::new(START, &[HELI], 2);
    crouch_jump_standing(&mut s, &coll);
    let t0 = s.zs.len();
    let mut vz = Vec::new();
    for _ in 0..200 {
        s.tick(&coll, n().press(button::CROSS));
        vz.push((s.hero.timer, f(s.hero.vel[2])));
        if s.hero.state != 0xf { break; }
    }
    let zs = &s.zs[t0..];
    let apex = zs.iter().copied().fold(0.0f32, f32::max) - 100.0;
    let air = zs.iter().filter(|&&z| z > 100.001).count();
    eprintln!("Heli high jump: apex {apex}, {air} ticks in the air, states {:x?}", dedup(&s.states));
    assert!(apex > 3.0 && apex < 6.0, "apex {apex}");
    assert!(air > 60, "air {air}");
    // In the lift window vz changes by (48 − 29.7)·dt² per tick.
    let d: Vec<f32> = vz.windows(2).filter(|w| (40..60).contains(&w[1].0)).map(|w| w[1].1 - w[0].1).collect();
    let want = (48.0 - 29.7) * DT2F;
    assert!(!d.is_empty() && d.iter().all(|x| (x - want).abs() < 2e-6), "lift {d:?} vs {want}");
}

/// The Thruster high jump: the 151·dt² burst of the curve after the 9-tick windup takes it higher than the plain
/// held jump.
#[test]
fn thruster_high_jump_bursts() {
    let coll = field();
    let mut s = Sim::new(START, &[], 3);
    crouch_jump_standing(&mut s, &coll);
    let t0 = s.zs.len();
    s.run(&coll, n().press(button::CROSS), 120);
    let zs = &s.zs[t0..];
    let apex = zs.iter().copied().fold(0.0f32, f32::max) - 100.0;
    let first_up = zs.iter().position(|&z| z > 100.001).unwrap();
    eprintln!("Thruster high jump: apex {apex}, leaves the ground at tick {first_up}, states {:x?}", dedup(&s.states));
    assert!(apex > 2.6, "apex {apex}");
    assert!((9..=11).contains(&first_up), "takeoff {first_up}");
    assert_eq!(s.hero.state, 0, "{:x?}", dedup(&s.states));
}

/// The Heli long jump: pushed 53·dt²·T in the 5-tick windup, low gravity 11·dt², no air control; it lands into
/// the skid 3 (momentum × 0.8) far ahead.
#[test]
fn heli_long_jump_distance_and_landing() {
    let coll = field();
    let mut s = Sim::new(START, &[HELI], 2);
    crouch_jump_running(&mut s, &coll);
    let x0 = s.pos()[0];
    let t = s.until(&coll, fwd(), 150, 3).expect("lands into the skid");
    let dx = s.pos()[0] - x0;
    eprintln!("Heli long jump: {dx} ahead in {t} ticks, states {:x?}", dedup(&s.states));
    assert!(dx > 7.0 && dx < 30.0, "distance {dx}");
    assert_eq!(s.anim.v.seq_b, 5, "the skid anim");
    let air_states = dedup(&s.states);
    assert!(air_states.windows(2).any(|w| w == [10, 3]), "{air_states:x?}");
}

/// The Thruster long jump: straight ahead at 11.5 u/s (a 0.4..0.9 hop, gravity 8.5·dt²) and the landing picker's
/// 0x10 branch (the skid 3).
#[test]
fn thruster_long_jump_speed() {
    let coll = field();
    let mut s = Sim::new(START, &[], 3);
    crouch_jump_running(&mut s, &coll);
    let mut top = 0.0f32;
    let x0 = s.pos()[0];
    let mut t = 0;
    while s.hero.state == 0x10 && t < 150 {
        s.tick(&coll, fwd());
        // In the air (the tick after the touchdown adds the crouch's momentum back through the windup branch).
        if 0.05 < f(s.hero.height) { top = top.max(f(s.hero.eff_len_xy) * 60.0); }
        if std::env::var("PACKS_DEBUG").is_ok() { eprintln!("t{} speed {} eff {} vel {:?} h {}", s.hero.timer, f(s.hero.speed) * 60.0, f(s.hero.eff_len_xy) * 60.0, to_f32x3(s.hero.vel), f(s.hero.height)); }
        t += 1;
    }
    let dx = s.pos()[0] - x0;
    eprintln!("Thruster long jump: top speed {top} u/s, {dx} ahead in {t} ticks, then {:x?}", dedup(&s.states));
    assert!((top - 11.5).abs() < 0.3, "top speed {top}");
    assert!(dx > 8.0, "distance {dx}");
    // The landing picker's 0x10 branch is the skid 3; with the stick still forward it is the run 2.
    assert_eq!(s.hero.state, 2, "{:x?}", dedup(&s.states));
}

/// The glide: ✕ held through a jump with the Heli-Pack owned → 8 after tick 20 above 1.0; it sinks at a steady
/// 2.16 u/s (3.6 with the Thruster-Pack on the back); releasing ✕ after 30 ticks drops into the fall 6.
#[test]
fn glide_sink_rates_and_release() {
    let coll = field();
    for (back, rate) in [(2, 2.16f32), (3, 3.6)] {
        let mut s = Sim::new(START, &[HELI], back);
        let t = double_jump_into_glide(&mut s, &coll).expect("glides");
        s.run(&coll, n().press(button::CROSS), 10);
        let z0 = s.pos()[2];
        s.run(&coll, n().press(button::CROSS), 10);
        let sink = (z0 - s.pos()[2]) * 6.0;
        eprintln!("back {back}: glide at tick {t} of the jump, sink {sink} u/s");
        assert!((sink - rate).abs() < 0.02, "sink {sink}");
        s.run(&coll, n().press(button::CROSS), 12);
        s.tick(&coll, n());
        assert_eq!(s.hero.state, 6, "{:x?}", dedup(&s.states));
    }
    // Without the Heli-Pack, or with the Hydro-Pack on the back: no glide.
    for (items, back) in [(&[][..], 2), (&[HELI][..], 4)] {
        let mut s = Sim::new(START, items, back);
        assert!(double_jump_into_glide(&mut s, &coll).is_none(), "{:x?}", dedup(&s.states));
    }
}

/// The glide's landing: standing still it lands into idle once within 0.4 of the ground.
#[test]
fn glide_lands_into_idle() {
    let coll = field();
    let mut s = Sim::new(START, &[HELI], 2);
    double_jump_into_glide(&mut s, &coll).expect("glides");
    let t = s.until(&coll, n().press(button::CROSS), 200, 0).expect("lands");
    eprintln!("glide lands after {t} ticks: {:x?}", dedup(&s.states));
    // Within 0.4 of the ground but still airborne: idle with the jump's landing frames (anim 7 from 0x17).
    assert_eq!((s.anim.v.seq_b, s.hero.state), (7, 0), "the landing anim");
}

/// The stomp: R1 in the air with the Thruster-Pack → 0x22: up to 5.7 u/s for 12 ticks, from tick 34 down at
/// 100·dt²; coming down it queues the moby hits under the feet; on the ground the shake request; idle when the anim
/// wraps. Without the Thruster-Pack R1 in the air does nothing.
#[test]
fn stomp() {
    let coll = field();
    let mut s = Sim::new(START, &[], 3);
    s.run(&coll, n(), 2);
    s.tick(&coll, n().press(button::CROSS));
    s.run(&coll, n().press(button::CROSS), 14);
    assert_eq!(s.hero.state, 7);
    s.tick(&coll, n().press(button::R1));
    assert_eq!((s.hero.state, s.anim.v.seq_b), (0x22, 0x2a));
    let mut hits = 0;
    let mut vz = Vec::new();
    let mut t = 0;
    while s.hero.state == 0x22 && t < 200 {
        s.tick(&coll, n());
        hits += s.hero.packs.hits.len();
        s.hero.packs.hits.clear();
        vz.push(f(s.hero.vel[2]) * 60.0);
        t += 1;
    }
    eprintln!("stomp: {t} ticks, {hits} hit queries, vz {:?}, shake {:?}, then {:x?}", &vz[..vz.len().min(50)], s.hero.fx.shakes, dedup(&s.states));
    assert!(vz[..11].iter().all(|&v| v > 0.0 && v <= 5.71), "rises at ≤ 5.7 u/s");
    assert!(hits > 0, "hits on the way down");
    assert_eq!(s.hero.fx.shakes, vec![crate::follow_camera::ShakeRequest { axis: crate::follow_camera::ShakeAxis::Up, amp: 0.2, ticks: 40 }]);
    assert_eq!(s.hero.state, 0, "{:x?}", dedup(&s.states));
    // No Thruster-Pack: no stomp.
    let mut s = Sim::new(START, &[HELI], 2);
    s.run(&coll, n(), 2);
    s.tick(&coll, n().press(button::CROSS));
    s.run(&coll, n().press(button::CROSS), 14);
    s.tick(&coll, n().press(button::R1));
    assert_eq!(s.hero.state, 7);
}

/// The Thruster hover: an R1 double tap on the ground → 0x81; the feet spring to 0.17 above the ground; after 20
/// ticks ✕ ends it (lockout 25, the latch cleared) and idle stays idle.
#[test]
fn hover_double_tap() {
    let coll = field();
    let mut s = Sim::new(START, &[], 3);
    give_wrench(&mut s.hero);
    s.run(&coll, n(), 2);
    s.tick(&coll, n().press(button::R1));
    s.run(&coll, n(), 3);
    s.tick(&coll, n().press(button::R1));
    assert_eq!((s.hero.state, s.anim.v.seq_b), (0x81, 0x13), "{:x?}", dedup(&s.states));
    assert_eq!(s.hero.packs.hover_latch, 1);
    // The spring's velocity 0x13f654 is cleared by the step snap every tick above the ground, so the feet close 2%
    // of the gap per tick: 0.12 after 60 ticks, 0.17 after a few seconds.
    s.run(&coll, n(), 60);
    let h60 = s.pos()[2] - 100.0;
    s.run(&coll, n(), 240);
    let h = s.pos()[2] - 100.0;
    eprintln!("hover height {h60} after 60 ticks, {h} after 300");
    assert!((h60 - 0.17 * (1.0 - 0.98f32.powi(60))).abs() < 0.01, "hover height {h60}");
    assert!((h - 0.17).abs() < 0.005, "hover height {h}");
    s.tick(&coll, n().press(button::CROSS));
    assert_eq!(s.hero.state, 0);
    assert_eq!((s.hero.packs.hover_latch, s.hero.lockout), (0, 25));
    // Above the ground (0.17 is past the 0.02 ground snap): the jump's landing frames (anim 7 from 0x17).
    assert_eq!((s.anim.v.seq_b, s.anim.v.frame as i32), (7, 0x17));
    s.run(&coll, n(), 30);
    assert_eq!(s.hero.state, 0, "{:x?}", dedup(&s.states));
    // Without the Thruster-Pack the double tap is just the crouch.
    let mut s = Sim::new(START, &[HELI], 2);
    give_wrench(&mut s.hero);
    s.run(&coll, n(), 2);
    s.tick(&coll, n().press(button::R1));
    s.run(&coll, n(), 3);
    s.tick(&coll, n().press(button::R1));
    assert_eq!(s.hero.state, 4);
}

/// The long jumps into a wall: the rebound 0x7a (anim 0x29 from frame 5) pushes back from it and ends in idle.
#[test]
fn long_jump_into_a_wall_rebounds() {
    let mut q = flat(100.0, 340.0, 500.0, 380.0, 460.0);
    q.extend(wall(371.0, 400.0, 440.0, 100.0, 112.0));
    let coll = mesh(cells_of(&q));
    for back in [2, 3] {
        let mut s = Sim::new(START, &[HELI], back);
        crouch_jump_running(&mut s, &coll);
        let jump = s.hero.state;
        let t = s.until(&coll, fwd(), 120, 0x7a);
        eprintln!("back {back}: {jump:#x} → {:x?} at {:?}", dedup(&s.states), s.pos());
        assert!(t.is_some(), "no rebound from {jump:#x}: {:x?}", dedup(&s.states));
        assert_eq!(s.anim.v.seq_b, 0x29);
        let x = s.pos()[0];
        s.run(&coll, n(), 8);
        assert!(s.pos()[0] < x, "pushed back from the wall");
        assert!(s.until(&coll, n(), 60, 0).is_some(), "{:x?}", dedup(&s.states));
    }
}

/// The hero's looping sound slots: the glide starts the pack's loop (slot 3: sound 2 with the Heli-Pack; slot 4:
/// 0x12 with the Thruster-Pack), the group change out of it stops it.
#[test]
fn pack_loops_start_and_stop() {
    struct Rec(Vec<String>, i32);
    impl super::super::HeroSounds for Rec {
        fn anim_advanced(&mut self, _: &Moby, _: &AnimView, _: &AnimView, _: &mut Rng) {}
        fn voice(&mut self, _: &Moby, index: i32, flags: u32, _: &mut Rng) -> i32 {
            self.1 += 1;
            self.0.push(format!("play {index:#x} {flags}"));
            self.1
        }
        fn release(&mut self, _: &Moby, slot: i32) { self.0.push(format!("release {slot}")); }
    }
    let coll = field();
    let mut s = Sim::new(START, &[HELI], 2);
    let mut rec = Rec(Vec::new(), 40);
    let (cam_rows, cam_yaw) = cam_x();
    for t in 0..160 {
        let inp = match t {
            2 | 18.. if t < 110 => n().press(button::CROSS),
            _ => n(),
        };
        s.pad.update(Some(&inp.bytes()), false);
        let env = Env { coll: &coll, pad: &s.pad, cam_yaw, cam_rows, mirror: false, death_z: Pf::ZERO, mobys: None, hero_moby: None, water: None, world: None };
        super::super::hero_update_with_sounds(&mut s.hero, &mut s.moby, &env, &mut s.anim, &mut s.rng, &mut rec);
    }
    eprintln!("{:?}", rec.0);
    assert_eq!(rec.0.first().map(String::as_str), Some("play 0x2 4"));
    assert_eq!(rec.0.iter().filter(|x| x.starts_with("play")).count(), 1, "one loop start");
    assert!(rec.0.contains(&"release 41".to_string()), "stopped on leaving the glide");
    assert_eq!(s.hero.packs.loops, [-1; 8]);
}

/// The double jump's boost with the Thruster-Pack on the back is larger (×1.25; with the Heli-Pack owned it keeps
/// more of it higher up).
#[test]
fn double_jump_boost_by_pack() {
    let coll = field();
    let apex = |items: &[usize], back: i32| {
        let mut s = Sim::new(START, items, back);
        s.run(&coll, n(), 2);
        s.tick(&coll, n().press(button::CROSS));
        s.run(&coll, n().press(button::CROSS), 16);
        s.tick(&coll, n());
        s.tick(&coll, n().press(button::CROSS));
        assert_eq!(s.hero.state, 0xe, "{:x?}", dedup(&s.states));
        s.run(&coll, n(), 60);
        s.zs.iter().copied().fold(0.0f32, f32::max) - 100.0
    };
    let (plain, thruster, both) = (apex(&[], 2), apex(&[], 3), apex(&[HELI], 3));
    eprintln!("double jump apex: plain {plain}, Thruster {thruster}, Heli owned + Thruster {both}");
    assert!(thruster > plain + 0.1, "{thruster} vs {plain}");
    assert!(both >= thruster, "{both} vs {thruster}");
}
