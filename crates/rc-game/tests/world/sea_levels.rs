//! The seas and liquid surfaces on every level (docs/plan/world_animation.md §3.3, `rc_game::water::sea`): the inventory
//! of the sea ports by code identity (which levels run which port, their data read from the level's overlay), and each
//! port's update run headless on its level (init, registration on the game's list, the state it keeps). Skipped without
//! `extracted/`.

use rc_formats::gameplay;
use rc_formats::level_overlay::LevelOverlay;
use rc_game::hero::Hero;
use rc_game::moby_runtime::{Moby, MobyTable};
use rc_game::moby_update::classes::draw_callbacks::Callback;
use rc_game::moby_update::classes::{dispatch, ClassUpdate};
use rc_game::moby_update::services::World;
use rc_game::moby_update::{ClassTable, Services};
use rc_game::rng::Rng;
use rc_game::water::sea::{self, SeaData, PORTS};
use rc_game::water::world::{LevelWaterData, WaterWorld};
use std::sync::Arc;

fn overlay(level: u32) -> Option<Arc<LevelOverlay>> { crate::common::overlay(level) }

fn water_data(level: u32) -> LevelWaterData { crate::common::water_data(level).unwrap() }

/// The sea ports each level has (port indices into `PORTS`): the liquid grids on 03, 05, 07, 08, 09, 14, the ocean 1111
/// on 11 and 16 (the same code), the Hoven liquid on 12; 07's 460 and 09's 317 run one function (port 2). The liquid
/// mesh users (batch 5, `MeshSet`): 8 = Aridia's 854, 9 = Hoven's 293 strips, 10 = level 14's 1418, 11 = Novalis' 1848;
/// 12 / 13 = Pokitaru's pool overlays 1903 / 1919; 14–18 = Kalebo's reflections 1943, 1947, 1948, 1952, 1953; 21–26 =
/// Veldin Orbit's glass 1907.. (1952's code, their own draws).
const EXPECTED: [(u32, &[usize]); 12] = [(1, &[11]), (2, &[8]), (3, &[0]), (5, &[1]), (7, &[2, 3]), (8, &[4, 19, 20]), (9, &[2]), (11, &[6, 12, 13]), (12, &[7, 9]), (14, &[5, 10]), (16, &[6, 14, 15, 16, 17, 18]), (17, &[21, 22, 23, 24, 25, 26])];

