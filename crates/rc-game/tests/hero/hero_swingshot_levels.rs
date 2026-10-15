//! Package P6 (docs/plan/hero_states.md "P6"): the Swingshot headless on the levels' own data. The level runs as the
//! engine ticks it (the static mobys, the scheduler's load pass and moby loop with the ported target classes 758 /
//! 803, the moby collision, the hand item created from the gadget table): Ratchet is put next to real targets on
//! Aridia (level 2) and Kerwan (level 3), fires with ○, is pulled to a pull target and swings from a swing target.
//! Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, parse_sequences, MobyAnimClass, MobySequence};
use rc_formats::{collision, gadget, gameplay, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::items::{ItemClass, ItemData, ItemDef, HERO_LISTS};
use rc_game::hero::swingshot::{PULL_CLASS, SWINGSHOT, SWING_CLASS};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::scheduler::{class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::{SharedServices, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::pad::{button, PadInput};
use rc_game::rng::Rng;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Lv {
    n: u32,
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
    items: ItemData,
    volumes: rc_formats::volumes::Volumes,
}

fn load(n: u32) -> Option<Lv> {
    let dir = rc_formats::test_data::root().join(format!("levels/{n:02}"));
    let data = rc_formats::test_data::core_data(n)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(n)?;
    let settings = rc_formats::test_data::gameplay_section(n, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    // The level's own class table (code identity): the swing targets are level 03's port, not level 01's.
    let overlay = |l: u32| -> Option<Arc<rc_formats::level_overlay::LevelOverlay>> {
        let b = std::fs::read(rc_formats::test_data::level_dir(l).join("overlay.bin")).ok()?;
        Some(Arc::new(rc_formats::level_overlay::LevelOverlay::parse(&b).ok()?))
    };
    let ports = rc_game::moby_update::classes::LevelPorts::from_overlays(&*overlay(n)?, &overlay, &[]);
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let splines = gameplay::parse_splines(&gp).unwrap();
    let volumes = rc_formats::volumes::parse_volumes(&gp).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(n, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, ports.update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    let ratchet_blob = rc_formats::test_data::core_block(n, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&ratchet_blob).unwrap();
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(n, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let ratchet = MobyAnimClass::new(&class, seqs);
    // The hand items as the engine builds them (crate rc-engine `item_data`): the gadget classes, Ratchet's joint
    // lists, and the two definitions used here (level01 0x179f40: 8 = the wrench 71 on list 0, 12 = the Swingshot
    // 0xd0 on list 1).
    let mut defs = vec![ItemDef::default(); 37];
    defs[8] = ItemDef { slot: 0, attach: 0, o_class: 71, b18: 0 };
    defs[12] = ItemDef { slot: 0, attach: 1, o_class: 0xd0, b18: 1 };
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
    Some(Lv { n, mesh, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs, ratchet, items, volumes })
}

/// One tick of a run.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    state: i32,
    pos: [f32; 3],
    item: u8,
    hooked: bool,
    rope: f32,
}

/// Level `lv` with its mobys, Ratchet at `at` holding the Swingshot, `input(t)` for `ticks` ticks.
fn run(lv: &Lv, at: ([f32; 3], f32), input: &dyn Fn(u32) -> PadInput, ticks: u32) -> Vec<Row> {
    let classes = Arc::new(ClassTable { classes: lv.classes.classes.clone() });
    let mut ct = ClassTable { classes: lv.classes.classes.clone() };
    let mut statics = load_static_mobys(&lv.instances, &mut ct, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let (p, yaw) = at;
    statics[hero_idx].position = [p[0], p[1], p[2], 1.0];
    statics[hero_idx].rotation = [0.0, 0.0, yaw, 0.0];
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE;
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    game.hero.idle.level = lv.n as i32;
    game.hero.grant_items(&[SWINGSHOT as usize]);
    game.item_data = Some(lv.items.clone());
    game.item_globals.saved = SWINGSHOT;
    game.item_globals.wrench_flag = 0;
    let mut svc = Services::new();
    svc.level = lv.n;
    svc.set_splines(&lv.splines);
    svc.set_volumes(lv.volumes.clone());
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
        let r = game.tick(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        if r.hero == rc_game::hero::HeroTick::OutOfBounds { break; }
        let s = &game.hero.swing;
        rows.push(Row { state: game.hero.state, pos: game.hero.position(), item: s.item.state, hooked: s.hooked, rope: s.rope });
    }
    rows
}

/// `RC_SWING_DUMP=1`: every 5th row.
fn dump(rows: &[Row]) {
    if std::env::var("RC_SWING_DUMP").is_err() { return; }
    for (t, r) in rows.iter().enumerate().step_by(5) { eprintln!("  {t:3} {r:?}"); }
}

fn states(rows: &[Row]) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for r in rows { if v.last() != Some(&r.state) { v.push(r.state); } }
    v
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// The targets of class `oc` on the level, (instance, position).
fn targets(lv: &Lv, oc: i16) -> Vec<(usize, [f32; 3])> {
    lv.instances.iter().enumerate().filter(|(_, m)| m.o_class as i16 == oc).map(|(i, m)| (i, m.position)).collect()
}

/// Ground spots around target `t` at horizontal distance `r` (12 directions) whose floor is `dz` = 2..`max_dz`
/// below the target, with a clear line to it; (spot, yaw toward the target).
fn spots(lv: &Lv, t: [f32; 3], r: f32, max_dz: f32) -> Vec<([f32; 3], f32)> {
    (0..12)
        .filter_map(|k| {
            let a = k as f32 * std::f32::consts::TAU / 12.0;
            let (x, y) = (t[0] + r * a.cos(), t[1] + r * a.sin());
            let hit = coll_line(&lv.mesh, [x, y, t[2] + 0.5], [x, y, t[2] - max_dz - 1.0], QueryFlags(2))?;
            let z = hit.point[2];
            let dz = t[2] - z;
            if !(2.0..=max_dz).contains(&dz) { return None; }
            // Level floor around the spot (not a wall edge).
            let n = hit.normal;
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if n[2] / l < 0.9 { return None; }
            if coll_line(&lv.mesh, [x, y, z + 0.5], t, QueryFlags(2)).is_some() { return None; }
            Some(([x, y, z], (t[1] - y).atan2(t[0] - x)))
        })
        .collect()
}

/// ○ at tick 20, held until `release`.
fn hold(release: u32) -> impl Fn(u32) -> PadInput {
    move |t| if (20..release).contains(&t) { PadInput::neutral().press(button::CIRCLE) } else { PadInput::neutral() }
}

/// On Aridia and Kerwan: from a spot below a pull target, ○ held: 0x24, the hook flies, 0x25 flies Ratchet to the
/// target; he ends near it. Deterministic.
#[test]
fn pull_on_real_targets() {
    let mut done = 0;
    for n in [2, 3] {
        let Some(lv) = load(n) else { eprintln!("skipped: no extracted/"); return };
        for (i, t) in targets(&lv, PULL_CLASS) {
            let Some(&(spot, yaw)) = spots(&lv, t, 10.0, 8.0).first() else { continue };
            let rows = run(&lv, (spot, yaw), &hold(200), 200);
            let st = states(&rows);
            eprintln!("level {n} pull target {i} at {t:?} from {spot:?} yaw {yaw:.3}: states {st:x?}");
            dump(&rows);
            if !st.contains(&0x25) { continue; }
            assert!(st.windows(2).any(|w| w == [0x24, 0x25]), "0x24 → 0x25: {st:x?}");
            let closest = rows.iter().map(|r| dist(r.pos, t)).fold(f32::MAX, f32::min);
            assert!(closest < 3.5, "flew to the target: closest {closest}");
            let again = run(&lv, (spot, yaw), &hold(200), 200);
            assert_eq!(rows, again, "deterministic");
            eprintln!("RC_LEVEL={n} RC_HERO_AT={:.2},{:.2},{:.2},{:.3} (pull)", spot[0], spot[1], spot[2], yaw);
            done += 1;
            break;
        }
    }
    assert!(done >= 1, "no pull target could be reached");
}

/// On Aridia and Kerwan: from a spot near a swing target, ○ held: 0x2c, the hook holds, the rope settles toward
/// the record's length and he swings; released: 0x2d and back on the ground or still falling.
#[test]
fn swing_on_real_targets() {
    let mut done = 0;
    for n in [2, 3] {
        let Some(lv) = load(n) else { eprintln!("skipped: no extracted/"); return };
        for (i, t) in targets(&lv, SWING_CLASS) {
            let Some(&(spot, yaw)) = spots(&lv, t, 9.0, 7.0).first() else { continue };
            let rows = run(&lv, (spot, yaw), &hold(160), 260);
            let st = states(&rows);
            eprintln!("level {n} swing target {i} at {t:?} from {spot:?} yaw {yaw:.3}: states {st:x?}");
            dump(&rows);
            if !st.contains(&0x2c) { continue; }
            assert!(rows.iter().any(|r| r.hooked), "the hook held");
            let last_swing = rows.iter().rev().find(|r| r.state == 0x2c).unwrap();
            assert!(last_swing.rope < dist(spot, t), "the rope shortened: {}", last_swing.rope);
            assert!(st.contains(&0x2d), "released into 0x2d: {st:x?}");
            let again = run(&lv, (spot, yaw), &hold(160), 260);
            assert_eq!(rows, again, "deterministic");
            eprintln!("RC_LEVEL={n} RC_HERO_AT={:.2},{:.2},{:.2},{:.3} (swing)", spot[0], spot[1], spot[2], yaw);
            done += 1;
            break;
        }
    }
    assert!(done >= 1, "no swing target could be used");
}
