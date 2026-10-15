//! The census "cheap win" units of round 2 (class_census.md "In the port: cheap wins round 2", G-CLS-027;
//! `rc_game::moby_update::classes::units`): each unit resolves to its port on exactly its census levels with the
//! census's created counts, and runs headless on one of them, covering the behaviour and the side effects its
//! coverage table marks ported. Skipped when `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_c:: --nocapture` prints the per-unit survey.

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::units::{self, kalebo_barrier, orb_holder, quartu_alarm, swing_laser};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar as p, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use std::collections::HashMap;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// (unit row, class, the levels whose table runs it, created instances on them; 0: not checked).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[
    ("U470 77", 77, &[15, 17], 36),
    ("U480 408", 408, &[15, 17], 48),
    ("U474 123", 123, &[15, 17], 46),
    ("U411 127", 127, &[13], 0),
    ("U411 127", 128, &[13], 0),
    ("U411 127", 159, &[13], 0),
    ("U411 127", 169, &[13], 0),
    ("U215 1038", 1038, &[6, 10, 17], 21),
    ("U215 orb", 1040, &[6, 10, 17], 0),
    ("U503 552", 552, &[16], 20),
    ("U502 546", 546, &[16], 6),
    ("U514 1387", 1387, &[16], 4),
    ("U185 843", 843, &[5], 16),
    ("U473 93", 93, &[15], 12),
    ("U307 1172", 1172, &[9], 10),
    ("U477 196", 196, &[15], 0),
    ("U477 196", 197, &[15, 17], 0),
    ("U477 196", 1958, &[17], 0),
];

/// Created instances per unit (all its classes and levels), the census's counts.
const TOTALS: &[(&str, usize)] = &[("U470 77", 36), ("U480 408", 48), ("U474 123", 46), ("U411 127", 24), ("U215 1038", 21), ("U477 196", 10)];

/// Every unit's classes resolve to the unit on its levels (and to nothing else), with the census's created counts;
/// the unit addresses are distinct from every other port's (the scheduler maps a moby's update address back).
#[test]
fn units_resolve_on_their_levels() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut addrs: HashMap<u32, ClassUpdate> = HashMap::new();
    for u in ClassUpdate::every() {
        if let Some(o) = addrs.insert(u.key(), u) { panic!("{u:?} and {o:?} share 0x{:x}", u.key()); }
    }
    let mut created: HashMap<(&str, i16), usize> = HashMap::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(name, oc, levels, _) in EXPECTED {
            let u = unit(name);
            if levels.contains(&level) {
                assert_eq!(p.get(oc), Some(u), "level {level:02} class {oc}");
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                *created.entry((name, oc)).or_default() += n;
            } else if p.in_table(oc) {
                assert_ne!(p.get(oc), Some(u), "level {level:02} class {oc}");
            }
        }
    }
    eprintln!("created instances now ported: {created:?}");
    for &(name, oc, _, n) in EXPECTED { if n > 0 { assert_eq!(created[&(name, oc)], n, "{name} class {oc}"); } }
    let mut by_unit: HashMap<&str, usize> = HashMap::new();
    for (&(name, _), &n) in &created { *by_unit.entry(name).or_default() += n; }
    eprintln!("by unit: {by_unit:?}");
    for &(name, n) in TOTALS { assert_eq!(by_unit[name], n, "{name}"); }
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
    missions: rc_game::moby_update::services::LevelMissions,
    /// The particle system (the type-60 sparks and the explosions' records land in `svc.fx.part_spawns`).
    parts: rc_game::particles::Particles,
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
            if ports.needs_joint_lists(oc) {
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
    svc.groups = statics.groups(&gp);
    svc.set_moby_collision(rc_formats::moby_collision::parse_level(&core.core, &core.data).unwrap());
    svc.set_volumes(rc_formats::volumes::parse_volumes(&gp).unwrap());
    svc.pvar_shared = gameplay::parse_pvar_shared_data(&gp).unwrap();
    svc.joint_lists = joints;
    svc.build_grid(&mut table);
    Some(Lv { table, classes, svc, sched: Scheduler::new(), rng: Rng::new(), mesh, counter: 0, missions: rc_game::moby_update::services::LevelMissions::fresh_load(level, [0xff; 16]), parts: rc_game::particles::Particles::new(None, Vec::new()) })
}

impl Lv {
    fn world<'a>(&'a mut self, hero: &'a Hero) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.camera = hero.pos;
        w.missions = &self.missions;
        w.particles = Some(&mut self.parts);
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
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.tick(&mut w); }
        self.sched = sched;
    }
    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
    /// Runs one moby's update directly (a state forced by the test).
    fn run(&mut self, hero: &Hero, id: MobyId, u: ClassUpdate) {
        let mut w = self.world(hero);
        rc_game::moby_update::classes::dispatch(u, &mut w, id);
    }
}

fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h.body_point = rc_game::hero::physics::v4(p[0], p[1], p[2] + 0.7);
    h.shadow_point = h.pos;
    h
}


