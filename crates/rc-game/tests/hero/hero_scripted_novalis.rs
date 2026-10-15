//! The scripted hold 0x1f from its real caller on Novalis, headless (docs/plan/hero_states.md "Scripted control",
//! G-HERO-002): the level as the engine ticks it (the loader's spawn test, the scheduler's load pass and moby loop,
//! the hero, the camera). Ratchet wears the Sonic Summoner and stands in the mouse 1818's summon cuboid pushing the
//! stick: the mouse's summon puts him in 0x1f (`SetState(0x1f, 0)` + `SetAnim(6, 0, 0)`), where the idle physics
//! hold him and the pad does nothing, until the mouse arrives beside him and gives him back (`SetState(0, 1)`); then
//! the stick walks him. Before G-HERO-002 the hero froze in 0x1f (`HeroTick::Unimplemented`). Deterministic. Skipped
//! when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level, moby_spawn};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::scripted;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::classes::mouse;
use rc_game::moby_update::scheduler::{self, class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar, SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::PadInput;
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
    Some(Lv { mesh, instances, pvars, tests, splines, gp: gp.to_vec(), volumes, classes, spawnable, death_z, coll_blobs, ratchet })
}

/// One tick of the run.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    state: i32,
    group: i32,
    no_control: u8,
    pos: [f32; 3],
    seq_b: u8,
    mouse_state: u8,
    rng: u32,
}

/// Novalis with its mobys; Ratchet with the Sonic Summoner on at the mouse's summon cuboid, the stick pushed.
fn run(lv: &Lv, ticks: u32) -> Vec<Row> {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
    let mut mobys = statics.mobys.clone();
    let mouse_id = mobys.iter().position(|m| m.o_class == mouse::MOUSE_CLASSES[0]).expect("the mouse");
    let cub = pvar::i32(&mobys[mouse_id].pvars, mouse::pvo::CUBOID);
    let at = lv.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cub).expect("summon cuboid").centre();
    let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
    mobys[hero_idx].position = [at[0], at[1], at[2], 1.0];
    let mut table = MobyTable::new(mobys, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    // The Sonic Summoner owned and worn (head item 5, its moby class 0x1b1).
    game.hero.owned.0[mouse::SUMMONER as usize] = 1;
    game.hero.head_slot.id = mouse::SUMMONER;
    game.hero.head_slot.saved = mouse::SUMMONER;
    game.hero.head_slot.state = 2;
    game.hero.worn.head = Some(rc_game::hero::worn::HeadMoby { o_class: mouse::SUMMONER_CLASS, anim: rc_game::moby_runtime::Moby::zeroed().anim, snapshot: None });
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
        let pad = PadInput::neutral().stick(0.0, -1.0);
        let r = game.tick(Some(&pad.bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        assert_eq!(r.hero, rc_game::hero::HeroTick::Ran, "hero stopped in state {:#x} at tick {t}", game.hero.state);
        rows.push(Row {
            state: game.hero.state,
            group: game.hero.group,
            no_control: game.hero.items.f13fc,
            pos: game.hero.position(),
            seq_b: anim.state.seq_b,
            mouse_state: game.mobys.mobys[mouse_id].state,
            rng: game.rng.state,
        });
    }
    rows
}

/// The mouse's summon holds Ratchet in 0x1f (group 9, control kept) with the idle sequence, the pushed stick moving
/// nothing, until it arrives and gives him back; the stick then walks him. Two runs are identical.
#[test]
fn novalis_mouse_summon_holds_ratchet_in_0x1f() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return };
    let rows = run(&lv, 400);
    let held = rows.iter().position(|r| r.state == scripted::HELD).expect("never held by the mouse's summon");
    let back = rows[held..].iter().position(|r| r.state != scripted::HELD).map(|k| k + held).expect("never given back");
    eprintln!("held at tick {held}: {:?}; given back at tick {back}: {:?}", rows[held], rows[back]);
    for r in &rows[held..back] {
        assert_eq!((r.group, r.no_control, r.seq_b), (9, 0, 0), "{r:?}");
        assert!(r.mouse_state >= 2 && r.mouse_state <= 4, "the mouse runs out meanwhile: {r:?}");
    }
    let at = rows[held + 5].pos;
    for r in &rows[held + 5..back] {
        assert!((0..3).all(|k| (r.pos[k] - at[k]).abs() < 1e-3), "the stick moved him: {at:?} → {:?}", r.pos);
    }
    assert!(back - held > 10, "held for a while ({} ticks)", back - held);
    assert!(rows[back..].iter().any(|r| r.state == 2), "the stick walks him once given back");
    assert_eq!(rows, run(&lv, 400), "deterministic");
}
