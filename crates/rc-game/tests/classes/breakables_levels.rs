//! Breakable props on every level (docs/plan/breakables.md): the game's shared break template
//! (`moby_update::classes::breakables`) registered through each level's own class table, and headless breaks on
//! Novalis and on every level that places one. Skipped when `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter breakables_levels:: --nocapture` prints the survey (classes, placed instances,
//! pieces, remains, collision volumes).

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gameplay, level, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{MobyId, MobyTable};
use rc_game::moby_update::classes::breakables::{Piece, Remains, RECIPES};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{HitTemplate, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::ps2v::Pf;
use rc_game::tick::{Game, GameOptions};
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

/// Every class a recipe's break creates: (pieces, remains).
fn made_classes(i: usize) -> (Vec<i16>, Option<i16>) {
    let r = &RECIPES[i];
    let mut p = Vec::new();
    for x in r.pieces {
        match *x {
            Piece::Single(c) | Piece::Ring4(c) => p.push(c),
            Piece::Burst { a, na, b, nb, .. } => {
                p.extend((0..na).map(|k| a + k as i16));
                p.extend((0..nb).map(|k| b + k as i16));
            }
        }
    }
    p.sort();
    p.dedup();
    let rem = match r.remains {
        Remains::None => None,
        Remains::Scaled(c) | Remains::Plain(c) => Some(c),
    };
    (p, rem)
}

#[test]
fn every_breakable_class_resolves_its_data_on_every_level() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut runs = vec![Vec::new(); RECIPES.len()];
    for level in 0..19u32 {
        let ov = overlay(level).unwrap();
        let p = ports(level).unwrap();
        let core = rc_formats::test_data::core(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let data = rc_formats::test_data::core_data(level).unwrap();
        let idx = std::fs::read(rc_formats::test_data::level_dir(level).join("core_index.bin")).unwrap();
        let lc = level::parse_level_core(&idx, data.len()).unwrap();
        let coll: std::collections::HashMap<i32, _> = rc_formats::moby_collision::parse_level(&lc, &data).unwrap().into_iter().collect();
        for e in ov.vtbl() {
            let oc = e.o_class as i16;
            let Some(ClassUpdate::Breakable(i)) = p.get(oc) else { continue };
            let i = i as usize;
            runs[i].push((level, oc));
            let r = &RECIPES[i];
            let placed = inst.iter().filter(|m| m.o_class as i16 == oc).count();
            // The class and every class the break creates have their class data on this level.
            assert!(core.block(&format!("moby_class/{oc:04}")).is_some(), "level {level:02} class {oc}: no class blob");
            let (pieces, rem) = made_classes(i);
            for c in &pieces {
                assert!(core.block(&format!("moby_class/{c:04}")).is_some(), "level {level:02} piece {c}: no class blob");
                assert_eq!(p.get(*c), Some(ClassUpdate::FxPiece), "level {level:02} piece {c} runs FxGroupUpdate");
            }
            if let Some(c) = rem {
                assert!(p.in_table(c), "level {level:02} remains {c} in the class table");
                assert!(core.block(&format!("moby_class/{c:04}")).is_some(), "level {level:02} remains {c}: no class blob");
            }
            let bursts = r.pieces.iter().any(|x| matches!(x, Piece::Burst { .. }));
            let prims = coll.get(&(oc as i32)).map(|c| c.prims.len()).unwrap_or(0);
            if bursts { assert!(prims > 0, "level {level:02} class {oc}: the burst needs a collision volume"); }
            eprintln!(
                "level {level:02} class {oc:5}  recipe {i:2} ({:#x} of level {:02})  {placed:3} placed  kept {}  pieces {pieces:?}  remains {rem:?}  volume prims {prims}",
                r.func, r.level, r.keep_broken
            );
        }
    }
    for (i, r) in RECIPES.iter().enumerate() {
        for c in r.classes { assert!(runs[i].contains(&(r.level, *c)), "recipe {i}: class {c} on level {:02}", r.level); }
    }
}

// ---------------------------------------------------------------------------------------------------
// Headless breaks

struct Lv {
    mesh: collision::Collision,
    instances: Vec<gameplay::MobyInstance>,
    pvars: Vec<Option<Vec<u8>>>,
    tests: Vec<moby_spawn::SpawnTest>,
    gp: Vec<u8>,
    classes: ClassTable,
    coll: Vec<(i32, rc_formats::moby_collision::MobyCollision)>,
    spawnable: usize,
    death_z: f32,
}

fn load(level: u32) -> Option<Lv> {
    let dir = rc_formats::test_data::root().join(format!("levels/{level:02}"));
    let data = rc_formats::test_data::core_data(level)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let settings = rc_formats::test_data::gameplay_section(level, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).unwrap();
    let mesh = collision::parse_collision(&core, &data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4) as usize;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let ports = ports(level)?;
    let mut classes = ClassTable::default();
    for (slot, e) in core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = rc_formats::test_data::core_block(level, &format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(&b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(&blob, &c).unwrap_or_default());
            let info = class_info(&c, slot as u8, ports.update_fn(oc));
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let coll = rc_formats::moby_collision::parse_level(&core, &data).unwrap();
    Some(Lv { mesh, instances, pvars, tests, gp: gp.to_vec(), classes, coll, spawnable, death_z })
}

struct Sim {
    game: Game,
    svc: Services,
    sched: Scheduler,
    classes: ClassTable,
}

impl Sim {
    fn new(lv: &Lv, level: u32) -> Sim {
        let mut ct = ClassTable { classes: lv.classes.classes.clone() };
        let statics = load_level_mobys(&lv.instances, &mut ct, &lv.pvars, &lv.tests);
        let mobys = statics.mobys.clone();
        let hero_idx = mobys.iter().position(|m| m.o_class == 0).expect("Ratchet");
        let table = MobyTable::new(mobys, lv.spawnable);
        let mut game = Game::new(&lv.mesh, table, hero_idx, GameOptions::default(), lv.death_z);
        let mut svc = Services::new();
        svc.level = level;
        svc.groups = statics.groups(&lv.gp);
        svc.set_volumes(rc_formats::volumes::parse_volumes(&lv.gp).unwrap());
        svc.set_moby_collision(lv.coll.clone());
        let classes = ClassTable { classes: lv.classes.classes.clone() };
        let mut sched = Scheduler::new();
        {
            let hero: Hero = game.hero.clone();
            let mut w = World::new(&mut game.mobys, &hero, &mut game.rng, &classes, &mut svc, 0);
            w.camera = game.camera.out.pos;
            w.coll = Some(&lv.mesh);
            sched.load_pass(&mut w);
        }
        game.finish_load();
        Sim { game, svc, sched, classes }
    }

    fn tick(&mut self, cam: [f32; 3]) {
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        w.camera = [Pf::f(cam[0]), Pf::f(cam[1]), Pf::f(cam[2]), Pf::ZERO];
        self.sched.tick(&mut w);
        self.game.counter += 1;
    }

    fn hit(&mut self, id: MobyId, t: &HitTemplate) {
        let hero: Hero = self.game.hero.clone();
        let counter = self.game.counter;
        let mut w = World::new(&mut self.game.mobys, &hero, &mut self.game.rng, &self.classes, &mut self.svc, counter);
        w.deliver_hit(id, t);
    }

    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.game.mobys.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
    fn count(&self, oc: i16) -> usize { self.of_class(oc).len() }
    fn bolts(&self) -> usize { (13..=16).map(|c| self.count(c)).sum() }
    fn alive(&self, id: MobyId) -> bool { self.game.mobys.mobys[id].state < 0x80 }

    /// Ratchet 2 units from `id` (within `BreakFxA`'s 7), the wrench's hit, then `n` ticks.
    fn wrench(&mut self, id: MobyId, n: usize) {
        let at = self.game.mobys.mobys[id].position;
        self.game.hero.pos = [Pf::f(at[0] + 2.0), Pf::f(at[1]), Pf::f(at[2]), Pf::ONE];
        let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
        self.tick(cam);
        self.hit(id, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
        for _ in 0..n { self.tick(cam); }
    }
}

/// Whether the loader creates instance `k` again with this visit's death bits (`0x14c190` killed bits, `0x1ba950`
/// visit deaths, `0x1bbb04` collected flags).
fn spawns_again(lv: &Lv, sim: &Sim, level: u32, k: usize) -> bool {
    let mut save = moby_spawn::SpawnSave::default();
    for &(l, id) in &sim.svc.save.death {
        if l == level && id >= 0 { save.killed[id as usize >> 3] |= 1 << (id & 7); }
    }
    for &id in &sim.svc.save.death_level { save.visit_death.insert(id as i32); }
    for (&id, &f) in &sim.svc.save.collected { save.id_flags.insert(id as i32, f); }
    moby_spawn::loader_spawns(&lv.instances[k..k + 1], &mut save)[0].spawn
}

/// The placed instance of a live moby with a spawn id.
fn instance_of(lv: &Lv, sim: &Sim, id: MobyId) -> Option<usize> {
    let m = &sim.game.mobys.mobys[id];
    (m.spawn_id >= 0).then(|| lv.instances.iter().position(|i| i.o_class as i16 == m.o_class && i.spawn_id == m.spawn_id as i32)).flatten()
}

#[test]
fn novalis_breaks_every_breakable_kind() {
    let Some(lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let mut sim = Sim::new(&lv, 1);
    // The template: a pot (four shards + remains) and a box (one piece + remains), bolts from BreakFxA.
    for (oc, piece, n) in [(754i16, 1817i16, 4usize), (1813, 1815, 1)] {
        let id = sim.of_class(oc)[0];
        let (p0, r0, b0) = (sim.count(piece), sim.count(1816), sim.bolts());
        sim.wrench(id, 2);
        assert!(!sim.alive(id), "class {oc} deleted");
        assert_eq!(sim.count(piece), p0 + n, "class {oc} pieces");
        assert_eq!(sim.count(1816), r0 + 1, "class {oc} remains");
        let got = sim.bolts() - b0;
        assert!((4..=7).contains(&got) || got > 0, "class {oc}: {got} bolts");
        // The template sets no death bits: the game creates the prop again on the next visit.
        assert!(!sim.svc.save.death.iter().any(|&(l, s)| l == 1 && s == sim.game.mobys.mobys[id].spawn_id && s >= 0));
        eprintln!("class {oc}: {got} bolt mobys");
    }
    // Far from Ratchet (> 7): no bolts (`BoltBurst` with flags 0).
    let pots = sim.of_class(754);
    let id = pots[0];
    let at = sim.game.mobys.mobys[id].position;
    sim.game.hero.pos = [Pf::f(at[0] + 30.0), Pf::f(at[1]), Pf::f(at[2]), Pf::ONE];
    let b0 = sim.bolts();
    let cam = [at[0] + 5.0, at[1], at[2] + 2.0];
    sim.hit(id, &HitTemplate { flags: 0x1_0000, damage: Pf::ONE, ..Default::default() });
    sim.tick(cam);
    sim.tick(cam);
    assert!(!sim.alive(id));
    assert_eq!(sim.bolts(), b0, "no bolts beyond 7 units");
    // A bolt crate: its placed bolts, and its death bits keep it broken.
    let crate_ = sim.of_class(500).into_iter().find(|&i| sim.game.mobys.mobys[i].b4 > 0 && sim.game.mobys.mobys[i].spawn_id >= 0).expect("a bolt crate with bolts");
    let k = instance_of(&lv, &sim, crate_).expect("instance");
    let b0 = sim.bolts();
    sim.wrench(crate_, 3);
    assert!(!sim.alive(crate_) || sim.game.mobys.mobys[crate_].state != 1, "crate broken");
    assert!(sim.bolts() > b0, "crate bolts");
    assert!(!spawns_again(&lv, &sim, 1, k), "a broken crate stays broken");
    // The rock 704 (Novalis's own mechanism): its death bits too.
    let rock = sim.of_class(704)[0];
    let k = instance_of(&lv, &sim, rock);
    sim.wrench(rock, 1);
    assert!(!sim.alive(rock));
    // Its death bits are set; whether the loader reads them is the placement's spawn flags (0: always created).
    assert!(sim.svc.save.death.contains(&(1, sim.game.mobys.mobys[rock].spawn_id)) || sim.game.mobys.mobys[rock].spawn_id < 0);
    eprintln!("rock: spawn flags {:#x}", k.map_or(-1, |k| lv.instances[k].spawn_flags));
    if let Some(k) = k.filter(|&k| lv.instances[k].spawn_flags & 3 == 0 && lv.instances[k].spawn_flags & 0xc != 0) { assert!(!spawns_again(&lv, &sim, 1, k), "a broken rock stays broken"); }
}

/// Every level that places a template breakable: break the first instance of each class, check the bolts, the
/// pieces, the remains, the kept-broken death bits, and that two runs make the same draws.
#[test]
fn every_placed_breakable_breaks_on_its_level() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut broke = 0;
    for level in 0..19u32 {
        let Some(p) = ports(level) else { continue };
        let lv = load(level).unwrap();
        let mut todo: Vec<(i16, usize)> = Vec::new();
        for e in overlay(level).unwrap().vtbl() {
            if let Some(ClassUpdate::Breakable(i)) = p.get(e.o_class as i16) { todo.push((e.o_class as i16, i as usize)); }
        }
        if todo.is_empty() { continue; }
        let mut states = Vec::new();
        for run in 0..2 {
            let mut sim = Sim::new(&lv, level);
            for &(oc, i) in &todo {
                let Some(&id) = sim.of_class(oc).first() else {
                    if run == 0 { eprintln!("level {level:02} class {oc}: none created on a first visit"); }
                    continue;
                };
                let r = &RECIPES[i];
                let (pieces, rem) = made_classes(i);
                let p0: usize = pieces.iter().map(|&c| sim.count(c)).sum();
                let r0 = rem.map(|c| sim.count(c)).unwrap_or(0);
                let b0 = sim.bolts();
                let k = instance_of(&lv, &sim, id);
                sim.wrench(id, 2);
                assert!(!sim.alive(id), "level {level:02} class {oc}: deleted");
                let got_p: usize = pieces.iter().map(|&c| sim.count(c)).sum::<usize>() - p0;
                let min_p: usize = r.pieces.iter().map(|x| match *x { Piece::Single(_) => 1, Piece::Ring4(_) => 4, Piece::Burst { na, .. } => na as usize }).sum();
                assert!(got_p >= min_p, "level {level:02} class {oc}: {got_p} pieces, at least {min_p}");
                // Remains whose class runs `FxGroupUpdate` (Blarg's 1656 / 1661 / 1665) are deleted by their first update
                // (its state 0, the tick after the break): those breaks leave nothing.
                if let Some(c) = rem {
                    if p.get(c) == Some(ClassUpdate::FxPiece) {
                        assert!(sim.count(c) > r0, "level {level:02} class {oc}: remains {c} made");
                        let at = sim.game.hero.pos.map(|v| v.to_f32());
                        sim.tick([at[0] + 3.0, at[1], at[2] + 2.0]);
                        assert_eq!(sim.count(c), r0, "level {level:02} class {oc}: remains {c} gone");
                    } else {
                        assert!(sim.count(c) > r0 || c == oc, "level {level:02} class {oc}: remains {c}");
                    }
                }
                let got_b = sim.bolts() - b0;
                assert!(got_b > 0, "level {level:02} class {oc}: bolts");
                if let (true, Some(k)) = (r.keep_broken, k.filter(|&k| lv.instances[k].spawn_flags & 3 == 0 && lv.instances[k].spawn_flags & 0xc != 0)) {
                    assert!(!spawns_again(&lv, &sim, level, k), "level {level:02} class {oc}: kept broken");
                }
                if run == 0 {
                    eprintln!("level {level:02} class {oc:5}: {got_p} pieces (template min {min_p}), {got_b} bolt mobys, remains {rem:?}");
                    broke += 1;
                }
            }
            states.push(sim.game.rng.state);
        }
        assert_eq!(states[0], states[1], "level {level:02}: two runs, same draws");
    }
    assert!(broke >= 10, "{broke} breakable classes broken");
}