// ---------------------------------------------------------------------------------------------------
// U480 / U470: Quartu's alarms 408 and the alarm drones 77 they release

#[test]
fn quartu_alarm_releases_drones_that_home_and_explode() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let drones = lv.of_class(77);
    let alarms = lv.of_class(408);
    assert_eq!((drones.len(), alarms.len()), (20, 24));
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for _ in 0..3 { lv.tick(&far); }
    for &d in &drones {
        let m = &lv.table.mobys[d];
        assert_eq!((m.state, m.has_collision), (1, false), "drone {d} rests");
        assert!(m.pvars.len() >= 0x70);
    }
    let kinds: Vec<i32> = alarms.iter().map(|&a| p::i32(&lv.table.mobys[a].pvars, quartu_alarm::pv::KIND)).collect();
    eprintln!("alarm kinds {kinds:?}, states {:?}", alarms.iter().map(|&a| lv.table.mobys[a].state).collect::<Vec<_>>());
    for (&a, &k) in alarms.iter().zip(&kinds) { assert_eq!(lv.table.mobys[a].state, if k == 2 { 6 } else { 3 }, "alarm {a} kind {k}"); }
    // Set one alarm off with Ratchet 4 units away (in its line of sight): it opens, then releases drones.
    let a = alarms[0];
    let at = lv.table.mobys[a].position;
    let group = p::i32(&lv.table.mobys[a].pvars, quartu_alarm::pv::GROUP);
    let mut near = None;
    for off in [[4.0, 0.0], [-4.0, 0.0], [0.0, 4.0], [0.0, -4.0], [2.0, 2.0], [-2.0, -2.0]] {
        let h = hero_at([at[0] + off[0], at[1] + off[1], at[2]]);
        let w = lv.world(&h);
        let from = [at[0], at[1], at[2] + 1.0, at[3]];
        if w.line(rc_game::moby_update::services::pv(from), h.body_point, 2, None).is_none() { near = Some(h); break; }
    }
    let hero = near.expect("a clear line from the alarm");
    // Ratchet's moby stands where the hero is (the drones' sphere hit finds it).
    if let Some(h) = lv.table.mobys.iter().position(|m| m.o_class == 0) {
        let q = hero.pos.map(|x| f32::from_bits(x.0));
        lv.table.mobys[h].position = [q[0], q[1], q[2], 1.0];
    }
    { let mut w = lv.world(&hero); quartu_alarm::set_off(&mut w, a, 180); }
    let sounds0 = lv.svc.sounds.len();
    let mut released = None;
    let mut states = Vec::new();
    let mut dstates = Vec::new();
    let flashes0 = lv.svc.fx.flashes;
    let (mut fade_max, mut tinted, mut glow) = (0f32, false, None);
    let row = units::PORTS.iter().position(|u| u.unit == "U470 77").unwrap() as u16;
    for _ in 0..240 {
        lv.tick(&hero);
        states.push(lv.table.mobys[a].state);
        if released.is_none() { released = drones.iter().copied().find(|&d| lv.table.mobys[d].state != 1); }
        if let Some(d) = released { dstates.push(lv.table.mobys[d].state); }
        fade_max = fade_max.max(f32::from_bits(lv.svc.units.word(quartu_alarm::lw::FADE)));
        tinted |= quartu_alarm::tint(&lv.svc, lv.counter).is_some();
        // The drones' glow: one registration per group and tick, four quads per live member.
        let regs: Vec<MobyId> = lv.svc.draw_callbacks.list1.iter().filter(|(c, _)| *c == rc_game::moby_update::classes::draw_callbacks::Callback::UnitQuads(row)).map(|&(_, id)| id).collect();
        if glow.is_none() && !regs.is_empty() {
            assert_eq!(regs.len(), 1, "one per group and tick");
            let q = units::fx_quads(&lv.table, &lv.svc, row, regs[0]).unwrap();
            let g = lv.table.mobys[regs[0]].group;
            let live = drones.iter().filter(|&&k| lv.table.mobys[k].group == g && lv.table.mobys[k].state != 1).count();
            glow = Some((q.quads.len(), live));
        }
    }
    states.dedup();
    eprintln!("alarm {a} (group {group}) states {states:?}; released {released:?}; fade max {fade_max}; glow {glow:?}");
    let d = released.expect("a drone released");
    assert!(states.contains(&8), "the alarm ran");
    assert_eq!(lv.table.mobys[d].group as i32, group, "from the alarm's drone group");
    assert!(lv.svc.sounds[sounds0..].iter().any(|s| (s.index, s.flags, s.o_class) == (5, 0x11, 408)), "the alarm loop");
    assert!(fade_max > 0.5 && tinted, "the alarm fade and the tint");
    assert!(lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0) > 0, "the live drones' type-60 sparks");
    let (nq, live) = glow.expect("the glow registered");
    assert!(live > 0 && nq == 4 * live, "{nq} quads for {live} live drones");
    // The released drone rose (2), homed (3), exploded on Ratchet (4) and went back to rest (1).
    dstates.dedup();
    let hit = lv.svc.hits.records.iter().any(|r| r.attacker == Some(d) && r.flags == 0x1_0001);
    eprintln!("drone {d} states {dstates:?}; its sphere hit logged: {hit}");
    assert_eq!(dstates[..4], [2, 3, 4, 1]);
    let m = &lv.table.mobys[d];
    assert!(m.state != 1 || (!m.has_collision && m.mode & mode::HIDDEN != 0));
    assert!(lv.svc.fx.flashes > flashes0, "the death explosion's flashes");
    assert!(hit, "the sphere hit (0x10001) on what it found (Ratchet's moby)");
}

