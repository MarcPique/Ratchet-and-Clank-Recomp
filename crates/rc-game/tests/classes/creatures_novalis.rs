//! The Novalis creatures headless (docs/plan/creatures.md): the level as the engine ticks it (the loader's spawn test
//! on a first visit, the static mobys, the scheduler's load pass and moby loop, the moby collision, the hero with the
//! wrench, the tick's hit path) with Ratchet placed next to a critter group. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::damage::DamageEvent;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, HERO_LISTS};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::particles::Particles;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Lv {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    tests: Vec<moby_spawn::SpawnTest>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spheres: std::collections::HashMap<i16, [f32; 4]>,
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
    items: ItemData,
}

fn load() -> Option<Lv> {
    let dir = rc_formats::test_data::root().join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    let mut spheres = std::collections::HashMap::new();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            spheres.insert(oc, c.header.bsphere);
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: scheduler::port_update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let ratchet_blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&ratchet_blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    // The wrench as the engine builds it (item definition 8 = class 71 on list 0).
    let mut defs = vec![ItemDef::default(); 37];
    defs[8] = ItemDef { slot: 0, attach: 0, o_class: 71, b18: 0 };
    let hero_chains = HERO_LISTS.iter().map(|&l| gadget::joint_list(&ratchet_blob, &class.header, l).map(|(a, _)| a).unwrap_or_default()).collect();
    let item_classes = gadget::parse_gadget_classes(&core, &data)
        .unwrap()
        .iter()
        .map(|g| {
            let c = &g.moby.class;
            let chains = (0..16).map_while(|l| gadget::joint_list(&g.blob, &c.header, l).ok().map(|(a, _)| a)).collect();
            ItemClass { o_class: g.moby.o_class as i16, anim: MobyAnimClass::new(c, parse_sequences(&g.blob, c).unwrap_or_default()), scale: c.header.scale, chains }
        })
        .collect();
    let items = ItemData { defs, hero_chains, classes: item_classes };
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spheres, spawnable, death_z, coll_blobs, ratchet, items })
}

/// One tick of a run: the awake creatures' (moby, state, position), the hero's state and health, the rng.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    critters: Vec<(usize, u8, [f32; 3])>,
    hero_state: i32,
    health: i32,
    hero_pos: [f32; 3],
    rng: u32,
    dynamic: Vec<i16>,
}

/// The hero's hit sink over the moby loop's services (as the engine's): the wrench's line sweep and spheres.
struct CellHits<'a, 'b> {
    svc: &'a std::cell::RefCell<&'b mut Services>,
    classes: &'a ClassTable,
    coll: &'a collision::Collision,
}

impl rc_game::hero::items::HitSink for CellHits<'_, '_> {
    fn sphere(&mut self, table: &mut MobyTable, r: rc_game::ps2v::Pf, centre: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<usize> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.sphere(table, r, centre, flags, ignore, tmpl)
    }
    fn line(&mut self, table: &mut MobyTable, a: rc_game::hero::physics::V4, b: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &rc_game::moby_update::services::HitTemplate) -> Option<Option<usize>> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.line(table, a, b, flags, ignore, tmpl)
    }
}

struct RunOut {
    rows: Vec<Row>,
    hits_taken: usize,
    unported: Vec<(String, u64)>,
    bolts_dropped: usize,
    pieces: usize,
    lights: u64,
}

