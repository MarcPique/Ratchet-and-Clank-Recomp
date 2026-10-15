//! The path-helper class units (docs/plan/class_census.md, gaps.md G-CLS-025; `rc_game::path` and
//! `rc_game::moby_update::classes::units`): each unit resolves to its port on its levels and runs headless on one of
//! them, covering the behaviour and the side effects its coverage table marks ported. Skipped when `extracted/` is
//! absent.
//!
//! `cargo xtask test-job --test classes --filter path_classes:: --nocapture` prints the per-unit survey.

use rc_formats::level_overlay::LevelOverlay;
use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::units::{self, kalebo_traffic, path_glider, rail_car};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, group_ids, load_level_mobys, Scheduler};
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

/// (unit, class, the levels whose table runs it, created instances on them).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[
    ("U36", 1564, &[0, 7, 10, 18], 92),
    ("U495", 257, &[16], 21),
    ("U523", 1667, &[16], 18),
    ("U523", 1668, &[16], 18),
    ("U523", 1669, &[16], 18),
    ("U523", 1670, &[16], 18),
    ("U523", 1671, &[16], 18),
];

/// Every unit's classes resolve to the unit on its levels (and to nothing else), with the census's instance counts.
#[test]
fn units_resolve_on_their_levels() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
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
    for &(name, oc, _, n) in EXPECTED { assert_eq!(created[&(name, oc)], n, "{name} class {oc}"); }
}

// ---------------------------------------------------------------------------------------------------
// Headless levels

/// A sound sink that owns its slots: `SoundIsAlive` is true for the slot's owner (the game's check, which the
/// sink-less default reduces to "slot ≠ −1").
#[derive(Default)]
struct RecSink {
    slots: Vec<MobyId>,
}

impl rc_game::moby_update::services::SoundSink for RecSink {
    fn play_class_sound(&mut self, ev: &rc_game::moby_update::services::SoundEvent, _rng: &mut Rng) -> i32 {
        self.slots.push(ev.moby);
        self.slots.len() as i32 - 1
    }
    fn alive(&self, slot: i32, moby: MobyId) -> bool { usize::try_from(slot).ok().and_then(|s| self.slots.get(s)) == Some(&moby) }
}

struct Lv {
    sink: RecSink,
    table: MobyTable,
    classes: ClassTable,
    svc: Services,
    sched: Scheduler,
    rng: Rng,
    mesh: collision::Collision,
    counter: u64,
    missions: rc_game::moby_update::services::LevelMissions,
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
    svc.joint_lists = joints;
    svc.build_grid(&mut table);
    Some(Lv { sink: RecSink::default(), table, classes, svc, sched: Scheduler::new(), rng: Rng::new(), mesh, counter: 0, missions: rc_game::moby_update::services::LevelMissions::fresh_load(level, [0xff; 16]) })
}

impl Lv {
    fn world<'a>(&'a mut self, hero: &'a Hero) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.sound = Some(&mut self.sink);
        w.camera = hero.pos;
        w.missions = &self.missions;
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
    h
}


// ---------------------------------------------------------------------------------------------------
// U36: the path gliders 1564 (level 00)

