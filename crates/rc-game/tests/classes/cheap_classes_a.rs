//! The census "cheap win" units whose first copy is on levels 00–08 (class_census.md, G-CLS-027;
//! `rc_game::moby_update::classes::units`): each unit resolves to its port on its levels (and nowhere else), and runs
//! headless on one of them, covering the behaviour and the side effects its coverage table marks ported. Skipped when
//! `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_a:: --nocapture` prints the per-unit survey.

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::draw_callbacks::Callback;
use rc_game::moby_update::classes::units::{self, empty, lamp, loose_piece};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar as p, HitTemplate, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use std::collections::HashMap;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// (level, class, created instances).
type Row = (u32, i16, usize);

/// (unit, [(level, class, created instances)]): the census's rows (`rc-trace class-census`, classes.tsv).
const EXPECTED: &[(&str, &[Row])] = &[
    ("U27", &[(0, 1060, 21), (2, 1060, 17), (5, 1060, 25), (18, 1060, 271)]),
    ("U99", &[(2, 690, 1), (2, 789, 1), (5, 1140, 15), (6, 87, 1), (6, 1672, 5), (7, 1104, 5), (10, 87, 1), (12, 346, 1), (12, 1278, 5), (12, 1280, 1), (14, 765, 1), (15, 283, 6), (15, 419, 1), (16, 1140, 22), (18, 419, 1)]),
    ("U268", &[(8, 344, 0), (8, 547, 8), (8, 548, 4), (8, 549, 6), (8, 550, 13), (8, 551, 10), (8, 588, 4), (8, 589, 4), (8, 590, 4), (8, 591, 4), (8, 592, 4), (8, 593, 4), (8, 594, 4), (8, 595, 4), (8, 596, 4), (8, 597, 4), (8, 598, 4), (8, 782, 0), (8, 783, 0), (8, 784, 0), (8, 785, 0)]),
    ("U95", &[(2, 652, 13), (2, 653, 4), (12, 296, 25)]),
    ("U241", &[(7, 886, 21), (12, 886, 9)]),
    ("U247", &[(7, 1063, 1), (7, 1078, 1), (7, 1079, 2), (7, 1131, 1), (13, 129, 8), (13, 130, 8), (13, 182, 3), (13, 183, 3), (13, 360, 1)]),
    ("U229", &[(6, 1512, 26)]),
    ("U280", &[(8, 621, 25)]),
    ("U185", &[(5, 852, 14), (5, 853, 7)]),
    ("U221", &[(6, 1091, 1), (6, 1092, 1), (6, 1093, 4), (6, 1094, 4), (6, 1095, 3), (6, 1096, 3), (6, 1097, 1), (6, 1098, 3), (6, 1103, 1)]),
    ("U139", &[(3, 915, 5), (3, 916, 4), (3, 917, 5), (10, 915, 5)]),
    ("U281", &[(8, 648, 15), (10, 648, 6)]),
    ("U170", &[(5, 341, 3), (7, 341, 5), (11, 341, 1), (12, 341, 6), (18, 341, 2)]),
    ("U204", &[(6, 367, 8)]),
];

/// Every unit's classes resolve to the unit on its levels, with the census's created counts, and a unit's port runs
/// on no level the census does not list for it (beyond the empty update's unplaced classes).
#[test]
fn units_resolve_on_their_levels() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut addrs: HashMap<u32, ClassUpdate> = HashMap::new();
    for u in ClassUpdate::every() {
        if let Some(o) = addrs.insert(u.key(), u) { panic!("{u:?} and {o:?} share 0x{:x}", u.key()); }
    }
    let mut total = 0;
    let mut extra = Vec::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let ov = overlay(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(name, rows) in EXPECTED {
            let u = unit(name);
            for e in ov.vtbl() {
                let oc = e.o_class as i16;
                let listed = rows.iter().find(|r| r.0 == level && r.1 == oc);
                let created = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                if let Some(r) = listed {
                    assert_eq!(p.get(oc), Some(u), "{name}: level {level:02} class {oc}");
                    assert_eq!(created, r.2, "{name}: level {level:02} class {oc} created");
                    total += created;
                } else if p.get(oc) == Some(u) {
                    // The same code also runs classes the census does not list: only unplaced ones (e.g. 980 on 05 runs
                    // the loose piece; the empty update's never-placed 433 / 618 / 1289 …).
                    assert_eq!(created, 0, "{name}: level {level:02} class {oc} also runs it");
                    extra.push((name, level, oc));
                }
            }
        }
    }
    eprintln!("created instances of the levels 00–08 units now ported: {total}; unplaced classes running them too: {extra:?}");
}

