//! Package P1 on Novalis (level 1): the platform carry on the path lift 726, headless — the level's mobys through
//! the loader and the scheduler, their collision for the hero's probes (`SharedServices`), Ratchet ticked by
//! `Game::tick` with the engine's check script `0-130:stick 0.33 -0.94` (docs/plan/triggers.md §7): he runs from
//! the spawn onto the lift at the plateau's edge, the lift departs and carries him down the cliff to the meadow.
//! Skipped when `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gameplay, level};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::PadInput;
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
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let Some(blob) = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)) else { continue };
        let Ok(c) = rc_formats::moby::parse_moby_class(&blob) else { continue };
        let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
        let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
        info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
        classes.classes.insert(oc, (info, Some(anim)));
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let blob = rc_formats::test_data::core_block(1, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(1, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    Some(Level { mesh, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet })
}

/// One tick's record: (hero state, hero position, lift position, ground moby, carry flags).
type Rec = (i32, [f32; 3], [f32; 3], Option<usize>, u32);

/// Runs the level for `ticks` gameplay ticks with `input(t)` (t = the tick index, as the engine's script).
fn run(lv: &Level, ticks: u32, input: impl Fn(u32) -> PadInput) -> Vec<Rec> {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_static_mobys(&lv.instances, &mut ct, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let lift = statics.iter().position(|m| m.o_class == 726).expect("the lift 726");
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = 1;
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.groups = Groups::parse(&lv.gp, &|i| (i < lv.instances.len()).then_some(i));
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
    let mut out = Vec::new();
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
        let bytes = input(t).bytes();
        game.hero.idle.counter = game.counter as i32;
        game.tick(Some(&bytes), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        let l = game.mobys.mobys[lift].position;
        out.push((game.hero.state, game.hero.position(), [l[0], l[1], l[2]], game.hero.ground_moby, game.hero.carry.flags));
    }
    out
}

fn stick(t: u32) -> PadInput { if t <= 130 { PadInput::neutral().stick(0.33, -0.94) } else { PadInput::neutral() } }

/// The lift carries Ratchet down the cliff: he boards it at the top, stays on it (ground moby = the lift) while
/// it moves, and gets off at the bottom (174.5, 159.5, 41.0) with it. Before the carry he was left behind on the
/// first moving tick and fell to the meadow (triggers.md §7).
#[test]
fn novalis_lift_carries_ratchet_down() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let recs = run(&lv, 460, stick);
    let lift_id = lv.instances.iter().position(|i| i.o_class == 726);
    let on: Vec<u32> = recs.iter().enumerate().filter(|(_, r)| r.3.is_some() && r.3 == lift_id).map(|(t, _)| t as u32).collect();
    let first = *on.first().expect("Ratchet never stands on the lift");
    let start = recs[first as usize].2;
    let moving: Vec<u32> = (first as usize..recs.len()).filter(|&t| recs[t].2 != recs[t - 1].2).map(|t| t as u32).collect();
    eprintln!("on the lift from tick {first} (lift at {start:?}); the lift moves on {} ticks ({:?}..{:?})", moving.len(), moving.first(), moving.last());
    for t in [100usize, 150, 200, 250, 300, 350, 400, 459] {
        let r = &recs[t];
        eprintln!("tick {t}: state {:#x} hero {:?} lift {:?} ground {:?} carry flags {}", r.0, r.1, r.2, r.3, r.4);
    }
    // Every moving tick: Ratchet stays on the deck (ground moby the lift), within the deck around the lift's
    // position, never airborne for long.
    let last_move = *moving.last().expect("the lift never moved");
    assert!(moving.len() > 200, "the lift moved only {} ticks", moving.len());
    let mut off = 0;
    for &t in &moving {
        let r = &recs[t as usize];
        let (h, l) = (r.1, r.2);
        let dxy = ((h[0] - l[0]).powi(2) + (h[1] - l[1]).powi(2)).sqrt();
        assert!(dxy < 3.0 && (h[2] - l[2]).abs() < 2.0, "tick {t}: Ratchet {h:?} left the lift at {l:?}");
        if r.3 != lift_id { off += 1; }
    }
    assert!(off < 10, "{off} moving ticks without the lift under the feet");
    // The last path point (174.502, 159.499, 41.002).
    let end = recs[last_move as usize].2;
    assert!((end[0] - 174.5).abs() < 0.01 && (end[1] - 159.5).abs() < 0.01 && (end[2] - 41.0).abs() < 0.01, "lift ended at {end:?}");
    let h = recs.last().unwrap().1;
    assert!((h[2] - end[2]).abs() < 1.0, "Ratchet ended at {h:?}, the lift at {end:?}");
}

/// Determinism: two headless runs give the same per-tick records.
#[test]
fn novalis_lift_is_deterministic() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    assert_eq!(run(&lv, 320, stick), run(&lv, 320, stick));
}

/// Off the lift at the bottom, walking into the meadow's bank (faces of 55°–68° at (178..181, 157, 42)): steeper
/// than 50° is not ground, so Ratchet is held at its foot (z 40) and does not climb it (the engine check
/// `0-130:stick 0.33 -0.94,400-519:stick 0.7 0.7`, frame 520).
#[test]
fn novalis_steep_bank_holds_him_at_its_foot() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    let recs = run(&lv, 560, |t| if t <= 130 { stick(t) } else if (400..520).contains(&t) { PadInput::neutral().stick(0.7, 0.7) } else { PadInput::neutral() });
    for r in &recs[430..560] {
        assert!((40.0..40.01).contains(&r.1[2]), "climbed to {:?}", r.1);
        assert!(r.1[1] > 157.2, "went through the bank: {:?}", r.1);
    }
    let end = recs[519].1;
    assert!((end[0] - 178.39).abs() < 0.05 && (end[1] - 157.26).abs() < 0.01, "{end:?}");
}
