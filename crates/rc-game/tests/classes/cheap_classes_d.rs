//! The census "cheap win" units of round 3 (G-CLS-027; `rc_game::moby_update::classes::units`): each unit resolves to
//! its port on exactly its census levels with the census's created counts, and runs (headless on its level, or on a
//! synthetic table) covering the side effects its coverage table marks ported. The level tests skip when
//! `extracted/` is absent.
//!
//! `cargo xtask test-job --test classes --filter cheap_classes_d:: --nocapture` prints the per-unit survey.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, Moby, MobyId, MobyTable};
use rc_game::moby_update::classes::units::{self, anim_idler, blarg_clank_lift, board_sparkle, linked_slider, petal_door, popup_turret, switched_mover, trip_block};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys};
use rc_game::moby_update::services::{pvar as p, HitTemplate, World};
use rc_game::moby_update::{ClassTable, Services};
use rc_game::particles::BSphereView;
use rc_game::ps2v::Pf;
use rc_game::rng::Rng;
use std::collections::HashMap;

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// (unit row, class, the levels whose table runs it, created instances on them; 0: not checked).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[
    ("U203 1139", 1139, &[5, 16], 12),
    ("U440 30", 30, &[14], 27),
    ("U440 681", 681, &[14], 0),
    ("U212 1021", 1021, &[6, 17, 18], 14),
    ("U390 339", 339, &[12], 9),
    ("U248 1013", 1013, &[7], 0),
    ("U248 1013", 1014, &[7], 0),
    ("U248 1013", 1064, &[7], 0),
    ("U248 1013", 1065, &[7], 0),
    ("U329 1015", 1015, &[10], 0),
    ("U329 1015", 1282, &[10], 0),
    ("U162 1101", 1101, &[4], 0),
    ("U162 1101", 1102, &[4], 0),
    ("U162 1101", 1531, &[4], 0),
    ("U162 1101", 1532, &[4], 0),
    ("U487 1209", 1209, &[15, 17], 15),
    ("U211 911", 911, &[6], 14),
    ("U542 669", 669, &[17], 15),
    ("U349 1544", 1544, &[10], 14),
];

/// Created instances per unit (all its classes and levels), the census's counts.
const TOTALS: &[(&str, usize)] = &[("U203 1139", 12), ("U440 30", 27), ("U212 1021", 14), ("U390 339", 9), ("U248 1013", 6), ("U329 1015", 9), ("U162 1101", 8), ("U487 1209", 15), ("U211 911", 14), ("U542 669", 15), ("U349 1544", 14)];

