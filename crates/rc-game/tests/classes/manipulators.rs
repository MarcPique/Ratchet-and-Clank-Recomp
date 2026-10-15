//! The class manipulators on real level data (`rc_game::moby_update::manip`, docs/plan/moby_animation.md §9): the
//! loader's joint-list targets reach `AttachManipulator`, and each consumer's nodes act on the joints the class data
//! names: the vendor 11's hologram 1143 (Novalis), the talking NPC 774's look-at (Novalis), the Gadgetron logos 1143
//! (Kalebo, U514), the searchlights 823 (Rilgar, U180) and the floats 481 (Eudora, U155). Skipped when `extracted/` is
//! absent.

use rc_formats::moby_anim::{parse_sequences, MobyAnimClass};
use rc_formats::{collision, gadget, gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId, MobyTable};
use rc_game::moby_update::classes::{self, units, vendor, LevelPorts};
use rc_game::moby_update::scheduler::{class_info, load_level_mobys};
use rc_game::moby_update::services::World;
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use std::collections::HashMap;

struct Lv {
    table: MobyTable,
    classes: ClassTable,
    svc: Services,
    rng: Rng,
    mesh: collision::Collision,
    ports: LevelPorts,
    /// Per class, each joint list's target (the loader's `Services::joint_targets` for every class).
    targets: HashMap<i16, Vec<u8>>,
}

