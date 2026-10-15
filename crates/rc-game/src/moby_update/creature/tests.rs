//! Unit tests of the shared creature layer and the critter on hand-built floors.

use super::*;
use crate::hero::physics::v4;
use crate::hero::testkit;
use crate::hero::Hero;
use crate::moby_runtime::{ClassInfo, Moby, MobyTable};
use crate::moby_update::scheduler::{self, Scheduler};
use crate::moby_update::services::{HitRecord, Services};
use crate::moby_update::ClassTable;
use crate::ps2v::Pf;
use crate::rng::{Rng, LEVEL_SEED};
use rc_formats::collision::Collision;

fn seeded() -> Rng { let mut r = Rng::new(); r.srand(LEVEL_SEED); r }

fn draws(from: &Rng, to: &Rng) -> usize {
    let mut r = *from;
    for n in 0..100_000 {
        if r.state == to.state { return n; }
        r.rand();
    }
    panic!("state not reached");
}

#[test]
fn rotations_wrap_like_the_game() {
    assert!((add_rot(3.0, 0.5) - (3.5 - 2.0 * PI)).abs() < 1e-6);
    assert!((sub_rot(-3.0, 0.5) - (-3.5 + 2.0 * PI)).abs() < 1e-6);
    assert!((diff_rots(3.0, -3.0) - (2.0 * PI - 6.0)).abs() < 1e-6);
}

#[test]
fn turn_toward_stops_on_the_target() {
    // Braking in time: at most the last step's residual past the heading (the game's reversal branch adds the braked
    // velocity without the snap), then it settles.
    let (mut a, mut v) = (0.0f32, 0.0f32);
    for _ in 0..600 {
        turn::turn_toward(2.0, DT2 * 12.566, DT2 * 12.566, DT * 25.13, &mut a, &mut v);
        assert!(a <= 2.0 + 0.005, "overshoot {a}");
    }
    assert!((a - 2.0).abs() < 1e-3, "settled at {a}");
    let mut x = 0.0;
    assert_eq!(turn::approach(1.0, 0.3, &mut x), 0.7);
}

#[test]
fn rate_slot_takes_the_last_free_slot() {
    let mut s = [0i32; 3];
    assert_eq!(fx::rate_slot(&mut s, 10, 60), 2);
    assert_eq!(fx::rate_slot(&mut s, 11, 60), 1);
    assert_eq!(fx::rate_slot(&mut s, 12, 60), 0);
    assert_eq!(fx::rate_slot(&mut s, 13, 60), -1);
    assert_eq!(fx::rate_slot(&mut s, 71, 60), 2);
}

// -------------------------------------------------------------------------------------------------
// A small world: Ratchet (index 0) and one creature (index 1) on a floor.

struct Sim {
    table: MobyTable,
    classes: ClassTable,
    hero: Hero,
    rng: Rng,
    svc: Services,
    sched: Scheduler,
    mesh: Collision,
    counter: u64,
}

