//! Novalis in-level cinematics, headless (docs/plan/cutscenes.md §6): the level's mobys through the loader and the
//! scheduler, the moby services as the hero's and the camera's world, Ratchet ticked by `Game::tick`. Covers the
//! arrival trigger, the mission NPC's drop-in cutaway (camera trigger 860), the hinged-bridge cutaway (camera
//! trigger 863 and the bridge 746) and the gunship fly-by over the bridge (gunship 695): trigger tick, control lock,
//! letterbox flag, script camera, release. Skipped when `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level, volumes};
use rc_game::cinematic::EngineRequest;
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{LevelMissions, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::PadInput;
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Level {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    death_z: f32,
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    ratchet: MobyAnimClass,
    vol: volumes::Volumes,
}

fn load() -> Option<Level> {
    let dir = rc_formats::test_data::root().join("levels/01");
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
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            // A class without a blob in the core (the camera trigger 737: no geometry) keeps its update.
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
    Some(Level { mesh, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, vol })
}

/// One tick's record.
#[derive(Clone, Debug, PartialEq)]
struct Rec {
    counter: u64,
    hero_state: i32,
    hero: [f32; 3],
    script: bool,
    cam: [f32; 3],
    cam_fwd: [f32; 3],
    letterbox: bool,
    npc_state: u8,
    npc_z: f32,
    bridge_pitch: f32,
    /// Camera triggers 860 / 863: (state, +0xbc, class).
    triggers: [(u8, u8, i16); 2],
    requests: Vec<EngineRequest>,
}

/// What a run does besides the pad.
#[derive(Clone, Copy)]
enum Act {
    /// Put Ratchet at this point (feet), velocity cleared.
    Place([f32; 3]),
    /// Delete moby `k` (the kills the mission waits for).
    Kill(usize),
}

/// Runs the level for `ticks` gameplay ticks (t = the tick index) with the pad `input(t)` and the actions.
fn run(lv: &Level, ticks: u32, input: impl Fn(u32) -> PadInput, acts: &[(u32, Act)]) -> Vec<Rec> {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_static_mobys(&lv.instances, &mut ct, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let npc = statics.iter().position(|m| m.o_class == 790).expect("mission NPC 790");
    let bridge = statics.iter().position(|m| m.o_class == 746).expect("hinged bridge 746");
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.set_volumes(lv.vol.clone());
    svc.groups = Groups::parse(&lv.gp, &|i| (i < lv.instances.len()).then_some(i));
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    // A new game: every Novalis mission open.
    let missions = LevelMissions::fresh_load(1, [0; 16]);
    let mut sched = Scheduler::new();
    {
        let hero: Hero = game.hero.clone();
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &*classes, &mut svc, 0);
        w.camera = game.camera.out.pos;
        w.coll = Some(&lv.mesh);
        w.missions = &missions;
        sched.load_pass(&mut w);
    }
    game.finish_load();
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let svc_cell = std::cell::RefCell::new(&mut svc);
    let mut out = Vec::new();
    for t in 0..ticks {
        for (at, a) in acts {
            if *at != t { continue; }
            match *a {
                Act::Place(p) => {
                    game.hero.pos = [Pf::f(p[0]), Pf::f(p[1]), Pf::f(p[2]), game.hero.pos[3]];
                    game.hero.vel = [Pf::ZERO; 4];
                }
                Act::Kill(k) => game.mobys.delete(k, game.counter),
            }
        }
        let classes_ref: &ClassTable = &classes;
        let missions_ref = &missions;
        let mut mobys = |table: &mut MobyTable, hero: &Hero, rng: &mut Rng, cam: &rc_game::follow_camera::CameraView, coll: &collision::Collision, counter: u64| {
            let mut s = svc_cell.borrow_mut();
            let mut w = World::new(table, hero, rng, classes_ref, &mut s, counter);
            w.camera = cam.pos;
            w.coll = Some(coll);
            w.missions = missions_ref;
            sched.tick(&mut w);
        };
        let mut parts = |_: &Hero, _: &rc_game::follow_camera::CameraView, _: &mut Rng, _: u64| {};
        let mut world = SharedServices { svc: &svc_cell, classes: classes.clone() };
        let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: Some(&mut world) };
        let bytes = input(t).bytes();
        game.hero.idle.counter = game.counter as i32;
        let counter = game.counter;
        game.tick(Some(&bytes), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        let mut s = svc_cell.borrow_mut();
        let requests = std::mem::take(&mut s.cinematic.requests);
        // `start_scene` stores game mode 2 at once; the harness plays no scenes, so a requested scene ends at once (the
        // engine's mode-2 reset for a scene it does not play): the mission NPC's chain then goes on to the movie and
        // the next scene as it does after each scene's end.
        if s.game_mode == 2 && requests.iter().any(|r| matches!(r, EngineRequest::StartScene { .. })) { s.game_mode = 0; }
        let (n, b) = (&game.mobys.mobys[npc], &game.mobys.mobys[bridge]);
        out.push(Rec {
            counter,
            hero_state: game.hero.state,
            hero: game.hero.position(),
            script: game.camera.script_active(),
            cam: game.camera.out.pos_f32(),
            cam_fwd: game.camera.out.rows_f32()[0],
            letterbox: s.creatures.cutscene,
            npc_state: n.state,
            npc_z: n.position[2],
            bridge_pitch: b.rotation[1],
            triggers: [860usize, 863].map(|k| { let m = &game.mobys.mobys[k]; (m.state, m.cmd, m.o_class) }),
            requests,
        });
    }
    out
}

fn still(_: u32) -> PadInput { PadInput::neutral() }

fn cuboid(lv: &Level, i: usize) -> ([f32; 3], [f32; 3]) { (lv.vol.cuboids[i].centre(), lv.vol.cuboids[i].euler) }

fn near(a: [f32; 3], b: [f32; 3], d: f32) -> bool { (0..3).all(|k| (a[k] - b[k]).abs() < d) }

/// The mission NPC asks for the arrival scene in the first gameplay tick (counter 1), once, and hides the ship.
#[test]
fn arrival_scene_is_asked_for_in_the_first_tick() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let r = run(&lv, 5, still, &[]);
    let asks: Vec<(u64, EngineRequest)> = r.iter().flat_map(|x| x.requests.iter().filter(|q| matches!(q, EngineRequest::StartScene { .. })).map(move |q| (x.counter, *q))).collect();
    assert_eq!(asks, [(1, EngineRequest::StartScene { scene: 5, arrival: true })]);
    assert!(r[0].requests.contains(&EngineRequest::ShipHidden(true)));
    assert!(r.iter().all(|x| !x.script && !x.letterbox && x.npc_state == 1));
}

