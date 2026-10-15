//! The Bomb Glove's aim and reticle on Novalis, headless (docs/plan/hero_gameplay.md §8, `rc_game::targeting`): the
//! level as the engine ticks it (the loader's spawn test, the scheduler's load pass and moby loop with its run list,
//! the moby collision, the hero with the glove, the tick's hit path, the camera). Ratchet stands in front of a
//! targetable enemy with the glove: the glove's search picks it (0x13fda0), the held bomb's landing preview snaps
//! the reticle onto it (the registration of `0x2c23c0`), and the thrown bomb lands on it. Skipped when `extracted/`
//! is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
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
    // The wrench (item 8 = class 71 on list 0) and the Bomb Glove (item 10 = class 192 on list 6), as the engine
    // builds them from the item definitions.
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, items })
}

/// The hero's hit sink over the moby loop's services (as the engine's `CellHits`): the wrench's lines and spheres,
/// and the glove's bomb (`CreateMoby` / `DeleteMoby`).
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

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    yaw: f32,
    /// 0x13fda0 (the glove's target) and its position.
    aim: Option<(usize, [f32; 3])>,
    /// The targetable mobys within 15 of Ratchet: (moby, position).
    near: Vec<(usize, [f32; 3])>,
    /// The reticles registered this tick: (point, normal).
    reticles: Vec<([f32; 3], [f32; 3])>,
    /// The live bombs (class 121): (state, position, locked moby + 1 (+0x60)).
    bombs: Vec<(u8, [f32; 3], i32)>,
    ammo: i32,
    /// The camera after the tick.
    cam: [f32; 3],
    rng: u32,
}

/// The targetable mobys at the end of a run: (moby, class, position, has a target record).
type Targets = Vec<(usize, i16, [f32; 3], bool)>;

/// Novalis with its mobys, Ratchet at `at` (yaw) with the glove (10 bombs), `input(t)` for `ticks` ticks.
fn run(lv: &Lv, at: ([f32; 3], f32), input: &dyn Fn(u32) -> PadInput, ticks: u32) -> (Vec<Row>, Targets) {
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
    game.item_globals.request = 10;
    game.item_globals.saved = 10;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
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
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let parts_cell = std::cell::RefCell::new(&mut particles);
    let mut rows = Vec::new();
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
        let counter = game.counter;
        let r = game.tick_with_hits(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks, &mut hits);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let h = &game.hero;
        let aim = h.weapons.aim.map(|e| { let m = &game.mobys.mobys[e]; (e, [m.position[0], m.position[1], m.position[2]]) });
        let hp = h.position();
        let near = game.mobys.mobys.iter().enumerate().filter(|(_, m)| m.mode & mode::TARGETABLE != 0 && m.state < 0x80 && d2([m.position[0], m.position[1], m.position[2]], hp) < 15.0).map(|(i, m)| (i, [m.position[0], m.position[1], m.position[2]])).collect();
        let reticles = svc_cell.borrow().reticles.of_tick(counter).iter().map(|r| (r.point, r.normal)).collect();
        let bombs = game.mobys.mobys.iter().filter(|m| m.o_class == 121 && m.state < 0xfd).map(|m| (m.state, [m.position[0], m.position[1], m.position[2]], i32::from_le_bytes(m.pvars[0x60..0x64].try_into().unwrap()))).collect();
        rows.push(Row { state: h.state, pos: h.position(), yaw: h.rot[2].to_f32(), aim, near, reticles, bombs, ammo: h.weapons.ammo[10], cam: game.camera.out.pos_f32(), rng: game.rng.state });
    }
    let targets = game.mobys.mobys.iter().enumerate().filter(|(_, m)| m.mode & mode::TARGETABLE != 0 && m.state < 0x80).map(|(i, m)| (i, m.o_class, [m.position[0], m.position[1], m.position[2]], rc_game::targeting::aim_height(m).is_some())).collect();
    (rows, targets)
}

fn dump(rows: &[Row]) {
    if std::env::var("RC_TARGETING_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate() {
        eprintln!("{t:4} {:#x} {:?} yaw {:.2} aim {:?} ret {:?} bombs {:?} ammo {}", r.state, r.pos, r.yaw, r.aim, r.reticles, r.bombs, r.ammo);
    }
}

fn d2(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }

/// Survey: the targetable mobys after a few ticks at the spawn (`cargo xtask test-job --test weapons --filter hero_targeting_novalis::
/// --ignored --nocapture`).
#[test]
#[ignore]
fn targeting_survey() {
    let Some(lv) = load() else { return };
    let spawn = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    let (_, targets) = run(&lv, ([spawn[0], spawn[1], spawn[2]], 0.0), &|_| PadInput::neutral(), 30);
    println!("spawn {spawn:?}");
    for (i, c, p, rec) in targets { println!("moby {i} class {c} at {p:?} record {rec} d {:.1}", d2(p, [spawn[0], spawn[1], spawn[2]])); }
}