/// Eight sequences of 20 keys, key k at time k (one key per tick), for the creature classes.
fn anim_class() -> rc_formats::moby_anim::MobyAnimClass {
    use rc_formats::moby_anim::{MobyAnimClass, MobyFrame, MobyFrameHeader, MobySequence, MobySequenceHeader, IDENTITY};
    let key = |k: i16| MobyFrame {
        header: MobyFrameHeader { rate: 1.0, time: k * 16, qwc: 1, quat_bytes: 8, scale_count: 0, trans_offset: 8, trans_count: 0 },
        quats: vec![[0, 0, 0, 0x7fff]],
        scales: vec![],
        trans: vec![],
        payload: vec![0, 0, 0, 0, 0, 0, 0xff, 0x7f, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let seq = || Some(MobySequence { header: MobySequenceHeader { frame_count: 20, loop_sound: 0xff, ..Default::default() }, frames: (0..20).map(key).collect(), triggers: vec![] });
    MobyAnimClass { joint_count: 1, skeleton: vec![IDENTITY], rest: vec![[0.0; 3]], parent_word: vec![0], sequences: (0..8).map(|_| seq()).collect() }
}

fn classes() -> ClassTable {
    let mut t = ClassTable::default();
    for (slot, oc) in [(1u8, 577i16), (2, 0x23c), (3, 1747), (4, 1748), (5, 1749), (6, 0x70), (7, 13), (8, 0x27f), (9, 270), (10, 428)] {
        let info = ClassInfo { slot, update_fn: scheduler::port_update_fn(oc), scale: 1.0, has_collision: false, ..Default::default() };
        let anim = matches!(oc, 577 | 0x23c | 270).then(anim_class);
        t.classes.insert(oc, (info, anim));
    }
    t
}

/// A critter with Novalis's pvars (instance 579: range 5, speed 4, ±30°, hoverer).
fn critter_pvars() -> Vec<u8> {
    let mut p = vec![0u8; 0x270];
    let set = |p: &mut Vec<u8>, o: usize, b: &[u8]| p[o..o + b.len()].copy_from_slice(b);
    set(&mut p, 0, &0x20u32.to_le_bytes());
    set(&mut p, 0xc, &0x110u32.to_le_bytes());
    set(&mut p, 0x10, &0x120u32.to_le_bytes());
    set(&mut p, 0x14, &0x60u32.to_le_bytes());
    set(&mut p, 0x18, &0x180u32.to_le_bytes());
    set(&mut p, 0x110, &[0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0, 0, 0, 4, 0, 15, 0]);
    for (o, x) in [(0x130, 0.008f32), (0x138, 0.2), (0x13c, 0.125), (0x148, 0.5), (0x150, 0.4), (0x168, 0.2), (0x16c, 0.0333), (0x1f8, 30.0), (0x1fc, 5.0), (0x200, 20.0), (0x204, 4.0)] {
        set(&mut p, o, &x.to_le_bytes());
    }
    set(&mut p, 0x140, &0x200i32.to_le_bytes());
    set(&mut p, 0x216, &8i16.to_le_bytes());
    set(&mut p, 0x21c, &(-1i32).to_le_bytes());
    p
}

impl Sim {
    fn new(creature: i16, at: [f32; 3], pvars: Vec<u8>, hero_at: [f32; 3]) -> Sim {
        let mut h = Moby::init_instance(0, 0, Some(&ClassInfo { scale: 1.0, ..Default::default() }));
        h.mode |= crate::moby_runtime::mode::NO_UPDATE;
        h.position = [hero_at[0], hero_at[1], hero_at[2], 0.0];
        let classes = classes();
        let info = classes.classes[&creature].0;
        let mut m = Moby::init_instance(1, creature, Some(&info));
        m.position = [at[0], at[1], at[2], 0.0];
        m.update_dist = 0xff;
        m.mode |= PVAR_HEADER;
        m.pvars = pvars;
        m.rows = rc_formats::moby_anim::IDENTITY;
        let table = MobyTable::new(vec![h, m], 16);
        let mut hero = Hero::new();
        hero.pos = v4(hero_at[0], hero_at[1], hero_at[2]);
        hero.body_point = v4(hero_at[0], hero_at[1], hero_at[2] + 0.7);
        Sim { table, classes, hero, rng: seeded(), svc: Services::new(), sched: Scheduler::new(), mesh: testkit::floor(10.0, 20, 40, 20, 40), counter: 0 }
    }
    fn load(&mut self) {
        let mut w = World::new(&mut self.table, &self.hero, &mut self.rng, &self.classes, &mut self.svc, 0);
        w.coll = Some(&self.mesh);
        self.sched.load_pass(&mut w);
        self.counter = 1;
    }
    fn tick(&mut self) {
        self.table.free_slot_pass(self.counter);
        let mut w = World::new(&mut self.table, &self.hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.camera = self.hero.pos;
        self.sched.tick(&mut w);
        self.counter += 1;
    }
    fn critter(&self) -> &Moby { &self.table.mobys[1] }
}

#[test]
fn critter_hovers_with_three_draws_per_pick() {
    // Ratchet far away: the hoverer lifts 6 at init (1 draw: the mirror), then picks a hover spot every 120 ticks.
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    let r0 = s.rng;
    s.load();
    assert_eq!(draws(&r0, &s.rng), 1);
    assert_eq!((s.critter().state, s.critter().position[2]), (0xe, 16.0));
    let r1 = s.rng;
    for _ in 0..600 { s.tick(); }
    let n = draws(&r1, &s.rng);
    assert!(n.is_multiple_of(3) && (12..=18).contains(&n), "hover draws {n}");
    let p = s.critter().position;
    let d = ((p[0] - 120.0).powi(2) + (p[1] - 120.0).powi(2)).sqrt();
    assert!(d <= 3.01 && (15.0..=17.0).contains(&p[2]), "hover spot {p:?}");
}

#[test]
fn critter_lands_walks_and_bites_ratchet() {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [126.0, 120.0, 10.0]);
    s.load();
    let mut seen = std::collections::BTreeSet::new();
    let mut bitten = None;
    for t in 0..900 {
        s.tick();
        seen.insert(s.critter().state);
        let slot = s.table.mobys[0].hit_slot;
        if slot != 0xff && bitten.is_none() {
            let r: HitRecord = s.svc.hits.records[slot as usize];
            assert_eq!((r.target, r.attacker, r.flags, r.damage), (0, Some(1), 1, Pf::ONE));
            bitten = Some(t);
        }
        s.table.mobys[0].hit_slot = 0xff;
    }
    eprintln!("critter states {seen:?}, bitten at {bitten:?}, now at {:?}", s.critter().position);
    assert!(seen.contains(&0x11) && seen.contains(&3) && seen.contains(&4), "states {seen:?}");
    assert!(bitten.is_some(), "never bit Ratchet");
}

#[test]
fn a_hit_kills_the_critter_with_pieces_explosion_and_bolts() {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    s.table.mobys[1].spawn_flag = 0xff; // a spawn-flagged instance drops bolts (SetDeathBits)
    s.table.mobys[1].b4 = 3;
    s.load();
    for _ in 0..5 { s.tick(); }
    // The wrench's record: exact push, type 0 / subtype 1, class 0x47, damage 1, no attacker.
    let dir = [Pf::f(1.0), Pf::ZERO, Pf::ONE, Pf::f(crate::hero::damage::EXACT_PUSH_W)];
    let t = crate::moby_update::services::HitTemplate { dir, attacker: None, flags: 0x1_0000, b18: 0, b19: 1, h1a: 0x47, damage: Pf::ONE, w20: 1 };
    crate::moby_update::services::deliver_hit_in(&mut s.table, &mut s.svc.hits, 1, &t);
    s.tick();
    assert_eq!(s.critter().state, 99, "death flight");
    assert_eq!(s.critter().mode & crate::moby_runtime::mode::TARGETABLE, 0);
    let mut dyn_classes = std::collections::BTreeMap::new();
    for _ in 0..240 {
        s.tick();
        for m in &s.table.mobys[s.table.first_dynamic..] {
            if m.state < 0xfd { *dyn_classes.entry(m.o_class).or_insert(0) += 1; }
        }
    }
    eprintln!("dynamic mobys seen (class → ticks alive): {dyn_classes:?}; unported {:?}", s.svc.fx.unported);
    assert!(s.critter().is_deleted(), "critter not deleted");
    for c in [1747, 1748, 1749] { assert!(dyn_classes.contains_key(&c), "piece {c}"); }
    assert!(dyn_classes.keys().any(|c| (13..=16).contains(c)), "no bolts");
    assert!(dyn_classes.contains_key(&0x27f), "no explosion light");
}

#[test]
fn resolver_cools_down_repeated_hits_of_one_kind() {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    let rec = |dmg: f32| HitRecord { dir: [Pf::f(1.0), Pf::ZERO, Pf::ONE, Pf::f(crate::hero::damage::EXACT_PUSH_W)], b28: 0, b29: 1, h2a: 0x47, damage: Pf::f(dmg), flags: 0x10000, target: 1, ..Default::default() };
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    let a = damage::resolve(&mut w, 1, Some(rec(1.0)), 0x20, 0, 4);
    assert_eq!((a.kind, a.damage, a.out5), (Some(1), 1.0, 2));
    assert_eq!(pi16(&w, 1, 0x3c), 15, "cooldown ticks(15)");
    // The wrench push is redirected away from Ratchet (at (150, 150)): unit xy length.
    let d = a.hit.unwrap().dir.map(|x| f32::from_bits(x.0));
    assert!((len2(d) - 1.0).abs() < 1e-5 && d[0] < 0.0 && d[1] < 0.0, "push {d:?}");
    let b = damage::resolve(&mut w, 1, Some(rec(1.0)), 0x20, 0, 4);
    assert_eq!((b.damage, b.out5, b.reaction), (0.0, 1, 0xb), "second hit inside the cooldown");
    for _ in 0..14 { damage::resolve(&mut w, 1, None, 0x20, 0, 4); }
    let c = damage::resolve(&mut w, 1, Some(rec(1.0)), 0x20, 0, 4);
    assert_eq!(c.damage, 1.0, "after the cooldown");
}

#[test]
fn flash_ramps_to_red_and_back() {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    flash::start(&mut w, 1, 0x110);
    let mut reds = Vec::new();
    for _ in 0..22 {
        flash::update(&mut w, 1, 0x110);
        reds.push(w.m(1).ambient[0]);
    }
    assert_eq!(reds[3], 0x80, "full red after the 4-tick fade-in: {reds:?}");
    assert_eq!(*reds.last().unwrap(), 0x40, "back to the ambient: {reds:?}");
    assert!(reds.windows(2).skip(4).all(|p| p[1] <= p[0]), "fade-out monotonic: {reds:?}");
}

#[test]
fn region_walls_and_waypoints() {
    // A 10×10 arena (walls: every edge, w ≠ 0) and a graph of two points seeing each other.
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    let sq: Vec<[f32; 4]> = vec![[0.0, 0.0, 0.0, 1.0], [10.0, 0.0, 0.0, 1.0], [10.0, 10.0, 0.0, 1.0], [0.0, 10.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]];
    let wall: Vec<[f32; 4]> = vec![[5.0, 2.0, 0.0, 1.0], [5.0, 8.0, 0.0, 1.0]];
    let graph: Vec<[f32; 4]> = vec![[5.0, 9.0, 0.0, f32::from_bits(2)], [2.0, 9.0, 0.0, f32::from_bits(1)]];
    s.svc.set_splines(&[sq, wall, graph]);
    let w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    assert!(region::point_in_polygon(&w, 0, [5.0, 5.0, 0.0, 0.0]));
    assert!(!region::point_in_polygon(&w, 0, [15.0, 5.0, 0.0, 0.0]));
    let (a, b) = ([2.0, 5.0, 0.0, 0.0], [8.0, 5.0, 0.0, 0.0]);
    assert!(region::crosses(&w, 1, a, b));
    let (hit, p) = region::clamp(&w, 1, a, b);
    assert!(hit && (p[0] - 5.0).abs() < 1e-5, "{p:?}");
    // Blocked straight: the way round the wall goes through the graph point both ends see.
    let via = region::line_of_sight(&w, 0.0, &[1], 2, a, b).expect("a way round");
    assert_eq!([via[0], via[1]], [5.0, 9.0]);
    assert_eq!(region::line_of_sight(&w, 0.0, &[1], 2, a, [3.0, 5.0, 0.0, 0.0]), Some([3.0, 5.0, 0.0, 0.0]));
}

// -------------------------------------------------------------------------------------------------
// The enemy additions: SpringTurn, the projectile helpers.

#[test]
fn spring_turn_is_spring_turn2_on_any_angle() {
    // SpringTurn2 is SpringTurn on the moby's yaw: same velocity, same result, never past the target.
    let mut sim = Sim::new(577, [30.0, 30.0, 10.0], critter_pvars(), [25.0, 30.0, 10.0]);
    let (mut v1, mut v2, mut a) = (0.0f32, 0.0f32, 0.4f32);
    let mut w = World::new(&mut sim.table, &sim.hero, &mut sim.rng, &sim.classes, &mut sim.svc, 0);
    set_yaw(&mut w, 1, 0.4);
    for _ in 0..200 {
        a = turn::spring_turn(a, -2.9, DT2 * 2.0 * PI, DT2 * PI, DT * 2.0 * PI, &mut v1);
        turn::spring_turn2(&mut w, 1, -2.9, DT2 * 2.0 * PI, DT2 * PI, DT * 2.0 * PI, &mut v2);
        assert_eq!(a.to_bits(), yaw(&w, 1).to_bits());
        assert_eq!(v1.to_bits(), v2.to_bits());
    }
    assert!(diff_rots(a, -2.9) < 1e-3, "settled at {a}");
    // The wrap: from 3.0 to −3.0 it turns forwards through π.
    let mut v = 0.0;
    let b = turn::spring_turn(3.0, -3.0, 0.02, 0.3, 0.1, &mut v);
    assert!(v > 0.0 && (b > 3.0 || b < -3.0), "{b} {v}");
}

#[test]
fn projectile_parts_draw_like_the_spawners() {
    let mut sim = Sim::new(577, [30.0, 30.0, 10.0], critter_pvars(), [25.0, 30.0, 10.0]);
    let mut w = World::new(&mut sim.table, &sim.hero, &mut sim.rng, &sim.classes, &mut sim.svc, 0);
    // Types 22 / 26: one raw rand() with a record (no particle system: as with a free record).
    let before = *w.rng;
    projectile::part22(&mut w, &crate::particles::type22::Spawn { size: 1.0, pos: [5.0; 4], vel: [0.0; 4], c1: 0, c2: 0, life: 10 });
    assert_eq!(draws(&before, w.rng), 1);
    let before = *w.rng;
    projectile::part26(&mut w, 1.0, 0, 0x8080_8080, 10);
    assert_eq!(draws(&before, w.rng), 1);
    // Type 16: nothing for life 0; the throttle draws only over the frame loads 0.9 / 0.95 / 1.0.
    let t16 = |life| crate::particles::type16::Spawn { size: 1.0, pos: [5.0; 4], vel: [0.0; 4], c1: 0, c2: 0, life, kind: 0 };
    let before = *w.rng;
    projectile::part16(&mut w, &t16(0));
    assert_eq!(draws(&before, w.rng), 0);
    let before = *w.rng;
    projectile::part16(&mut w, &t16(60));
    assert_eq!(draws(&before, w.rng), 1);
    w.svc.frame_load[0] = Pf::f(0.96);
    let before = *w.rng;
    projectile::part16(&mut w, &t16(60));
    let n = draws(&before, w.rng);
    assert!((1..=3).contains(&n), "{n}");
    assert_eq!(w.svc.fx.part_spawns.get(&22), Some(&1));
    assert!(projectile::in_world([2.0, 1021.0, 500.0, 0.0]) && !projectile::in_world([1.9, 5.0, 5.0, 0.0]));
}

#[test]
fn projectile_sweep_hits_the_floor_and_misses_the_air() {
    let mut sim = Sim::new(577, [30.0, 30.0, 10.0], critter_pvars(), [25.0, 30.0, 10.0]);
    let mut w = World::new(&mut sim.table, &sim.hero, &mut sim.rng, &sim.classes, &mut sim.svc, 0);
    w.coll = Some(&sim.mesh);
    let t = projectile::template(&w, 1, [0.4, 0.0, 1.0, crate::hero::damage::EXACT_PUSH_W], 0x1_0001, (1, 1), 1.0, 1);
    assert_eq!((t.flags, t.b18, t.b19, t.h1a, t.attacker), (0x1_0001, 1, 1, 577, Some(1)));
    let hit = projectile::sweep(&mut w, [120.0, 120.0, 11.0, 0.0], [120.0, 120.0, 9.0, 0.0], 0, 0.5, Some(1), &t).expect("the floor at z 10");
    assert!((hit[2] - 10.0).abs() < 1e-3, "{hit:?}");
    assert!(projectile::sweep(&mut w, [120.0, 120.0, 13.0, 0.0], [120.4, 120.0, 12.9, 0.0], 0, 0.5, Some(1), &t).is_none());
}

// -------------------------------------------------------------------------------------------------
// The Morph-o-Ray's morph and the chicken 270 (crate::moby_update::classes::chicken)

use crate::moby_update::classes::chicken;

fn morph_sim(hero_at: [f32; 3], gold: u8) -> (Sim, usize) {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), hero_at);
    s.table.mobys[1].spawn_flag = 0xff;
    s.table.mobys[1].b4 = 2;
    s.load();
    for _ in 0..3 { s.tick(); }
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, s.counter);
    w.coll = Some(&s.mesh);
    let ch = chicken::morph(&mut w, 1, gold).expect("no slot");
    (s, ch)
}