// ---------------------------------------------------------------------------------------------------
// Headless levels

struct Lv {
    table: MobyTable,
    classes: ClassTable,
    svc: Services,
    sched: Scheduler,
    rng: Rng,
    mesh: collision::Collision,
    counter: u64,
}

fn load(level: u32) -> Option<Lv> {
    let core = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let ports = ports(level)?;
    let mesh = collision::parse_collision(&core.core, &core.data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4).max(1) as usize;
    let mut classes = ClassTable::default();
    let mut joints = HashMap::new();
    for (slot, e) in core.core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = core.block(&format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(blob, &c).unwrap_or_default());
            let mut info = class_info(&c, slot as u8, ports.update_fn(oc));
            info.seq0 = anim.sequence(0).map(|q| rc_game::moby_runtime::Seq0Info { frame_count: q.header.frame_count, loop_sound_bit7: q.header.loop_sound & 0x80 != 0 });
            if ports.get(oc).is_some_and(|u| u.needs_joint_lists()) {
                joints.insert(oc, (0..16).map_while(|l| gadget::joint_list(blob, &c.header, l).ok().map(|(a, _)| a)).collect());
            }
            classes.classes.insert(oc, (info, Some(anim)));
        } else {
            let info = rc_game::moby_runtime::ClassInfo { slot: slot as u8, no_header: true, update_fn: ports.update_fn(oc), ..Default::default() };
            classes.classes.entry(oc).or_insert((info, None));
        }
    }
    let statics = load_level_mobys(&instances, &mut classes, &pvars, &tests);
    let mut table = MobyTable::new(statics.mobys.clone(), spawnable);
    if let Some(h) = table.mobys.iter().position(|m| m.o_class == 0) { table.mobys[h].mode |= mode::NO_UPDATE; }
    let mut svc = Services::new();
    svc.level = level;
    svc.set_splines(&gameplay::parse_splines(&gp).unwrap());
    svc.pvar_shared = gameplay::parse_pvar_shared_data(&gp).unwrap();
    svc.groups = statics.groups(&gp);
    svc.set_moby_collision(rc_formats::moby_collision::parse_level(&core.core, &core.data).unwrap());
    svc.set_volumes(rc_formats::volumes::parse_volumes(&gp).unwrap());
    svc.joint_lists = joints;
    svc.build_grid(&mut table);
    Some(Lv { table, classes, svc, sched: Scheduler::new(), rng: Rng::new(), mesh, counter: 0 })
}

impl Lv {
    fn world<'a>(&'a mut self, hero: &'a Hero) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.camera = hero.pos;
        w
    }
    fn load_pass(&mut self, hero: &Hero) {
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.load_pass(&mut w); }
        self.sched = sched;
    }
    fn tick(&mut self, hero: &Hero) {
        self.counter += 1;
        let c = self.counter;
        self.table.free_slot_pass(c);
        self.svc.draw_callbacks = Default::default();
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.tick(&mut w); }
        self.sched = sched;
    }
    /// One update of `id` alone (its port), outside the scheduler's active set.
    fn run(&mut self, hero: &Hero, id: MobyId) {
        let u = rc_game::moby_update::scheduler::ported(self.table.mobys[id].update_fn.unwrap()).unwrap();
        let mut w = self.world(hero);
        rc_game::moby_update::classes::dispatch(u, &mut w, id);
    }
    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
}

fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h
}

fn pos3(lv: &Lv, id: MobyId) -> [f32; 3] { let p = lv.table.mobys[id].position; [p[0], p[1], p[2]] }

// ---------------------------------------------------------------------------------------------------
// U27 lamps

