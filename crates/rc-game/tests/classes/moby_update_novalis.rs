//! The moby scheduler and the ported classes (bolts, crates, grass) on Novalis (level 1): the level's
//! instances through the loader, the load pass, then 300 ticks with Ratchet standing at his spawn and the
//! camera where `Game::new` snaps it. Skipped when `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gameplay, level};
use rc_game::hero::physics as ph;
use rc_game::moby_runtime::{mode, MobyTable, Seq0Info};
use rc_game::moby_update::classes::bolt;
use rc_game::moby_update::scheduler::{self, class_info, load_static_mobys, Groups, Scheduler};
use rc_game::moby_update::services::World;
use rc_game::moby_update::{ClassTable, Services};
use rc_game::particles::Particles;
use rc_game::ps2v::Pf;
use rc_game::tick::{Game, GameOptions};
use std::collections::BTreeMap;

struct Level {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    splines: Vec<Vec<[f32; 4]>>,
    gp: Vec<u8>,
    classes: ClassTable,
    spawnable: usize,
    death_z: f32,
    /// The class collision blobs (moby collision).
    coll_blobs: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
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
    let sec = rd(0x44) as usize;
    let spawnable = rd(sec + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    // Class slots: the order of the core's class list (`MobyClassesRelocate`, slot counter 0x15ffc0).
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let Some(blob) = rc_formats::test_data::core_block(1, &format!("moby_class/{:04}", e.o_class)) else { continue };
        let Ok(c) = rc_formats::moby::parse_moby_class(&blob) else { continue };
        let seqs = parse_sequences(&blob, &c).unwrap_or_default();
        let anim = MobyAnimClass::new(&c, seqs);
        let mut info = class_info(&c, slot as u8, scheduler::port_update_fn(oc));
        info.seq0 = anim.sequence(0).map(|q| Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
        classes.classes.insert(oc, (info, Some(anim)));
    }
    let coll_blobs = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    Some(Level { mesh, instances, pvars, splines, gp: gp.to_vec(), classes, spawnable, death_z, coll_blobs })
}

/// A MobyProc stand-in for +0x31 (the renderer's "drawn this frame"): drawn when not hidden (`mode & 0x81`),
/// not deleted, and within the draw distance (+0x32) of the camera. The real test also has the frustum and
/// the occlusion; the active-rule check below reads whatever +0x31 holds, so it does not depend on this.
fn mobyproc_standin(t: &mut MobyTable, cam: [f32; 3]) {
    for m in t.mobys.iter_mut() {
        if m.state == 0xff { break; }
        let d = ((m.position[0] - cam[0]).powi(2) + (m.position[1] - cam[1]).powi(2) + (m.position[2] - cam[2]).powi(2)).sqrt();
        m.visible = (m.state < 0x80 && m.mode & 0x81 == 0 && d <= m.draw_dist as f32) as u8;
    }
}

/// `fun_0020d868`'s rule, recomputed in f64: (count, directly active ids).
fn active_count(t: &MobyTable, cam: [f32; 3], groups: &Groups) -> usize {
    let mut n = 0;
    let mut flagged = [false; 0x70];
    for m in &t.mobys {
        if m.state == 0xff { break; }
        if m.state >= 0x80 || m.mode & 2 != 0 { continue; }
        let d2: f64 = (0..3).map(|k| (m.position[k] as f64 - cam[k] as f64).powi(2)).sum();
        let r = m.update_dist as f64;
        let active = m.visible != 0 || m.update_dist == 0xff || d2 <= r * r;
        if !active { continue; }
        if m.group >= 0 { flagged[m.group as usize] = true; } else { n += 1; }
    }
    for (g, &f) in flagged.iter().enumerate() {
        if !f { continue; }
        if let Some(Some(l)) = groups.lists.get(g) {
            n += l.iter().filter(|&&e| t.mobys[e as usize].state < 0x80).count();
        }
    }
    n
}

struct Run {
    rng: u32,
    bolts: i32,
    collected: Vec<usize>,
    active: Vec<usize>,
}

fn run(lv: &Level, ticks: u64, verbose: bool, hero_at: Option<[f32; 3]>) -> Run {
    let mut classes = ClassTable { classes: lv.classes.classes.clone() };
    let statics = load_static_mobys(&lv.instances, &mut classes, &lv.pvars);
    let hero_idx = statics.iter().position(|m| m.o_class == 0).expect("Ratchet");
    let mut table = MobyTable::new(statics, lv.spawnable);
    table.mobys[hero_idx].mode |= mode::NO_UPDATE; // hero init 0x226b70
    if let Some(p) = hero_at { table.mobys[hero_idx].position = [p[0], p[1], p[2], 0.0]; }
    let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
    let cam = game.camera.out.pos;
    let cam_f = ph::to_f32x3(cam);
    let mut svc = Services::new();
    svc.level = 1;
    svc.set_splines(&lv.splines);
    svc.groups = Groups::parse(&lv.gp, &|i| (i < lv.instances.len()).then_some(i));
    svc.set_moby_collision(lv.coll_blobs.clone());
    svc.build_grid(&mut game.mobys);
    let mut particles = Particles::new(None, Vec::new());
    let mut sched = Scheduler::new();
    // The hero fields a hero tick would have set (the hero is not ticked: he stands): body point
    // pos + R·(0, 0, 0.7) (0x23c710) and the ground point under the feet.
    let mut hero = game.hero.clone();
    hero.body_point = ph::vadd(ph::mul_rows4(&hero.rows, [Pf::ZERO, Pf::ZERO, Pf::b(0x3f33_3333), Pf::ZERO]), hero.pos);
    hero.ground_point = hero.pos;
    let pv = |m: &rc_game::moby_runtime::Moby| [m.position[0], m.position[1], m.position[2]];
    // Load pass (tick counter 0).
    {
        let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &classes, &mut svc, 0);
        w.camera = cam;
        w.coll = Some(&lv.mesh);
        w.particles = Some(&mut particles);
        let n = sched.load_pass(&mut w);
        if verbose { eprintln!("load pass: {n} mobys run, rng {:#010x}", game.rng.state); }
    }
    let hp = hero.pos;
    let mut collected = Vec::new();
    let mut active = Vec::new();
    for counter in 0..ticks {
        mobyproc_standin(&mut game.mobys, cam_f);
        let expect = active_count(&game.mobys, cam_f, &svc.groups);
        // Bolts in the idle state and their pickup test inputs, before the tick.
        let idle: Vec<(usize, [f32; 3])> = game.mobys.mobys.iter().enumerate()
            .filter(|(_, m)| (13..=16).contains(&m.o_class) && m.state == 3).map(|(i, m)| (i, pv(m))).collect();
        game.mobys.free_slot_pass(counter);
        let n = {
            let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &classes, &mut svc, counter);
            w.camera = cam;
            w.coll = Some(&lv.mesh);
                w.particles = Some(&mut particles);
            sched.tick(&mut w)
        };
        assert_eq!(n, expect, "tick {counter}: active count vs the independent rule");
        active.push(n);
        let (rx, rz) = if counter == 0 { (2.125f32, 1.25f32) } else { (3.0, 1.75) };
        for (i, p) in idle {
            if !sched.last_list.contains(&i) { continue; }
            let m = &game.mobys.mobys[i];
            let d = ((p[0] - hp[0].to_f32()).powi(2) + (p[1] - hp[1].to_f32()).powi(2)).sqrt();
            let near = d < rx && (p[2] - hp[2].to_f32()).abs() < rz;
            let flying = m.state == 4 || m.is_deleted();
            assert_eq!(near, flying, "tick {counter}: bolt {i} at xy distance {d}: near {near}, state {}", m.state);
            if flying { collected.push(i); }
        }
        particles.update_parts(&mut game.rng);
        svc.glints.update();
    }
    let bolts = svc.counters.bolts;
    for &i in &collected {
        assert!(game.mobys.mobys[i].is_deleted(), "bolt {i} flew but was not collected");
    }
    if verbose {
        let mut by_class: BTreeMap<i16, usize> = BTreeMap::new();
        let mut states: BTreeMap<(i16, u8), usize> = BTreeMap::new();
        for m in &game.mobys.mobys {
            if m.state == 0xff { break; }
            *by_class.entry(m.o_class).or_default() += 1;
            if scheduler::port_update_fn(m.o_class).is_some() { *states.entry((m.o_class, m.state)).or_default() += 1; }
        }
        eprintln!("camera {cam_f:?}, hero {:?}", ph::to_f32x3(hp));
        let crates: Vec<usize> = (0..lv.instances.len()).filter(|&i| (500..=511).contains(&lv.instances[i].o_class)).collect();
        let stacked = crates.iter().filter(|&&i| game.mobys.mobys[i].pvars.get(0xa4..0xa8).map(|b| b != [0, 0, 0, 0]).unwrap_or(false)).count();
        let moved = crates.iter().map(|&i| {
            let (m, p) = (&game.mobys.mobys[i], lv.instances[i].position);
            (0..3).map(|k| (m.position[k] - p[k]).abs()).fold(0.0f32, f32::max)
        }).fold(0.0f32, f32::max);
        let with_bolts = crates.iter().filter(|&&i| lv.instances[i].unknown_10 != 0 && lv.instances[i].spawn_flags != 0).count();
        let paths = crates.iter().filter(|&&i| game.mobys.mobys[i].pvars.get(0xc0..0xc4).map(|b| b != [0xff; 4]).unwrap_or(false)).count();
        let mut movers: Vec<(f32, usize)> = crates.iter().map(|&i| {
            let (m, p) = (&game.mobys.mobys[i], lv.instances[i].position);
            ((0..3).map(|k| (m.position[k] - p[k]).abs()).fold(0.0f32, f32::max), i)
        }).collect();
        movers.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(d, i) in movers.iter().take(4) {
            let m = &game.mobys.mobys[i];
            eprintln!("  crate {i} class {} moved {d}: {:?} → {:?}, below {:?}", m.o_class, lv.instances[i].position, &m.position[..3],
                i32::from_le_bytes(m.pvars[0xa4..0xa8].try_into().unwrap()) - 1);
        }
        eprintln!("crates: {} ({} on another crate, {} with a bolt drop, {} with a path), largest move from the instance {moved}",
            crates.len(), stacked, with_bolts, paths);
        eprintln!("active per tick: first {:?} … last {:?}; min {} max {}", &active[..5], &active[active.len() - 3..],
            active.iter().min().unwrap(), active.iter().max().unwrap());
        eprintln!("ported class states (class, state) → count: {states:?}");
        eprintln!("collected bolts (index, class, value): {:?}", collected.iter().map(|&i| (i, lv.instances[i].o_class, bolt::value(lv.instances[i].o_class as i16))).collect::<Vec<_>>());
        eprintln!("bolt counter {bolts}, sounds {}, glints live {}, particles created {}, rng {:#010x}", svc.sounds.len(),
            svc.glints.entries.iter().filter(|g| g.timer > 0).count(), particles.stats.created, game.rng.state);
    }
    Run { rng: game.rng.state, bolts, collected, active }
}