/// A critter 577 in the pit north-west of the spawn (moby 592 of the table, on flat ground), and Ratchet 7 units west of it,
/// facing it.
const CRITTER: usize = 592;

fn facing_moby(lv: &Lv, id: usize, d: f32, from: f32) -> ([f32; 3], f32) {
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let c = statics.mobys[id].position;
    let p = [c[0] + d * from.cos(), c[1] + d * from.sin(), c[2] + 0.5];
    (p, (c[1] - p[1]).atan2(c[0] - p[0]))
}

fn throw_at(t: u32) -> PadInput { if (120..=121).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() } }

/// The glove's search picks a critter (0x13fda0; two graze here and it takes the one its greedy rule prefers), the
/// held bomb's preview snaps the reticle onto it (the point is the critter's position pulled 5 % toward the camera,
/// and the bomb locks it in +0x60), and the throw lands on it: the bomb explodes where the critter was when it left
/// the glove. Twice identical.
#[test]
fn novalis_reticle_snaps_to_a_critter_and_the_bomb_lands_on_it() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let at = facing_moby(&lv, CRITTER, 7.0, std::f32::consts::PI);
    let (rows, _) = run(&lv, at, &throw_at, 240);
    dump(&rows);
    let critters = [590usize, 592];
    // Held, before the throw: a critter targeted and the reticle by it every tick; snapped onto it (the critter's
    // position pulled 5 % toward the camera, the held bomb locking it in +0x60) whenever the arc's step meets its
    // collision primitive — most ticks (a hop can lift it off the arc: the reticle then lies on the ground under it).
    let mut snapped = 0;
    for r in &rows[80..120] {
        let (id, e) = r.aim.expect("no target");
        assert!(critters.contains(&id), "the glove targets moby {id}");
        assert_eq!(r.reticles.len(), 1, "one reticle a tick");
        let (point, _) = r.reticles[0];
        // By a critter: the target, or the other one when the arc meets it first.
        let by = r.near.iter().filter(|n| critters.contains(&n.0)).find(|n| d2(point, n.1) < 1.5);
        let Some(&(on, p)) = by else { panic!("reticle {point:?} away from the critters {:?} (target {e:?})", r.near) };
        let want = rc_game::targeting::pull_toward(r.cam, p);
        // The camera the preview pulled toward is one tick older and the critter moves a little: a small tolerance.
        if d2(point, want) < 0.3 && (point[2] - want[2]).abs() < 0.3 && r.bombs.iter().any(|b| b.0 == 0 && b.2 == on as i32 + 1) { snapped += 1; }
    }
    assert!(snapped >= 20, "snapped on {snapped} of 40 ticks");
    // The throw: 0x23, one bomb used; the bomb leaves the glove aimed at the critter and explodes where it was.
    assert!(rows[120..].iter().any(|r| r.state == 0x23), "no throw");
    let out = rows[120..].iter().position(|r| r.bombs.iter().any(|b| b.0 == 1)).expect("no bomb flew") + 120;
    let aimed = rows[out - 1].aim.expect("no target at the release").1;
    let boom = rows[out..].iter().position(|r| r.bombs.iter().any(|b| b.0 == 2)).expect("no explosion") + out;
    let b = rows[boom].bombs.iter().find(|b| b.0 == 2).unwrap().1;
    assert!(d2(b, aimed) < 1.5 && (b[2] - aimed[2]).abs() < 1.5, "the bomb exploded at {b:?}, the critter was at {aimed:?}");
    assert_eq!(rows.last().unwrap().ammo, 9);
    let (again, _) = run(&lv, at, &throw_at, 240);
    assert_eq!(rows, again);
}

/// Survey: the ground around the critters (`-- --ignored --nocapture`).
#[test]
#[ignore]
fn critter_ground_survey() {
    let Some(lv) = load() else { return };
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    for id in [579usize, 590, 591, 592, 586, 587] {
        let c = statics.mobys[id].position;
        let g = |x: f32, y: f32| rc_game::hero::physics::line_world(&lv.mesh, rc_game::hero::physics::v4(x, y, c[2] + 3.0), rc_game::hero::physics::v4(x, y, c[2] - 8.0), 2).map(|o| o.point[2]);
        println!("moby {id} at {c:?} ground {:?}", g(c[0], c[1]));
        for dist in [5.0f32, 7.0] {
            let mut line = String::new();
            for q in 0..8 {
                let a = q as f32 * std::f32::consts::FRAC_PI_4;
                line += &format!(" {q}:{}", g(c[0] + dist * a.cos(), c[1] + dist * a.sin()).map_or("-".to_string(), |z| format!("{z:.1}")));
            }
            println!("    d{dist}:{line}");
        }
    }
}


