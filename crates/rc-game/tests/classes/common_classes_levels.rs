//! The classes common to many levels (the Gemlik pass's unported list): 605 buried bolt caches, 832 Visibomb range
//! limiters, 604 / 1818 / 1633 the Sonic Summoner's house, mouse and shot, 258 activation zones. Each runs one port on
//! every level whose class table names the same code (`LevelPorts`); headless behaviour on two levels per class.
//! Skipped when `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter common_classes_levels:: --nocapture` prints the per-level survey.

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::{activation_zone, buried_bolts, mouse, rc_range, ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys, Scheduler};
use rc_game::moby_update::services::{pvar as p, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use std::collections::HashMap;
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

/// (class, port, the levels whose table runs it: the survey of the Gemlik pass).
const EXPECTED: &[(i16, ClassUpdate, &[u32])] = &[
    (605, ClassUpdate::BuriedBolts, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18]),
    (832, ClassUpdate::RcRange, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 18]),
    (604, ClassUpdate::MouseHouse, &[1, 2, 3, 4, 5, 6, 8, 11, 12, 14]),
    (1818, ClassUpdate::Mouse, &[1, 2, 3, 4, 5, 6, 8, 11, 12, 14]),
    (258, ClassUpdate::ActivationZone, &[5, 7, 9, 10, 12, 13, 14, 15, 17]),
    (830, ClassUpdate::FloorSwitch, &[5, 11, 15, 17, 18]),
];

/// Every newly registered class resolves to its port on each of its levels (and on no other level's table entry
/// with different code), and the placed instances they cover.
#[test]
fn common_classes_resolve_on_their_levels() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut covered: HashMap<i16, usize> = HashMap::new();
    let mut shot_levels = Vec::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        for &(oc, u, levels) in EXPECTED {
            let placed = inst.iter().filter(|m| m.o_class as i16 == oc).count();
            if levels.contains(&level) {
                assert_eq!(p.get(oc), Some(u), "level {level:02} class {oc}");
                *covered.entry(oc).or_default() += placed;
            } else if p.level_update(oc).is_some_and(|f| f != 0) {
                assert_ne!(p.get(oc), Some(u), "level {level:02} class {oc}: other code");
            }
        }
        if p.get(mouse::SHOT_CLASS) == Some(ClassUpdate::MouseShot) { shot_levels.push(level); }
    }
    eprintln!("placed instances now ported: {covered:?}; the mouse's shot 1633 runs its port on {shot_levels:?}");
    assert_eq!(covered[&605], 358);
    assert_eq!(covered[&258], 50);
    assert_eq!((covered[&832], covered[&604], covered[&1818], covered[&830]), (16, 10, 10, 30));
    // The shot is created only by the mouse: its class runs the port wherever the mouse does.
    for l in EXPECTED[3].2 { assert!(shot_levels.contains(l), "level {l:02}: 1633"); }
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
        let mut sched = std::mem::take(&mut self.sched);
        { let mut w = self.world(hero); sched.tick(&mut w); }
        self.sched = sched;
    }
    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
    fn cuboid_centre(&self, i: i32) -> Option<[f32; 3]> { self.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).map(|s| s.centre()) }
}

fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h
}

// ---------------------------------------------------------------------------------------------------
// 605