#[test]
fn the_morph_spawns_a_chicken_and_deletes_the_target() {
    let (s, ch) = morph_sim([130.0, 120.0, 10.0], 0);
    assert!(s.table.mobys[1].state >= 0xfd, "the critter deleted");
    let c = &s.table.mobys[ch];
    assert_eq!((c.o_class, c.state, c.pvars.len()), (270, 1, chicken::PVARS));
    assert_eq!(c.position[..2], s.table.mobys[1].position[..2], "at the critter's place");
    assert!(std::f32::consts::PI - c.rotation[2].abs() < 0.01, "facing away from Ratchet: {}", c.rotation[2]);
    assert_eq!(c.mode & (PVAR_HEADER | crate::moby_runtime::mode::TARGETABLE), PVAR_HEADER | crate::moby_runtime::mode::TARGETABLE);
    let word = |o: usize| u32::from_le_bytes(c.pvars[o..o + 4].try_into().unwrap()) as usize;
    assert_eq!((word(0x10), word(0x14), word(0)), (chicken::KNOCK, chicken::SUCK, 0));
    assert_eq!(s.svc.creatures.react.chickens[0], ch as u32 + 1);
    assert_eq!(s.svc.creatures.react.chicken_next, 1);
    assert!(s.svc.fx.part_spawns.contains_key(&5), "the type-5 flash counted");
    let r = chicken::SUCK;
    assert_eq!(f32::from_le_bytes(c.pvars[r + 0x80..r + 0x84].try_into().unwrap()), f32::from_bits(0x3f5c_ed91));
}