#[test]
fn sea_inventory_all_levels() {
    if overlay(1).is_none() { eprintln!("skipped: no extracted/"); return; }
    for level in 0..19u32 {
        let d = water_data(level);
        let ports = crate::common::ports(level, &[]).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let mut found: Vec<usize> = d.sea.iter().map(|p| p.port).collect();
        found.sort();
        let want = EXPECTED.iter().find(|(l, _)| *l == level).map_or(&[][..], |e| e.1);
        assert_eq!(found, want, "level {level:02}: sea ports");
        for p in &d.sea {
            // The level's class table runs the port for a placed class.
            let classes: Vec<i32> = overlay(level).unwrap().vtbl().into_iter().filter(|e| ports.get(e.o_class as i16) == Some(ClassUpdate::Sea(p.port as u8))).map(|e| e.o_class).collect();
            let placed = inst.iter().filter(|m| classes.contains(&m.o_class)).count();
            assert!(placed >= 1, "level {level:02}: port {} ({}) has a placed instance (classes {classes:?})", p.port, PORTS[p.port].name);
            match &p.data {
                SeaData::Grid(g) => {
                    let r = &g.grid;
                    let [bx, by] = r.blocks();
                    assert_eq!(r.flags.len(), bx * by);
                    assert_eq!(r.colours.len(), bx * by);
                    assert_eq!(r.cull_dist2, 160_000.0, "level {level:02}: the block cull distance");
                    // The strip covers every one of the block's 8×8 points.
                    let mut seen = [false; 64];
                    for &o in &g.module.order { seen[o as usize] = true; }
                    assert!(seen.iter().all(|&s| s), "level {level:02}: the strip order covers the block");
                    eprintln!(
                        "level {level:02} {}: origin {:?} cell {:?} {}×{} blocks ({} with water) fog {:?} {:02x?} anim FX {}+{} / {} ticks",
                        PORTS[p.port].name, r.origin, r.cell, bx, by, r.flags.iter().filter(|&&f| f != 0).count(), r.fog, r.fog_rgb, r.tex, r.frames, r.period
                    );
                }
                SeaData::Ocean(t) => eprintln!("level {level:02} {}: {t:?}", PORTS[p.port].name),
                SeaData::Hoven(h) => {
                    assert_eq!((h.groups[0].len(), h.groups[1].len()), (10, 5));
                    assert!(h.groups.iter().flatten().all(|s| s.pos.len() == s.st.len() && s.pos.len() == s.rgba.len()));
                    eprintln!("level {level:02} {}: FIX2 {:#x} speeds {:?} {:?}", PORTS[p.port].name, h.fix2, h.g1_speed, h.layer_speed);
                }
                // The G-REN-026 ports (2026-10-01): checked by their own request (docs/test-requests/2026-10-01-water-liquids.md).
                SeaData::Meshes(m) => {
                    if let sea::SeaKind::Reflect(r) = PORTS[p.port].kind {
                        assert_eq!(m.meshes.len(), r.meshes, "level {level:02} {}: meshes", PORTS[p.port].name);
                        assert!(m.meshes.iter().flat_map(|m| &m.strips).all(|s| !s.pos.is_empty()), "level {level:02} {}: empty strip", PORTS[p.port].name);
                        eprintln!("level {level:02} {}: {} meshes, {} vertices", PORTS[p.port].name, m.meshes.len(), m.meshes.iter().flat_map(|m| &m.strips).map(|s| s.pos.len()).sum::<usize>());
                    }
                }
                SeaData::GridSet(_) => {}
                SeaData::Pool(d) => {
                    let sea::SeaKind::Pool(m) = PORTS[p.port].kind else { panic!("pool data on a non-pool port") };
                    assert_eq!(d.layers.each_ref().map(Vec::len), m.layers.map(|l| l.4));
                    assert!(d.layers.iter().flatten().all(|s| !s.pos.is_empty() && s.pos.len() == s.st.len() && s.pos.len() == s.rgba.len()));
                    eprintln!("level {level:02} {}: centre {:?} FIX bases {:?} speeds {:?} {:?} {:?}", PORTS[p.port].name, d.centre, d.fix, d.l0_speed, d.l1_speed, d.l2_speed);
                }
            }
        }
    }
}

/// One port's placed instance run alone for `ticks` ticks at camera `cam`: the moby, the services after the run and
/// the callbacks registered on the last tick.
fn run(level: u32, port: usize, cam: [f32; 3], ticks: u64) -> (Moby, Services, Vec<(bool, Callback)>) {
    let d = water_data(level);
    let ports = crate::common::ports(level, &[]).unwrap();
    let gp = rc_formats::test_data::gameplay(level).unwrap();
    let inst = gameplay::parse_moby_instances(&gp).unwrap();
    let pvars = gameplay::parse_pvars(&gp).unwrap();
    let i = inst.iter().find(|m| ports.get(m.o_class as i16) == Some(ClassUpdate::Sea(port as u8))).expect("placed instance");
    let m = Moby { o_class: i.o_class as i16, position: [i.position[0], i.position[1], i.position[2], 1.0], pvars: i.pvar(&pvars).map(<[u8]>::to_vec).unwrap_or_default(), ..Default::default() };
    let mut table = MobyTable::new(vec![m], 1);
    let hero = Hero::default();
    let classes = ClassTable::default();
    let mut svc = Services::new();
    svc.set_volumes(rc_formats::volumes::parse_volumes(&gp).unwrap());
    svc.water = WaterWorld::new(d);
    let mut rng = Rng::new();
    let mut last = Vec::new();
    for k in 0..ticks {
        svc.draw_callbacks = Default::default();
        let mut w = World::new(&mut table, &hero, &mut rng, &classes, &mut svc, k);
        w.camera = [cam[0] + k as f32 * 0.1, cam[1], cam[2], 1.0].map(rc_game::ps2v::Pf::f);
        dispatch(ClassUpdate::Sea(port as u8), &mut w, 0);
        last = svc.draw_callbacks.ties.iter().map(|e| (true, e.0)).chain(svc.draw_callbacks.list1.iter().map(|e| (false, e.0))).collect();
    }
    (table.mobys[0].clone(), svc, last)
}