/// Every unit's classes resolve to the unit on its levels (and to nothing else), with the census's created counts.
#[test]
fn units_resolve_on_their_levels() {
    let Some(_) = crate::common::overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut created: HashMap<(&str, i16), usize> = HashMap::new();
    let mut bad = Vec::new();
    for level in 0..19u32 {
        let p = ports(level).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(name, oc, levels, _) in EXPECTED {
            let u = unit(name);
            if levels.contains(&level) {
                if p.get(oc) != Some(u) { bad.push(format!("level {level:02} class {oc}: not the unit")); }
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                *created.entry((name, oc)).or_default() += n;
            } else if p.in_table(oc) && p.get(oc) == Some(u) {
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                bad.push(format!("level {level:02} class {oc}: also the unit ({n} created)"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    eprintln!("created instances now ported: {created:?}");
    for &(name, oc, _, n) in EXPECTED { if n > 0 { assert_eq!(created[&(name, oc)], n, "{name} class {oc}"); } }
    let mut by_unit: HashMap<&str, usize> = HashMap::new();
    for (&(name, _), &n) in &created { *by_unit.entry(name).or_default() += n; }
    eprintln!("by unit: {by_unit:?}");
    for &(name, n) in TOTALS { assert_eq!(by_unit[name], n, "{name}"); }
}

// ---------------------------------------------------------------------------------------------------
// Synthetic tables

pub(crate) struct Syn {
    pub(crate) t: MobyTable,
    pub(crate) hero: Hero,
    pub(crate) rng: Rng,
    pub(crate) classes: ClassTable,
    pub(crate) svc: Services,
}

pub(crate) fn syn(mobys: Vec<Moby>) -> Syn {
    Syn { t: MobyTable::new(mobys, 16), hero: Hero::new(), rng: Rng::new(), classes: ClassTable::default(), svc: Services::new(), }
}

impl Syn {
    pub(crate) fn w(&mut self, counter: u64) -> World<'_> { World::new(&mut self.t, &self.hero, &mut self.rng, &self.classes, &mut self.svc, counter) }
}

pub(crate) fn moby(o_class: i16, pvars: usize) -> Moby {
    let mut m = Moby { o_class, pvars: vec![0; pvars], ..Moby::default() };
    m.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    m.position[3] = 1.0;
    m
}

// ---------------------------------------------------------------------------------------------------
// U203: the hoverboard sparkles 1139

#[test]
fn board_sparkle_starts_three_sparks_on_the_board_in_view() {
    let mut m = moby(1139, 0);
    m.position = [10.0, 0.0, 0.0, 1.0];
    let mut s = syn(vec![m]);
    let view = BSphereView::from_camera([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.63, 0.63 * 0.775);
    {
        let mut w = s.w(0);
        board_sparkle::update(&mut w, 0);
        let m = w.m(0);
        assert_eq!((m.state, m.update_dist, m.mode & 0x41), (1, 0x80, 0x41));
        board_sparkle::update(&mut w, 0);
        assert!(w.svc.fx.part_spawns.is_empty(), "not on the board, no view");
    }
    s.hero.group = board_sparkle::BOARD_GROUP;
    {
        let mut w = s.w(1);
        board_sparkle::update(&mut w, 0);
        assert!(w.svc.fx.part_spawns.is_empty(), "no view: out of view");
        w.view = Some(&view);
        board_sparkle::update(&mut w, 0);
        assert_eq!(w.svc.fx.part_spawns.get(&rc_game::particles::type67::TYPE), Some(&3));
        assert!(w.svc.sounds.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------------
// U212: Blarg's petal doors 1021

#[test]
fn petal_door_makes_seven_petals_and_opens_with_them() {
    let mut door = moby(1021, 0x40);
    door.position = [100.0, 100.0, 50.0, 1.0];
    p::set_i32(&mut door.pvars, 0x1c, -1);
    let mut s = syn(vec![door]);
    s.hero.pos = rc_game::hero::physics::v4(100.0, 102.0, 50.0);
    let mut w = s.w(0);
    petal_door::update(&mut w, 0);
    assert_eq!(w.m(0).state, 2);
    let petals: Vec<MobyId> = (0..7).map(|k| (p::i32(&w.m(0).pvars, 0x24 + 4 * k) - 1) as usize).collect();
    for (k, &q) in petals.iter().enumerate() {
        let m = w.m(q);
        assert_eq!((m.o_class, m.state, m.update_dist, m.draw_dist), (1021, 1, 0x40, 0x40));
        assert!(rc_game::moby_update::creature::sub_rot(m.rotation[0], ((k + 1) as f32 * 45.0).to_radians()).abs() < 1e-4, "petal {k} turned");
    }
    for &q in &petals { petal_door::update(&mut w, q); }
    assert!(petals.iter().all(|&q| w.m(q).state == 3));
    let out = w.m(petals[1]).position;
    assert!((rc_game::moby_update::creature::dist3(out, [100.0, 100.0, 50.0, 1.0]) - petal_door::OUT).abs() < 1e-4, "stands out along row 2");
    petal_door::update(&mut w, 0);
    assert_eq!(w.m(0).state, 4, "Ratchet within 6: open");
    assert!(petals.iter().all(|&q| w.m(q).state == 5));
    assert_eq!(w.svc.sounds.iter().map(|e| e.index).collect::<Vec<_>>(), vec![0]);
    for _ in 0..120 {
        petal_door::update(&mut w, 0);
        for &q in &petals { petal_door::update(&mut w, q); }
    }
    assert_eq!(w.m(0).state, 6);
    assert!(petals.iter().all(|&q| w.m(q).state == 7 && w.m(q).mode & mode::HIDDEN != 0 && w.m(q).visible == 0));
}

/// The Clank lift's field_open / field_lower / field_close (0x2f54a0 / 0x2f5360 / 0x2f53e8) read a door's seven
/// petals from its pvars +0x24..+0x3c: the slots `petal_door` filled with the moby index + 1 (0 = none). Reading
/// them raw skips the door's own first petal and hits the next moby instead (the next door's first petal: the G34
/// airlock doors desynced by one petal each).
#[test]
fn clank_lift_fields_act_on_the_doors_own_seven_petals() {
    // Two doors, far from the hero and unlinked: their own triggers never fire.
    let mut a = moby(1021, 0x40);
    a.position = [100.0, 100.0, 50.0, 1.0];
    let mut b = moby(1021, 0x40);
    b.position = [200.0, 200.0, 50.0, 1.0];
    for d in [&mut a, &mut b] { p::set_i32(&mut d.pvars, 0x1c, -1); }
    let mut s = syn(vec![a, b]);
    s.hero.pos = rc_game::hero::physics::v4(0.0, 0.0, 0.0);
    let petals = |w: &World, d: MobyId| -> Vec<MobyId> { (0..7).map(|k| (p::i32(&w.m(d).pvars, 0x24 + 4 * k) - 1) as usize).collect() };
    {
        let mut w = s.w(0);
        petal_door::update(&mut w, 0);
        petal_door::update(&mut w, 1);
    }
    let (pa, pb) = {
        let w = s.w(1);
        (petals(&w, 0), petals(&w, 1))
    };
    // The petals' state-1 pass, then both doors swung open by the lift.
    {
        let mut w = s.w(2);
        for &q in pa.iter().chain(&pb) { petal_door::update(&mut w, q); }
        blarg_clank_lift::field_lower(&mut w, Some(0));
        blarg_clank_lift::field_lower(&mut w, Some(1));
    }
    for t in 0..120 {
        let mut w = s.w(3 + t);
        for q in [0, 1].into_iter().chain(pa.iter().chain(&pb).copied()) { petal_door::update(&mut w, q); }
    }
    {
        let w = s.w(124);
        assert!(pa.iter().chain(&pb).all(|&q| w.m(q).state == 7), "both doors open");
    }
    // Leaving Clank: door 0 reset shut. Every one of its own petals, petal 0 included, shuts with it; door 1 and
    // its petals stay open.
    {
        let mut w = s.w(125);
        blarg_clank_lift::field_open(&mut w, Some(0));
        let w = s.w(126);
        assert_eq!((w.m(0).state, w.m(0).mode & mode::HIDDEN), (2, 0), "door 0 shut");
        for (k, &q) in pa.iter().enumerate() {
            assert_eq!((w.m(q).state, w.m(q).mode & mode::HIDDEN), (3, 0), "door 0 petal {k} shut");
        }
        assert_eq!((w.m(1).state, w.m(1).mode & mode::HIDDEN), (6, mode::HIDDEN), "door 1 still open");
        for (k, &q) in pb.iter().enumerate() {
            assert_eq!((w.m(q).state, w.m(q).mode & mode::HIDDEN), (7, mode::HIDDEN), "door 1 petal {k} still open");
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// U390: Hoven's animated idlers 339

#[test]
fn anim_idler_waits_then_plays_a_clip() {
    let mut s = syn(vec![moby(339, 0)]);
    {
        let mut w = s.w(0);
        anim_idler::update(&mut w, 0);
        assert_eq!(w.m(0).state, 1);
        w.mm(0).cmd = 1;
        for _ in 0..10 { anim_idler::update(&mut w, 0); }
        assert_eq!((w.m(0).state, w.m(0).cmd), (1, 1), "Ratchet within 9 holds it in sequence 0");
    }
    s.hero.pos = rc_game::hero::physics::v4(50.0, 0.0, 0.0);
    let mut w = s.w(1);
    anim_idler::update(&mut w, 0);
    assert_eq!(w.m(0).state, 2, "the wait done: a clip");
    anim_idler::update(&mut w, 0);
    assert_eq!(w.m(0).state, 2, "the clip plays until it wraps");
    w.mm(0).anim.flags = 2;
    anim_idler::update(&mut w, 0);
    assert_eq!(w.m(0).state, 1);
    assert!((w.m(0).cmd as i32) < 180);
}

// ---------------------------------------------------------------------------------------------------
// U248: the Pokitaru panels on a linked moby's state

#[test]
fn linked_slider_slides_out_and_home_with_its_sounds() {
    let mut panel = moby(1013, 0x20);
    panel.mission = 0xff;
    panel.pvars[0xe] = 3;
    panel.pvars[0xf] = 4;
    p::set_i32(&mut panel.pvars, 0x10, 1);
    p::set_i16(&mut panel.pvars, 0x14, -1);
    p::set_i16(&mut panel.pvars, 0x16, 1);
    p::set_i32(&mut panel.pvars, 0x18, 2);
    p::set_ff(&mut panel.pvars, 0x1c, 0.1);
    let link = Moby { o_class: 1, state: 1, ..Moby::default() };
    let mut s = syn(vec![panel, link]);
    let mut w = s.w(0);
    linked_slider::update(&mut w, 0);
    linked_slider::update(&mut w, 0);
    assert_eq!(w.m(0).state, 2, "link in A: start sound, out");
    for _ in 0..11 { linked_slider::update(&mut w, 0); }
    assert!(w.m(0).position[1] > 0.1, "1013 slides toward +y");
    linked_slider::update(&mut w, 0);
    assert_eq!(w.m(0).state, 3, "past the travel: stop sound");
    w.mm(1).state = 2;
    linked_slider::update(&mut w, 0);
    assert_eq!(w.m(0).state, 4, "link in B: back");
    for _ in 0..20 { linked_slider::update(&mut w, 0); }
    assert_eq!(w.m(0).state, 1);
    assert_eq!(w.m(0).position[1], 0.0);
    assert_eq!(w.svc.sounds.iter().map(|e| e.index).collect::<Vec<_>>(), vec![3, 4, 3, 4]);
}

// ---------------------------------------------------------------------------------------------------
// U329: Orxon's trip blocks

#[test]
fn trip_block_rises_on_ratchet_in_its_box_and_sets_its_flag() {
    let mut b = moby(1015, 0x18);
    b.position = [50.0, 50.0, 10.0, 1.0];
    p::set_i32(&mut b.pvars, 0x10, -1);
    p::set_i32(&mut b.pvars, 0x14, 3);
    let mut s = syn(vec![b]);
    s.svc.interact.game.flags.resize(0x80, 0);
    s.hero.pos = rc_game::hero::physics::v4(51.0, 50.2, 30.0);
    let mut w = s.w(0);
    w.camera = [Pf::ZERO; 4];
    trip_block::update(&mut w, 0);
    assert_eq!(w.m(0).state, 2);
    w.mm(0).state = 1;
    trip_block::update(&mut w, 0);
    assert_eq!(w.m(0).state, 3, "Ratchet in the box (any z)");
    for _ in 0..25 { trip_block::update(&mut w, 0); }
    assert_eq!((w.m(0).state, w.m(0).position[2]), (4, 14.0));
    assert_eq!(w.svc.interact.game.flags[trip_block::FLAG_BASE + 3], 1);
}

// ---------------------------------------------------------------------------------------------------
// U162: Eudora's switched movers

#[test]
fn switched_movers_move_one_second_and_set_their_death_bits() {
    let mk = |oc: i16, id: i16| {
        let mut m = moby(oc, 0x10);
        m.spawn_id = id;
        p::set_i32(&mut m.pvars, 0, 4);
        m
    };
    let switch = Moby { o_class: 0x267, state: 3, ..Moby::default() };
    let mut s = syn(vec![mk(1101, 1), mk(1102, 2), mk(1531, 3), mk(1532, 4), switch]);
    s.svc.level = 4;
    let mut w = s.w(0);
    for id in 0..4 { switched_mover::update(&mut w, id); switched_mover::update(&mut w, id); }
    assert!((0..4).all(|id| w.m(id).state == 1), "the switch not in state 4");
    w.mm(4).state = 4;
    for _ in 0..61 { for id in 0..4 { switched_mover::update(&mut w, id); } }
    let z: Vec<f32> = (0..4).map(|id| w.m(id).position[2]).collect();
    eprintln!("z {z:?}");
    assert!((0..4).all(|id| w.m(id).state == 3));
    assert!((z[0] - 1.3).abs() < 0.03 && (z[1] + 2.05).abs() < 0.05 && (z[2] + 2.55).abs() < 0.05 && (z[3] - 1.3).abs() < 0.03);
    assert!((1..=4).all(|i| w.svc.save.death.contains(&(4, i)) && w.svc.save.death_level.contains(&i)));
    let snd: Vec<(i32, i16, i16)> = w.svc.sounds.iter().map(|e| (e.index, e.o_class, e.sound_class)).collect();
    assert_eq!(snd, vec![(0, 1101, 0x44d), (0, 1532, 0x44d), (1, 1101, 1101)]);
}

// ---------------------------------------------------------------------------------------------------
// U440: Oltanis's pop-up turrets 30 and their shots 681 (headless on level 14)

pub(crate) struct Lv {
    pub(crate) table: MobyTable,
    pub(crate) classes: ClassTable,
    pub(crate) svc: Services,
    pub(crate) rng: Rng,
    pub(crate) mesh: collision::Collision,
    pub(crate) counter: u64,
    pub(crate) missions: rc_game::moby_update::services::LevelMissions,
    pub(crate) parts: rc_game::particles::Particles,
}

pub(crate) fn load(level: u32) -> Option<Lv> {
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
    Some(Lv { table, classes, svc, rng: Rng::new(), mesh, counter: 0, missions: rc_game::moby_update::services::LevelMissions::fresh_load(level, [0xff; 16]), parts: rc_game::particles::Particles::new(None, Vec::new()) })
}

impl Lv {
    pub(crate) fn world<'a>(&'a mut self, hero: &'a Hero) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, self.counter);
        w.coll = Some(&self.mesh);
        w.camera = hero.pos;
        w.missions = &self.missions;
        w.particles = Some(&mut self.parts);
        w
    }
    pub(crate) fn run(&mut self, hero: &Hero, id: MobyId, f: fn(&mut World, MobyId)) {
        self.counter += 1;
        let mut w = self.world(hero);
        f(&mut w, id);
    }
    pub(crate) fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
}

pub(crate) fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h.body_point = rc_game::hero::physics::v4(p[0], p[1], p[2] + 0.7);
    h.shadow_point = h.pos;
    h
}

#[test]
fn popup_turret_rises_wakes_fires_and_blows_up() {
    let Some(mut lv) = load(14) else { eprintln!("skipped"); return };
    let turrets = lv.of_class(30);
    assert_eq!(turrets.len(), 27);
    let far = hero_at([1.0, 1.0, 1.0]);
    for &t in &turrets { lv.run(&far, t, popup_turret::update); }
    let states: Vec<u8> = turrets.iter().map(|&t| lv.table.mobys[t].state).collect();
    eprintln!("turret states after init {states:?}");
    let t = turrets.iter().copied().find(|&t| lv.table.mobys[t].state == 1).expect("a sunk turret");
    let home = p::ff(&lv.table.mobys[t].pvars, popup_turret::pv::HOME_Z);
    assert!((lv.table.mobys[t].position[2] - (home - popup_turret::k::SINK)).abs() < 1e-5 && !lv.table.mobys[t].has_collision);
    // Told to rise.
    p::set_i16(&mut lv.table.mobys[t].pvars, popup_turret::pv::RAISE, 1);
    p::set_i32(&mut lv.table.mobys[t].pvars, popup_turret::pv::REARM_T, 1);
    lv.run(&far, t, popup_turret::update);
    assert_eq!(lv.table.mobys[t].state, 2);
    for _ in 0..61 { lv.run(&far, t, popup_turret::update); }
    let m = &lv.table.mobys[t];
    assert_eq!((m.state, m.position[2]), (3, home), "risen");
    assert!(m.mode & mode::TARGETABLE != 0);
    // Ratchet (and his moby) in the waking cuboid wakes it; it fires.
    let cub = p::i32(&lv.table.mobys[t].pvars, popup_turret::pv::WAKE_CUBOID);
    let c = lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, cub).unwrap().centre();
    let hero = hero_at(c);
    if let Some(h) = lv.table.mobys.iter().position(|m| m.o_class == 0) { lv.table.mobys[h].position = [c[0], c[1], c[2], 1.0]; }
    lv.run(&hero, t, popup_turret::update);
    assert_eq!(lv.table.mobys[t].state, 4, "woken");
    for _ in 0..40 { lv.run(&hero, t, popup_turret::update); }
    let shots = lv.of_class(681);
    eprintln!("shots {}", shots.len());
    assert!(!shots.is_empty(), "the turret fired");
    assert_eq!(lv.table.mobys[shots[0]].ambient, [0xe0, 0xe0, 0xe0, 0]);
    // The shots fly until they hit or leave their 30 of reach.
    for _ in 0..200 { for &s in &shots { if lv.table.mobys[s].state < 0x80 { lv.run(&hero, s, popup_turret::shot_update); } } }
    assert!(shots.iter().all(|&s| lv.table.mobys[s].state >= 0x80), "every shot ended");
    // A hit that takes its health blows it up.
    let health = p::ff(&lv.table.mobys[t].pvars, popup_turret::pv::RECORD);
    let tmpl = HitTemplate { dir: [Pf::ZERO; 4], attacker: None, flags: 0x10_0000, b18: 0, b19: 0, h1a: 0, damage: Pf::f(health + 10.0), w20: 0 };
    { let mut w = lv.world(&hero); w.deliver_hit(t, &tmpl); }
    let flashes = lv.svc.fx.flashes;
    lv.run(&hero, t, popup_turret::update);
    assert!(lv.table.mobys[t].state >= 0x80 && lv.svc.fx.flashes > flashes, "killed (health {health}): the beam explosion in the same update, deleted");
    let b2 = lv.table.mobys[t].spawn_id;
    assert!(b2 < 0 || lv.svc.save.death.contains(&(14, b2)), "its death bits");
}

/// Gemlik's fighters 111 in Qwark's shield phase (his command 4): every one comes out and flies with a real position.
/// A fighter appears on its path node with the last node the same; the move's `a / l` is then 0 / 0, +MAX on the EE (it
/// clamps to 1), where an `f32` NaN left six of the eight nowhere and Qwark's shield up for good.
#[test]
fn gemlik_fighters_all_fly_in_the_shield_phase() {
    use rc_game::moby_update::classes::units::gemlik_fighter;
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let fs = lv.of_class(111);
    assert_eq!(fs.len(), 8);
    let q = lv.of_class(388)[0];
    let qp = lv.table.mobys[q].position;
    let hero = hero_at([qp[0] - 60.0, qp[1], qp[2]]);
    for &f in &fs { lv.run(&hero, f, gemlik_fighter::update); }
    lv.table.mobys[q].cmd = 4;
    for _ in 0..300 {
        for &f in &fs { lv.run(&hero, f, gemlik_fighter::update); }
    }
    for &f in &fs {
        let m = &lv.table.mobys[f];
        assert_eq!(m.state, 2, "fighter {f} flying");
        assert!(m.position[..3].iter().all(|x| x.is_finite()), "fighter {f} at {:?}", m.position);
    }
}
