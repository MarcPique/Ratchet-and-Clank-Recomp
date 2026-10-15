//! The Novalis enemies headless (docs/plan/creatures.md): the robot troopers 459 and their fire globs 722, the
//! dropship 666 that brings three of them, the gunships 688 with their shells 686, fires 700 and embers 696–698, and
//! the spawners 815 — on the level as the engine ticks it (the loader's spawn test on a first visit, the scheduler's
//! load pass and moby loop, the moby collision, the trigger volumes, the hero with the wrench and the Bomb Glove, the
//! tick's hit path). Skipped when `extracted/` is absent.
//!
//! The harness stands in for the renderer's `MobyProc` visibility write-back (+0x31, the engine's `write_visible`):
//! after every tick a moby is "drawn" when it is live, not hidden (mode 0x81) and within its draw distance of the
//! camera. The troopers only fire when drawn.

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
use std::collections::HashMap;
use std::sync::Arc;

struct Lv {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    tests: Vec<moby_spawn::SpawnTest>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spheres: HashMap<i16, [f32; 4]>,
    joints: HashMap<i16, Vec<Vec<u8>>>,
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
    let mut spheres = HashMap::new();
    let mut joints = HashMap::new();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            spheres.insert(oc, c.header.bsphere);
            if rc_game::moby_update::classes::needs_joint_lists(oc) {
                let lists = (0..16).map_while(|l| gadget::joint_list(&blob, &c.header, l).ok().map(|(a, _)| a)).collect();
                joints.insert(oc, lists);
            }
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
    // The wrench (item 8 = class 71 on list 0) and the Bomb Glove (item 10 = class 192 on list 6).
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spheres, joints, spawnable, death_z, coll_blobs, ratchet, items })
}

/// The hero's hit sink over the moby loop's services (as the engine's): the wrench's lines and spheres, and the
/// glove's bomb.
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
    fn deliver(&mut self, table: &mut MobyTable, target: usize, tmpl: &rc_game::moby_update::services::HitTemplate) {
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

/// The classes the rows record.
const WATCH: [i16; 10] = [459, 666, 688, 686, 722, 700, 696, 697, 698, 815];

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    /// (moby, class, state, mode, position) of every live watched moby.
    enemies: Vec<(usize, i16, u8, u16, [f32; 3])>,
    hero_state: i32,
    health: i32,
    hero_pos: [f32; 3],
    rng: u32,
}

struct RunOut {
    rows: Vec<Row>,
    /// Instance index → runtime moby index.
    moby_of: Vec<Option<usize>>,
    hits_taken: usize,
    /// Spawns seen per class (dynamic slots, by (slot, class, reuse)).
    spawned: HashMap<i16, usize>,
    unported: Vec<(String, u64)>,
    cutscene_seen: bool,
    part_spawns: HashMap<u8, u64>,
    /// The creature script requests the tick handed to the cinematic layer (`Cinematic::creature_log`), in order.
    scripts: Vec<rc_game::moby_update::creature::ScriptRequest>,
}

impl RunOut {
    fn states(&self, id: usize) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::new();
        for r in &self.rows {
            let s = r.enemies.iter().find(|e| e.0 == id).map_or(0xfd, |e| e.2);
            if v.last() != Some(&s) { v.push(s); }
        }
        v
    }
    fn at(&self, t: usize, id: usize) -> Option<(u8, u16, [f32; 3])> { self.rows[t].enemies.iter().find(|e| e.0 == id).map(|e| (e.2, e.3, e.4)) }
    fn moby(&self, inst: usize) -> usize { self.moby_of[inst].expect("instance created") }
}

/// Novalis with its mobys, Ratchet at `at` (yaw), `hand` (8 the wrench, 10 the glove with 10 bombs), `input(t)` for
/// `ticks` ticks.
fn run(lv: &Lv, at: ([f32; 3], f32), hand: i32, input: &dyn Fn(u32) -> PadInput, ticks: u32) -> RunOut { run_with(lv, at, hand, input, ticks, &[]) }