fn buried_on(level: u32) {
    let Some(mut lv) = load(level) else { eprintln!("skipped"); return };
    // Two digs already made at cache #1 (the high nibble of byte 0): its value drops by 10.
    let mut bits = vec![[0u8; 16]; 20];
    bits[level as usize][0] = 0x20;
    lv.svc.interact.game.metal_detector_bits = bits;
    let caches = lv.of_class(605);
    let values: Vec<i32> = caches.iter().map(|&c| p::i32(&lv.table.mobys[c].pvars, buried_bolts::pv::VALUE)).collect();
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    let mut n = 0u8;
    for (k, &c) in caches.iter().enumerate() {
        let m = &lv.table.mobys[c];
        if m.state >= 0x80 { assert!(k == 0 && values[0] <= 10, "only cache #1 can be dug out"); n += 1; continue; }
        n += 1;
        assert_eq!((m.state, m.cmd, m.update_dist), (1, n, 0xff), "level {level:02} cache {k}: numbered in load order");
        assert_eq!(m.mode & (mode::HIDDEN | mode::NO_ANIM), mode::HIDDEN | mode::NO_ANIM);
        let v = p::i32(&m.pvars, buried_bolts::pv::VALUE);
        assert_eq!(v, values[k] - if k == 0 { 10 } else { 0 }, "level {level:02} cache {k} value");
    }
    // Without the Metal Detector nothing is reported; with it, Ratchet at a cache (inside its area) gets the alert.
    let target = *caches.iter().rev().find(|&&c| lv.table.mobys[c].state < 0x80).unwrap();
    let tp = lv.table.mobys[target].position;
    let pv = lv.table.mobys[target].pvars.clone();
    let area = [p::i32(&pv, buried_bolts::pv::PATH), p::i32(&pv, buried_bolts::pv::CUBOID)];
    let mut h = hero_at([tp[0] + 3.0, tp[1], tp[2]]);
    if area[1] != -1 {
        let c = lv.cuboid_centre(area[1]).unwrap();
        h = hero_at(c);
    }
    lv.tick(&h);
    assert!(!lv.svc.buried.alert && lv.svc.buried.alert_requests == 0, "no detector");
    h.owned.0[buried_bolts::METAL_DETECTOR] = 1;
    lv.tick(&h);
    lv.tick(&h);
    let reqs = lv.svc.buried.alert_requests;
    eprintln!("level {level:02}: {} caches (values {values:?}), area {area:?}, alert requests {reqs}", caches.len());
    assert!(reqs >= 1, "level {level:02}: a cache within 20 of Ratchet raises the alert");
    assert_eq!(lv.svc.buried.nearest, None, "the alert frame clears the search");
    // Far away: no alert.
    let mut far = far;
    far.owned.0[buried_bolts::METAL_DETECTOR] = 1;
    lv.tick(&far);
    lv.tick(&far);
    assert!(!lv.svc.buried.alert);
}

#[test]
fn buried_bolts_novalis() { buried_on(1) }
#[test]
fn buried_bolts_gemlik() { buried_on(13) }

// ---------------------------------------------------------------------------------------------------
// 832