#[test]
fn lamps_rilgar_pulse_and_register_one_glow_per_group() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let lamps = lv.of_class(1060);
    assert_eq!(lamps.len(), 25);
    let h = hero_at(pos3(&lv, lamps[0]));
    lv.load_pass(&h);
    lv.tick(&h);
    lv.tick(&h);
    let groups: std::collections::HashSet<i8> = lamps.iter().map(|&l| lv.table.mobys[l].group).filter(|&g| g >= 0).collect();
    for &l in &lamps {
        let m = &lv.table.mobys[l];
        assert_eq!(m.state, 1);
        let rgba = p::u32(&m.pvars, 4);
        assert_eq!(rgba >> 24, 0x38, "Rilgar's alpha");
        assert_eq!(m.ambient[..3], [rgba as u8, (rgba >> 16) as u8, 0], "ambient = (red, blue, 0) of the tween");
    }
    let regs: Vec<_> = lv.svc.draw_callbacks.list1.iter().filter(|(c, _)| matches!(c, Callback::UnitGlow(_))).collect();
    assert_eq!(regs.len(), groups.len(), "one glow callback per group and tick");
    // The group's shared word holds the tick.
    let word = p::i32(&lv.table.mobys[lamps[0]].pvars, 8);
    assert_eq!(lv.svc.shared_i32(word), lv.counter as i32);
    // The callback's quads: one per visible lamp of the group.
    for &l in &lamps { lv.table.mobys[l].visible = 1; }
    let (cb, id) = *regs[0];
    let Callback::UnitGlow(i) = cb else { unreachable!() };
    let q = units::glow_quads(&lv.table, &lv.svc, i, id);
    let g = lv.table.mobys[id].group;
    assert_eq!(q.len(), lamps.iter().filter(|&&l| lv.table.mobys[l].group == g).count());
    assert!(q.iter().all(|q| q.size == lamp::glow_size(5) && q.pull == 0.5));
    eprintln!("Rilgar lamps: {} in groups {groups:?}", lamps.len());
}

// ---------------------------------------------------------------------------------------------------
// U99 the empty update

#[test]
fn empty_update_classes_are_updated_not_parked() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let ids = lv.of_class(1140);
    assert_eq!(ids.len(), 15);
    for &i in &ids {
        let m = &lv.table.mobys[i];
        assert_eq!(m.update_fn, Some(empty::UPDATE_FN));
        assert_eq!(m.mode & mode::NO_UPDATE, 0, "an update function: not parked with mode 2");
    }
    let h = hero_at(pos3(&lv, ids[0]));
    lv.load_pass(&h);
    let before: Vec<_> = ids.iter().map(|&i| lv.table.mobys[i].clone()).collect();
    lv.run(&h, ids[0]);
    assert_eq!(lv.table.mobys[ids[0]].pvars, before[0].pvars);
    assert_eq!(lv.table.mobys[ids[0]].state, before[0].state);
}

// ---------------------------------------------------------------------------------------------------
// U268 loose pieces

#[test]
fn loose_pieces_batalia_rest_then_tumble_fade_and_go() {
    let Some(mut lv) = load(8) else { eprintln!("skipped"); return };
    let pieces: Vec<MobyId> = loose_piece::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(pieces.len(), 85);
    let h = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&h);
    assert!(pieces.iter().all(|&i| lv.table.mobys[i].state == 2), "resting");
    let id = pieces[0];
    let z0 = lv.table.mobys[id].position[2];
    // Set loose (the mover's part): up and sideways, spinning.
    lv.table.mobys[id].state = 1;
    p::set_v4f(&mut lv.table.mobys[id].pvars, 0x30, [0.05, 0.0, 0.2, 0.0]);
    p::set_v4f(&mut lv.table.mobys[id].pvars, 0x20, [0.01, 0.02, 0.03, 0.0]);
    lv.run(&h, id);
    assert!(lv.table.mobys[id].position[2] > z0, "rises");
    let mut t = 0;
    while lv.table.mobys[id].state < 0x80 && t < 2000 { lv.run(&h, id); t += 1; }
    eprintln!("Batalia loose piece {id}: gone after {t} ticks, alpha {}", lv.table.mobys[id].alpha);
    assert!(lv.table.mobys[id].state >= 0x80, "faded out or fell below z 5");
}

// ---------------------------------------------------------------------------------------------------
// U95 conveyors