/// [`run`] with the persistent death bits (`0x14c190`) of these spawn ids of Novalis set, as a later visit has them.
fn run_with(lv: &Lv, at: ([f32; 3], f32), hand: i32, input: &dyn Fn(u32) -> PadInput, ticks: u32, death: &[i16]) -> RunOut {
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
    game.hero.grant_items(&[10]);
    game.hero.weapons.uses_ammo[10] = true;
    game.hero.weapons.ammo[10] = 10;
    if hand != 8 {
        game.item_globals.request = hand;
        game.item_globals.saved = hand;
    }
    let mut svc = Services::new();
    svc.level = 1;
    for &sid in death { svc.save.death.insert((1, sid)); }
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.set_volumes(rc_formats::volumes::parse_volumes(&lv.gp).unwrap());
    svc.joint_lists = lv.joints.clone();
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
    let mut hits_taken = 0;
    let mut spawned: HashMap<i16, usize> = HashMap::new();
    let mut seen: std::collections::HashSet<(usize, i16)> = std::collections::HashSet::new();
    let mut cutscene_seen = false;
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
        cutscene_seen |= svc_cell.borrow().creatures.cutscene;
        // MobyProc's +0x31 (module doc).
        let cam = game.camera.out.pos_f32();
        for m in game.mobys.mobys.iter_mut() {
            if m.state == rc_game::moby_runtime::state::END { break; }
            let d = ((m.position[0] - cam[0]).powi(2) + (m.position[1] - cam[1]).powi(2) + (m.position[2] - cam[2]).powi(2)).sqrt();
            m.visible = (m.state < 0x80 && m.mode & 0x81 == 0 && d < m.draw_dist.max(1) as f32) as u8;
        }
        let mut enemies = Vec::new();
        for (i, m) in game.mobys.mobys.iter().enumerate() {
            if m.state == rc_game::moby_runtime::state::END { break; }
            if m.state >= 0xfd || !WATCH.contains(&m.o_class) { continue; }
            enemies.push((i, m.o_class, m.state, m.mode, [m.position[0], m.position[1], m.position[2]]));
        }
        // A spawn = a live dynamic moby whose slot held no live moby of its class after the last tick.
        let mut live = std::collections::HashSet::new();
        for (i, m) in game.mobys.mobys.iter().enumerate().skip(n_static) {
            if m.state >= 0xfd { continue; }
            live.insert((i, m.o_class));
            if !seen.contains(&(i, m.o_class)) { *spawned.entry(m.o_class).or_default() += 1; }
        }
        seen = live;
        rows.push(Row { enemies, hero_state: game.hero.state, health: game.hero.health, hero_pos: game.hero.position(), rng: game.rng.state });
    }
    let s = svc_cell.borrow();
    RunOut {
        rows,
        moby_of: statics.instance_to_moby.clone(),
        hits_taken,
        spawned,
        unported: s.fx.unported.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        cutscene_seen,
        part_spawns: s.fx.part_spawns.clone(),
        scripts: s.cinematic.creature_log.iter().copied().collect(),
    }
}

fn dump(out: &RunOut, every: usize) {
    if std::env::var("RC_ENEMY_DUMP").is_err() { return; }
    for (t, r) in out.rows.iter().enumerate().step_by(every) {
        let e: Vec<_> = r.enemies.iter().filter(|e| e.1 != 700 && e.1 != 815).map(|e| (e.0, e.1, format!("{:#x}", e.2), e.4.map(|x| (x * 10.0).round() / 10.0))).collect();
        eprintln!("{t:4} hero {:#x} hp {} {:?} {:?}", r.hero_state, r.health, r.hero_pos.map(|x| (x * 10.0).round() / 10.0), e);
    }
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }

/// Ratchet's spawn point, inside the dropship's trigger cuboid 27.
const SPAWN: ([f32; 3], f32) = ([162.53032, 136.39348, 60.5], 2.4);