fn range_on(level: u32) {
    let Some(mut lv) = load(level) else { eprintln!("skipped"); return };
    let ctl = lv.of_class(832)[0];
    let pv = lv.table.mobys[ctl].pvars.clone();
    let recs: Vec<[i32; 2]> = (0..rc_range::RECORDS).map(|k| [p::i32(&pv, 16 * k), p::i32(&pv, 16 * k + 12)]).collect();
    let mut h = hero_at([500.0, 500.0, 50.0]);
    lv.load_pass(&h);
    // A Visibomb missile (class 172 stands in: its data is not needed) far outside every area, then in one.
    let missile = lv.table.create(rc_range::MISSILE_CLASS, None, 0).unwrap();
    lv.table.mobys[missile].position = [3.0, 3.0, 1000.0, 1.0];
    lv.table.mobys[missile].state = 1;
    for _ in 0..5 { lv.tick(&h); }
    assert_eq!(p::i32(&lv.table.mobys[ctl].pvars, rc_range::COUNTER), 0, "not steering");
    h.state = rc_range::STEERING_STATE;
    for _ in 0..5 { lv.tick(&h); }
    assert_eq!(p::i32(&lv.table.mobys[ctl].pvars, rc_range::COUNTER), 5, "level {level:02}: out of range");
    let inside = recs.iter().find_map(|r| (r[1] != -1).then(|| lv.cuboid_centre(r[1])).flatten());
    eprintln!("level {level:02}: records {:?}", recs.iter().filter(|r| r[0] != -1 || r[1] != -1).collect::<Vec<_>>());
    if let Some(c) = inside {
        lv.table.mobys[missile].position = [c[0], c[1], c[2], 1.0];
        lv.tick(&h);
        assert_eq!(p::i32(&lv.table.mobys[ctl].pvars, rc_range::COUNTER), 0, "level {level:02}: back in range");
    }
    // Out of range again with the missile view on (0x15f30c, set by the launch `0x2cb338`): the static 0x302438 is
    // registered on list 2 and drawn by the next frame (a 17 × 14 grid of quads); past ticks(90) outside the flight
    // ends (0x2cb788: the missile deleted, Ratchet's SetState(0, 1)).
    lv.svc.visibomb.view.overlay = true;
    lv.table.mobys[missile].pvars.resize(0x80, 0);
    lv.table.mobys[missile].position = [3.0, 3.0, 1000.0, 1.0];
    lv.tick(&h);
    assert_eq!(p::i32(&lv.table.mobys[ctl].pvars, rc_range::COUNTER), 1);
    use rc_game::moby_update::classes::draw_callbacks::Callback;
    assert!(lv.svc.draw_callbacks.list2.iter().any(|&(c, id)| c == Callback::RangeStatic && id == ctl), "level {level:02}: the static registered");
    lv.tick(&h);
    assert_eq!(lv.svc.visibomb.static_quads.len(), 17 * 14, "level {level:02}: the static drawn");
    for _ in 0..88 { lv.tick(&h); }
    assert_eq!(p::i32(&lv.table.mobys[ctl].pvars, rc_range::COUNTER), 90);
    assert!(lv.table.mobys[missile].state < 0x80, "still flying at 90 ticks out");
    lv.tick(&h);
    assert!(lv.table.mobys[missile].state >= 0x80, "level {level:02}: the flight ended past ticks(90)");
    let calls = lv.svc.hero_writes.as_ref().map(|(_, f)| f.calls).unwrap_or_default();
    assert!(calls.contains(&Some(rc_game::moby_update::services::HeroCall::SetState { id: 0, play: true })), "{calls:?}");
    assert!(!lv.svc.visibomb.view.overlay && lv.svc.visibomb.missile.is_none());
}

#[test]
fn rc_range_novalis() { range_on(1) }
#[test]
fn rc_range_gemlik() { range_on(13) }

// ---------------------------------------------------------------------------------------------------
// 258