fn load(level: u32) -> Option<Lv> {
    let core = rc_formats::test_data::core(level)?;
    let gp = rc_formats::test_data::gameplay(level)?;
    let ports = crate::common::ports(level, &[])?;
    let mesh = collision::parse_collision(&core.core, &core.data).unwrap();
    let instances = gameplay::parse_moby_instances(&gp).unwrap();
    let tests = moby_spawn::loader_spawns(&instances, &mut moby_spawn::SpawnSave::default());
    let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
    let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
    let rd = |o: usize| i32::from_le_bytes(gp[o..o + 4].try_into().unwrap());
    let spawnable = rd(rd(0x44) as usize + 4).max(1) as usize;
    let mut classes = ClassTable::default();
    let (mut joints, mut targets, mut all) = (HashMap::new(), HashMap::new(), HashMap::new());
    for (slot, e) in core.core.moby_classes.iter().enumerate() {
        let oc = e.o_class as i16;
        let parsed = core.block(&format!("moby_class/{:04}", e.o_class)).and_then(|b| rc_formats::moby::parse_moby_class(b).ok().map(|c| (b, c)));
        if let Some((blob, c)) = parsed {
            let anim = MobyAnimClass::new(&c, parse_sequences(blob, &c).unwrap_or_default());
            let info = class_info(&c, slot as u8, ports.update_fn(oc));
            let lists: Vec<(Vec<u8>, Vec<u8>)> = (0..16).map_while(|l| gadget::joint_list(blob, &c.header, l).ok()).collect();
            let t: Vec<u8> = lists.iter().map(|(_, s)| rc_formats::moby_anim::list_target(s).unwrap_or(0xff)).collect();
            if ports.needs_joint_lists(oc) {
                joints.insert(oc, lists.into_iter().map(|(a, _)| a).collect::<Vec<_>>());
                targets.insert(oc, t.clone());
            }
            classes.classes.insert(oc, (info, Some(anim)));
            all.insert(oc, t);
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
    svc.joint_lists = joints;
    svc.joint_targets = targets;
    svc.build_grid(&mut table);
    Some(Lv { table, classes, svc, rng: Rng::new(), mesh, ports, targets: all })
}

impl Lv {
    fn world<'a>(&'a mut self, hero: &'a Hero, counter: u64) -> World<'a> {
        let mut w = World::new(&mut self.table, hero, &mut self.rng, &self.classes, &mut self.svc, counter);
        w.coll = Some(&self.mesh);
        w.camera = hero.pos;
        w
    }
    fn of_class(&self, oc: i16) -> Vec<MobyId> { self.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == oc && m.state < 0x80).map(|(i, _)| i).collect() }
    /// `n` updates of every moby of `oc` (their own update, directly).
    fn run(&mut self, hero: &Hero, oc: i16, n: u64) {
        let u = self.ports.get(oc).unwrap_or_else(|| panic!("class {oc} has a port"));
        for t in 0..n {
            for id in self.of_class(oc) {
                let mut w = self.world(hero, t);
                classes::dispatch(u, &mut w, id);
            }
        }
    }
    fn target(&self, oc: i16, list: usize) -> u8 { self.targets[&oc][list] }
}

fn hero_at(p: [f32; 3]) -> Hero {
    let mut h = Hero::new();
    h.pos = rc_game::hero::physics::v4(p[0], p[1], p[2]);
    h.body_point = rc_game::hero::physics::v4(p[0], p[1], p[2] + 0.7);
    h
}

/// The loader fills the targets for exactly the classes a port needs them for (the vendor's hologram with it).
#[test]
fn targets_are_loaded_for_the_manipulator_classes() {
    let Some(lv) = load(1) else { eprintln!("skipped"); return };
    for oc in [vendor::HOLOGRAM, 774, 172] {
        assert!(lv.ports.needs_joint_lists(oc), "class {oc}");
        assert_eq!(lv.svc.joint_targets.get(&oc), lv.targets.get(&oc), "class {oc}");
    }
}

/// Novalis: the vendor creates its hologram and links its two nodes on the hologram's lists 0 / 1; they turn by the
/// phases each tick.
#[test]
fn novalis_vendor_turns_its_hologram() {
    let Some(mut lv) = load(1) else { eprintln!("skipped"); return };
    let v = lv.of_class(11)[0];
    let far = hero_at([1.0, 1.0, 1.0]);
    lv.run(&far, 11, 3);
    let (c, _, _) = vendor::hologram(&lv.table, v).expect("the hologram");
    let want = [lv.target(vendor::HOLOGRAM, 1), lv.target(vendor::HOLOGRAM, 0)];
    let m = &lv.table.mobys[c];
    assert_eq!(m.joint_mods.iter().map(|n| n.joint).collect::<Vec<_>>(), want.to_vec());
    let spin = lv.svc.interact.vendor.spin;
    // The quaternion of the last tick's phase (the phase moved on by ±0.01 after it).
    let q0 = rc_game::hero::idle::axis_quat(rc_game::moby_update::interact::add_rot(spin[0], -0.01), 2);
    assert_eq!(m.joint_mods[1].quat, q0);
    println!("vendor {v} at {:?} yaw {}: hologram {c}, nodes on joints {want:?}", lv.table.mobys[v].position, lv.table.mobys[v].rotation[2]);
}

/// Novalis: the Water Pump Worker looks at Ratchet standing in front of him (both records linked, the head turned
/// toward him), and turns back when he leaves.
#[test]
fn novalis_npc_looks_at_ratchet() {
    let Some(mut lv) = load(1) else { eprintln!("skipped"); return };
    let n = lv.of_class(774)[0];
    let (p, yaw) = (lv.table.mobys[n].position, lv.table.mobys[n].rotation[2]);
    // 3 units in front, 1 to the side (so the head turns).
    let (c, s) = (yaw.cos(), yaw.sin());
    let front = hero_at([p[0] + 3.0 * c - s, p[1] + 3.0 * s + c, p[2]]);
    lv.run(&front, 774, 40);
    let m = &lv.table.mobys[n];
    let joints: Vec<u8> = m.joint_mods.iter().map(|x| x.joint).collect();
    assert!(joints.contains(&lv.target(774, 0)) && joints.contains(&lv.target(774, 1)), "{joints:?}");
    assert!(m.joint_mods.iter().all(|x| x.quat[3] < 1.0), "the head is turned");
    println!("npc {n} at {p:?} yaw {yaw}: nodes {joints:?}, quats {:?}", m.joint_mods.iter().map(|x| x.quat).collect::<Vec<_>>());
}

/// Kalebo III: the ten Gadgetron logos turn their yaw and spin list 1's joint the other way.
#[test]
fn kalebo_logos_spin() {
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let logos = lv.of_class(1143);
    assert_eq!(logos.len(), 10);
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.run(&hero, 1143, 2);
    let j = lv.target(1143, 1);
    for &id in &logos {
        let m = &lv.table.mobys[id];
        assert_eq!(m.joint_mods.iter().map(|x| x.joint).collect::<Vec<_>>(), vec![j]);
        assert_ne!(m.joint_mods[0].quat, [0.0, 0.0, 0.0, 1.0]);
    }
    assert_eq!(units::PORTS.iter().filter(|u| u.unit == "U514 1143").count(), 1);
}

/// Rilgar: the searchlights tilt their head (list 0) and register the beam from its joint point.
#[test]
fn rilgar_searchlights_tilt_and_draw_their_beam() {
    let Some(mut lv) = load(5) else { eprintln!("skipped"); return };
    let lights = lv.of_class(823);
    assert!(!lights.is_empty());
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.run(&hero, 823, 2);
    let i = units::PORTS.iter().position(|u| u.unit == "U180 823").unwrap() as u16;
    for &id in &lights {
        let m = &lv.table.mobys[id];
        assert_eq!(m.joint_mods.iter().map(|x| x.joint).collect::<Vec<_>>(), vec![lv.target(823, 0)]);
        let q = units::fx_quads(&lv.table, &lv.svc, i, id).expect("the beam");
        assert_eq!(q.quads.len(), 20);
        // The beam starts at the head's joint point, above the moby's origin.
        let head = lv.svc.draw_callbacks.matrices[&id][3];
        assert!(head[2] > m.position[2], "head {head:?} above {:?}", m.position);
    }
}

/// Eudora: the floats link three records (lists 0..2) and turn their parts.
#[test]
fn eudora_floats_spin_their_parts() {
    let Some(mut lv) = load(4) else { eprintln!("skipped"); return };
    let floats = lv.of_class(481);
    assert!(!floats.is_empty());
    let hero = hero_at([1.0, 1.0, 1.0]);
    lv.run(&hero, 481, 3);
    let want: Vec<u8> = (0..3).rev().map(|l| lv.target(481, l)).collect();
    for &id in &floats {
        assert_eq!(lv.table.mobys[id].joint_mods.iter().map(|x| x.joint).collect::<Vec<_>>(), want);
    }
}