/// The dropship 666 (instance 687) flies in as soon as Ratchet is in cuboid 27 (the landing pad), carries troopers
/// 309 / 311 / 312 along path 41, opens at node 6 and releases them one by one; each glides down onto its drop cuboid
/// (38, 40, 41 at z 40) and starts its patrol. The gunship 694 bombards the town meanwhile.
#[test]
fn novalis_dropship_brings_three_troopers() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let out = run(&lv, SPAWN, 8, &|_| PadInput::neutral(), 1500);
    dump(&out, 30);
    let ship = out.moby(687);
    let troopers = [309, 311, 312].map(|i| out.moby(i));
    eprintln!("dropship states {:?}", out.states(ship));
    for (k, &t) in troopers.iter().enumerate() { eprintln!("trooper {} states {:?} last {:?}", [309, 311, 312][k], out.states(t), out.at(out.rows.len() - 1, t)); }
    eprintln!("spawned {:?} particles {:?} unported {:?}", out.spawned, out.part_spawns, out.unported);
    // The ship is hidden (0x41) and waiting in the load pass's tick, then flies (shown).
    let (_, m0, _) = out.at(0, ship).expect("dropship alive");
    assert_eq!(m0 & 0x41, 0, "the dropship shows up at once (Ratchet starts in its cuboid)");
    // Carried: mode 6, riding the ship.
    let (_, cm, cp) = out.at(1, troopers[0]).expect("trooper alive");
    assert_eq!(cm & 6, 6, "a carried trooper runs no update of its own");
    let (_, _, sp) = out.at(1, ship).unwrap();
    assert!(dist2(cp, sp) < 3.0, "the trooper rides the ship");
    // All three are released, glide down and land by their drop cuboids.
    let drops = [[155.49107, 167.23257, 40.0], [127.88299, 171.25891, 40.0], [128.02644, 165.02086, 40.0]];
    for (k, &t) in troopers.iter().enumerate() {
        let st = out.states(t);
        assert!(st.contains(&3), "trooper {k} never came down (states {st:?})");
        assert!(st.contains(&1), "trooper {k} never landed (states {st:?})");
        let first_land = out.rows.iter().position(|r| r.enemies.iter().any(|e| e.0 == t && e.2 == 1)).unwrap();
        let p = out.at(first_land, t).unwrap().2;
        assert!(dist2(p, drops[k]) < 1.5 && (p[2] - 40.0).abs() < 1.5, "trooper {k} landed at {p:?}, drop cuboid {:?}", drops[k]);
    }
    let again = run(&lv, SPAWN, 8, &|_| PadInput::neutral(), 1500);
    assert_eq!(out.rows, again.rows, "not deterministic");
}

/// Trooper 310 (patrol path 38 from (128.1, 175.8, 40), yaw −0.29; lifted 20, hidden): Ratchet 3 units in front of
/// it wakes it (within 30); it comes down on its jetpack facing him and sprays fire globs (they burst after 10 ticks,
/// 4 units out) that hurt him.
const NEAR_310: ([f32; 3], f32) = ([130.94, 174.93, 40.5], 2.85);

#[test]
fn novalis_trooper_drops_in_fires_and_hurts_ratchet() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let out = run(&lv, NEAR_310, 8, &|_| PadInput::neutral(), 900);
    dump(&out, 15);
    let t = out.moby(310);
    let st = out.states(t);
    eprintln!("trooper 310 states {st:?}; hits {}; health {} -> {}; spawned {:?}; unported {:?}", out.hits_taken, out.rows[0].health, out.rows.last().unwrap().health, out.spawned, out.unported);
    // The load pass's init lifted it 20 above its path's start; Ratchet (within 30) wakes it in the first tick.
    let (s0, m0, p0) = out.at(0, t).unwrap();
    assert_eq!(s0, 3, "woken in the first tick");
    assert_eq!(m0 & 0x41, 0, "shown once woken");
    assert!((p0[2] - 60.0).abs() < 0.2, "lifted 20 above its path's start ({p0:?})");
    assert!(st.contains(&3) && st.contains(&1) && st.contains(&6), "no arrival / patrol / fire ({st:?})");
    assert!(out.spawned.get(&722).copied().unwrap_or(0) > 0, "no fire glob");
    assert!(out.hits_taken > 0, "Ratchet was never hit");
    assert!(out.rows.iter().any(|r| r.hero_state == 0x16), "Ratchet never entered the hurt state");
    let again = run(&lv, NEAR_310, 8, &|_| PadInput::neutral(), 900);
    assert_eq!(out.rows, again.rows, "not deterministic");
}

