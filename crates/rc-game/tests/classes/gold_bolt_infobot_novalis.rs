//! Gold bolts (class 1134) and the Infobot on Novalis, headless (docs/plan/hero_gameplay.md §6): the level's mobys
//! through the loader and the scheduler, the moby services as the hero's and the camera's world, Ratchet ticked by
//! `Game::tick`, the saved-game writes applied to the game state of a first Novalis arrival (as the engine does).
//!
//! * Gold bolt 959 (index 0, the orbit camera) and 958 (index 2, the camera between cuboids 0x51 / 0x50): Ratchet
//!   placed next to it → the pickup cutaway (FadeToBlack, Ratchet held facing it, the script camera, the letterbox,
//!   his animation 0x82, the hand item hidden) → control back, the banner "Gold Bolt Acquired", the collected byte of
//!   save chunk 3003 set, the count +1; a second visit with that game state finds no bolt.
//! * The Infobot: the Water Pump Worker 774's talk table (scene 0, "Buy Infobot for 500 bolts", scene 1 → movie 2
//!   → scene 2, the hand-offs ended by the test as the engine ends them) → bolts −500, Aridia unlocked (planet bits
//!   and map order), the planet banner.
//! * Determinism: two runs, identical records.
//!
//! Skipped when `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::save_game::{ChunkTables, ItemTables, SaveGameLump};
use rc_formats::{collision, gadget, gameplay, level, strings, volumes};
use rc_game::cinematic::{BannerCall, EngineRequest};
use rc_game::game_state::{GameState, SessionState};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::Hero;
use rc_game::menus::Overlay;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::classes::gold_bolt;
use rc_game::moby_update::interact::{Handoff, TalkTables};
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::particles::Particles;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::collections::HashMap;
use std::sync::Arc;

struct Level {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    joints: HashMap<i16, Vec<Vec<u8>>>,
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
    vol: volumes::Volumes,
    overlay: Overlay,
    messages: Vec<strings::Message>,
    state: GameState,
}

