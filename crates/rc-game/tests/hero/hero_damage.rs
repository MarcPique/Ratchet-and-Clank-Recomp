//! Package P2 (docs/plan/hero_states.md): the hit intake, the hurt / death states, the death flag, the stances
//! and the hero's sounds, on hand-built floors through the real tick (`Game::tick_with_hero_sounds`): a hit
//! message is handed to the tick by a stub moby system exactly as the moby loop's hit log would.

use rc_game::hero::damage::{take_damage, DamageEvent};
use rc_game::hero::testkit::{floor, Runner};
use rc_game::hero::{AnimCtl, AnimView, HeroSounds, HeroTick, RecordingAnim};
use rc_game::moby_runtime::{Moby, MobyId, MobyTable};
use rc_game::moby_update::services::{HitRecord, HitTemplate};
use rc_game::pad::{button, PadInput};
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, MobySystem, TickHooks};
use rc_formats::collision::Collision;
use std::cell::RefCell;

/// [`RecordingAnim`] whose sequences wrap after `len` frames (flags bit 1 on the wrap tick).
#[derive(Default)]
struct WrapAnim {
    inner: RecordingAnim,
    len: f32,
}

impl AnimCtl for WrapAnim {
    fn set_anim(&mut self, blend: Pf, seq: u8, frame: i32) { self.inner.set_anim(blend, seq, frame) }
    fn advance(&mut self, speed: Pf) {
        self.inner.advance(speed);
        let v = &mut self.inner.v;
        if v.seq_a == v.seq_b && self.len <= v.frame {
            v.frame -= self.len;
            v.flags |= 2;
        }
    }
    fn view(&self) -> AnimView { self.inner.view() }
    fn frame_count(&self, s: u8) -> u8 { self.inner.frame_count(s) }
    fn set_loop(&mut self, a: i32, b: i32) { self.inner.set_loop(a, b) }
    fn clear_loop(&mut self) { self.inner.clear_loop() }
}

/// The moby system's hit log as the hero sees it: the record queued for the next tick.
#[derive(Default)]
struct HitStub {
    next: RefCell<Option<HitRecord>>,
}

impl MobySystem for HitStub {
    fn scene(&mut self, _: &MobyTable) -> Option<rc_game::collision_query::OwnedScene> { None }
    fn build_matrix(&mut self, _: &mut MobyTable, _: MobyId) {}
    fn deliver_hit(&mut self, _: &mut MobyTable, _: MobyId, _: &HitTemplate) {}
    fn hit_message(&self, table: &MobyTable, target: MobyId) -> Option<HitRecord> {
        let r = self.next.borrow_mut().take()?;
        (r.target == target && table.mobys[target].hit_slot != 0xff).then_some(r)
    }
}

/// The hero's sounds as calls, each voice taking one draw (a pitch bend).
#[derive(Default)]
struct SoundLog {
    advanced: usize,
    voices: Vec<(i32, u32, u32)>,
}

impl HeroSounds for SoundLog {
    fn anim_advanced(&mut self, _: &Moby, _: &AnimView, _: &AnimView, _: &mut Rng) { self.advanced += 1; }
    fn voice(&mut self, _: &Moby, index: i32, flags: u32, rng: &mut Rng) -> i32 {
        self.voices.push((index, flags, rng.state));
        rng.rand();
        3
    }
}

struct World {
    coll: Collision,
    game: Game,
    anim: WrapAnim,
    hits: HitStub,
    sounds: SoundLog,
}

const HERO: MobyId = 0;
const ENEMY: MobyId = 1;