// ---------------------------------------------------------------------------------------------------
// U474: the swinging lasers 123 (Quartu)

#[test]
fn swinging_lasers_move_on_their_paths_and_their_beams_hit() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let lasers = lv.of_class(123);
    assert!(!lasers.is_empty());
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    let row = units::PORTS.iter().position(|u| u.unit == "U474 123").unwrap() as u16;
    let mut kinds = HashMap::new();
    for &l in &lasers {
        let s0 = lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0);
        for _ in 0..30 { lv.run(&far, l, unit("U474 123")); }
        assert!(lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0) >= s0 + 40 * 30, "40 sparks a tick");
        lv.svc.draw_callbacks.list1.clear();
        lv.run(&far, l, unit("U474 123"));
        assert_eq!(lv.svc.draw_callbacks.list1, vec![(rc_game::moby_update::classes::draw_callbacks::Callback::UnitQuads(row), l)]);
        let q = units::fx_quads(&lv.table, &lv.svc, row, l).unwrap();
        assert_eq!((q.quads.len(), q.additive), (1, true));
        let m = &lv.table.mobys[l];
        let kind = p::i32(&m.pvars, swing_laser::pv::KIND);
        *kinds.entry(kind).or_insert(0) += 1;
        let home = p::v4f(&m.pvars, swing_laser::pv::HOME);
        let r = p::ff(&m.pvars, swing_laser::pv::RADIUS);
        let d = ((m.position[0] - home[0]).powi(2) + (m.position[1] - home[1]).powi(2) + (m.position[2] - home[2]).powi(2)).sqrt();
        assert!(m.state == [1, 3, 3, 2, 2][kind as usize] && d <= r + 1e-3, "laser {l} kind {kind} state {} off {d} r {r}", m.state);
        if kind != 1 && kind != 2 { assert!((d - r).abs() < 1e-2, "laser {l} kind {kind}: on its circle ({d} vs {r})"); }
        let end = p::v4f(&m.pvars, swing_laser::pv::END);
        let reach = ((end[0] - m.position[0]).powi(2) + (end[1] - m.position[1]).powi(2) + (end[2] - m.position[2]).powi(2)).sqrt();
        assert!(reach <= swing_laser::REACH + 1e-3, "laser {l}: beam {reach}");
    }
    eprintln!("laser kinds {kinds:?}");
    // The beam through a hittable moby (mode 0x4000, with collision; the laser put 3 units before it on its z row):
    // the template hit (0x10001, damage 1) on it.
    let l = lasers[0];
    let h = lv.table.mobys.iter().position(|m| m.mode & 0x4000 != 0 && m.has_collision && m.o_class != 123 && m.o_class != 0 && m.state < 0x80).expect("a hittable moby");
    let c = lv.table.mobys[h].bsphere.map(|x| x / 1024.0);
    let z = lv.table.mobys[l].rows[2];
    lv.table.mobys[l].position = [c[0] - z[0] * 3.0, c[1] - z[1] * 3.0, c[2] - z[2] * 3.0, 1.0];
    { let mut w = lv.world(&far); swing_laser::beam(&mut w, l); }
    let hit = lv.svc.hits.records.iter().any(|r| r.attacker == Some(l) && r.flags == 0x1_0001 && r.target == h);
    let end = p::v4f(&lv.table.mobys[l].pvars, swing_laser::pv::END);
    eprintln!("moby {h} (class {}) on the beam of {l}: hit {hit}; the beam ends at {end:?}", lv.table.mobys[h].o_class);
    assert!(hit, "the beam's template hit");
}

// ---------------------------------------------------------------------------------------------------
// U411: Gemlik's linked rotators 127 / 128 / 159 / 169

