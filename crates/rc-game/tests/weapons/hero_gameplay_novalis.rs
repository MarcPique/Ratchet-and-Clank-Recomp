//! Crates, pickups, health, ammo and the weapon arm on Novalis, headless (docs/plan/hero_gameplay.md): the level as
//! the engine ticks it (the loader's spawn test on a first visit, the static mobys, the scheduler's load pass and moby
//! loop, the particles, the moby collision, the hero with the wrench and the Bomb Glove, the hit path, the camera),
//! with Ratchet placed next to a crate that is broken by a wrench-like hit. A bolt crate pays bolts, a nanotech crate's
//! orbs heal him (up to max health), an ammo crate's pickups fill the glove (up to its max); a glove throw while
//! running plays on the arm layer while his legs keep the run. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, WeaponDef, HERO_LISTS};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::classes::pickup::ItemTables;
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{HitTemplate, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::particles::Particles;
use rc_game::ps2v::Pf;
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
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
    arm_joints: [Vec<u8>; 3],
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
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
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
    let arm_joints = rc_game::hero::anim::LAYER_LISTS.map(|l| gadget::joint_list(&ratchet_blob, &class.header, l as usize).map(|(_, b)| b).unwrap_or_default());
    let mut defs = vec![ItemDef::default(); 37];
    defs[8] = ItemDef { slot: 0, attach: 0, o_class: 71, b18: 0 };
    defs[10] = ItemDef { slot: 0, attach: 6, o_class: 192, b18: 0 };
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, arm_joints, items })
}

struct CellHits<'a, 'b> {
    svc: &'a std::cell::RefCell<&'b mut Services>,
    classes: &'a ClassTable,
    coll: &'a collision::Collision,
}

impl rc_game::hero::items::HitSink for CellHits<'_, '_> {
    fn sphere(&mut self, table: &mut MobyTable, r: Pf, centre: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &HitTemplate) -> Option<usize> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.sphere(table, r, centre, flags, ignore, tmpl)
    }
    fn line(&mut self, table: &mut MobyTable, a: rc_game::hero::physics::V4, b: rc_game::hero::physics::V4, flags: u32, ignore: Option<usize>, tmpl: &HitTemplate) -> Option<Option<usize>> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::ServiceHits { svc: &mut s, classes: self.classes, coll: Some(self.coll) }.line(table, a, b, flags, ignore, tmpl)
    }
    fn deliver(&mut self, table: &mut MobyTable, target: usize, tmpl: &HitTemplate) {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::services::deliver_hit_in(table, &mut s.hits, target, tmpl);
    }
    fn create_moby(&mut self, table: &mut MobyTable, o_class: i16, counter: u64) -> Option<usize> {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::classes::bomb::create_from_hero(table, &mut s, self.classes, o_class, counter)
    }
    fn delete_moby(&mut self, table: &mut MobyTable, id: usize, counter: u64) {
        let mut s = self.svc.borrow_mut();
        rc_game::moby_update::classes::bomb::delete_from_hero(table, &mut s, id, counter);
    }
}

/// What a run gives back per tick.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    health: i32,
    bolts: i32,
    ammo: i32,
    crate_state: u8,
    /// The live pickups (ammo classes) and nanotech clusters (806): (class, state).
    pickups: Vec<(i16, u8)>,
    /// Live type-62 records (orbs and trails).
    orbs: u32,
    /// Ratchet's main animation (seq B) and the arm layer (seq B, weight) when out.
    seq: u8,
    layer: Option<(u8, f32)>,
    rng: u32,
}

struct Setup {
    at: ([f32; 3], f32),
    hand: i32,
    watch: usize,
    health: i32,
    ammo: i32,
    /// Tick at which the watched crate takes a wrench-like hit (flags 0x10000, damage 1).
    hit_at: Option<u32>,
}

/// The Bomb Glove's price record as the item tables read it (level01 0x1c4530 + 10·0x18): max ammo 40, 3 bombs a
/// pickup; the vendor list holds the glove.
fn item_tables() -> ItemTables {
    let mut recs = vec![[0u8; 0x18]; 37];
    recs[10][0xc] = 3;
    recs[10][0xe] = 40;
    let mut list = [0xff; 12];
    list[0] = 0x4a;
    ItemTables::new(&recs, list)
}

/// The palette with and without the arm layer at the first full-weight tick (layered joints first).
type Palettes = Option<Vec<[[f32; 4]; 4]>>;

