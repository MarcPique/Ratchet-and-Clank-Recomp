//! Bolt cranks on Novalis, headless (docs/plan/hero_states.md "Bolt crank"): the level as the engine ticks it (the
//! loader's spawn test, the static mobys, the scheduler's load pass and moby loop, the moby collision, the hero with
//! the wrench, the camera). Ratchet stands at crank #296 (spawn id 0x34), holds □, walks round the bolt pushing
//! the stick along his facing, and the crank opens the door pair 665 #683 / #684; let go early, the crank unwinds and
//! the doors close again. Deterministic. Skipped when `extracted/` is absent.
//!
//! `RC_CRANK_PROBE=1` prints Ratchet's settled height around the bolt (the placement search).

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::crank;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, HERO_LISTS};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::classes::bolt_crank;
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
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
    volumes: rc_formats::volumes::Volumes,
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
    let volumes = rc_formats::volumes::parse_volumes(&gp).unwrap();
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
    // The wrench (item 8 = class 71 on list 0), as the engine builds it from the item definitions.
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), volumes, classes, spawnable, death_z, coll_blobs, ratchet, items })
}

/// Crank #296 (spawn id 0x34) and its doors #683 / #684 (gameplay instance indices).
const CRANK: usize = 296;
const DOORS: [usize; 2] = [683, 684];

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    yaw: f32,
    seq_b: u8,
    crank_state: u8,
    crank_z: f32,
    crank_yaw: f32,
    progress: f32,
    doors: [[f32; 2]; 2],
    rng: u32,
}

/// The stick that pushes Ratchet along his facing: his facing turned into the camera's frame (forward = −y).
fn stick_along_facing(g: &Game) -> (f32, f32) {
    let yaw = g.hero.rot[2].to_f32();
    let c = g.camera.out.yaw().to_f32();
    let (fwd, left) = ((yaw - c).cos(), (yaw - c).sin());
    (-left, -fwd)
}

/// Novalis with its mobys, Ratchet at `at` (yaw) with the wrench, `input(t, game)` for `ticks` ticks.
fn run(lv: &Lv, at: ([f32; 3], f32), input: &dyn Fn(u32, &Game) -> PadInput, ticks: u32) -> Vec<Row> {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let moby_of = |inst: usize| statics.moby_to_instance.iter().position(|&i| i == inst).expect("instance created");
    let (crank_id, doors) = (moby_of(CRANK), DOORS.map(moby_of));
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
    svc.volumes = Arc::new(lv.volumes.clone());
    svc.groups = statics.groups(&lv.gp);
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let mut rows = Vec::new();
    for t in 0..ticks {
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            sched.tick(&mut w);
        };
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, _: &mut Rng, _: u64| {};
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        game.hero.idle.counter = game.counter as i32;
        let pad = input(t, &game);
        let r = game.tick(Some(&pad.bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        let c = &game.mobys.mobys[crank_id];
        rows.push(Row {
            state: game.hero.state,
            pos: game.hero.position(),
            yaw: game.hero.rot[2].to_f32(),
            seq_b: anim.state.seq_b,
            crank_state: c.state,
            crank_z: c.position[2],
            crank_yaw: c.rotation[2],
            progress: pvar::ff(&c.pvars, 0),
            doors: doors.map(|d| [game.mobys.mobys[d].position[0], game.mobys.mobys[d].position[1]]),
            rng: game.rng.state,
        });
    }
    rows
}

/// Ratchet 1.2 from crank #296's bolt on the side the probe found walkable, facing it.
fn at_crank(lv: &Lv) -> ([f32; 3], f32) {
    let b = lv.instances[CRANK].position;
    let a = APPROACH;
    ([b[0] + 1.2 * a.cos(), b[1] + 1.2 * a.sin(), b[2] + 0.35], a + std::f32::consts::PI)
}

/// The direction from the bolt Ratchet stands in (radians; found with `RC_CRANK_PROBE`).
const APPROACH: f32 = 0.0;

fn press_square_then(t: u32, g: &Game, from: u32, until: u32) -> PadInput {
    let mut p = PadInput::neutral();
    if t == 40 { p = p.press(button::SQUARE); }
    if (from..until).contains(&t) {
        let (x, y) = stick_along_facing(g);
        p = p.stick(x, y);
    }
    p
}

#[test]
fn probe_placement() {
    if std::env::var("RC_CRANK_PROBE").is_err() { return; }
    let Some(lv) = load() else { return };
    let b = lv.instances[CRANK].position;
    eprintln!("crank #{CRANK} at {b:?}; doors {:?}", DOORS.map(|d| lv.instances[d].position));
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let at = ([b[0] + 1.2 * a.cos(), b[1] + 1.2 * a.sin(), b[2] + 0.35], a + std::f32::consts::PI);
        let rows = run(&lv, at, &|_, _| PadInput::neutral(), 30);
        let r = rows.last().unwrap();
        eprintln!("dir {a:.3}: settled {:?} state {:#x} (feet − bolt {:.3})", r.pos, r.state, r.pos[2] - b[2]);
    }
}