fn load() -> Option<Level> {
    let root = rc_formats::test_data::root();
    let dir = root.join("levels/01");
    let data = rc_formats::test_data::core_data(1)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(1)?;
    let settings = rc_formats::test_data::gameplay_section(1, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let vol = volumes::parse_volumes(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    let mut joints = HashMap::new();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
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
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    // The game state of a first Novalis arrival (game_state.md §4), 5000 bolts.
    let elf = std::fs::read(root.join("boot/SCUS_971.99")).ok()?;
    let lump = SaveGameLump::parse(&std::fs::read(root.join("global/save_game.bin")).ok()?).unwrap();
    let ov0 = std::fs::read(root.join("levels/00/overlay.bin")).ok()?;
    let ov1 = std::fs::read(dir.join("overlay.bin")).ok()?;
    let tables = ChunkTables::from_boot_elf(&elf).unwrap();
    let mut state = GameState::new_game(tables, &lump.template).unwrap();
    let mut session = SessionState::default();
    state.apply_level_start(0, &ItemTables::load(&elf, &ov0).unwrap(), &mut session);
    state.on_veldin_clank_init(&mut session);
    state.apply_transition(1);
    state.apply_level_start(1, &ItemTables::load(&elf, &ov1).unwrap(), &mut session);
    state.global.bolts = 5000;
    let messages = strings::parse_strings(&gp, 0).unwrap();
    let overlay = Overlay::parse(&ov1).unwrap();
    Some(Level { mesh, instances, pvars, splines, gp: gp.to_vec(), classes, joints, spawnable, death_z, coll_blobs, ratchet, vol, overlay, messages, state })
}

/// One tick's record.
#[derive(Clone, Debug, PartialEq)]
struct Rec {
    counter: u64,
    hero_state: i32,
    hero: [f32; 3],
    hero_seq: u8,
    hand_hidden: bool,
    script: bool,
    cam: [f32; 3],
    letterbox: bool,
    /// The watched moby: state (0xff when deleted), anim sequence A, position.
    watch: (u8, u8, [f32; 3]),
    banner: BannerCall,
    requests: Vec<EngineRequest>,
    handoffs: Vec<Handoff>,
    bolts: i32,
    gold_bolts: i32,
    game_mode: i32,
}

struct Run {
    recs: Vec<Rec>,
    state: GameState,
    part59: u64,
    part60: u64,
}

/// Runs `ticks` gameplay ticks with Ratchet placed at `at` (feet, yaw), pad `input(t)`, watching moby `watch`; the
/// talkers' scene / movie hand-offs end `end_after` ticks after they were asked for (game mode 2 meanwhile for a
/// scene), as crate::scene_render / movie_render end them.
fn run(lv: &Level, gs0: &GameState, at: [f32; 4], ticks: u32, watch: usize, input: impl Fn(u32) -> PadInput, end_after: u32) -> Run {
    run_moved(lv, gs0, at, None, ticks, watch, input, end_after)
}

/// [`run`] with Ratchet moved to `moved.1` (feet, velocity cleared) before tick `moved.0`.
#[allow(clippy::too_many_arguments)]
fn run_moved(lv: &Level, gs0: &GameState, at: [f32; 4], moved: Option<(u32, [f32; 3])>, ticks: u32, watch: usize, input: impl Fn(u32) -> PadInput, end_after: u32) -> Run {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let mut statics = load_static_mobys(&lv.instances, &mut ct, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    statics[hero_idx].position = [at[0], at[1], at[2], 0.0];
    statics[hero_idx].rotation[2] = at[3];
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    let mut gs = gs0.clone();
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.set_volumes(lv.vol.clone());
    svc.groups = Groups::parse(&lv.gp, &|i| (i < lv.instances.len()).then_some(i));
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.joint_lists = lv.joints.clone();
    svc.build_grid(&mut game.mobys);
    svc.counters.bolts = gs.global.bolts;
    let tables = TalkTables::load(&lv.overlay).expect("talk tables");
    svc.interact.tables = Arc::new(tables);
    svc.interact.messages = Arc::new(lv.messages.clone());
    svc.interact.register_talk_mobys(lv.instances.iter().enumerate().map(|(i, x)| (i, x.unknown_74)));
    svc.interact.sync_game(&gs);
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
    let mut recs = Vec::new();
    // The running hand-off: (tick it ends, talker, a scene).
    let mut running: Option<(u32, Option<usize>, bool)> = None;
    for t in 0..ticks {
        if let Some((mt, p)) = moved.filter(|m| m.0 == t) {
            let _ = mt;
            game.hero.pos = [rc_game::ps2v::Pf::f(p[0]), rc_game::ps2v::Pf::f(p[1]), rc_game::ps2v::Pf::f(p[2]), game.hero.pos[3]];
            game.hero.vel = [rc_game::ps2v::Pf::ZERO; 4];
        }
        {
            let mut s = svc_cell.borrow_mut();
            if let Some((end, npc, scene)) = running {
                s.game_mode = if scene { 2 } else { 0 };
                if t == end {
                    s.game_mode = 0;
                    s.interact.talker = npc;
                    s.interact.scene_ended = npc.is_some();
                    running = None;
                }
            }
            s.interact.sync_game(&gs);
        }
        let classes_ref: &ClassTable = &classes;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut p = parts_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.particles = Some(&mut **p);
            w.svc.interact.begin_tick(hero.loop_in.pad.pressed, hero.state);
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
        let counter = game.counter;
        game.tick(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        let mut s = svc_cell.borrow_mut();
        // The engine's after-tick work: the saved-game writes, the bolts, the requests and hand-offs.
        s.interact.apply_writes(&mut gs);
        gs.global.bolts = s.counters.bolts;
        let requests = std::mem::take(&mut s.cinematic.requests);
        let handoffs = std::mem::take(&mut s.interact.handoffs);
        for h in &handoffs {
            match *h {
                Handoff::Scene { npc, .. } => running = Some((t + end_after, npc, true)),
                Handoff::Movie { npc, .. } => running = Some((t + end_after, npc, false)),
                _ => {}
            }
        }
        let m = &game.mobys.mobys[watch];
        recs.push(Rec {
            counter,
            hero_state: game.hero.state,
            hero: game.hero.position(),
            hero_seq: anim.state.seq_b,
            hand_hidden: game.hero.f13ff != 0,
            script: game.camera.script_active(),
            cam: game.camera.out.pos_f32(),
            letterbox: s.creatures.cutscene,
            watch: (if m.is_deleted() { 0xff } else { m.state }, m.anim.seq_a, [m.position[0], m.position[1], m.position[2]]),
            banner: s.cinematic.banner,
            requests,
            handoffs,
            bolts: gs.global.bolts,
            gold_bolts: gs.levels.iter().map(|l| l.gold_bolts.iter().filter(|&&b| b != 0).count() as i32).sum(),
            game_mode: s.game_mode,
        });
    }
    let fx = svc_cell.borrow().fx.part_spawns.clone();
    Run { recs, state: gs, part59: fx.get(&59).copied().unwrap_or(0), part60: fx.get(&60).copied().unwrap_or(0) }
}

fn still(_: u32) -> PadInput { PadInput::neutral() }

/// The instance index of Novalis gold bolt `index`.
fn gold_bolt(lv: &Level, index: i32) -> usize {
    lv.instances
        .iter()
        .enumerate()
        .position(|(i, m)| {
            m.o_class == 1134 && usize::try_from(m.pvar_index).ok().and_then(|k| lv.pvars.get(k).cloned().flatten()).is_some_and(|p| i32::from_le_bytes(p[0..4].try_into().unwrap()) == index) && i < lv.instances.len()
        })
        .expect("gold bolt")
}

/// Ratchet 1.5 units from the gold bolt, on its spawn height.
fn next_to(lv: &Level, k: usize) -> [f32; 4] {
    let p = lv.instances[k].position;
    [p[0] + 1.5, p[1], p[2], 0.0]
}

/// Ratchet 6 units away (out of reach) for the first `WALK_IN` ticks, then next to it.
const WALK_IN: u32 = 30;
fn away_from(lv: &Level, k: usize) -> [f32; 4] {
    let p = lv.instances[k].position;
    [p[0] + 6.0, p[1], p[2], 0.0]
}

/// The pickup cutaway and its end, checked on one gold bolt; returns the run.
fn check_pickup(lv: &Level, index: i32) -> Run {
    let k = gold_bolt(lv, index);
    let n = next_to(lv, k);
    let r = run_moved(lv, &lv.state, away_from(lv, k), Some((WALK_IN, [n[0], n[1], n[2]])), 700, k, still, 5);
    let fade = r.recs.iter().position(|x| x.requests.contains(&EngineRequest::FadeToBlack { frames: 10 })).expect("no FadeToBlack");
    let st: Vec<u8> = r.recs.iter().map(|x| x.watch.0).collect();
    eprintln!("gold bolt {index} (moby {k}): pickup at tick {fade}; states {:?}", dedup(&st));
    assert_eq!(fade as u32, WALK_IN, "picked up on the tick Ratchet is in reach");
    assert!(r.recs[..fade].iter().all(|x| x.watch.0 == 1 && !x.letterbox), "idle before the pickup");
    // Idle: it bobs (z within spawn + 1 ± 0.3) and spins.
    let z0 = lv.instances[k].position[2];
    assert!(r.recs[1..fade].iter().all(|x| (x.watch.2[2] - (z0 + 1.0)).abs() <= 0.3 + 1e-4), "bob");
    let a = &r.recs[fade];
    assert_eq!(a.watch.0, 2, "the cutaway state");
    assert!(a.letterbox && a.script, "letterbox and script camera from the pickup tick");
    // Ratchet held (0x72) with the pickup animation 0x82 and the hand item hidden from the next tick.
    let b = &r.recs[fade + 1];
    assert_eq!(b.hero_state, 0x72);
    assert_eq!(b.hero_seq, gold_bolt::HERO_ANIM);
    assert!(b.hand_hidden);
    // The bolt sits on Ratchet.
    let held = r.recs[fade].hero;
    assert!((0..3).all(|c| (b.watch.2[c] - held[c]).abs() < 1e-3), "bolt {:?} hero {:?}", b.watch.2, held);
    // The end: control back, banner, byte, count.
    let end = r.recs.iter().position(|x| x.watch.0 == 0xff).expect("the bolt never finished");
    let e = &r.recs[end];
    eprintln!("  cutaway {} ticks; banner {:?}; camera at the pickup {:?}, mid {:?}", end - fade, e.banner, a.cam, r.recs[(fade + end) / 2].cam);
    assert!(end - fade > 60, "the cutaway lasts the bolt's sequence plus 60 ticks");
    assert!(r.recs[fade..end].iter().all(|x| x.letterbox && x.script), "held throughout");
    assert!(!e.letterbox);
    assert_eq!(e.banner.msg, gold_bolt::BANNER_MSG);
    assert_eq!(e.banner.ticks, 180);
    assert!(e.requests.contains(&EngineRequest::Save));
    let after = &r.recs[end + 2];
    assert!(!after.script, "the follow camera is back");
    assert_eq!(after.hero_state, 0, "control back (SetState(0))");
    assert!(!after.hand_hidden);
    assert_eq!(e.gold_bolts, r.recs[0].gold_bolts + 1, "count +1");
    assert_eq!(r.state.levels[1].gold_bolts[index as usize], 1, "save chunk 3003 byte");
    assert!(r.part59 > 0 && r.part60 > 0, "glows {} glints {}", r.part59, r.part60);
    r
}

fn dedup<T: PartialEq + Copy>(v: &[T]) -> Vec<T> {
    let mut o: Vec<T> = Vec::new();
    for &x in v { if o.last() != Some(&x) { o.push(x); } }
    o
}

#[test]
fn gold_bolt_pickup_orbit_camera() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let r = check_pickup(&lv, 0);
    // Stays collected: a second visit with this game state has no gold bolt 0 (deleted by its init), the others idle.
    let k = gold_bolt(&lv, 0);
    let again = run(&lv, &r.state, next_to(&lv, k), 30, k, still, 5);
    assert!(again.recs.iter().all(|x| x.watch.0 == 0xff && !x.letterbox), "collected: gone on the revisit");
    assert_eq!(again.recs[0].gold_bolts, 1);
    let other = run(&lv, &r.state, [0.0; 4], 3, gold_bolt(&lv, 1), still, 5);
    assert_eq!(other.recs[0].watch.0, 1, "the other gold bolts are still there");
}

#[test]
fn gold_bolt_pickup_cuboid_camera() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let r = check_pickup(&lv, 2);
    // The camera between cuboids 0x51 and 0x50: at the first cutaway tick near 0x51's centre.
    let fade = r.recs.iter().position(|x| x.watch.0 == 2).unwrap();
    let c = lv.vol.cuboids[0x51].centre();
    let cam = r.recs[fade + 1].cam;
    eprintln!("cuboid 0x51 {c:?}, camera {cam:?}");
    assert!((0..3).all(|k| (cam[k] - c[k]).abs() < 0.5), "camera {cam:?} vs cuboid {c:?}");
}

#[test]
fn gold_bolt_is_deterministic() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let k = gold_bolt(&lv, 0);
    let (a, b) = (run(&lv, &lv.state, next_to(&lv, k), 400, k, still, 5), run(&lv, &lv.state, next_to(&lv, k), 400, k, still, 5));
    assert_eq!(a.recs, b.recs);
    assert_eq!((a.part59, a.part60), (b.part59, b.part60));
}

/// In front of the Water Pump Worker 774, facing him (docs/plan/interaction.md §7).
const WORKER: [f32; 4] = [253.13, 186.67, 95.57, 2.5066];

fn buy_infobot(lv: &Level) -> Run {
    let npc = lv.instances.iter().position(|m| m.o_class == 774).expect("774");
    run(lv, &lv.state, WORKER, 200, npc, |t| if t == 40 { PadInput::neutral().press(button::TRIANGLE) } else { PadInput::neutral() }, 8)
}

#[test]
fn infobot_bought_from_the_water_pump_worker() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    assert_eq!(lv.state.global.planet_unlocked[2], 0);
    let r = buy_infobot(&lv);
    let hs: Vec<(u64, Handoff)> = r.recs.iter().flat_map(|x| x.handoffs.iter().map(move |h| (x.counter, h.clone()))).collect();
    eprintln!("hand-offs {hs:?}");
    let kinds: Vec<(bool, i32)> = hs.iter().filter_map(|(_, h)| match *h {
        Handoff::Scene { scene, .. } => Some((true, scene)),
        Handoff::Movie { movie, .. } => Some((false, movie)),
        _ => None,
    }).collect();
    // Scene 0 (auto), then after △: scene 1 → movie 2 → scene 2, in that order.
    assert_eq!(kinds, [(true, 0), (true, 1), (false, 2), (true, 2)]);
    let buy = r.recs.iter().position(|x| x.bolts == 4500).expect("500 bolts spent");
    assert_eq!(r.recs[buy - 1].bolts, 5000);
    // Aridia unlocked (after scene 2), appended to the map order, the planet banner.
    let g = &r.state.global;
    assert_eq!(g.planet_unlocked[2], 1);
    assert!(g.map_order.contains(&2), "{:?}", g.map_order);
    let last = r.recs.last().unwrap();
    assert_eq!(last.banner.msg, rc_game::cinematic::PLANET_BANNERS[2]);
    assert_eq!(last.banner.ticks, rc_game::cinematic::PLANET_BANNER_TICKS);
    assert_eq!(last.watch.0, 0xff, "the worker leaves (state 3: deleted)");
    let text = strings::display(strings::lookup(&lv.messages, last.banner.msg));
    eprintln!("banner: {text:?}; gold bolt banner: {:?}", strings::display(strings::lookup(&lv.messages, gold_bolt::BANNER_MSG)));
    // Deterministic.
    assert_eq!(buy_infobot(&lv).recs, r.recs);
}