fn run(lv: &Lv, s: &Setup, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> (Vec<Row>, Palettes) {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let watch = statics.moby_to_instance.iter().position(|&i| i == s.watch).expect("the crate is created");
    let mut mobys = statics.mobys.clone();
    let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let (p, yaw) = s.at;
    mobys[hero_idx].position = [p[0], p[1], p[2], 1.0];
    mobys[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    let mut table = MobyTable::new(mobys, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    game.item_data = Some(lv.items.clone());
    game.item_globals.wrench_flag = 1;
    game.hero.grant_items(&[10]);
    game.hero.weapons.uses_ammo[10] = true;
    game.hero.weapons.ammo[10] = s.ammo;
    game.hero.weapons.defs = vec![WeaponDef::default(); 37];
    game.hero.weapons.defs[10] = WeaponDef { w18: 0, anims: [44, 66, 44], w30: 0 };
    game.hero.health = s.health;
    if s.hand != 8 {
        game.item_globals.request = s.hand;
        game.item_globals.saved = s.hand;
    }
    let mut svc = Services::new();
    svc.level = 1;
    svc.counters.max_hp = 4;
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut particles = Particles::new(None, Vec::new());
    let mut sched = Scheduler::new();
    let base = item_tables();
    {
        let hero: Hero = game.hero.clone();
        let inv = base.clone().with_hero(&hero);
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        w.particles = Some(&mut particles);
        w.inventory = &inv;
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&lv.ratchet);
    anim.layer_joints = lv.arm_joints.clone();
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let parts_cell = std::cell::RefCell::new(&mut particles);
    let mut rows = Vec::new();
    let mut layered = None;
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let hit = s.hit_at == Some(t);
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut sv = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let inv = base.clone().with_hero(hero);
            let mut w = World::new(table, hero, rng, classes_ref, &mut sv, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            w.inventory = &inv;
            if hit { w.deliver_hit(watch, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() }); }
            sched.tick(&mut w);
        };
        let mut parts = |h: &Hero, _: &rc_game::follow_camera::CameraView, rng: &mut Rng, _: u64| {
            let mut p = parts_cell.borrow_mut();
            p.hero = rc_game::hero::physics::to_f32x3(h.pos);
            p.update_parts(rng);
        };
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        let mut hits = CellHits { svc: &svc_cell, classes: &classes, coll: &lv.mesh };
        let r = game.tick_with_hits(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let pickups = game.mobys.mobys.iter().filter(|m| m.state < 0x80 && (rc_game::moby_update::classes::pickup::AMMO_CLASSES.contains(&m.o_class) || m.o_class == 806)).map(|m| (m.o_class, m.state)).collect();
        let layer = h.weapons.layers[0].map(|l| (l.seq_b, l.weight));
        if std::env::var("RC_GAMEPLAY_DUMP").is_ok() {
            for m in game.mobys.mobys.iter().filter(|m| m.state < 0x80 && m.o_class == 226) {
                eprintln!("  pickup st {} pos {:?} vel {:?} t20 {} alpha {} scale {}", m.state, &m.position[..3], rc_game::moby_update::services::pvar::v4f(&m.pvars, 0x10), rc_game::moby_update::services::pvar::i32(&m.pvars, 0x20), m.alpha, m.scale);
            }
        }
        if layer.is_some_and(|l| l.1 == 1.0) && layered.is_none() {
            // The palette with and without the arm layer, at a tick the layer is at full weight.
            let pl = rc_game::hero::anim::pose_layers(&h.weapons.layers, &lv.arm_joints);
            let with = rc_formats::moby_anim::evaluate_layered(&lv.ratchet, &anim.state, anim.snapshot.as_ref(), &pl);
            let without = rc_formats::moby_anim::evaluate_with_snapshot(&lv.ratchet, &anim.state, anim.snapshot.as_ref());
            layered = Some(with.into_iter().chain(without).collect());
        }
        rows.push(Row {
            state: h.state,
            pos: h.position(),
            health: h.health,
            bolts: svc_cell.borrow().counters.bolts,
            ammo: h.weapons.ammo[10],
            crate_state: game.mobys.mobys[watch].state,
            pickups,
            orbs: parts_cell.borrow().live_by_type()[62],
            seq: anim.state.seq_b,
            layer,
            rng: game.rng.state,
        });
    }
    (rows, layered)
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_GAMEPLAY_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() {
        eprintln!("{t:4} {:#x} {:?} hp {} bolts {} ammo {} crate {:#x} pickups {:?} orbs {} seq {} layer {:?}", r.state, r.pos, r.health, r.bolts, r.ammo, r.crate_state, r.pickups, r.orbs, r.seq, r.layer);
    }
}

/// A place `d` units from crate `inst` (at angle `from`), facing it.
fn facing(lv: &Lv, inst: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let c = lv.instances[inst].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2]];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

/// Survey helper: the spawned crates of each drop kind near the spawn (`cargo xtask test-job --test weapons --filter hero_gameplay_novalis::
/// --ignored --nocapture`).
#[test]
#[ignore]
fn crate_survey() {
    let Some(lv) = load() else { return };
    let hero = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    println!("spawn {hero:?}");
    for (k, (i, t)) in lv.instances.iter().zip(&lv.tests).enumerate() {
        if !matches!(i.o_class, 500 | 501 | 511) || !t.spawn { continue; }
        let d = ((i.position[0] - hero[0]).powi(2) + (i.position[1] - hero[1]).powi(2)).sqrt();
        if d < 120.0 { println!("inst {k} class {} pos {:?} d {d:.1} b4 {} flags {}", i.o_class, i.position, i.unknown_10, i.spawn_flags); }
    }
    // The moby ids (for the engine's RC_DEBUG_HIT) of the crates the tests use.
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    for inst in [BOLT_CRATE, NANOTECH_CRATE, AMMO_CRATE] {
        println!("instance {inst} = moby {:?}", statics.moby_to_instance.iter().position(|&i| i == inst));
    }
}

/// Crate 376 (class 500 at (147.4, 125.8, 57.0), below the spawn plateau); Ratchet west of it.
const BOLT_CRATE: usize = 376;
const WEST: f32 = std::f32::consts::PI;

#[test]
fn novalis_bolt_crate_pays_bolts() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let s = Setup { at: facing(&lv, BOLT_CRATE, 1.5, WEST), hand: 8, watch: BOLT_CRATE, health: 4, ammo: 10, hit_at: Some(20) };
    let (rows, _) = run(&lv, &s, &|_| PadInput::neutral(), 240);
    dump(&rows);
    assert!(rows[..20].iter().all(|r| r.bolts == 0));
    assert!(rows[21..].iter().any(|r| r.crate_state >= 0xfd || r.crate_state == 3), "the crate did not break");
    assert!(rows.last().unwrap().bolts > 0, "no bolts collected");
    let (again, _) = run(&lv, &s, &|_| PadInput::neutral(), 240);
    assert_eq!(rows, again, "deterministic");
}