#[test]
fn the_chicken_runs_from_ratchet() {
    let (mut s, ch) = morph_sim([123.0, 120.0, 10.0], 0);
    // Born in the air (the critter hovered 6 up): the first flee is refused (a drop), it falls and pecks until its timer
    // runs out, then runs (1 unit a second while Ratchet is 3..6 away).
    let mut states = Vec::new();
    for _ in 0..400 {
        s.tick();
        let st = s.table.mobys[ch].state;
        if states.last() != Some(&st) { states.push(st); }
    }
    let p = s.table.mobys[ch].position;
    eprintln!("chicken states {states:?} at {p:?}");
    assert!(states.contains(&3) || states.contains(&4), "states {states:?}");
    assert!(p[0] < 117.0, "ran away from Ratchet (at x 123): {p:?}");
    assert!((p[2] - 10.0).abs() < 0.01, "on the floor: {p:?}");
}

#[test]
fn a_hit_bursts_the_chicken_into_feathers() {
    let (mut s, ch) = morph_sim([140.0, 140.0, 10.0], 0);
    s.tick();
    let dir = [Pf::f(1.0), Pf::ZERO, Pf::ONE, Pf::ZERO];
    let t = crate::moby_update::services::HitTemplate { dir, attacker: None, flags: 0x1_0000, b18: 0, b19: 1, h1a: 0x47, damage: Pf::ONE, w20: 1 };
    crate::moby_update::services::deliver_hit_in(&mut s.table, &mut s.svc.hits, ch, &t);
    let before = s.svc.fx.part_spawns.get(&22).copied().unwrap_or(0);
    s.tick();
    let c = &s.table.mobys[ch];
    assert_eq!(c.state, 6, "burst and hidden");
    assert_eq!(c.mode & (crate::moby_runtime::mode::HIDDEN | crate::moby_runtime::mode::TARGETABLE), crate::moby_runtime::mode::HIDDEN);
    assert_eq!(s.svc.fx.part_spawns.get(&22).copied().unwrap_or(0) - before, 20, "20 puffs (size 1)");
    let feathers: Vec<usize> = (0..s.table.mobys.len()).filter(|&i| s.table.mobys[i].o_class == chicken::FEATHER && s.table.mobys[i].state < 0xfd).collect();
    assert!((5..=8).contains(&feathers.len()), "feathers {}", feathers.len());
    // The feathers drift down, rock, and are gone within their life (visible: the MobyProc flag set each tick).
    let z0 = s.table.mobys[feathers[0]].position[2];
    for _ in 0..30 {
        for &f in &feathers { s.table.mobys[f].visible = 1; }
        s.tick();
    }
    assert!(s.table.mobys[feathers[0]].position[2] < z0 + 0.5, "falling");
    for _ in 0..400 {
        for &f in &feathers { if s.table.mobys[f].state < 0xfd { s.table.mobys[f].visible = 1; } }
        s.tick();
    }
    assert!(feathers.iter().all(|&f| s.table.mobys[f].state >= 0xfd || s.table.mobys[f].o_class != chicken::FEATHER), "the feathers ended");
}