#[test]
fn gliders_ride_their_paths_above_the_ground_and_flap() {
    let Some(mut lv) = load(0) else { eprintln!("skipped"); return };
    let ids = lv.of_class(1564);
    assert_eq!(ids.len(), 25);
    let far = hero_at([1.0, 1.0, 1.0]);
    let scale0: Vec<f32> = ids.iter().map(|&i| lv.table.mobys[i].scale).collect();
    lv.load_pass(&far);
    lv.tick(&far);
    let mut wrapped = 0;
    for (k, &i) in ids.iter().enumerate() {
        let m = &lv.table.mobys[i];
        assert_eq!(m.state, 1, "glider {i}");
        assert_eq!(m.update_dist, 0xff);
        assert_eq!(m.scale, scale0[k] * path_glider::SCALE);
        let path = p::i32(&m.pvars, path_glider::pv::PATH);
        let pts = &lv.svc.splines[path as usize];
        let n = pts.len();
        // The step: 6 units/s over the first segment; t started at the start fraction · count and advanced once.
        let (a, b) = (pts[0].map(f32::from_bits), pts[1].map(f32::from_bits));
        let l01 = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        let step = p::ff(&m.pvars, path_glider::pv::STEP);
        assert!((step - (path_glider::SPEED / 60.0) / l01).abs() < 1e-6, "glider {i} step {step}");
        let start = p::ff(&m.pvars, path_glider::pv::START);
        let t = p::ff(&m.pvars, path_glider::pv::T);
        // The load pass ran the update once (the init and the first move), the tick once more.
        let mut want = start * n as f32 + step;
        if (n as f32) < want { want -= n as f32; wrapped += 1; }
        want += step;
        if (n as f32) < want { want -= n as f32; wrapped += 1; }
        assert!((t - want).abs() < 1e-4, "glider {i} t {t} want {want}");
        // The position: the pose at the previous t, lifted by the height.
        let (pos, rot) = rc_game::path::pose(pts, p::i32(&m.pvars, path_glider::pv::CLOSED) != 0, t - step, true);
        let h = p::ff(&m.pvars, path_glider::pv::HEIGHT);
        assert!((m.position[0] - pos[0]).abs() < 1e-3 && (m.position[1] - pos[1]).abs() < 1e-3 && (m.position[2] - (pos[2] + h)).abs() < 1e-3, "glider {i} at {:?} vs {pos:?} + {h}", m.position);
        assert!((m.rotation[1] - rot[1] * 0.5).abs() < 1e-6 && m.rotation[2] == rot[2]);
    }
    eprintln!("gliders: {} on level 00, {wrapped} wrapped on the first tick; the first three at {:?}", ids.len(), ids.iter().take(3).map(|&i| lv.table.mobys[i].position).collect::<Vec<_>>());
    // Ten seconds on: every glider is still on its path (t within count) and moved.
    let before: Vec<[f32; 4]> = ids.iter().map(|&i| lv.table.mobys[i].position).collect();
    for _ in 0..600 { lv.tick(&far); }
    for (k, &i) in ids.iter().enumerate() {
        let m = &lv.table.mobys[i];
        let n = lv.svc.splines[p::i32(&m.pvars, path_glider::pv::PATH) as usize].len() as f32;
        let t = p::ff(&m.pvars, path_glider::pv::T);
        assert!(0.0 <= t && t <= n, "glider {i} t {t} of {n}");
        assert_ne!(m.position, before[k]);
    }
    // The flap: an animation wrap while not climbing blends to sequence 1 with a 1-in-3 chance, else to 0.
    let g = ids[0];
    let mut flaps = 0;
    let mut glides = 0;
    for _ in 0..60 {
        let z = lv.table.mobys[g].position[2];
        lv.table.mobys[g].anim.flags |= 2;
        // Level: the path's next pose is what it is; force "not climbing" by the height offset being constant and
        // the glider's z compared before / after the update inside the port.
        let seq0 = lv.table.mobys[g].anim.seq_b;
        lv.run(&far, g, unit("U36"));
        let m = &lv.table.mobys[g];
        if m.anim.seq_b != seq0 { if m.anim.seq_b == 1 { flaps += 1 } else { glides += 1 } }
        let _ = z;
    }
    eprintln!("glider {g}: {flaps} flaps, {glides} glides in 60 forced wraps");
    assert!(flaps + glides > 0, "the animation blends never happened");
    assert!(lv.svc.sounds.is_empty(), "gliders make no sound");
}

// ---------------------------------------------------------------------------------------------------
// U495: Kalebo's rail cars 257 (level 16)