fn zones_on(level: u32) {
    let Some(mut lv) = load(level) else { eprintln!("skipped"); return };
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    lv.tick(&far);
    let zones = lv.of_class(258);
    let mut entered = 0;
    for &z in &zones {
        let m = lv.table.mobys[z].clone();
        assert_eq!((m.state, m.cmd), (1, 0), "level {level:02} zone {z}: applied once, outside");
        let rules = [0x7c, 0x7d, 0x7f].map(|o| m.pvars[o] as i8);
        let groups: Vec<i32> = (0..12).map(|k| p::i32(&m.pvars, activation_zone::pv::GROUPS + 4 * k)).filter(|&g| g != -1).collect();
        let singles: Vec<i32> = (0..11).map(|k| p::i32(&m.pvars, activation_zone::pv::MOBYS + 4 * k)).filter(|&g| g != -1).collect();
        let radius = p::ff(&m.pvars, activation_zone::pv::RADIUS);
        eprintln!("level {level:02} zone {z}: rules {rules:?} radius {radius} path {} groups {groups:?} mobys {singles:?} camera {}", p::i32(&m.pvars, 0), m.pvars[0x7e]);
        // Outside: the draw rule d > 0 hides its mobys.
        let targets: Vec<usize> = singles.iter().map(|&s| s as usize).chain(groups.iter().flat_map(|&g| lv.svc.groups.lists.get(g as usize).cloned().flatten().unwrap_or_default().into_iter().map(|x| x as usize))).collect();
        if rules[1] > 0 {
            for &t in &targets {
                let c = lv.table.mobys[t].o_class;
                if !activation_zone::KEEP_DRAW.contains(&c) { assert!(lv.table.mobys[t].mode & mode::HIDDEN != 0, "level {level:02} zone {z}: moby {t} hidden outside"); }
            }
        }
        // Inside (the probe, Ratchet or here the camera, at the sphere's centre, the first cuboid's centre or the
        // path's centroid when that is inside it): the answer flips and the rules apply the other way.
        let path = p::i32(&m.pvars, activation_zone::pv::PATH);
        let cub = (0..6).map(|k| p::i32(&m.pvars, activation_zone::pv::CUBOIDS + 4 * k)).find(|&c| c != -1);
        let probe = if 0.0 < radius {
            Some([m.position[0], m.position[1], m.position[2]])
        } else if let Some(c) = cub {
            lv.cuboid_centre(c)
        } else if let Some(pts) = usize::try_from(path).ok().and_then(|i| lv.svc.volumes.paths.get(i)) {
            let n = pts.len() as f32;
            let c = pts.iter().fold([0.0f32; 3], |a, q| [a[0] + q[0] / n, a[1] + q[1] / n, a[2] + q[2] / n]);
            rc_game::moby_update::triggers::point_in_path_polygon(c, pts).then_some(c)
        } else {
            None
        };
        if let Some(q) = probe {
            eprintln!("level {level:02} zone {z}: probe inside at {q:?}");
            let h = hero_at(q);
            lv.tick(&h);
            assert_eq!((lv.table.mobys[z].state, lv.table.mobys[z].cmd), (2, 1), "level {level:02} zone {z}: inside");
            if rules[1] > 0 {
                for &t in &targets {
                    let c = lv.table.mobys[t].o_class;
                    if !activation_zone::KEEP_DRAW.contains(&c) { assert!(lv.table.mobys[t].mode & mode::HIDDEN == 0, "moby {t} shown inside"); }
                }
            }
            entered += 1;
            lv.tick(&far);
            assert_eq!((lv.table.mobys[z].state, lv.table.mobys[z].cmd), (1, 0));
        }
    }
    eprintln!("level {level:02}: {} zones, {entered} entered", zones.len());
    assert!(!zones.is_empty());
}

#[test]
fn activation_zones_rilgar() { zones_on(5) }
#[test]
fn activation_zones_gemlik() { zones_on(13) }

// ---------------------------------------------------------------------------------------------------
// 604 / 1818 / 1633