#[test]
fn linked_rotators_turn_on_their_links_state() {
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let rot: Vec<MobyId> = [127, 128, 159, 169].iter().flat_map(|&c| lv.of_class(c)).collect();
    assert_eq!(rot.len(), 24);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for &r in &rot {
        let m = &lv.table.mobys[r];
        assert!(matches!(m.state, 1 | 2), "rotator {r} initialised: state {}", m.state);
        let home = p::v4f(&m.pvars, 0);
        assert_eq!(home[..3], m.rotation[..3]);
    }
    // A linked rotator: turn it fully out and back by driving its link through states B and A.
    let r = *rot.iter().find(|&&r| p::i32(&lv.table.mobys[r].pvars, 0x10) >= 0).expect("a linked rotator");
    let link = p::i32(&lv.table.mobys[r].pvars, 0x10) as usize;
    let (a, b) = (p::i32(&lv.table.mobys[r].pvars, 0x14) as u8, p::i32(&lv.table.mobys[r].pvars, 0x18) as u8);
    let step = p::v4f(&lv.table.mobys[r].pvars, 0x1c);
    let secs = p::ff(&lv.table.mobys[r].pvars, 0x2c);
    eprintln!("rotator {r} (class {}): link {link} (class {}), A {a} B {b}, step {step:?}, {secs} s", lv.table.mobys[r].o_class, lv.table.mobys[link].o_class);
    lv.table.mobys[r].state = 1;
    lv.table.mobys[link].state = a;
    let sounds0 = lv.svc.sounds.len();
    lv.run(&far, r, unit("U411 127"));
    assert_eq!(lv.table.mobys[r].state, 2);
    lv.table.mobys[link].state = if a == 0x7f { 0x7e } else { 0x7f };
    let n = (secs * 60.0).ceil() as usize + 1;
    for _ in 0..n { lv.run(&far, r, unit("U411 127")); }
    assert_eq!(lv.table.mobys[r].state, 3, "turned out");
    let home = p::v4f(&lv.table.mobys[r].pvars, 0);
    let turned = lv.table.mobys[r].rotation;
    for k in 0..3 { assert!((rc_game::moby_update::creature::sub_rot(turned[k], home[k]) - rc_game::moby_update::creature::sub_rot(step[k].to_radians(), 0.0)).abs() < 1e-3, "axis {k}"); }
    lv.table.mobys[link].state = b;
    lv.run(&far, r, unit("U411 127"));
    lv.table.mobys[link].state = if b == 0x7f { 0x7e } else { 0x7f };
    for _ in 0..n { lv.run(&far, r, unit("U411 127")); }
    assert_eq!(lv.table.mobys[r].state, 1, "back home");
    assert_eq!(lv.table.mobys[r].rotation[..3], home[..3]);
    let played = lv.svc.sounds[sounds0..].iter().filter(|s| s.moby == r).count();
    let (s1, s2) = (p::i32(&lv.table.mobys[r].pvars, 0x30), p::i32(&lv.table.mobys[r].pvars, 0x34));
    eprintln!("sounds {s1} / {s2}: {played} played");
    // 1 → 2 plays +0x30 (not −1); the end of the turn plays +0x30 when +0x34 ≠ −1 (sic); B plays +0x30 (not −1);
    // home plays +0x34 when +0x30 ≠ −1 (sic).
    let want = (s1 != -1) as usize + (s2 != -1) as usize + (s1 != -1) as usize + (s1 != -1) as usize;
    assert_eq!(played, want);
}

// ---------------------------------------------------------------------------------------------------
// U215: the orb holders 1038 and their orbs 1040 (Blarg)

#[test]
fn orb_holders_make_orbs_that_break_and_stay_broken() {
    let Some(mut lv) = load(6) else { eprintln!("skipped"); return };
    let holders = lv.of_class(orb_holder::CLASSES[0]);
    assert_eq!(holders.len(), 11);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for _ in 0..8 { lv.tick(&far); }
    let orbs = lv.of_class(orb_holder::ORB);
    eprintln!("{} holders, {} orbs", holders.len(), orbs.len());
    assert_eq!(orbs.len(), holders.len(), "one orb each");
    let h = holders[0];
    let o = p::i32(&lv.table.mobys[h].pvars, orb_holder::pv::ORB) as usize;
    let (hp, op) = (lv.table.mobys[h].position, lv.table.mobys[o].position);
    assert!((op[2] - hp[2] - 0.85).abs() < 1e-4 && op[0] == hp[0]);
    // The glow (type 59) and the sparks (type 60) while drawn; the orb's loop.
    lv.table.mobys[h].visible = 1;
    let (g0, s0) = (lv.svc.fx.part_spawns.get(&59).copied().unwrap_or(0), lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0));
    for _ in 0..4 { lv.counter += 1; lv.run(&far, h, unit("U215 1038")); }
    assert_eq!(lv.svc.fx.part_spawns[&59] - g0, 16, "four glow sprites a tick");
    assert_eq!(lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0) - s0, 1, "one spark every fourth tick");
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index, s.flags) == (o, 0, 4)), "the orb's loop");
    // A weapon hit on the holder: passed to the orb, which bursts (40 sparks), plays sound 1 and goes; the holder
    // sets its death bits and command byte.
    let t = rc_game::moby_update::services::HitTemplate { flags: 0x10_0000, ..Default::default() };
    { let mut w = lv.world(&far); w.deliver_hit(h, &t); }
    lv.run(&far, h, unit("U215 1038"));
    let s1 = lv.svc.fx.part_spawns.get(&60).copied().unwrap_or(0);
    lv.run(&far, o, unit("U215 orb"));
    assert!(lv.table.mobys[o].state >= 0x80, "the orb is deleted");
    assert_eq!(lv.svc.fx.part_spawns[&60] - s1, 40);
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index) == (o, 1)));
    lv.run(&far, h, unit("U215 1038"));
    let b2 = lv.table.mobys[h].spawn_id;
    assert_eq!((lv.table.mobys[h].state, lv.table.mobys[h].cmd), (2, 1));
    assert!(lv.svc.save.death.contains(&(6, b2)) && lv.svc.save.death_level.contains(&b2), "death bits of spawn id {b2}");
}