/// Holding □ at the bolt latches Ratchet (0x3b, the latch sequence); pushing along his facing walks him round the
/// bolt on its ring, turns it, and the doors slide to their open positions as the progress reaches 1; done, he is
/// let go (idle) and the crank sinks 0.5 and stays done.
#[test]
fn novalis_crank_opens_the_doors() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let rows = run(&lv, at_crank(&lv), &|t, g| press_square_then(t, g, 50, 600), 700);
    let b = lv.instances[CRANK].position;
    let latched = rows.iter().position(|r| r.state == crank::STATE).expect("Ratchet never latched onto the bolt");
    eprintln!("latched at tick {latched}: {:?}", rows[latched]);
    assert!((40..=41).contains(&latched), "latched at {latched}");
    assert_eq!(rows[latched].seq_b, crank::SEQ_LATCH);
    let done = rows.iter().position(|r| r.progress >= 1.0).expect("the crank was never done");
    eprintln!("done at tick {done}: {:?}", rows[done]);
    for t in (latched..done).step_by(40) { eprintln!("tick {t}: {:?}", rows[t]); }
    // On the ring while turning (after the spring has pulled him on).
    for r in &rows[latched + 30..done] {
        let d = ((r.pos[0] - b[0]).powi(2) + (r.pos[1] - b[1]).powi(2)).sqrt();
        assert!((d - crank::RING).abs() < 0.1, "off the ring: {d} {r:?}");
        assert_eq!(r.state, crank::STATE);
    }
    // The turn animations played.
    let seqs: std::collections::BTreeSet<u8> = rows[latched..done].iter().map(|r| r.seq_b).collect();
    assert!(seqs.contains(&crank::SEQ_TURN) || seqs.contains(&crank::SEQ_TURN_FAST), "{seqs:x?}");
    // The doors at their targets (the sliders may run before the crank in the loop: one tick later).
    for (k, &d) in DOORS.iter().enumerate() {
        let pv = lv.pvars[lv.instances[d].pvar_index as usize].as_ref().unwrap();
        let target = [pvar::ff(pv, 0xc), pvar::ff(pv, 0x10)];
        assert_eq!(rows[done + 1].doors[k], target, "door #{d}");
        assert_eq!(rows[0].doors[k], [lv.instances[d].position[0], lv.instances[d].position[1]]);
    }
    // Let go on the done tick's next update; idle; the crank sinks to 0.5 below its top and stays done.
    let free = rows[done..].iter().position(|r| r.state != crank::STATE).map(|k| k + done).expect("never let go");
    assert!(free <= done + 2, "let go at {free}");
    let last = rows.last().unwrap();
    assert_eq!(last.crank_state, 5);
    assert!((last.crank_z - (b[2] - 0.5)).abs() < 1e-4, "crank z {}", last.crank_z);
    assert_eq!(last.progress, 1.0);
}