/// The wrench: Ratchet swings (□ in bursts) at trooper 310 after it landed; it staggers, dies (death flight 0xe),
/// explodes into its four pieces and drops bolts.
#[test]
fn novalis_trooper_dies_to_the_wrench() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = ([125.5, 175.8, 40.5], 0.0);
    let input = |t: u32| if t >= 150 && t % 20 < 2 { PadInput::neutral().press(button::SQUARE) } else { PadInput::neutral() };
    let out = run(&lv, at, 8, &input, 900);
    dump(&out, 15);
    let t = out.moby(310);
    let st = out.states(t);
    eprintln!("trooper 310 states {st:?}; spawned {:?}; unported {:?}", out.spawned, out.unported);
    assert!(st.contains(&0xe), "no death flight ({st:?})");
    assert_eq!(*st.last().unwrap(), 0xfd, "not deleted ({st:?})");
    let pieces: usize = [1736, 1737, 1738, 1770].iter().map(|c| out.spawned.get(c).copied().unwrap_or(0)).sum();
    assert!(pieces >= 4, "no body pieces ({pieces})");
    let bolts: usize = (13..=16).map(|c| out.spawned.get(&c).copied().unwrap_or(0)).sum();
    assert!(bolts >= 1, "no bolts");
    let again = run(&lv, at, 8, &input, 900);
    assert_eq!(out.rows, again.rows, "not deterministic");
}

/// The Bomb Glove: bombs (○) thrown at trooper 310 from 7 units kill it.
#[test]
fn novalis_trooper_dies_to_the_bomb() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = ([121.0, 176.0, 40.5], 0.1);
    let input = |t: u32| if t >= 150 && t % 45 < 2 { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() };
    let out = run(&lv, at, 10, &input, 1000);
    dump(&out, 15);
    let t = out.moby(310);
    let st = out.states(t);
    eprintln!("trooper 310 states {st:?}; spawned {:?}", out.spawned);
    assert!(out.spawned.get(&121).copied().unwrap_or(0) > 0, "no bomb thrown");
    assert_eq!(*st.last().unwrap(), 0xfd, "not killed ({st:?})");
    // The blast (`0x26f8f8`: push 1 away from the bomb, 1 up) throws it: the death flight (0xe) carries it away
    // before it explodes, as a wrench kill does.
    let flight: Vec<[f32; 3]> = (0..out.rows.len()).filter_map(|i| out.at(i, t)).filter(|e| e.0 == 0xe).map(|e| e.2).collect();
    let travel = flight.first().zip(flight.last()).map_or(0.0, |(a, b)| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt());
    eprintln!("death flight: {} ticks, {travel:.2} units across", flight.len());
    assert!(flight.len() > 1 && travel > 0.5, "no death flight away from the blast ({} ticks, {travel:.2} units)", flight.len());
    let pieces: usize = [1736, 1737, 1738, 1770].iter().map(|c| out.spawned.get(c).copied().unwrap_or(0)).sum();
    assert!(pieces >= 4, "no body pieces ({pieces})");
    let again = run(&lv, at, 10, &input, 1000);
    assert_eq!(out.rows, again.rows, "not deterministic");
}

/// The bridge fly-by on a later visit (its spawn id's persistent death bit set when it ended): no sequence, the
/// troopers drop in at once and the gunship is deleted (0x2f7728).
#[test]
fn novalis_gunship_bridge_fly_by_plays_once() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let sid = lv.instances[695].spawn_id as i16;
    let at = ([72.26054, 189.26149, 47.0], 1.09);
    let out = run_with(&lv, at, 8, &|_| PadInput::neutral(), 120, &[sid]);
    let g = out.moby(695);
    eprintln!("gunship 695 (spawn id {sid}) states {:?}; script requests {:?}", out.states(g), out.scripts);
    assert!(out.scripts.is_empty(), "the fly-by played again: {:?}", out.scripts);
    assert!(out.rows.iter().all(|r| r.hero_state != 0x72), "Ratchet held for a fly-by already seen");
    assert_eq!(*out.states(g).last().unwrap(), 0xfd, "the gunship was not deleted");
}