// ---------------------------------------------------------------------------------------------------
// U503 / U502 / U514: Kalebo's barrier posts 552, their switches 546 and the walls 1387

#[test]
fn kalebo_barriers_pair_up_and_switch_off() {
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let posts = lv.of_class(kalebo_barrier::POST);
    let switches = lv.of_class(kalebo_barrier::SWITCH);
    let walls = lv.of_class(kalebo_barrier::WALL);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for _ in 0..3 { lv.tick(&far); }
    // Walls: rows scaled, hidden, kept rows.
    for &k in &walls { assert_eq!((lv.table.mobys[k].state, lv.table.mobys[k].mode & 0x101), (1, 0x101)); }
    // Posts: one runner per group, every post with a level partner.
    let runners: Vec<MobyId> = posts.iter().copied().filter(|&k| lv.table.mobys[k].mode & mode::NO_UPDATE == 0).collect();
    let paired = posts.iter().filter(|&&k| p::i32(&lv.table.mobys[k].pvars, 0) != 0).count();
    let groups: std::collections::HashSet<i8> = posts.iter().map(|&k| lv.table.mobys[k].group).collect();
    eprintln!("{} posts in {} groups, {} runners, {} paired; switches {:?}; walls {:?}", posts.len(), groups.len(), runners.len(), paired, switches.iter().map(|&k| (lv.table.mobys[k].state, p::i32(&lv.table.mobys[k].pvars, 0x60), p::i32(&lv.table.mobys[k].pvars, 0x64))).collect::<Vec<_>>(), walls);
    assert_eq!(runners.len(), groups.len());
    assert_eq!(paired, posts.len());
    // A running post of a group whose switches are all armed: its beams and its loop.
    let row = units::PORTS.iter().position(|u| u.unit == "U503 552").unwrap() as u16;
    lv.svc.draw_callbacks.list1.clear();
    // (No sound sink here: `sound_alive` then answers "slot ≠ −1"; start the posts' slots free.)
    for &k in &runners { eprintln!("post {k} slot word {}", p::i32(&lv.table.mobys[k].pvars, 0xc)); p::set_i32(&mut lv.table.mobys[k].pvars, 0xc, -1); }
    for &k in &runners { lv.run(&far, k, unit("U503 552")); }
    let lit: Vec<MobyId> = lv.svc.draw_callbacks.list1.iter().map(|&(c, id)| { assert_eq!(c, rc_game::moby_update::classes::draw_callbacks::Callback::UnitQuads(row)); id }).collect();
    eprintln!("lit runners {lit:?}");
    assert_eq!(lit.len(), 3, "the group with the thrown one-way switch stays dark");
    let r = lit[0];
    let g = lv.table.mobys[r].group;
    let in_group = posts.iter().filter(|&&k| lv.table.mobys[k].group == g).count();
    let q = units::fx_quads(&lv.table, &lv.svc, row, r).unwrap();
    assert_eq!(q.quads.len(), 4 * (in_group / 2), "four strips per pair");
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index, s.flags) == (r, 0, 4)), "the barrier loop");
    // Throw the group's switch: the beams go, the loop is released, the wall loses its collision.
    let sw = *switches.iter().find(|&&k| lv.table.mobys[k].group == g).expect("the group's switch");
    let wall = p::i32(&lv.table.mobys[sw].pvars, 0x60);
    let t = rc_game::moby_update::services::HitTemplate { flags: 0x10_0000, ..Default::default() };
    { let mut w = lv.world(&far); w.deliver_hit(sw, &t); }
    lv.run(&far, sw, unit("U502 546"));
    assert_eq!((lv.table.mobys[sw].state, lv.table.mobys[sw].cmd, lv.table.mobys[sw].glow), (2, 1, 0x8020_8020));
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index, s.flags) == (sw, 0, 0)), "the switch sound");
    if wall >= 0 { assert!(!lv.table.mobys[wall as usize].has_collision, "wall {wall} off"); }
    lv.svc.draw_callbacks.list1.clear();
    lv.run(&far, r, unit("U503 552"));
    assert!(lv.svc.draw_callbacks.list1.is_empty(), "no beams while thrown");
    assert_eq!(p::i32(&lv.table.mobys[r].pvars, 8), 0);
}