#[test]
fn sea_ports_run_on_their_levels() {
    if overlay(1).is_none() { eprintln!("skipped: no extracted/"); return; }
    // (level, port, camera, on the after-ties list)
    for (level, port, cam, ties) in [
        (3u32, 0usize, [300.0f32, 200.0, 60.0], false),
        (5, 1, [300.0, 60.0, 90.0], true),
        (7, 2, [300.0, 300.0, 90.0], true),
        (8, 4, [300.0, 200.0, 70.0], false),
        (9, 2, [300.0, 100.0, 90.0], true),
        (14, 5, [300.0, 200.0, 80.0], false),
        (11, 6, [470.0, 380.0, 245.0], false),
        (16, 6, [250.0, 150.0, 140.0], false),
        (12, 7, [300.0, 280.0, 70.0], true),
    ] {
        let (m, svc, last) = run(level, port, cam, 400);
        let r = svc.water.sea.run[port];
        assert_eq!(last, vec![(ties, Callback::Sea(port as u8))], "level {level:02} port {port}: registered on its list");
        assert!(r.inited || matches!(PORTS[port].kind, sea::SeaKind::Ocean), "level {level:02}: init ran");
        if port != 4 { assert_eq!((m.state, m.update_dist), (1, 0xff), "level {level:02}: state 1, always updated"); }
        match (PORTS[port].kind, sea::data(&svc.water, port).unwrap()) {
            (sea::SeaKind::Grid(g), SeaData::Grid(d)) => {
                assert_eq!(r.fix, g.fix);
                if port == 1 {
                    // 879: 59.5 + 0.25·sin at tick 399 (counter % 360 = 39).
                    assert!((r.z - (59.5 + 0.25 * sea::bob(399))).abs() < 1e-5, "Rilgar sea z {}", r.z);
                } else {
                    assert_eq!(r.z, d.grid.origin[2]);
                }
            }
            (sea::SeaKind::Ocean, SeaData::Ocean(_)) => {
                let base = f32::from_le_bytes(m.pvars[0x14..0x18].try_into().unwrap());
                assert!((r.sea_z - (base + 0.25 * sea::bob(399))).abs() < 1e-5);
                assert!(r.scroll.iter().flatten().all(|x| (-1.0..=1.0).contains(x)) && r.scroll != [[0.0; 2]; 2], "scrolls {:?}", r.scroll);
            }
            (sea::SeaKind::Hoven, SeaData::Hoven(_)) => {
                assert!(r.scroll != [[0.0; 2]; 2] && r.g1_scroll != [0.0; 2]);
            }
            k => panic!("level {level:02}: port {port} data {k:?}"),
        }
        eprintln!("level {level:02} {}: {r:?}", PORTS[port].name);
    }
}

/// The ocean does not register while the camera is at or below pvar 8 (level 11: z 180).
#[test]
fn pokitaru_ocean_needs_the_camera_above_its_floor() {
    if overlay(1).is_none() { eprintln!("skipped: no extracted/"); return; }
    let (_, _, last) = run(11, sea::OCEAN, [470.0, 380.0, 170.0], 10);
    assert!(last.is_empty());
}