#[test]
fn the_gold_chicken_is_a_tough_decoy() {
    let (mut s, ch) = morph_sim([140.0, 140.0, 10.0], 1);
    let c = &s.table.mobys[ch];
    assert_eq!((c.cmd, c.mode & PVAR_HEADER), (1, 0), "a decoy (+0xbc) without records");
    for _ in 0..60 { s.tick(); }
    let k = s.table.mobys[ch].scale / s.classes.classes[&270].0.scale;
    assert!((k - 4.0).abs() < 0.01, "grew to 4× its class scale: {k}");
}

#[test]
fn a_displaced_chicken_goes_when_out_of_view() {
    // The ring wraps (20 slots): the chicken already in the next slot is displaced (+0x70 = 1) and, not drawn, deleted
    // on its next tick.
    let (mut s, first) = morph_sim([140.0, 140.0, 10.0], 0);
    s.svc.creatures.react.chicken_next = 0;
    let id = {
        let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, s.counter);
        let id = w.create_moby(577).expect("slot");
        w.mm(id).position = [110.0, 110.0, 10.0, 0.0];
        id
    };
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, s.counter);
    w.coll = Some(&s.mesh);
    let second = chicken::morph(&mut w, id, 0).expect("slot");
    assert_eq!(i32::from_le_bytes(s.table.mobys[first].pvars[0x70..0x74].try_into().unwrap()), 1, "displaced");
    s.table.mobys[first].visible = 0;
    s.tick();
    assert!(s.table.mobys[first].state >= 0xfd, "the displaced chicken went");
    assert_eq!(s.table.mobys[second].o_class, 270);
    assert!(s.table.mobys[second].state < 0xfd);
}