/// Novalis with its mobys, Ratchet at `at` (yaw), `input(t)` for `ticks` ticks.
fn run(lv: &Lv, at: ([f32; 3], f32), input: &dyn Fn(u32) -> PadInput, ticks: u32) -> RunOut {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let mut mobys = statics.mobys.clone();
    let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let (p, yaw) = at;
    mobys[hero_idx].position = [p[0], p[1], p[2], 1.0];
    mobys[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    let mut table = MobyTable::new(mobys, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    game.item_data = Some(lv.items.clone());
    game.item_globals.wrench_flag = 1;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    svc.creatures.class_spheres = lv.spheres.clone();
    let mut particles = Particles::new(None, Vec::new());
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        w.particles = Some(&mut particles);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let n_static = game.mobys.first_dynamic;
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let parts_cell = std::cell::RefCell::new(&mut particles);
    let mut rows = Vec::new();
    let (mut hits_taken, mut bolts_dropped, mut pieces) = (0, 0, 0);
    let mut seen_dyn = std::collections::HashSet::new();
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            sched.tick(&mut w);
        };
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| { parts_cell.borrow_mut().update_parts(rng); };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh };
        let r = game.tick_with_hits(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits);
        if r.hero == rc_game::hero::HeroTick::OutOfBounds { break; }
        hits_taken += game.hero.damage.events.iter().filter(|e| matches!(e, DamageEvent::Hit)).count();
        game.hero.damage.events.clear();
        let critters = game.mobys.mobys.iter().enumerate().filter(|(_, m)| [577, 0x23c, 0x361, 0x362].contains(&m.o_class) && m.state < 0xfd && m.state != 0xc).map(|(i, m)| (i, m.state, [m.position[0], m.position[1], m.position[2]])).collect();
        let mut dynamic = Vec::new();
        for (i, m) in game.mobys.mobys.iter().enumerate().skip(n_static) {
            if m.state >= 0xfd { continue; }
            dynamic.push(m.o_class);
            if seen_dyn.insert((i, m.uid_hi >> 16, m.o_class, m.delete_tick)) {
                if (13..=16).contains(&m.o_class) { bolts_dropped += 1; }
                if (1747..=1749).contains(&m.o_class) { pieces += 1; }
            }
        }
        rows.push(Row { critters, hero_state: game.hero.state, health: game.hero.health, hero_pos: game.hero.position(), rng: game.rng.state, dynamic });
    }
    let s = svc_cell.borrow();
    RunOut { rows, hits_taken, unported: s.fx.unported.iter().map(|(k, v)| (k.to_string(), *v)).collect(), bolts_dropped, pieces, lights: s.creatures.lights }
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_CREATURE_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate().step_by(10) {
        eprintln!("{t:4} hero {:#x} hp {} {:?} crit {:?} dyn {:?}", r.hero_state, r.health, r.hero_pos.map(|x| (x * 10.0).round() / 10.0), r.critters.iter().map(|c| (c.0, c.1, c.2.map(|x| (x * 10.0).round() / 10.0))).collect::<Vec<_>>(), r.dynamic);
    }
}

/// Critter group 1 (instances 579 / 591, hoverers at (142.5, 160.5, 40) and (139.9, 163.0, 40)): Ratchet 4 units away.
const NEAR_GROUP1: ([f32; 3], f32) = ([146.5, 160.5, 40.5], std::f32::consts::PI);

#[test]
fn novalis_critters_notice_and_bite_ratchet() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let out = run(&lv, NEAR_GROUP1, &|_| PadInput::neutral(), 600);
    dump(&out.rows);
    let first = &out.rows[0];
    let last = out.rows.last().unwrap();
    eprintln!("critters at tick 0: {:?}", first.critters);
    eprintln!("hits taken {}, health {} -> {}, unported {:?}", out.hits_taken, first.health, last.health, out.unported);
    assert_eq!(first.critters.len(), 14 + 5, "the 14 critters and the 5 big amoeboids the first visit creates (awake)");
    // The two nearby hoverers dropped to the ground and walked (left states 0xe / 0xf).
    let moved: Vec<_> = last.critters.iter().filter(|c| first.critters.iter().any(|f| f.0 == c.0 && ((f.2[0] - c.2[0]).powi(2) + (f.2[1] - c.2[1]).powi(2)).sqrt() > 1.0)).collect();
    assert!(!moved.is_empty(), "no critter moved");
    assert!(out.rows.iter().any(|r| r.critters.iter().any(|c| c.1 == 4)), "no critter reached the bite state");
    assert!(out.hits_taken > 0, "Ratchet was never hit");
    assert!(last.health < first.health, "no health lost");
    assert!(out.rows.iter().any(|r| r.hero_state == 0x16), "Ratchet never entered the hurt state 0x16");
    // Deterministic.
    let again = run(&lv, NEAR_GROUP1, &|_| PadInput::neutral(), 600);
    assert_eq!(out.rows, again.rows);
}