impl World {
    /// Ratchet (moby 0) standing at (410, 410, 100) facing +x on a floor, an enemy (moby 1, `class`) at `enemy`.
    fn new(class: i16, enemy: [f32; 3], death_z: f32) -> World {
        let coll = floor(100.0, 100, 106, 100, 106);
        let mut hero = Moby::zeroed();
        hero.position = [410.0, 410.0, 100.0, 1.0];
        hero.hit_slot = 0xff;
        hero.state = 1;
        let mut e = Moby::zeroed();
        e.o_class = class;
        e.position = [enemy[0], enemy[1], enemy[2], 1.0];
        e.mission = 3;
        e.state = 1;
        let table = MobyTable::new(vec![hero, e], 4);
        let game = Game::new(&coll, table, HERO, GameOptions::default(), death_z);
        let mut w = World { coll, game, anim: WrapAnim { len: 30.0, ..Default::default() }, hits: HitStub::default(), sounds: SoundLog::default() };
        w.game.finish_load();
        w
    }

    fn tick(&mut self, input: PadInput) -> HeroTick {
        let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut Rng, _: &rc_game::follow_camera::CameraView, _: &Collision, _: u64| {};
        let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut Rng, _: u64| {};
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut self.hits) };
        let mut no_hits = rc_game::hero::items::NoHits;
        self.game.tick_with_hero_sounds(Some(&input.bytes()), &self.coll, &mut self.anim, &mut hooks, &mut no_hits, None, &mut self.sounds).hero
    }

    /// A hit record for Ratchet from `attacker` (flags 1, `damage`), as `FUN_0026e968` leaves it.
    fn hit(&mut self, attacker: Option<MobyId>, damage: f32) {
        self.game.mobys.mobys[HERO].hit_slot = 5;
        *self.hits.next.borrow_mut() = Some(HitRecord { attacker, flags: 1, damage: Pf::f(damage), target: HERO, ..Default::default() });
    }

    fn run(&mut self, n: usize) { for _ in 0..n { self.tick(PadInput::neutral()); } }
}

#[test]
fn take_damage_takes_at_most_one() {
    let mut h = rc_game::hero::Hero::new();
    assert_eq!(h.health, 4);
    take_damage(&mut h, 3);
    assert_eq!(h.health, 3);
    take_damage(&mut h, 0);
    assert_eq!(h.health, 3);
    h.health = 0;
    take_damage(&mut h, 1);
    assert_eq!(h.health, 0);
}

/// A contact hit from an enemy in front: hurt 0x16, one health point, 77 invulnerable ticks, the hit flash, the
/// push away from the enemy (5.7 u/s back, 2.4 u/s up) added to the velocity; the hit slot is consumed.
#[test]
fn hit_knocks_back_and_hurts() {
    let mut w = World::new(0x100, [412.0, 410.0, 100.0], 0.0);
    w.run(3);
    assert_eq!(w.game.hero.state, 0);
    w.hit(Some(ENEMY), 2.0);
    w.tick(PadInput::neutral());
    let h = &w.game.hero;
    assert_eq!(h.state, 0x16);
    assert_eq!(h.group, 7);
    assert_eq!(h.health, 3, "HeroTakeDamage takes min(damage, 1)");
    assert_eq!(h.f510, 77);
    assert_eq!(h.f53e, 45);
    let v = rc_game::hero::physics::to_f32x3(h.vel);
    eprintln!("velocity after the hit {v:?}");
    assert!((v[0] + 5.7 / 60.0).abs() < 1e-3 && v[1].abs() < 1e-4 && (v[2] - 2.4 / 60.0).abs() < 1e-3, "{v:?}");
    assert_eq!(h.damage.killer.map(|k| (k.id, k.o_class, k.mission)), Some((ENEMY, 0x100, 3)));
    assert_eq!(h.damage.events, vec![DamageEvent::Hit]);
    assert_eq!(w.game.mobys.mobys[HERO].hit_slot, 0xff);
    assert_eq!(w.anim.inner.calls.last().map(|c| (c.0, c.1, c.2)), Some((Pf::b(0xc040_0000), 0x10, 3)));
    // Pushed back and up: airborne, then down again; a second hit while invulnerable is ignored.
    let x0 = h.pos[0].to_f32();
    w.run(5);
    assert!(w.game.hero.pos[0].to_f32() < x0 - 0.2, "pushed back");
    w.hit(Some(ENEMY), 1.0);
    w.tick(PadInput::neutral());
    assert_eq!(w.game.hero.health, 3);
    assert_eq!(w.game.hero.state, 0x16);
    // After 27 ticks in the hurt state: back to idle (SetState(0, 0)); the invulnerability runs out at 77.
    let mut n = 0;
    while w.game.hero.state == 0x16 && n < 100 { w.tick(PadInput::neutral()); n += 1; }
    assert_eq!(w.game.hero.state, 0);
    assert!(w.game.hero.f510 > 0);
    w.run(80);
    assert_eq!(w.game.hero.f510, 0);
    w.hit(None, 1.0);
    w.tick(PadInput::neutral());
    assert_eq!((w.game.hero.state, w.game.hero.health), (0x16, 2));
    // No attacker: pushed straight back from the facing (+x) → −x.
    assert!(w.game.hero.vel[0].to_f32() < -0.05);
    assert_eq!(w.game.hero.damage.killer, None);
}