#[test]
fn novalis_scheduler_300_ticks() {
    let Some(lv) = load() else { eprintln!("skipped: no extracted/"); return; };
    // The level table maps the ported classes to the ported addresses (read from the level overlay).
    let ov = rc_formats::level_overlay::LevelOverlay::parse(&std::fs::read(rc_formats::test_data::level_dir(1).join("overlay.bin")).unwrap()).unwrap();
    {
        let map: BTreeMap<i16, u32> = ov.vtbl().iter().map(|e| (e.o_class as i16, e.update)).collect();
        for oc in [13i16, 14, 15, 16, 500, 501, 502, 505, 511, 724, 725] {
            assert_eq!(map.get(&oc).copied(), scheduler::port_update_fn(oc), "class {oc}");
        }
        eprintln!("level table: {} entries, ported classes match", map.len());
    }
    let a = run(&lv, 300, true, None);
    // Placed bolts within the pickup range were collected: the counter is the sum of their values.
    let expect: i32 = a.collected.iter().map(|&i| bolt::value(lv.instances[i].o_class as i16)).sum();
    assert_eq!(a.bolts, expect);
    // Deterministic: a second run gives the same stream and the same results.
    let b = run(&lv, 300, false, None);
    assert_eq!((a.rng, a.bolts, &a.collected, &a.active), (b.rng, b.bolts, &b.collected, &b.active));
    eprintln!("rng after 300 ticks: {:#010x} (deterministic)", a.rng);

    // The same with Ratchet standing on the placed bolt nearest to his spawn: it and its neighbours in reach
    // fly to him and are collected.
    let spawn = lv.instances.iter().find(|m| m.o_class == 0).unwrap().position;
    let near = lv.instances.iter().enumerate().filter(|(_, m)| (13..=16).contains(&m.o_class))
        .min_by(|a, b| {
            let d = |m: &gameplay::MobyInstance| (0..3).map(|k| (m.position[k] - spawn[k]).powi(2)).sum::<f32>();
            d(a.1).total_cmp(&d(b.1))
        }).unwrap();
    eprintln!("nearest bolt to the spawn: instance {} class {} at {:?}", near.0, near.1.o_class, near.1.position);
    let c = run(&lv, 300, true, Some(near.1.position));
    let expect: i32 = c.collected.iter().map(|&i| bolt::value(lv.instances[i].o_class as i16)).sum();
    assert!(c.collected.contains(&near.0), "the bolt under Ratchet was not collected");
    assert_eq!(c.bolts, expect);
}