// ---------------------------------------------------------------------------------------------------
// U185 (this run): Rilgar's sliding blocks 843 placed by their cuboids

#[test]
fn cuboid_sliders_sit_on_their_cuboids_and_slide_when_their_link_fires() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let blocks = lv.of_class(843);
    assert_eq!(blocks.len(), 16);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for &b in &blocks { if lv.table.mobys[b].state == 0 { lv.run(&far, b, unit("U185 843")); } }
    for &b in &blocks {
        let m = &lv.table.mobys[b];
        assert_eq!(m.state, 1, "block {b}");
        let c = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, p::i32(&m.pvars, 0x10)).unwrap();
        let off = p::ff(&m.pvars, 0x18);
        let d = ((m.position[0] - c.matrix[3][0]).powi(2) + (m.position[1] - c.matrix[3][1]).powi(2) + (m.position[2] - c.matrix[3][2]).powi(2)).sqrt();
        assert!((d - off.abs()).abs() < 1e-3, "block {b}: {d} from its cuboid, offset {off}");
        assert_eq!(m.rotation[2], c.euler[2]);
    }
    // Fire one block's link: sound 0, out to three times the offset, then down 5.9 below the centre.
    let b = blocks[0];
    let link = p::i32(&lv.table.mobys[b].pvars, 0x14);
    eprintln!("block {b}: link {link} (class {})", if link >= 0 { lv.table.mobys[link as usize].o_class } else { -1 });
    assert!(link >= 0);
    lv.table.mobys[link as usize].cmd = 1;
    lv.run(&far, b, unit("U185 843"));
    assert_eq!(lv.table.mobys[b].state, 2);
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index, s.flags) == (b, 0, 0)));
    let mut n = 0;
    while lv.table.mobys[b].state == 2 && n < 900 { lv.run(&far, b, unit("U185 843")); n += 1; }
    let m = &lv.table.mobys[b];
    let c = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, p::i32(&m.pvars, 0x10)).unwrap();
    let d = ((m.position[0] - c.matrix[3][0]).powi(2) + (m.position[1] - c.matrix[3][1]).powi(2)).sqrt();
    eprintln!("slid out after {n} ticks to {d} (offset {})", p::ff(&m.pvars, 0x18));
    assert_eq!(m.state, 3);
    while lv.table.mobys[b].state == 3 && n < 1800 { lv.run(&far, b, unit("U185 843")); n += 1; }
    assert_eq!(lv.table.mobys[b].state, 4, "sunk");
}

// ---------------------------------------------------------------------------------------------------
// U473: Quartu's swing doors 93

#[test]
fn swing_doors_open_for_ratchet_in_their_cuboid_and_close_behind_him() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let doors = lv.of_class(93);
    assert_eq!(doors.len(), 12);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    let d = *doors.iter().find(|&&d| p::i32(&lv.table.mobys[d].pvars, 0x14) == -1 && p::i32(&lv.table.mobys[d].pvars, 4) >= 0).expect("a door on cuboids");
    if lv.table.mobys[d].state == 0 { lv.run(&far, d, unit("U473 93")); }
    let closed = p::ff(&lv.table.mobys[d].pvars, 0);
    let c1 = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, p::i32(&lv.table.mobys[d].pvars, 4)).unwrap().centre();
    let inside = hero_at(c1);
    lv.run(&inside, d, unit("U473 93"));
    assert_eq!(lv.table.mobys[d].state, 2, "Ratchet in cuboid 1 opens it");
    assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index) == (d, 0)));
    for _ in 0..14 { lv.run(&inside, d, unit("U473 93")); }
    assert_eq!(lv.table.mobys[d].state, 3);
    let open = lv.table.mobys[d].rotation[2];
    let swing = rc_game::moby_update::creature::sub_rot(open, closed).abs();
    assert!((swing - 35f32.to_radians()).abs() < 1e-4, "swung {swing}");
    // Ratchet (and the camera with him) far from both cuboids: it swings shut, unless a targetable moby stands in one.
    let mut n = 0;
    while lv.table.mobys[d].state != 1 && n < 60 { lv.run(&far, d, unit("U473 93")); n += 1; }
    eprintln!("door {d}: open {open}, closed {closed}; shut after {n} ticks, state {}", lv.table.mobys[d].state);
    assert_eq!(lv.table.mobys[d].state, 1);
    assert_eq!(lv.table.mobys[d].rotation[2], closed);
}

// ---------------------------------------------------------------------------------------------------
// U307: Gaspar's chain anchors 1172