#[test]
fn conveyors_hoven_carry_their_riders() {
    let Some(mut lv) = load(12) else { eprintln!("skipped"); return };
    let belts = lv.of_class(296);
    assert_eq!(belts.len(), 25);
    let h = hero_at(pos3(&lv, belts[0]));
    lv.load_pass(&h);
    for &b in &belts { lv.run(&h, b); }
    let one_way: Vec<_> = belts.iter().filter(|&&b| lv.table.mobys[b].state == 3).copied().collect();
    let reversing: Vec<_> = belts.iter().filter(|&&b| lv.table.mobys[b].state == 1).copied().collect();
    assert_eq!((one_way.len(), reversing.len()), (12, 13));
    let b = one_way[0];
    let yaw = lv.table.mobys[b].rotation[2];
    let d = rc_game::moby_update::triggers::platform_delta(&lv.table.mobys[b]).unwrap();
    let k = 2.5 / 60.0;
    assert!((d.displacement[0] - yaw.cos() * k).abs() < 1e-6 && (d.displacement[1] - yaw.sin() * k).abs() < 1e-6, "{d:?}");
    assert_eq!(p::u32(&lv.table.mobys[b].pvars, 0x9c) & 4, 4);
    // A reversing belt eases up, then (after 180 ticks) down and back the other way.
    let r = reversing[0];
    let mut speeds = Vec::new();
    for _ in 0..400 { lv.run(&h, r); speeds.push(lv.table.mobys[r].anim.speed); }
    assert!(speeds.iter().any(|&s| s < 0.0) && speeds.iter().any(|&s| s > 0.0), "both directions");
}

// ---------------------------------------------------------------------------------------------------
// U241 timed switches

#[test]
fn timed_switches_umbris_press_and_time_out() {
    let Some(mut lv) = load(7) else { eprintln!("skipped"); return };
    let sw = lv.of_class(886);
    assert_eq!(sw.len(), 21);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    for &s in &sw { lv.run(&far, s); }
    let g = lv.table.mobys[sw[0]].group;
    let group: Vec<_> = sw.iter().filter(|&&s| lv.table.mobys[s].group == g).copied().collect();
    assert!(group.iter().all(|&s| lv.table.mobys[s].state == 1 && lv.table.mobys[s].ambient[..3] == [0x80; 3]));
    let mut on = far.clone();
    on.ground_moby = Some(group[0]);
    lv.run(&on, group[0]);
    let m = &lv.table.mobys[group[0]];
    assert_eq!((m.cmd, m.ambient[1], p::i32(&m.pvars, 0), p::i32(&m.pvars, 8)), (1, 0xff, 15 * 60, 1));
    assert!(group.iter().all(|&s| lv.table.mobys[s].state == 2), "the group moves on");
    assert_eq!(lv.svc.sounds.last().map(|s| s.index), Some(3));
    // Let the countdown run out: sound 2 and the group back to 1.
    for _ in 0..(15 * 60 + 2) { lv.run(&far, group[0]); }
    assert!(lv.svc.sounds.iter().any(|s| s.index == 2));
    assert!(lv.svc.sounds.iter().filter(|s| s.index == 0).count() > 10, "the ticking");
    for &s in &group[1..] { lv.run(&far, s); }
    assert!(group.iter().all(|&s| lv.table.mobys[s].state == 1), "{:?}", group.iter().map(|&s| lv.table.mobys[s].state).collect::<Vec<_>>());
    eprintln!("Umbris timed switches: {} in {} groups", sw.len(), sw.iter().map(|&s| lv.table.mobys[s].group).collect::<std::collections::HashSet<_>>().len());
}

// ---------------------------------------------------------------------------------------------------
// U247 linked movers

#[test]
fn linked_movers_gemlik_follow_their_link() {
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let movers = lv.of_class(129);
    assert_eq!(movers.len(), 8);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    let m = movers[0];
    lv.run(&far, m);
    let link = p::i32(&lv.table.mobys[m].pvars, 0x10);
    assert!(link >= 0, "a runtime moby link");
    let (a, b) = (p::i32(&lv.table.mobys[m].pvars, 0x14) as u8, p::i32(&lv.table.mobys[m].pvars, 0x18) as u8);
    let home = lv.table.mobys[m].position;
    lv.table.mobys[link as usize].state = a;
    lv.run(&far, m);
    assert_eq!(lv.table.mobys[m].state, 2);
    for _ in 0..600 { lv.run(&far, m); if lv.table.mobys[m].state == 3 { break; } }
    assert_eq!(lv.table.mobys[m].state, 3, "reached the far end");
    let t = p::v4f(&lv.table.mobys[m].pvars, 0x1c);
    let moved = [0, 1, 2].map(|k| lv.table.mobys[m].position[k] - home[k]);
    assert!((rc_game::moby_update::creature::len3([moved[0], moved[1], moved[2], 0.0]) - rc_game::moby_update::creature::len3([t[0], t[1], t[2], 0.0])).abs() < 1e-4);
    lv.table.mobys[link as usize].state = b;
    for _ in 0..600 { lv.run(&far, m); if lv.table.mobys[m].state == 1 { break; } }
    // Home again (+0x1c gets the pvar's travelled distance: the 16-byte copy of the home record, as the game).
    assert_eq!((lv.table.mobys[m].state, lv.table.mobys[m].position[..3].to_vec()), (1, home[..3].to_vec()), "home again");
    eprintln!("Gemlik mover {m}: link {link}, A {a}, B {b}, travel {t:?}, sounds {}", lv.svc.sounds.len());
}