/// Nanotech crate 533 (class 501 at (148.2, 126.4, 58.0), stacked on the bolt crate 377); Ratchet 1.5 south of it.
const NANOTECH_CRATE: usize = 533;
/// Ammo crate 552 (class 511 at (171.1, 129.5, 60.8), near the spawn); Ratchet 1.5 south of it.
const AMMO_CRATE: usize = 552;
const SOUTH: f32 = -std::f32::consts::FRAC_PI_2;

#[test]
fn novalis_nanotech_crate_heals_up_to_max() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing(&lv, NANOTECH_CRATE, 1.5, SOUTH);
    let at = ([at.0[0], at.0[1], 57.0], at.1);
    // Hurt (2 of 4): the cluster over the crate shows its 8 orbs; the crate breaks, the orbs fly to him and the first
    // to arrive heals one point.
    let s = Setup { at, hand: 8, watch: NANOTECH_CRATE, health: 2, ammo: 10, hit_at: Some(30) };
    let (rows, _) = run(&lv, &s, &|_| PadInput::neutral(), 420);
    dump(&rows);
    assert!(rows[5..30].iter().all(|r| r.pickups.contains(&(806, 1)) && r.orbs >= 8), "the cluster and its orbs over the crate");
    assert!(rows[..30].iter().all(|r| r.health == 2));
    let flying = rows.iter().position(|r| r.pickups.contains(&(806, 3))).expect("the orbs never flew");
    let healed = rows.iter().position(|r| r.health == 3).expect("never healed");
    assert!(30 < flying && flying < healed, "break {flying} heal {healed}");
    assert_eq!(rows.last().unwrap().health, 3, "one crate heals one point");
    assert!(!rows.last().unwrap().pickups.iter().any(|p| p.0 == 806 && p.1 == 3), "the cluster is gone after its flight");
    // At max health (4 of 4) the free cluster waits on the ground: no flight, no heal.
    let full = Setup { health: 4, ..s };
    let (rows, _) = run(&lv, &full, &|_| PadInput::neutral(), 240);
    assert!(rows.iter().all(|r| r.health == 4 && !r.pickups.contains(&(806, 3))));
    assert!(rows.last().unwrap().pickups.contains(&(806, 2)), "the free cluster stays");
    // One below max: healed to max.
    let one = Setup { health: 3, ..s };
    let (rows, _) = run(&lv, &one, &|_| PadInput::neutral(), 420);
    assert_eq!(rows.last().unwrap().health, 4);
    let (again, _) = run(&lv, &one, &|_| PadInput::neutral(), 420);
    assert_eq!(rows, again, "deterministic");
}