#[test]
fn chain_anchors_break_under_fire_and_pass_it_down_the_chain() {
    let Some(mut lv) = load(9) else { eprintln!("skipped"); return };
    let anchors = lv.of_class(1172);
    assert_eq!(anchors.len(), 10);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for &a in &anchors { for _ in 0..8 { lv.counter += 1; lv.run(&far, a, unit("U307 1172")); } }
    for &a in &anchors {
        let m = &lv.table.mobys[a];
        assert_eq!(m.state, 1, "anchor {a}");
        assert!(lv.svc.sounds.iter().any(|s| (s.moby, s.index, s.flags) == (a, 1, 4)), "anchor {a}'s loop");
    }
    let a = *anchors.iter().find(|&&a| p::i32(&lv.table.mobys[a].pvars, 0x70) >= 0).expect("an anchor on a chain");
    let next = p::i32(&lv.table.mobys[a].pvars, 0x70) as usize;
    let health = p::ff(&lv.table.mobys[a].pvars, 0x20);
    eprintln!("anchor {a}: health {health}, next {next} (class {}), partner {}", lv.table.mobys[next].o_class, p::i32(&lv.table.mobys[a].pvars, 0x78));
    // A weapon hit worth more than its health: broken at once.
    let t = rc_game::moby_update::services::HitTemplate { flags: 0x10_0000, damage: rc_game::moby_update::services::pf(health + 10.0), ..Default::default() };
    { let mut w = lv.world(&far); w.deliver_hit(a, &t); }
    let flashes0 = lv.svc.fx.flashes;
    lv.run(&far, a, unit("U307 1172"));
    let b2 = lv.table.mobys[a].spawn_id;
    eprintln!("after the hit: state {}, next cmd {}, health {}", lv.table.mobys[a].state, lv.table.mobys[next].cmd, p::ff(&lv.table.mobys[a].pvars, 0x20));
    // The resolver runs before the state switch: broken (state 3) and blown up in the same tick.
    assert!(lv.table.mobys[a].state >= 0x80, "blown up");
    assert_eq!(lv.table.mobys[next].cmd, 1, "the chain told");
    assert_eq!(lv.table.mobys[a].mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    let b1 = lv.table.mobys[a].spawn_flag;
    eprintln!("spawn flag {b1:#x}, id {b2}");
    if b2 >= 0 && b1 != 0xfe { assert!(lv.svc.save.death.contains(&(9, b2)), "its death bits"); }
    assert!(lv.svc.fx.flashes > flashes0);
}

// ---------------------------------------------------------------------------------------------------
// U477: the sliding doors 196 / 197 / 1958 (Quartu, Fleet)

#[test]
fn slide_doors_open_their_travel_for_their_link_or_cuboid() {
    let Some(mut lv) = load(15) else { eprintln!("skipped"); return };
    let doors: Vec<MobyId> = [196, 197, 1958].iter().flat_map(|&c| lv.of_class(c)).collect();
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for &d in &doors { if lv.table.mobys[d].state == 0 { lv.run(&far, d, unit("U477 196")); } }
    let desc: Vec<_> = doors.iter().map(|&d| { let m = &lv.table.mobys[d]; (d, m.o_class, m.state, p::i32(&m.pvars, 0x10), p::i32(&m.pvars, 0x18), p::i32(&m.pvars, 0x1c)) }).collect();
    eprintln!("doors (id, class, state, link, A, B): {desc:?}");
    let d = doors.iter().copied().find(|&d| lv.table.mobys[d].state < 0x80).expect("a live door");
    let (st, link) = (lv.table.mobys[d].state, p::i32(&lv.table.mobys[d].pvars, 0x10));
    let hero = if st == 4 {
        let c = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, p::i32(&lv.table.mobys[d].pvars, 0x18)).unwrap().centre();
        hero_at(c)
    } else {
        lv.table.mobys[link as usize].state = 4;
        far
    };
    let home = lv.table.mobys[d].position;
    let oc = lv.table.mobys[d].o_class;
    let sounds0 = lv.svc.sounds.len();
    let mut n = 0;
    while !matches!(lv.table.mobys[d].state, 3 | 5) && n < 400 { lv.run(&hero, d, unit("U477 196")); n += 1; }
    let p1 = lv.table.mobys[d].position;
    let moved = ((p1[0] - home[0]).powi(2) + (p1[1] - home[1]).powi(2)).sqrt();
    let travel = 3.0 * lv.table.mobys[d].scale / lv.classes.classes.get(&oc).map_or(1.0, |c| c.0.scale);
    eprintln!("door {d} (class {oc}): open after {n} ticks, moved {moved}, travel {travel}, state {}", lv.table.mobys[d].state);
    assert!(matches!(lv.table.mobys[d].state, 3 | 5));
    assert!((moved - travel).abs() < 0.2);
    let loud = oc != 197;
    assert_eq!(lv.svc.sounds[sounds0..].iter().any(|s| s.moby == d && s.index == 0), loud);
}