#[test]
fn rail_car_runs_its_path_when_ratchet_grinds_in_its_cuboid() {
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let cars = lv.of_class(257);
    assert_eq!(cars.len(), 21);
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.load_pass(&far);
    lv.tick(&far);
    for &c in &cars {
        let m = &lv.table.mobys[c];
        assert_eq!((m.state, m.update_dist, m.cmd), (1, 100, 0), "car {c}");
        let pts = &lv.svc.splines[p::i32(&m.pvars, rail_car::pv::PATH) as usize];
        let (a, b) = (pts[0].map(f32::from_bits), pts[1].map(f32::from_bits));
        let l = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!((p::ff(&m.pvars, rail_car::pv::STEP) - p::ff(&m.pvars, rail_car::pv::SPEED) / 60.0 / l).abs() < 1e-6);
    }
    // Ratchet in the first car's cuboid but not grinding: nothing; grinding: the car sets off.
    let c = cars[0];
    let cub = p::i32(&lv.table.mobys[c].pvars, rail_car::pv::CUBOID);
    let centre = lv.svc.volumes.cuboids[cub as usize].centre();
    let mut hero = hero_at(centre);
    hero.body_point = hero.pos;
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[c].state, 1, "not grinding");
    hero.group = 0xf;
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[c].state, 2, "grinding in the cuboid");
    let pos0 = lv.table.mobys[c].position;
    lv.tick(&hero);
    let m = &lv.table.mobys[c];
    assert_eq!(m.state, 2);
    assert_ne!(m.position, pos0, "the car moves");
    let pts = lv.svc.splines[p::i32(&m.pvars, rail_car::pv::PATH) as usize].clone();
    let (want, _) = rc_game::path::pose(&pts, false, p::ff(&m.pvars, rail_car::pv::T), true);
    assert_eq!(m.position, want);
    // The loop sound 0 (flags 4): started once, then alive on its slot.
    let loops: Vec<_> = lv.svc.sounds.iter().filter(|s| s.moby == c && s.index == 0 && s.flags == 4).collect();
    assert_eq!(loops.len(), 1, "loop sound 0 started once: {:?}", lv.svc.sounds);
    assert!(lv.svc.sounds.iter().all(|s| s.moby != c || s.index != 1), "sound 1 needs Ratchet within 7");
    // Ratchet next to the car: sound 1, once.
    let at = lv.table.mobys[c].position;
    let mut near = hero_at([at[0] + 3.0, at[1], at[2] + 50.0]);
    near.group = 0xf;
    lv.tick(&near);
    lv.tick(&near);
    assert_eq!(lv.svc.sounds.iter().filter(|s| s.moby == c && s.index == 1 && s.flags == 0).count(), 1);
    // An external command (+0xbc) switches the collision off while it runs.
    lv.table.mobys[c].cmd = 1;
    lv.tick(&near);
    assert!(!lv.table.mobys[c].has_collision);
    // To the end of the path: hidden, then (no view: out of view; Ratchet not grinding) back at the start.
    let n = pts.len();
    p::set_ff(&mut lv.table.mobys[c].pvars, rail_car::pv::T, n as f32 - 1.5);
    let step = p::ff(&lv.table.mobys[c].pvars, rail_car::pv::STEP);
    let ticks = (0.5 / step) as usize + 2;
    for _ in 0..ticks { lv.tick(&near); }
    let m = &lv.table.mobys[c];
    assert_eq!((m.state, m.update_dist), (3, 0xff), "at the end");
    lv.table.mobys[c].visible = 0;
    lv.tick(&near);
    let m = &lv.table.mobys[c];
    assert_eq!(m.state, 3);
    assert!(m.mode & 1 != 0 && !m.has_collision, "hidden while Ratchet grinds");
    let walking = hero_at([at[0] + 3.0, at[1], at[2] + 50.0]);
    lv.tick(&walking);
    let m = &lv.table.mobys[c];
    assert_eq!(m.state, 1, "reset once out of view");
    assert_eq!((m.cmd, m.visible, m.mode & 1, m.has_collision), (0, 1, 0, true));
    assert_eq!(p::ff(&m.pvars, rail_car::pv::T), 0.0);
    assert_eq!(m.position, rc_game::path::pose(&pts, false, 0.0, true).0);
    eprintln!("rail cars: {} on level 16; car {c} ran path {} ({n} points) to its end and reset", cars.len(), p::i32(&m.pvars, rail_car::pv::PATH));
}