/// Drop-in cutaway: Ratchet reaches cuboid 42 → camera trigger 860 fires with the mission NPC's 420 ticks: Ratchet
/// placed on the cuboid's centre and held (0x72: the stick pushed all along does nothing), the letterbox flag, the
/// script camera at cuboid 43 turning to follow the NPC as it drops 30 units at 5 u/s and lands; then all released.
#[test]
fn drop_in_cutaway() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let (c42, _) = cuboid(&lv, 42);
    let (c43, _) = cuboid(&lv, 43);
    let push = |t: u32| if t >= 5 { PadInput::neutral().stick(0.0, -1.0) } else { PadInput::neutral() };
    let r = run(&lv, 560, push, &[(5, Act::Place(c42))]);
    let t0 = r.iter().position(|x| x.script).expect("the cutaway never fired");
    let x = &r[t0];
    eprintln!("tick {t0} (counter {}): hero {:?} state {:#x}, cam {:?} fwd {:?}, npc state {} z {}", x.counter, x.hero, x.hero_state, x.cam, x.cam_fwd, x.npc_state, x.npc_z);
    // HeroTeleport to cuboid 42's centre, held; the camera at cuboid 43.
    assert!(near(x.hero, c42, 0.6), "hero {:?} vs cuboid 42 {:?}", x.hero, c42);
    assert_eq!(x.hero_state, 0x72);
    assert!(near(x.cam, c43, 0.01), "camera {:?} vs cuboid 43 {:?}", x.cam, c43);
    assert!(x.letterbox);
    // Held for the whole cutaway although the stick is pushed; the camera follows the dropping NPC.
    let release = (t0..r.len()).find(|&t| !r[t].script).expect("never released");
    let held = release - t0;
    eprintln!("held {held} ticks; released at tick {release}: state {:#x} letterbox {}", r[release].hero_state, r[release].letterbox);
    assert!((419..=422).contains(&held), "held {held}");
    for t in t0 + 1..release {
        assert_eq!(r[t].hero_state, 0x72, "tick {t}");
        assert!(near(r[t].hero, r[t0].hero, 0.05), "tick {t}: moved to {:?}", r[t].hero);
        assert!(r[t].letterbox);
    }
    let land = r[release - 1].npc_z;
    assert!(r[t0].npc_z > land + 25.0 && r[release - 1].npc_state >= 3, "npc z {} → {land}, state {}", r[t0].npc_z, r[release - 1].npc_state);
    let (a, b) = (r[t0 + 30].cam_fwd, r[release - 1].cam_fwd);
    eprintln!("camera forward {a:?} → {b:?}");
    assert!(b[2] < a[2] - 0.2, "the camera did not follow the drop");
    // Released: control back (SetState(0, 1), and the pushed stick walks him at once), letterbox off, the follow
    // camera again.
    assert!(!r[release].letterbox);
    assert!(matches!(r[release].hero_state, 0 | 2), "state {:#x}", r[release].hero_state);
    assert_eq!(r[t0 - 1].triggers[0].0, 1);
    assert_eq!((r[release].triggers[0].0, r[release].triggers[0].1), (3, 0));
    assert!(!near(r[release + 60].hero, r[release].hero, 0.5), "no control after the cutaway");
}