/// Let go early (□ again after 60 ticks), the crank unwinds and the doors close again; Ratchet can latch again after
/// 30 ticks.
#[test]
fn novalis_crank_unwinds_when_let_go() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let input = |t: u32, g: &Game| {
        let mut p = press_square_then(t, g, 50, 140);
        if t == 150 { p = p.press(button::SQUARE); }
        p
    };
    let rows = run(&lv, at_crank(&lv), &input, 260);
    let held = rows[149].progress;
    eprintln!("progress when let go: {held}; {:?}", rows[149]);
    assert!(held > 0.05 && held < 1.0);
    let free = rows.iter().skip(150).position(|r| r.state != crank::STATE).expect("never let go") + 150;
    assert!(free <= 151, "let go at {free}");
    // Back to 0 within ~60 ticks, the doors back.
    let closed = rows.iter().skip(free).position(|r| r.progress == 0.0).expect("never unwound") + free;
    assert!(closed - free <= 62, "unwound in {}", closed - free);
    assert_eq!(rows[closed + 1].doors, rows[0].doors);
}

/// Two runs are identical tick for tick.
#[test]
fn novalis_crank_deterministic() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let a = run(&lv, at_crank(&lv), &|t, g| press_square_then(t, g, 50, 300), 320);
    let b = run(&lv, at_crank(&lv), &|t, g| press_square_then(t, g, 50, 300), 320);
    assert_eq!(a, b);
}

/// The crank registered on every level that has one (1, 4, 8), with the same update.
#[test]
fn cranks_on_levels_1_4_8() {
    let mut found = Vec::new();
    for lvl in 0..19u32 {
        let Some(gp) = rc_formats::test_data::gameplay(lvl) else { continue };
        let n = gameplay::parse_moby_instances(&gp).unwrap().iter().filter(|m| m.o_class as i16 == bolt_crank::CLASS).count();
        if n > 0 { found.push((lvl, n)); }
    }
    if found.is_empty() { eprintln!("skipped: no extracted/"); return; }
    assert_eq!(found, vec![(1, 2), (4, 9), (8, 5)]);
    assert_eq!(scheduler::port_update_fn(bolt_crank::CLASS), Some(bolt_crank::UPDATE_FN));
    assert_eq!(scheduler::port_update_fn(641), Some(bolt_crank::ROTATOR_UPDATE_FN));
    assert_eq!(scheduler::port_update_fn(665), Some(bolt_crank::SLIDER_UPDATE_FN));
}

/// `RC_CRANK_SCRIPT=<file>`: writes the engine's `RC_PLAY_SCRIPT` for the same run (□ at tick 40, then each tick's
/// stick along Ratchet's facing as the headless run computed it) and the `RC_HERO_AT` placement, for the engine
/// screenshots (the engine ticks the same game code, so the same inputs replay the same crank).
#[test]
fn write_engine_script() {
    let Ok(path) = std::env::var("RC_CRANK_SCRIPT") else { return };
    let Some(lv) = load() else { return };
    let log = std::cell::RefCell::new(Vec::new());
    let input = |t: u32, g: &Game| {
        let p = press_square_then(t, g, 50, 600);
        if (50..600).contains(&t) {
            let (x, y) = stick_along_facing(g);
            log.borrow_mut().push(format!("{t}:stick {x:.4} {y:.4}"));
        }
        p
    };
    let rows = run(&lv, at_crank(&lv), &input, 400);
    let mut items = vec!["40:press SQUARE".to_string()];
    items.extend(log.borrow().iter().cloned());
    let (p, yaw) = at_crank(&lv);
    let done = rows.iter().position(|r| r.progress >= 1.0);
    std::fs::write(&path, format!("RC_HERO_AT={},{},{},{}\nRC_PLAY_SCRIPT={}\n# done at tick {done:?}\n", p[0], p[1], p[2], yaw, items.join(","))).unwrap();
}