#[test]
fn novalis_ammo_crate_fills_the_glove_up_to_max() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    // South of the crate: its pickup lands 1.4 from him (inside the 3.0 / 1.75 pickup volume).
    let at = facing(&lv, AMMO_CRATE, 1.5, SOUTH);
    let s = Setup { at, hand: 8, watch: AMMO_CRATE, health: 4, ammo: 38, hit_at: Some(30) };
    let (rows, _) = run(&lv, &s, &|_| PadInput::neutral(), 240);
    dump(&rows);
    assert!(rows[..30].iter().all(|r| r.ammo == 38));
    assert!(rows.iter().any(|r| r.pickups.iter().any(|p| p.0 == 226)), "no Bomb Glove pickup (class 226) dropped");
    assert_eq!(rows.last().unwrap().ammo, 40, "3 bombs a pickup, capped at 40");
    // The crate drops 1 (4 in 5) or 2 pickups (its `randi(5)` on the level's shared RNG stream, which every class
    // draws from: the count changes with the classes ported); the first fills the glove, a second stays at max.
    let dropped = rows.iter().map(|r| r.pickups.iter().filter(|p| p.0 == 226).count()).max().unwrap();
    let left = rows.last().unwrap().pickups.iter().filter(|p| p.0 == 226).count();
    assert!(left < dropped && left <= 1, "the pickups were collected ({dropped} dropped, {left} left at full ammo)");
    // Full: the pickups stay on the ground (dim while he stands in them), the ammo stays at max.
    let full = Setup { ammo: 40, ..s };
    let (rows, _) = run(&lv, &full, &|_| PadInput::neutral(), 240);
    assert!(rows.iter().all(|r| r.ammo == 40));
    assert!(rows.last().unwrap().pickups.iter().any(|p| p.0 == 226), "a full glove leaves the pickups");
    let (again, _) = run(&lv, &s, &|_| PadInput::neutral(), 240);
    assert_eq!(run(&lv, &s, &|_| PadInput::neutral(), 240).0, again, "deterministic");
}

/// Running with the Bomb Glove, ○ at tick 70: the throw plays on the arm layer (Ratchet's joint list 12, the glove's
/// moving sequence 66 at full weight) while the main animation stays the run; the layer only moves the arm's joints.
#[test]
fn novalis_running_throw_uses_the_arm_layer() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let spawn = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    let s = Setup { at: (spawn, 0.0), hand: 10, watch: BOLT_CRATE, health: 4, ammo: 10, hit_at: None };
    let input = |t: u32| {
        let p = if (40..=160).contains(&t) { PadInput::neutral().stick(0.0, -1.0) } else { PadInput::neutral() };
        if (70..=71).contains(&t) { p.press(button::CIRCLE) } else { p }
    };
    let (rows, pal) = run(&lv, &s, &input, 200);
    dump(&rows);
    let out = rows.iter().position(|r| r.layer.is_some()).expect("no arm layer");
    assert!((70..=73).contains(&out), "the arm came up at {out}");
    let full = rows.iter().position(|r| r.layer.is_some_and(|l| l.1 == 1.0)).unwrap();
    let r = &rows[full];
    assert_eq!(r.layer.unwrap().0, 66, "the glove's moving sequence");
    assert_eq!(r.state, 2, "still running");
    assert!(matches!(r.seq, 3 | 4), "the main animation is the walk / run: {}", r.seq);
    assert!(rows[full..].iter().take(20).all(|r| r.state == 2 && matches!(r.seq, 3 | 4)), "the legs keep running");
    // The throw: one bomb used; the layer fades after its sequence ends and is freed.
    assert_eq!(rows.last().unwrap().ammo, 9);
    assert!(rows.iter().skip(full).any(|r| r.layer.is_some_and(|l| l.1 < 1.0)), "the layer fades");
    assert!(rows.last().unwrap().layer.is_none(), "the layer is freed");
    // The palette: the arm list's joints move, every other joint is the unlayered pose.
    let pal = pal.expect("a full-weight tick");
    let n = pal.len() / 2;
    let arm: std::collections::HashSet<usize> = lv.arm_joints[0].iter().map(|&j| j as usize).collect();
    assert!(!arm.is_empty());
    // A joint is affected when it or an ancestor is in the list: compare only the joints with no listed ancestor.
    let listed_or_below = |j: usize| {
        let mut k = Some(j);
        while let Some(i) = k {
            if arm.contains(&i) { return true; }
            k = lv.ratchet.parent(i);
        }
        false
    };
    let mut moved = 0;
    for j in 0..n {
        if listed_or_below(j) {
            if pal[j] != pal[n + j] { moved += 1; }
        } else {
            assert_eq!(pal[j], pal[n + j], "joint {j} outside the arm moved");
        }
    }
    assert!(moved > 0, "the arm layer changed nothing");
    let (again, _) = run(&lv, &s, &input, 200);
    assert_eq!(rows, again, "deterministic");
}