/// Survey (ignored): where each ported class's first instances stand, with the closest mesh-free camera spots used for
/// the frames (class_census.md "In the port: cheap wins round 2").
#[test]
#[ignore]
fn survey_positions() {
    for (level, classes) in [(15u32, &[408i16, 77, 123, 93][..]), (16, &[552, 546][..]), (6, &[1038][..]), (5, &[843][..]), (13, &[127, 128, 159, 169][..]), (9, &[1172][..])] {
        let Some(lv) = load(level) else { return };
        for &c in classes {
            let ids = lv.of_class(c);
            let ps: Vec<[f32; 3]> = ids.iter().take(3).map(|&i| { let q = lv.table.mobys[i].position; [q[0], q[1], q[2]] }).collect();
            eprintln!("level {level:02} class {c}: {} instances, first {ps:?}", ids.len());
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// U568: Kalebo's lifts 1826

/// The lifts ride their cylinders (the table 0x1600fc: cylinder 0 = 5.5 high about z 126 at the first lift, cylinder 1
/// = 6.368 about z 126.85 at the second): each starts at its top, e(+1) = half + centre z + 0.5, and Ratchet standing
/// on one sends it down to e(−1).
#[test]
fn kalebo_lifts_ride_their_cylinders() {
    use rc_game::moby_update::classes::units::kalebo_lift as kl;
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let lifts = lv.of_class(kl::CLASSES[0]);
    assert_eq!(lifts.len(), 2);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for &l in &lifts { lv.run(&far, l, unit("U568 1826")); }
    let ends: Vec<(f32, f32)> = lifts
        .iter()
        .map(|&l| {
            let s = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cylinder, p::i32(&lv.table.mobys[l].pvars, kl::pv::CYLINDER)).unwrap();
            (s.matrix[2][2] + s.matrix[3][2] + kl::STAND, -s.matrix[2][2] + s.matrix[3][2] + kl::STAND)
        })
        .collect();
    assert_eq!(ends, vec![(132.0, 121.0), (126.85 + 6.368 + 0.5, 126.85 - 6.368 + 0.5)]);
    for (&l, e) in lifts.iter().zip(&ends) { assert_eq!(lv.table.mobys[l].position[2], e.0, "at the top"); }
    let mut on = far.clone();
    on.ground_moby = Some(lifts[0]);
    for _ in 0..600 { lv.run(&on, lifts[0], unit("U568 1826")); }
    assert_eq!(lv.table.mobys[lifts[0]].position[2], ends[0].1, "ridden to the bottom");
}

// ---------------------------------------------------------------------------------------------------
// U495: Oltanis's floats 309

/// The floats' rows are a right-handed frame (`FastVecCross(out, a, b)` = b × a), so their collision faces up: a ray
/// from above meets the top of the model across the platform. Ratchet standing east of the centre leans the up row
/// toward him (+x), by at most 10°.
#[test]
fn oltanis_floats_are_solid_and_lean_toward_ratchet() {
    let Some(mut lv) = load(14) else { eprintln!("skipped"); return };
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    for _ in 0..5 { lv.tick(&far); }
    let core = rc_formats::test_data::core(14).unwrap();
    let cl = rc_formats::moby::parse_moby_class(core.block("moby_class/0309").unwrap()).unwrap();
    let top = cl.high_lod.iter().flat_map(|s| &s.vertices).map(|v| v.packed_position()[2] as f32).fold(f32::MIN, f32::max);
    let floats = lv.of_class(309);
    assert_eq!(floats.len(), 4);
    for &id in &floats {
        let m = lv.table.mobys[id].clone();
        let r = m.rows;
        let cross = [r[0][1] * r[1][2] - r[0][2] * r[1][1], r[0][2] * r[1][0] - r[0][0] * r[1][2], r[0][0] * r[1][1] - r[0][1] * r[1][0]];
        assert!((0..3).all(|k| (cross[k] - r[2][k]).abs() < 1e-5), "row 0 × row 1 = row 2: {r:?}");
        let parts = lv.svc.scene_parts(&lv.table, &lv.classes);
        let sc = lv.svc.scene(&parts);
        for dx in [0.0f32, 1.0, 2.0] {
            let (a, b) = ([m.position[0] + dx, m.position[1], m.position[2] + 6.0], [m.position[0] + dx, m.position[1], m.position[2] - 6.0]);
            let h = rc_game::collision_query::coll_line_m(&lv.mesh, Some(&sc), a, b, rc_game::collision_query::QueryFlags(0), None).expect("the top");
            assert_eq!(h.moby, Some(id));
            assert!((h.point[2] - (m.position[2] + top * m.scale / 1024.0)).abs() < 1e-3, "{dx}: {:?}", h.point);
        }
    }
    let id = floats[0];
    let p = lv.table.mobys[id].position;
    let mut on = hero_at([p[0] + 1.0, p[1], p[2] + 0.7]);
    on.ground_moby = Some(id);
    for _ in 0..300 { lv.run(&on, id, unit("U495 309")); }
    let up = lv.table.mobys[id].rows[2];
    assert!(up[0] > 0.1 && up[1].abs() < 1e-3, "leans toward +x: {up:?}");
    assert!(up[2] >= (10.0f32 * 0.017_453_292).cos() - 1e-3, "at most 10°: {up:?}");
}