// ---------------------------------------------------------------------------------------------------
// U229 vents

#[test]
fn vents_kalebo_blow_trail_blobs() {
    let Some(mut lv) = load(6) else { eprintln!("skipped"); return };
    let vents = lv.of_class(1512);
    assert_eq!(vents.len(), 26);
    let h = hero_at(pos3(&lv, vents[0]));
    lv.load_pass(&h);
    for &v in &vents { lv.run(&h, v); }
    let states: Vec<u8> = vents.iter().map(|&v| lv.table.mobys[v].state).collect();
    assert!(states.iter().all(|&s| s == 3 || s == 4 || s == 1));
    let before = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
    for _ in 0..120 { for &v in &vents { lv.run(&h, v); } }
    let n = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) - before;
    eprintln!("Kalebo vents: states {states:?}, {n} trail blobs in 120 ticks");
    assert!(n > 0);
}

// ---------------------------------------------------------------------------------------------------
// U280 rail mines

#[test]
fn rail_mines_batalia_blow_on_a_hit_or_a_grind() {
    let Some(mut lv) = load(8) else { eprintln!("skipped"); return };
    let mines = lv.of_class(621);
    assert_eq!(mines.len(), 25);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    for &m in &mines { lv.run(&far, m); }
    assert!(mines.iter().all(|&m| lv.table.mobys[m].state == 1));
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags: 0x10_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::ONE, w20: 0 };
    {
        let mut w = lv.world(&far);
        w.deliver_hit(mines[0], &tmpl);
    }
    lv.run(&far, mines[0]);
    assert!(lv.table.mobys[mines[0]].state >= 0x80);
    let p1 = pos3(&lv, mines[1]);
    let mut grinding = hero_at([p1[0] + 1.0, p1[1], p1[2]]);
    grinding.state = 0x42;
    lv.run(&grinding, mines[1]);
    assert!(lv.table.mobys[mines[1]].state >= 0x80);
    assert_eq!(lv.svc.sounds.iter().filter(|s| s.index == 0).count(), 2);
    assert!(lv.svc.fx.part_spawns.get(&15).copied().unwrap_or(0) >= 20, "the beam explosion's streaks");
}

// ---------------------------------------------------------------------------------------------------
// U185 rising blocks, U221 bobbing blocks, U139 markers, U281 smoke emitters, U170 Hydrodisplacer pads

#[test]
fn rising_blocks_rilgar_rise_on_their_link() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let blocks: Vec<MobyId> = [852, 853].iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(blocks.len(), 21);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    let b = blocks[0];
    let placed = lv.table.mobys[b].position[2] + 8.0;
    lv.run(&far, b);
    assert_eq!(lv.table.mobys[b].state, 1);
    let link = p::i32(&lv.table.mobys[b].pvars, 0);
    assert!(link >= 0);
    lv.table.mobys[link as usize].cmd = 1;
    lv.run(&far, b);
    assert_eq!(lv.table.mobys[b].state, 2);
    for _ in 0..600 { lv.run(&far, b); }
    let m = &lv.table.mobys[b];
    eprintln!("Rilgar rising block {b}: link {link}, z {} (placed {placed}), state {}", m.position[2], m.state);
    assert_eq!((m.state, m.rotation[1]), (3, 0.0));
    assert!((m.position[2] - (placed + 1.0)).abs() < 0.5, "risen to 1 above its placed height");
}