#[test]
fn move_ground_walks_a_floor_and_refuses_a_step() {
    let (mut s, ch) = morph_sim([140.0, 140.0, 10.0], 0);
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, s.counter);
    w.coll = Some(&s.mesh);
    let mut from = [120.0, 120.0, 10.0, 0.0];
    let mut to = [120.5, 120.0, 10.0, 0.0];
    assert_eq!(walker::move_ground(&mut w, ch, 0.2, 0.5, 0.333, 0.2618, &mut from, &mut to, 0), 1);
    assert_eq!((from[0], from[1]), (120.5, 120.0));
    // Off the floor's edge (x 40.. is the floor: 20..40 in cells of 1? the floor spans 20..40 units): more than 0.333
    // down → refused.
    let mut from = [120.0, 120.0, 12.0, 0.0];
    let mut to = [120.5, 120.0, 12.0, 0.0];
    assert_eq!(walker::move_ground(&mut w, ch, 0.2, 0.5, 0.333, 0.2618, &mut from, &mut to, 0), 0, "a drop of 2 is refused");
    assert_eq!(to[0], 120.0);
}


#[test]
fn lerp_rot_goes_the_short_way() {
    assert!((lerp_rot(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    // From 3 toward −3 (6.28 − 6 = 0.28 the short way, across π).
    let x = lerp_rot(3.0, -3.0, 0.5);
    assert!(diff_rots(x, 3.0 + (2.0 * PI - 6.0) / 2.0) < 1e-5, "{x}");
    assert_eq!(lerp_rot(1.0, 2.0, 0.0), 1.0);
}

/// A moby (class 577's test anim) with a wander record at pvar 0: home (120, 120, 10), radius 0.4, step 0.05, turn
/// 0.05, leash 1.5, shy 3, timer range 60..150 (749's values).
fn wander_sim(at: [f32; 3], hero: [f32; 3]) -> Sim {
    let mut p = vec![0u8; 0x80];
    let set = |p: &mut Vec<u8>, o: usize, b: &[u8]| p[o..o + b.len()].copy_from_slice(b);
    for (o, x) in [(0x0, 120.0f32), (0x4, 120.0), (0x8, 10.0), (0x10, 0.4), (0x14, 0.05), (0x18, 0.05), (0x1c, 1.5), (0x20, 3.0)] { set(&mut p, o, &x.to_le_bytes()); }
    set(&mut p, 0x2c, &60i16.to_le_bytes());
    set(&mut p, 0x2e, &150i16.to_le_bytes());
    let mut s = Sim::new(577, at, p, hero);
    s.table.mobys[1].mode &= !PVAR_HEADER;
    s
}

#[test]
fn wander_picks_turns_and_steps_on_the_ground() {
    let mut s = wander_sim([120.0, 120.0, 10.0], [150.0, 150.0, 10.0]);
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    w.coll = Some(&s.mesh);
    let r0 = *w.rng;
    walker::wander(&mut w, 1, 0.5, 0.5, 0);
    // The pick: randf_sym(π/4, 5π/6) added to the heading, rand_range(60, 150) turn ticks; nothing else draws.
    assert_eq!(draws(&r0, w.rng), 2);
    let h = pf(&w, 1, walker::wr::HEADING);
    assert!((0.785..=2.62).contains(&h.abs()), "heading {h}");
    let t = pi16(&w, 1, walker::wr::TIMER);
    assert!((60..=150).contains(&t), "timer {t}");
    assert_eq!(pi16(&w, 1, walker::wr::TURNING), 1);
    // The step: 0.05 along the yaw (0 before the turn), z on the floor.
    let p = pos(&w, 1);
    assert!((p[0] - 120.05).abs() < 1e-4 && (p[1] - 120.0).abs() < 1e-4 && p[2] == 10.0, "{p:?}");
    // Turning: the yaw approaches the heading by at most 0.05 a tick.
    let y0 = yaw(&w, 1);
    walker::wander(&mut w, 1, 0.5, 0.5, 0);
    assert!((yaw(&w, 1) - y0).abs() <= 0.05 + 1e-6 && yaw(&w, 1) != y0);
}

#[test]
fn wander_heads_home_past_the_leash_and_shies_from_ratchet() {
    let mut s = wander_sim([122.0, 120.0, 10.0], [150.0, 150.0, 10.0]);
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    w.coll = Some(&s.mesh);
    set_pi16(&mut w, 1, walker::wr::TURNING, 1);
    set_pi16(&mut w, 1, walker::wr::TIMER, 100);
    walker::wander(&mut w, 1, 0.5, 0.5, 0);
    // 2 from home (leash 1.5): the heading points home, a new turn of 30..90 ticks.
    let h = pf(&w, 1, walker::wr::HEADING);
    let p = pos(&w, 1);
    assert!((h - atan(120.0 - p[0], 120.0 - p[1])).abs() < 1e-5, "heading {h}");
    let t = pi16(&w, 1, walker::wr::TIMER);
    assert!((30..=90).contains(&t), "timer {t}");
    // Ratchet 1.5 away (shy 3), inside the leash: the heading turns half way (1.5 / 3) toward away from him.
    let mut s = wander_sim([120.0, 120.0, 10.0], [118.5, 120.0, 10.0]);
    let mut w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    w.coll = Some(&s.mesh);
    set_pi16(&mut w, 1, walker::wr::TURNING, 1);
    set_pi16(&mut w, 1, walker::wr::TIMER, 100);
    set_pf(&mut w, 1, walker::wr::HEADING, PI / 2.0);
    set_yaw(&mut w, 1, PI / 2.0);
    walker::wander(&mut w, 1, 0.5, 0.5, 0);
    let p = pos(&w, 1);
    let d = dist2(p, [118.5, 120.0, 10.0, 0.0]);
    let away = atan(p[0] - 118.5, p[1] - 120.0);
    let want = lerp_rot(PI / 2.0, away, d / 3.0);
    assert!((pf(&w, 1, walker::wr::HEADING) - want).abs() < 1e-5);
}

#[test]
fn the_sphere_hit_template_pushes_along_the_facing() {
    let mut s = Sim::new(577, [120.0, 120.0, 10.0], critter_pvars(), [150.0, 150.0, 10.0]);
    s.table.mobys[1].rotation[2] = PI / 2.0;
    let w = World::new(&mut s.table, &s.hero, &mut s.rng, &s.classes, &mut s.svc, 1);
    let t = attack::sphere_template(&w, 1, 1.0, 2.0, 1, 0, 1);
    let d = t.dir.map(|x| f32::from_bits(x.0));
    assert!(d[0].abs() < 1e-6 && (d[1] - 2.0).abs() < 1e-6 && d[2] == 1.0 && d[3] == crate::hero::damage::EXACT_PUSH_W, "{d:?}");
    assert_eq!((t.attacker, t.flags, t.b18, t.b19, t.h1a, t.damage, t.w20), (Some(1), 1, 0, 1, 577, Pf::ONE, 1));
}