// ---------------------------------------------------------------------------------------------------
// U523: Kalebo's air traffic 1667–1671 (level 16)

#[test]
fn air_traffic_groups_spread_along_their_paths_and_keep_their_spacing() {
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let all: Vec<MobyId> = (1667..=1671).flat_map(|c| lv.of_class(c)).collect();
    assert_eq!(all.len(), 90);
    let far = hero_at([1.0, 1.0, 1.0]);
    let scale0: HashMap<MobyId, f32> = all.iter().map(|&i| (i, lv.table.mobys[i].scale)).collect();
    // One member's first update (state 0) places its whole group: checked before any flight.
    let first = all[0];
    let g = lv.table.mobys[first].group;
    assert!(g >= 0);
    let list = { let w = lv.world(&far); group_ids(&w, g) };
    let n = list.len();
    lv.run(&far, first, unit("U523"));
    let last = p::ff(&lv.table.mobys[first].pvars, kalebo_traffic::pv::LAST);
    assert_eq!(last, lv.svc.splines[p::i32(&lv.table.mobys[first].pvars, kalebo_traffic::pv::PATH) as usize].len() as f32 - 1.0);
    let mut seen_k: Vec<f32> = Vec::new();
    for &i in &list {
        let m = &lv.table.mobys[i];
        assert_eq!(m.state, 1, "vehicle {i}");
        assert_eq!((m.update_dist, m.draw_dist, m.has_collision), (0xff, kalebo_traffic::k::DRAW_DIST, false));
        assert_eq!(m.scale, scale0[&i] * kalebo_traffic::k::SCALE);
        assert_eq!(p::ff(&m.pvars, kalebo_traffic::pv::LAST), last);
        let ahead = p::i32(&m.pvars, kalebo_traffic::pv::AHEAD) as MobyId;
        assert!(list.contains(&ahead) && (ahead != i || n == 1), "vehicle {i} follows {ahead} of its group");
        let h = p::ff(&m.pvars, kalebo_traffic::pv::HEIGHT);
        assert!((-4.25..=4.25).contains(&h));
        let sp = p::ff(&m.pvars, kalebo_traffic::pv::SPEED);
        assert!((5.0 / 60.0..=15.0 / 60.0).contains(&sp) && sp == p::ff(&m.pvars, kalebo_traffic::pv::TARGET), "vehicle {i} speed {sp}");
        let t = p::ff(&m.pvars, kalebo_traffic::pv::T);
        let k = (t / (last / n as f32)).round();
        assert!((t - k * (last / n as f32)).abs() < 1e-3, "vehicle {i} t {t}");
        seen_k.push(k);
    }
    // The ahead pointers form one ring over the group, and the start slots are k·(count − 1)/n, k = 1..n, one each.
    let mut cur = list[0];
    for _ in 0..n { cur = p::i32(&lv.table.mobys[cur].pvars, kalebo_traffic::pv::AHEAD) as MobyId; }
    assert_eq!(cur, list[0], "group {g}: a ring");
    seen_k.sort_by(|a, b| a.partial_cmp(b).unwrap());
    for (k, t) in seen_k.iter().enumerate() { assert_eq!(*t, (k + 1) as f32, "group {g} slot {k}"); }
    // The load pass and a tick: every vehicle of every group is flying; the even classes started their loop sound
    // 0 (flags 4), the odd ones make none.
    lv.load_pass(&far);
    lv.tick(&far);
    let mut groups: HashMap<i8, usize> = HashMap::new();
    for &i in &all {
        let m = &lv.table.mobys[i];
        assert_eq!(m.state, 1, "vehicle {i}");
        *groups.entry(m.group).or_default() += 1;
        let even = m.o_class & 1 == 0;
        let n = lv.svc.sounds.iter().filter(|s| s.moby == i).count();
        assert_eq!(n, even as usize, "vehicle {i} sounds");
        if even { assert!(lv.svc.sounds.iter().any(|s| s.moby == i && s.index == 0 && s.flags == 4)); }
    }
    eprintln!("air traffic: {} vehicles in {} groups; group {g} at {:?}", all.len(), groups.len(), list.iter().take(4).map(|&i| lv.table.mobys[i].position).collect::<Vec<_>>());
    // Out of view (no view here), a vehicle more than 7 behind the one ahead jumps 100·dt further each tick.
    let t_before: Vec<f32> = list.iter().map(|&i| p::ff(&lv.table.mobys[i].pvars, kalebo_traffic::pv::T)).collect();
    lv.tick(&far);
    let mut jumped = 0;
    for (k, &i) in list.iter().enumerate() {
        let m = &lv.table.mobys[i];
        let t = p::ff(&m.pvars, kalebo_traffic::pv::T);
        let sp = p::ff(&m.pvars, kalebo_traffic::pv::SPEED);
        let mut d = t - t_before[k];
        if d < 0.0 { d += last; }
        if (d - (sp + kalebo_traffic::k::JUMP / 60.0)).abs() < 1e-2 { jumped += 1; } else { assert!((d - sp).abs() < 1e-2, "vehicle {i} moved {d} at speed {sp}"); }
    }
    eprintln!("group {g}: {jumped} of {n} jumped ahead unseen");
    // Two seconds of flight: t stays within the path; the positions follow the path at the height offset.
    for _ in 0..120 { lv.tick(&far); }
    for &i in &list {
        let m = &lv.table.mobys[i];
        let pts = &lv.svc.splines[p::i32(&m.pvars, kalebo_traffic::pv::PATH) as usize];
        let t = p::ff(&m.pvars, kalebo_traffic::pv::T);
        assert!(0.0 <= t && t <= last, "vehicle {i} t {t}");
        let sp = p::ff(&m.pvars, kalebo_traffic::pv::SPEED);
        let h = p::ff(&m.pvars, kalebo_traffic::pv::HEIGHT);
        // The pose of the t the move started from (the jump, when taken, replaced t after the move).
        let (pos, _) = rc_game::path::pose(pts, false, t - sp, true);
        let (pos_j, _) = rc_game::path::pose(pts, false, t - sp - kalebo_traffic::k::JUMP / 60.0, true);
        assert!((m.position[2] - (pos[2] + h)).abs() < 0.5 || (m.position[2] - (pos_j[2] + h)).abs() < 0.5, "vehicle {i} height");
    }
    // The exhaust: an odd-class vehicle drawn near the camera trails type-22 puffs on its tick parity.
    let odd = *list.iter().find(|&&i| lv.table.mobys[i].o_class & 1 == 1).unwrap();
    let at = lv.table.mobys[odd].position;
    let near = hero_at([at[0] + 10.0, at[1], at[2]]);
    let puffs0 = lv.svc.fx.part_spawns.get(&22).copied().unwrap_or(0);
    for _ in 0..4 {
        lv.table.mobys[odd].visible = 1;
        lv.tick(&near);
    }
    let puffs = lv.svc.fx.part_spawns.get(&22).copied().unwrap_or(0) - puffs0;
    assert!(puffs >= 1, "the exhaust puffs: {puffs}");
    eprintln!("vehicle {odd}: {puffs} exhaust puffs in 4 ticks near the camera");
}