#[test]
fn novalis_critters_die_from_the_wrench_and_drop_bolts() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    // Swing the wrench (□) in bursts while the critters come to bite.
    let input = |t: u32| if t >= 60 && t % 24 < 2 { PadInput::neutral().press(button::SQUARE) } else { PadInput::neutral() };
    let out = run(&lv, NEAR_GROUP1, &input, 900);
    dump(&out.rows);
    eprintln!("bolts dropped {}, pieces {}, lights {}, unported {:?}", out.bolts_dropped, out.pieces, out.lights, out.unported);
    let first = &out.rows[0];
    let last = out.rows.last().unwrap();
    let killed = first.critters.iter().filter(|c| !last.critters.iter().any(|l| l.0 == c.0)).count();
    assert!(killed >= 1, "no critter died");
    assert!(out.rows.iter().any(|r| r.critters.iter().any(|c| c.1 == 99)), "no death flight");
    assert!(out.pieces >= 3, "no body pieces");
    assert!(out.bolts_dropped >= 1, "no bolts dropped");
    let again = run(&lv, NEAR_GROUP1, &input, 900);
    assert_eq!(out.rows, again.rows);
}

/// Amoeboid group 22 (big 572 at (51.8, 141.6, 40.1), two 865 and four 866 waiting): Ratchet 5 units away inside its
/// arena.
const NEAR_GROUP22: ([f32; 3], f32) = ([55.0, 145.0, 40.6], -2.3);

#[test]
fn novalis_amoeboids_chase_strike_split_and_die() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let input = |t: u32| if t >= 60 && t % 24 < 2 { PadInput::neutral().press(button::SQUARE) } else { PadInput::neutral() };
    let out = run(&lv, NEAR_GROUP22, &input, 900);
    let group = |r: &Row| -> Vec<(usize, u8)> { r.critters.iter().filter(|c| c.2[0] < 80.0 && c.2[1] > 130.0).map(|c| (c.0, c.1)).collect() };
    if std::env::var("RC_CREATURE_DUMP").is_ok() {
        for (t, r) in out.rows.iter().enumerate().step_by(10) { eprintln!("{t:4} hero {:#x} hp {} amoeboids {:?}", r.hero_state, r.health, group(r)); }
    }
    eprintln!("hits taken {}, bolts {}, unported {:?}", out.hits_taken, out.bolts_dropped, out.unported);
    let states: std::collections::BTreeSet<u8> = out.rows.iter().flat_map(|r| group(r).into_iter().map(|g| g.1)).collect();
    let ids: std::collections::BTreeSet<usize> = out.rows.iter().flat_map(|r| group(r).into_iter().map(|g| g.0)).collect();
    eprintln!("amoeboid states seen {states:?}, mobys {ids:?}");
    assert_eq!(group(&out.rows[0]).len(), 1, "one big amoeboid awake in group 22 at the start");
    for s in [4, 6, 8, 9] { assert!(states.contains(&s), "state {s:#x} never reached: {states:?}"); }
    assert!(ids.len() >= 5, "no split down to the small ones: {ids:?}");
    assert!(out.hits_taken >= 1, "Ratchet never hit");
    assert!(out.bolts_dropped >= 1, "no bolts");
    let again = run(&lv, NEAR_GROUP22, &input, 900);
    assert_eq!(out.rows, again.rows);
}

/// Survey helper: the Novalis creature instances and their pvars (`cargo xtask test-job --test classes --filter creatures_novalis:: --ignored`).
#[test]
#[ignore]
fn creatures_survey() {
    let Some(lv) = load() else { return };
    for oc in [577i16, 0x23c, 0x361, 0x362, 500] {
        let i = lv.classes.classes[&oc].0;
        println!("class {oc}: collision {} mode bits {:#x} scale {} blob {}", i.has_collision, i.mode_bits, i.scale, lv.coll_blobs.iter().any(|b| b.0 == oc as i32));
    }
    let hero = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    for (k, (i, t)) in lv.instances.iter().zip(&lv.tests).enumerate() {
        if ![577, 572, 865, 866, 459, 815, 666].contains(&i.o_class) || !t.spawn { continue; }
        let d = ((i.position[0] - hero[0]).powi(2) + (i.position[1] - hero[1]).powi(2)).sqrt();
        println!("inst {k} class {} id {} grp {} pos {:?} rot {:.2} d {d:.1}", i.o_class, i.spawn_id, i.group, i.position, i.rotation[2]);
    }
}