#[test]
fn bobbing_blocks_kalebo_bob_in_step() {
    let Some(mut lv) = load(6) else { eprintln!("skipped"); return };
    let blocks: Vec<MobyId> = units::bob_block::CLASSES.iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(blocks.len(), 21);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    let z0: Vec<f32> = blocks.iter().map(|&b| lv.table.mobys[b].position[2]).collect();
    lv.counter = 150;
    for &b in &blocks { lv.run(&far, b); }
    for (k, &b) in blocks.iter().enumerate() { assert!((lv.table.mobys[b].position[2] - (z0[k] + 3.0)).abs() < 1e-3, "block {b}: the top of the bob"); }
    let mut held = far.clone();
    held.state = 0x72;
    for &b in &blocks { lv.run(&held, b); }
    for (k, &b) in blocks.iter().enumerate() { assert_eq!(lv.table.mobys[b].position[2], z0[k]); }
}

#[test]
fn markers_kerwan_are_deleted_by_their_first_update() {
    let Some(mut lv) = load(3) else { eprintln!("skipped"); return };
    let ids: Vec<MobyId> = [915, 916, 917].iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(ids.len(), 14);
    let h = hero_at([0.0, 0.0, 0.0]);
    lv.load_pass(&h);
    assert!(ids.iter().all(|&i| lv.table.mobys[i].state >= 0x80), "the load pass runs each once: all gone");
}

#[test]
fn smoke_emitters_batalia_puff_on_their_period() {
    let Some(mut lv) = load(8) else { eprintln!("skipped"); return };
    let ids = lv.of_class(648);
    assert_eq!(ids.len(), 15);
    let h = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&h);
    let before = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0);
    for _ in 0..240 { for &i in &ids { lv.run(&h, i); } }
    let n = lv.svc.fx.part_spawns.get(&2).copied().unwrap_or(0) - before;
    let periods: Vec<i32> = ids.iter().map(|&i| if p::i32(&lv.table.mobys[i].pvars, 0x38) != 0 { 60 } else { p::i32(&lv.table.mobys[i].pvars, 0x14) }).collect();
    let expect: u64 = periods.iter().map(|&t| (240 / t.max(1)) as u64).sum();
    eprintln!("Batalia smoke: periods {periods:?}, {n} blobs in 240 ticks (≈ {expect})");
    assert!(n + ids.len() as u64 >= expect && n <= expect + 2 * ids.len() as u64);
}

#[test]
fn hydro_pads_rilgar_glow_under_ratchet() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let pads = lv.of_class(341);
    assert_eq!(pads.len(), 3);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    let pad = pads[0];
    lv.run(&far, pad);
    assert_eq!(lv.table.mobys[pad].glow, 0x8080_8080);
    let mut on = far.clone();
    on.ground_moby = Some(pad);
    for _ in 0..40 { lv.run(&on, pad); }
    assert_eq!(p::ff(&lv.table.mobys[pad].pvars, 0x1c), 1.0);
    assert_ne!(lv.table.mobys[pad].glow, 0x8080_8080);
    // The link's bit 0 raises the pad (sequence 1).
    let link = p::i32(&lv.table.mobys[pad].pvars, 0);
    eprintln!("Rilgar pad {pad}: link {link}, seq {}", lv.table.mobys[pad].anim.seq_b);
    if link >= 0 {
        lv.table.mobys[link as usize].cmd = 1;
        lv.run(&far, pad);
        assert_eq!(lv.table.mobys[pad].anim.seq_b, 1);
    }
}

#[test]
fn sliders_kalebo_slide_to_their_target() {
    let Some(mut lv) = load(6) else { eprintln!("skipped"); return };
    let ids = lv.of_class(367);
    assert_eq!(ids.len(), 8);
    let far = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&far);
    let s = ids[0];
    assert_eq!(lv.table.mobys[s].state, 1);
    let home = p::v4f(&lv.table.mobys[s].pvars, 0);
    let end = p::v4f(&lv.table.mobys[s].pvars, 0x10);
    let d = [0, 1, 2].map(|k| end[k] - home[k]);
    assert!(((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - 11.0).abs() < 1e-3, "11 along its second row");
    lv.table.mobys[s].state = 2;
    p::set_ff(&mut lv.table.mobys[s].pvars, 0x28, 1.0);
    for _ in 0..300 { lv.run(&far, s); }
    assert_eq!(lv.table.mobys[s].state, 1);
    assert_eq!(lv.table.mobys[s].position[..3], end[..3]);
}