/// The hazard classes 0x4eb / 0x558 kill at once (0x80, health 0, no push); the anim's wrap runs the death
/// sequence: the death flag 0x141401 and the death event with the killer's mission.
#[test]
fn hazard_hit_kills_and_raises_the_death_flag() {
    let mut w = World::new(0x4eb, [412.0, 410.0, 100.0], 0.0);
    w.run(3);
    w.hit(Some(ENEMY), 1.0);
    w.tick(PadInput::neutral());
    assert_eq!((w.game.hero.state, w.game.hero.group, w.game.hero.health), (0x80, 0x14, 0));
    assert_eq!(w.game.hero.fell_out, 0);
    let mut n = 0;
    while w.game.hero.fell_out == 0 && n < 100 { w.tick(PadInput::neutral()); n += 1; }
    assert_eq!(w.game.hero.fell_out, 1);
    assert!(w.game.hero.damage.events.contains(&DamageEvent::Died { killer_mission: Some(3), killer_class: Some(0x4eb) }));
    // Group 0x14 refuses every state but 100.
    assert_eq!(w.game.hero.state, 0x80);
}

/// The last health point: hurt (0x16, health 0), after 27 ticks the death 0x3d, its wrap → the death flag.
#[test]
fn death_at_zero_health() {
    let mut w = World::new(0x100, [412.0, 410.0, 100.0], 0.0);
    w.run(3);
    w.game.hero.health = 1;
    w.hit(Some(ENEMY), 1.0);
    w.tick(PadInput::neutral());
    assert_eq!((w.game.hero.state, w.game.hero.health), (0x16, 0));
    let mut seen = Vec::new();
    for _ in 0..120 {
        w.tick(PadInput::neutral());
        if seen.last() != Some(&w.game.hero.state) { seen.push(w.game.hero.state); }
        if w.game.hero.fell_out != 0 { break; }
    }
    assert_eq!(seen, vec![0x16, 0x3d]);
    assert_eq!(w.game.hero.fell_out, 1);
    assert_eq!(w.anim.inner.calls.last().map(|c| c.1), Some(0x45));
}