/// The bridge fly-by: Ratchet enters cuboid 22; gunship 695 starts its scripted sequence (Ratchet's state 0x72 and the
/// camera on cuboid 23 requested, `0x15f404` set), runs its first 120 ticks at once, shells the bridge (cuboids 19–21),
/// wakes troopers 320 / 321 / 333 with 60 ticks left, and ends (camera back, deleted) after 510 ticks.
#[test]
fn novalis_gunship_bridge_fly_by() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = ([72.26054, 189.26149, 47.0], 1.09);
    let out = run(&lv, at, 8, &|_| PadInput::neutral(), 900);
    dump(&out, 30);
    let g = out.moby(695);
    eprintln!("gunship 695 states {:?}; spawned {:?}", out.states(g), out.spawned);
    for i in [320, 321, 333] { eprintln!("trooper {i} states {:?}", out.states(out.moby(i))); }
    // The requests (`creature::Globals::scripts`) go through the tick's cinematic hand-off (`rc_game::cinematic`):
    // Ratchet is held in state 0x72 for the sequence and released at its end.
    let held: Vec<usize> = out.rows.iter().enumerate().filter(|(_, r)| r.hero_state == 0x72).map(|(t, _)| t).collect();
    eprintln!("Ratchet in 0x72 for ticks {:?}..{:?}", held.first(), held.last());
    assert!(!held.is_empty(), "Ratchet never held in state 0x72");
    // The hand-off itself: one Start (state 0x72, the camera on cuboid 23) then one End, both by the gunship.
    use rc_game::moby_update::creature::ScriptRequest;
    let vol = rc_formats::volumes::parse_volumes(&lv.gp).unwrap();
    let c23 = &vol.cuboids[23];
    eprintln!("script requests {:?}", out.scripts);
    match out.scripts.as_slice() {
        [ScriptRequest::Start { moby, hero_state, cuboid, centre, euler, .. }, ScriptRequest::End { moby: m2 }] => {
            assert_eq!((*moby, *m2, *hero_state, *cuboid), (g, g, 0x72, 23));
            assert_eq!((*centre, *euler), (c23.centre(), c23.euler));
        }
        other => panic!("no camera script start / end pair: {other:?}"),
    }
    assert!(out.rows.last().unwrap().hero_state != 0x72, "Ratchet never released");
    assert!(out.cutscene_seen, "0x15f404 never set");
    assert_eq!(*out.states(g).last().unwrap(), 0xfd, "the gunship is not deleted after its fly-by");
    assert!(out.spawned.get(&686).copied().unwrap_or(0) > 0, "no shell fired");
    for i in [320, 321, 333] {
        let st = out.states(out.moby(i));
        assert!(st.contains(&3), "trooper {i} not woken ({st:?})");
    }
    let again = run(&lv, at, 8, &|_| PadInput::neutral(), 900);
    assert_eq!(out.rows, again.rows, "not deterministic");
}

/// The bombardment: gunship 694 flies path 27 from the start and fires a shell at a town cuboid (12–15) at its fire
/// nodes 11, 27, 198 and 210 (unless the camera is in cuboid 16).
#[test]
fn novalis_gunship_bombards_the_town() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let out = run(&lv, SPAWN, 8, &|_| PadInput::neutral(), 2400);
    let g = out.moby(694);
    eprintln!("gunship 694 states {:?}; spawned {:?}; particles {:?}", out.states(g), out.spawned, out.part_spawns);
    assert!(out.spawned.get(&686).copied().unwrap_or(0) >= 1, "no shell fired");
    assert!(out.part_spawns.get(&4).copied().unwrap_or(0) > 0, "no smoke trail");
    assert!(out.part_spawns.get(&15).copied().unwrap_or(0) > 0, "no impact streaks");
}