fn summon_on(level: u32) {
    let Some(mut lv) = load(level) else { eprintln!("skipped"); return };
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    lv.tick(&far);
    let m = lv.of_class(1818)[0];
    let house = lv.of_class(604)[0];
    assert_eq!(p::i32(&lv.table.mobys[m].pvars, mouse::pvo::HOUSE), house as i32, "level {level:02}: the mouse's house");
    let hp = lv.table.mobys[house].position;
    let mp = lv.table.mobys[m].position;
    assert_eq!((lv.table.mobys[m].state, lv.table.mobys[m].draw_dist), (1, 0), "home, hidden");
    assert!((mp[2] - hp[2] - 0.02).abs() < 1e-4);
    // Ratchet with the Sonic Summoner on, in the mouse's cuboid.
    let cub = p::i32(&lv.table.mobys[m].pvars, mouse::pvo::CUBOID);
    let c = lv.cuboid_centre(cub).expect("summon cuboid");
    let mut h = hero_at(c);
    h.owned.0[mouse::SUMMONER as usize] = 1;
    h.head_slot.id = mouse::SUMMONER;
    h.head_slot.state = 2;
    h.worn.head = Some(rc_game::hero::worn::HeadMoby { o_class: mouse::SUMMONER_CLASS, anim: rc_game::moby_runtime::Moby::zeroed().anim, snapshot: None });
    let mut states = vec![];
    let mut house_states = vec![];
    for _ in 0..900 {
        lv.tick(&h);
        let s = lv.table.mobys[m].state;
        if states.last() != Some(&s) { states.push(s); }
        let hs = lv.table.mobys[house].state;
        if house_states.last() != Some(&hs) { house_states.push(hs); }
        if s == 5 { break; }
    }
    eprintln!("level {level:02}: mouse states {states:?}, house states {house_states:?}, mouse at {:?} (Ratchet {c:?}), summoned {:?}", lv.table.mobys[m].position, lv.svc.mouse.summoned);
    assert!(states.starts_with(&[2, 3]), "level {level:02}: summoned (1 → 2 on the first tick in the cuboid)");
    assert_eq!(lv.svc.mouse.summoned, Some(m));
    assert!(house_states.contains(&5), "the house opened");
    assert_eq!(*states.last().unwrap(), 5, "level {level:02}: beside Ratchet");
    // It follows for a while, then (its time run short here) vanishes and is home again.
    for _ in 0..60 { lv.tick(&h); }
    p::set_i32(&mut lv.table.mobys[m].pvars, mouse::pvo::LIFE, 3);
    for _ in 0..120 {
        lv.tick(&h);
        let s = lv.table.mobys[m].state;
        if states.last() != Some(&s) { states.push(s); }
        // (With Ratchet still in the cuboid wearing the Summoner, the next tick summons it again.)
        if s == 1 { break; }
    }
    eprintln!("level {level:02}: then {states:?}");
    assert!(states.ends_with(&[5, 6, 7, 1]), "level {level:02}: vanished and home");
    let home = p::v4f(&lv.table.mobys[m].pvars, mouse::pvo::HOME);
    assert_eq!(lv.table.mobys[m].position[..3], home[..3]);
    assert_eq!(lv.svc.mouse.summoned, None);
}

#[test]
fn summoner_mouse_novalis() { summon_on(1) }
#[test]
fn summoner_mouse_rilgar() { summon_on(5) }

// ---------------------------------------------------------------------------------------------------
// 830

fn switches_on(level: u32) {
    let Some(mut lv) = load(level) else { eprintln!("skipped"); return };
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    lv.tick(&far);
    let sw = lv.of_class(830);
    let armed: Vec<MobyId> = sw.iter().copied().filter(|&s| lv.table.mobys[s].state == 1).collect();
    eprintln!("level {level:02}: {} switches, {} armed", sw.len(), armed.len());
    let s = armed.iter().copied().find(|&s| p::i32(&lv.table.mobys[s].pvars, 0) != -1).or(armed.first().copied()).expect("an armed switch");
    let (path, key) = (p::i32(&lv.table.mobys[s].pvars, 0), p::ff(&lv.table.mobys[s].pvars, 4));
    let keyed = |lv: &Lv| usize::try_from(path).ok().and_then(|i| lv.svc.splines.get(i)).map(|pts| pts.iter().filter(|q| f32::from_bits(q[3]) == key).count()).unwrap_or(0);
    let before = keyed(&lv);
    let sp = lv.table.mobys[s].position;
    let mut h = hero_at([sp[0], sp[1], sp[2]]);
    h.ground_moby = Some(s);
    h.air_ticks = 0;
    lv.tick(&h);
    let m = &lv.table.mobys[s];
    eprintln!("level {level:02}: switch {s} path {path} key {key}: {before} points keyed → {}", keyed(&lv));
    assert_eq!((m.state, m.cmd, m.glow), (2, 1, rc_game::moby_update::classes::floor_switch::PRESSED_GLOW));
    assert_eq!(keyed(&lv), 0, "the path's keyed points cleared");
    assert!(lv.svc.save.killed.contains_key(&m.spawn_id));
}

#[test]
fn floor_switches_rilgar() { switches_on(5) }
#[test]
fn floor_switches_other() { switches_on(11) }