/// The door cutaway: once the three troopers are dead the mission NPC plays scene 3, movie 3 and scene 4 (engine
/// requests), then commands camera trigger 863 for 180 ticks while the hinged bridge halves lower (2 s), sets its
/// mission done and gives the control back.
#[test]
fn bridge_cutaway_after_the_mission() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let (c42, _) = cuboid(&lv, 42);
    let (c51, _) = cuboid(&lv, 51);
    let mut acts = vec![(5, Act::Place(c42))];
    acts.extend([325usize, 326, 327].map(|k| (500, Act::Kill(k))));
    let r = run(&lv, 1000, still, &acts);
    let asks: Vec<(usize, EngineRequest)> = r.iter().enumerate().flat_map(|(t, x)| x.requests.iter().filter(|q| !matches!(q, EngineRequest::ShipHidden(true))).map(move |q| (t, *q))).collect();
    eprintln!("requests: {asks:?}");
    let seq: Vec<EngineRequest> = asks.iter().map(|a| a.1).collect();
    assert!(seq.windows(3).any(|w| w == [EngineRequest::StartScene { scene: 3, arrival: false }, EngineRequest::StartMovie { movie: 3 }, EngineRequest::StartScene { scene: 4, arrival: false }]), "{seq:?}");
    assert!(seq.contains(&EngineRequest::MissionDone { mission: 0 }) && seq.contains(&EngineRequest::ShipHidden(false)));
    let done_t = asks.iter().find(|a| a.1 == EngineRequest::MissionDone { mission: 0 }).unwrap().0;
    let on = (done_t..r.len()).find(|&t| r[t].script).expect("the bridge cutaway never fired");
    let off = (on..r.len()).find(|&t| !r[t].script).expect("never released");
    eprintln!("mission done at tick {done_t}; bridge cutaway ticks {on}..{off}; bridge pitch {} → {}", r[on].bridge_pitch, r[off].bridge_pitch);
    assert!(near(r[on].cam, c51, 0.01), "camera {:?} vs cuboid 51 {:?}", r[on].cam, c51);
    assert!((179..=182).contains(&(off - on)), "held {}", off - on);
    assert!((r[on - 1].bridge_pitch - std::f32::consts::PI / 5.0).abs() < 1e-5);
    // Lowered within the cutaway (0.5 of the way per second: 120 ticks).
    let flat = (on..off).find(|&t| r[t].bridge_pitch == 0.0).expect("the bridge never lowered");
    assert!((118..=123).contains(&(flat - on)), "flat after {} ticks", flat - on);
    assert!(r[on..off].iter().all(|x| x.hero_state == 0x72 && x.letterbox));
    assert!(!r[off].letterbox && r[off].hero_state == 0);
}