/// Novalis' placed infobot (instance 874) is the scene actors' stand-in: hidden, inert (state 1).
#[test]
fn novalis_infobot_instance_is_inert() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let k = lv.instances.iter().position(|m| m.o_class == 750).expect("750");
    let r = run(&lv, &lv.state, [0.0; 4], 10, k, still, 5);
    assert!(r.recs.iter().all(|x| x.watch.0 == 1), "{:?}", r.recs.iter().map(|x| x.watch.0).collect::<Vec<_>>());
}

/// Survey (dev): every level's gold bolt / infobot instances and their pvars.
#[test]
#[ignore]
fn survey() {
    for l in 0..19u32 {
        let Some(gp) = rc_formats::test_data::gameplay(l) else { continue };
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let pv = gameplay::parse_pvars(&gp).unwrap();
        for (i, m) in inst.iter().enumerate() {
            if m.o_class != 1134 && m.o_class != 750 { continue; }
            let p = usize::try_from(m.pvar_index).ok().and_then(|k| pv.get(k).cloned().flatten()).unwrap_or_default();
            let words: Vec<String> = p.chunks(4).map(|c| format!("{:08x}", u32::from_le_bytes([c[0], c[1], *c.get(2).unwrap_or(&0), *c.get(3).unwrap_or(&0)]))).collect();
            println!("L{l:02} #{i} class {} spawn_id {} pos {:?} rot {:?} pvars[{}] {}", m.o_class, m.spawn_id, m.position, m.rotation, p.len(), words.join(" "));
        }
    }
}