/// Below the level's death height (and more than 2 above the ground): the death fall 0x77, its voice (0x17,
/// flags 0x20) then its two spin draws in that order, the tumble, and the death flag after 120 ticks.
#[test]
fn death_fall_voice_spin_and_flag() {
    let mut w = World::new(0x100, [0.0; 3], 150.0);
    // Put him 60 above the floor, 10 above the level's death height: he falls through it.
    w.run(3);
    w.game.hero.pos[2] = Pf::f(160.0);
    let mut t = 0;
    while w.game.hero.state != 0x77 && t < 400 { w.tick(PadInput::neutral()); t += 1; }
    assert_eq!(w.game.hero.state, 0x77, "never below the death height");
    assert_eq!(w.game.hero.group, 2);
    assert_eq!(w.sounds.voices.len(), 1);
    let (idx, flags, state) = w.sounds.voices[0];
    assert_eq!((idx, flags), (0x17, 0x20));
    assert_eq!(w.game.hero.damage.voice_slot, 3);
    // The spins are the two draws right after the voice's.
    let mut r = Rng { state };
    r.rand();
    let dt = Pf::b(0x3c88_8889);
    let (a, b) = ((dt * Pf::b(0x4049_0fdb)).to_f32(), (dt * Pf::f(4.363_323)).to_f32());
    let s0 = r.randf_sym(a, b);
    let s1 = r.randf_sym(a, b);
    assert_eq!(w.game.hero.damage.spin_target, [s0, s1]);
    // He tumbles (the Euler x / y leave 0) and the death flag rises after 120 ticks in the state.
    let mut n = 0;
    while w.game.hero.fell_out == 0 && n < 200 { w.tick(PadInput::neutral()); n += 1; }
    assert_eq!(w.game.hero.fell_out, 1);
    assert!((119..=122).contains(&n), "{n}");
    let r = rc_game::hero::physics::to_f32x3(w.game.hero.rot);
    assert!(r[0] != 0.0 || r[1] != 0.0, "{r:?}");
    assert!(matches!(w.game.hero.damage.events.last(), Some(DamageEvent::Died { .. })));
    // Ratchet's anim advance reaches the sound hook every tick.
    assert!(w.sounds.advanced > 100);
}

/// L1 enters the look stance and keeps it while held; releasing it goes back to idle, or to the crouch with R1
/// held; L1 from a walk too.
#[test]
fn look_stance_enters_and_leaves_on_its_buttons() {
    let coll = floor(100.0, 100, 106, 100, 106);
    let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
    r.run(&coll, PadInput::neutral(), 3);
    r.run(&coll, PadInput::neutral().press(button::L1), 10);
    assert_eq!(r.hero.state, 1);
    assert_eq!(r.hero.f15d4, 4);
    assert_eq!(r.hero.group, 0);
    assert_eq!(r.hero.position(), [410.0, 410.0, 100.0]);
    r.tick(&coll, PadInput::neutral());
    assert_eq!(r.hero.state, 0);
    r.run(&coll, PadInput::neutral().press(button::L2), 5);
    assert_eq!(r.hero.state, 1);
    r.tick(&coll, PadInput::neutral().press(button::L2 | button::R1));
    assert_eq!(r.anim.calls.last().map(|c| c.1), Some(0xd), "R1 held: the crouch sequence");
    r.tick(&coll, PadInput::neutral().press(button::R1));
    assert_eq!(r.hero.state, 4);
    // From a run.
    r.run(&coll, PadInput::neutral(), 20);
    r.run(&coll, PadInput::neutral().stick(0.0, -1.0), 20);
    assert_eq!(r.hero.state, 2);
    r.run(&coll, PadInput::neutral().stick(0.0, -1.0).press(button::L1), 2);
    assert_eq!(r.hero.state, 1);
}

/// Walk to a point (0x65 → 0x67), the scripted walk the vendors and the ship use.
#[test]
fn walk_to_point() {
    let coll = floor(100.0, 100, 106, 100, 106);
    let mut r = Runner::new([410.0, 410.0, 100.0], 0.0);
    r.run(&coll, PadInput::neutral(), 3);
    r.hero.walk_to.point = rc_game::hero::physics::v4(413.0, 411.0, 100.0);
    r.hero.walk_to.yaw = Pf::f(1.0);
    r.with_ctx(&coll, |h, c| h.set_state(c, 0x65, true));
    let mut n = 0;
    while r.hero.state != 0x67 && n < 400 { r.tick(&coll, PadInput::neutral()); n += 1; }
    assert_eq!(r.hero.state, 0x67, "arrived after {n}");
    let p = r.hero.position();
    assert!(((p[0] - 413.0).powi(2) + (p[1] - 411.0).powi(2)).sqrt() < 0.2, "{p:?}");
    assert!((r.hero.yaw().to_f32() - 1.0).abs() < 0.035);
}