/// Gunship 695 over the bridge: Ratchet in cuboid 22 → the fly-by with the camera on cuboid 23, Ratchet held and the
/// letterbox for 390 ticks after its 120-tick jump ahead, then the camera back.
#[test]
fn gunship_fly_by_over_the_bridge() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let (c22, _) = cuboid(&lv, 22);
    let (c23, _) = cuboid(&lv, 23);
    let r = run(&lv, 520, still, &[(5, Act::Place(c22))]);
    let Some(on) = r.iter().position(|x| x.script) else { panic!("the fly-by never started") };
    let off = (on..r.len()).find(|&t| !r[t].script).expect("never released");
    eprintln!("fly-by: script camera ticks {on}..{off} ({} ticks), hero {:#x}, camera {:?}", off - on, r[on].hero_state, r[on].cam);
    assert!(near(r[on].cam, c23, 0.01), "camera {:?} vs cuboid 23 {:?}", r[on].cam, c23);
    assert!(r[on..off].iter().all(|x| x.hero_state == 0x72 && x.letterbox));
    assert!((388..=392).contains(&(off - on)), "held {}", off - on);
    assert!(!r[off].letterbox && r[off].hero_state == 0);
}

/// Two runs of the drop-in and bridge sequence are identical tick for tick.
#[test]
fn cutscenes_are_deterministic() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let (c42, _) = cuboid(&lv, 42);
    let mut acts = vec![(5, Act::Place(c42))];
    acts.extend([325usize, 326, 327].map(|k| (500, Act::Kill(k))));
    assert_eq!(run(&lv, 800, still, &acts), run(&lv, 800, still, &acts));
}

/// Survey of the cinematic users placed on Novalis (instances, pvars, cuboids). `--ignored --nocapture`.
#[test]
#[ignore]
fn survey() {
    let Some(gp) = rc_formats::test_data::gameplay(1) else { return };
    let inst = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let vol = volumes::parse_volumes(&gp).unwrap();
    let want = [737, 688, 686, 700, 701, 1134, 280, 641, 665, 730, 790, 750, 11, 1341, 1546, 459, 666, 1818, 746];
    for (i, m) in inst.iter().enumerate() {
        if !want.contains(&m.o_class) { continue }
        println!("inst {i} class {} mission {} spawn_id {} pos {:?} rot {:?} pvar {}", m.o_class, m.unknown_4, m.spawn_id, m.position, m.rotation, m.pvar_index);
        if let Some(p) = m.pvar(&pvars).filter(|_| m.o_class != 459) {
            let words: Vec<String> = p.chunks(4).take(0x80).enumerate().map(|(k, c)| {
                let w = i32::from_le_bytes(c.try_into().unwrap_or([0; 4]));
                if (-100000..100000).contains(&w) { format!("{:#x}:{w}", k * 4) } else { format!("{:#x}:{:.3}", k * 4, f32::from_bits(w as u32)) }
            }).collect();
            println!("   {}", words.join(" "));
        }
    }
    for (i, c) in vol.cuboids.iter().enumerate() {
        println!("cuboid {i} centre {:?} euler {:?}", c.centre(), c.euler);
    }
}