/// The per-level registry (`LevelPorts`, the level's own class table matched against the level-01 functions) runs
/// the gold bolt port for 1134 on exactly the 17 overlays with the identical function (Gemlik's variant excluded),
/// and the infobot port for 750 on the 14 of its cluster.
#[test]
fn registered_on_the_levels_with_identical_code() {
    use rc_formats::level_overlay::LevelOverlay;
    use rc_game::moby_update::classes::{infobot, ClassUpdate};
    fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let (mut gb, mut ib) = (Vec::new(), Vec::new());
    for level in 0..19u32 {
        let ov = overlay(level).unwrap();
        let p = crate::common::ports(level, &[]).unwrap();
        let in_table = |oc: i16| ov.vtbl().iter().any(|e| e.o_class as i16 == oc);
        if in_table(1134) && p.get(1134) == Some(ClassUpdate::GoldBolt) { gb.push(level); }
        if in_table(750) && p.get(750) == Some(ClassUpdate::Infobot) { ib.push(level); }
        eprintln!("level {level:02}: 1134 {:?} ({:?}), 750 {:?} ({:?})", p.get(1134), p.level_update(1134).map(|a| format!("{a:#x}")), p.get(750), p.level_update(750).map(|a| format!("{a:#x}")));
    }
    assert_eq!(gb, gold_bolt::LEVELS);
    assert_eq!(ib, infobot::LEVELS);
}
