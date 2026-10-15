//! The level camera system on the levels' own data (`rc_game::follow_camera::level`, docs/plan/player_controller.md
//! §15): the class-17 regions of Kerwan's cables (records 10..12, mode 2 "while in group 0x1a") and Novalis's
//! cuboid-82 region (mode 0, a 23° scripted pitch while the camera faces within 80° of +y), each against the same run
//! without the records (the follow camera alone, as before the port). Headless on the level's world mesh; skipped
//! without `extracted/`.

use rc_formats::cameras::LevelCamera;
use rc_formats::moby_anim::{parse_sequence, MobyAnimClass, MobySequence};
use rc_formats::volumes::Volumes;
use rc_formats::{collision, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_game::follow_camera::level::{CameraPorts, LevelCameras};
use rc_game::hero::anim::RatchetAnim;
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::sync::Arc;

struct Lv {
    n: u32,
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    death_z: f32,
    vols: Arc<Volumes>,
    cams: Vec<LevelCamera>,
    ports: CameraPorts,
}

fn level_data(n: u32) -> Option<Lv> {
    let d = rc_formats::test_data::level_dir(n);
    let data = rc_formats::test_data::core_data(n)?;
    let idx = std::fs::read(d.join("core_index.bin")).ok()?;
    let settings = rc_formats::test_data::gameplay_section(n, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).ok()?;
    let mesh = collision::parse_collision(&core, &data).ok()?;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(n, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).ok()?;
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(n, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    let g = rc_formats::test_data::gameplay(n)?;
    let vols = Arc::new(rc_formats::volumes::parse_volumes(&g).ok()?);
    let cams = rc_formats::cameras::parse_level_cameras(&g).ok()?;
    let ports = match (crate::common::overlay(n), crate::common::overlay(1)) {
        (Some(t), Some(_)) => CameraPorts::from_overlays(&t, &crate::common::overlay),
        _ => return None,
    };
    Some(Lv { n, mesh, ratchet: MobyAnimClass::new(&class, seqs), death_z, vols, cams, ports })
}

/// One tick of a run.
#[derive(Clone, Debug, PartialEq)]
struct Rec {
    state: i32,
    hero: [f32; 3],
    cam: [f32; 3],
    fwd: [f32; 3],
    /// The slot holding the follow camera's settings (the region lock) and its counter.
    owner: Option<usize>,
    counter: i16,
    /// The follow camera's distance D+0x15c.
    dist: f32,
}

/// Ratchet on the level at `at` facing `yaw`, `ticks` ticks of `input`, with the level's camera records or without.
fn run(lv: &Lv, at: [f32; 3], yaw: f32, ticks: u32, records: bool, input: impl Fn(u32) -> PadInput) -> Vec<Rec> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), lv.death_z);
    g.finish_load();
    g.hero.idle.level = lv.n as i32;
    g.grind_paths = Arc::new(lv.vols.grind_paths.clone());
    if records { g.camera.set_level(LevelCameras::new(lv.n, &lv.cams, Some(lv.vols.clone()), lv.ports)); }
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    (0..ticks)
        .map(|t| {
            g.tick(Some(&input(t).bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
            let lc = &g.camera.level_cams;
            let counter = lc.owner.and_then(|o| lc.slots[o].region).map_or(0, |r| r.counter);
            Rec { state: g.hero.state, hero: g.hero.position(), cam: g.camera.out.pos_f32(), fwd: g.camera.out.rows_f32()[0], owner: lc.owner, counter, dist: g.camera.cam.dist.to_f32() }
        })
        .collect()
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() }

/// The distance from `p` to the segment a–b.
fn seg(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2])).clamp(0.0, 1.0);
    dist(p, [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t])
}

/// The ground under cable `i`, `d` along it, and the cable's yaw.
fn under(lv: &Lv, i: usize, d: f32) -> ([f32; 3], f32) {
    let p = &lv.vols.grind_paths[i].points;
    let (a, b) = (p[0], p[1]);
    let dir = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let q = [a[0] + dir[0] * d / l, a[1] + dir[1] * d / l, a[2] + dir[2] * d / l];
    let hit = coll_line(&lv.mesh, [q[0], q[1], q[2] - 0.1], [q[0], q[1], q[2] - 8.0], QueryFlags::NONE).expect("ground under the cable");
    (hit.point, dir[1].atan2(dir[0]))
}

/// Kerwan's three cables: jump up to each (✕), slide it to the end. With the records the cable's region (records
/// 10..12, cuboids 34..36, mode 2, priority 4) takes the follow camera as the slide starts and holds it for the whole
/// ride (inside without the cuboid test while in group 0x1a): the stick off, the camera springing out to 6.5 behind
/// and 2.0 above the pivot with the look 0.5 lower and the stiffer spring 0.04, turning toward the region's facing.
/// Without them (the port before) the follow camera keeps its 4.64. The camera stays off the cable either way; with
/// the records it rides further back from Ratchet. Deterministic.
#[test]
fn kerwan_cables_take_the_cable_regions() {
    let Some(lv) = level_data(3) else { eprintln!("skipped: no extracted/"); return };
    assert!(lv.ports.region, "Kerwan runs level 01's class-17 code");
    let jump = |t: u32| if t == 30 { PadInput::neutral().press(button::CROSS) } else { PadInput::neutral() };
    let mut owners = Vec::new();
    for i in 0..3 {
        let (g, yaw) = under(&lv, i, 1.0);
        let at = [g[0], g[1], g[2] + 0.05];
        let with = run(&lv, at, yaw, 600, true, jump);
        let without = run(&lv, at, yaw, 600, false, jump);
        let slide: Vec<usize> = (0..with.len()).filter(|&t| with[t].state == 0x74).collect();
        assert!(!slide.is_empty(), "cable {i}: the slide");
        let (s0, s1) = (slide[0], *slide.last().unwrap());
        let owner = with[s0 + 5].owner.expect("a region holds the camera on the cable");
        let p = &lv.vols.grind_paths[i].points;
        let cable = ([p[0][0], p[0][1], p[0][2]], [p[1][0], p[1][1], p[1][2]]);
        let mean = |r: &[Rec]| slide[slide.len() / 2..].iter().map(|&t| r[t].dist).sum::<f32>() / (slide.len() - slide.len() / 2) as f32;
        let (d_with, d_without) = (mean(&with), mean(&without));
        let off_with = slide.iter().map(|&t| seg(with[t].cam, cable.0, cable.1)).fold(f32::MAX, f32::min);
        let off_without = slide.iter().map(|&t| seg(without[t].cam, cable.0, cable.1)).fold(f32::MAX, f32::min);
        eprintln!(
            "cable {i}: slide ticks {s0}..{s1}, owner slot {owner} (class {}), counter at the end {}; the follow camera's distance over the second half {d_with:.2} (without the records {d_without:.2}); closest to the cable {off_with:.2} ({off_without:.2})",
            lv.cams[owner].record.class, with[s1].counter
        );
        assert_eq!(lv.cams[owner].record.class, 17);
        assert!((10..=12).contains(&owner), "cable {i}: region record {owner}");
        assert!(slide.iter().all(|&t| with[t].owner == Some(owner)), "cable {i}: held for the whole ride");
        assert_eq!(with[s1].counter, 200, "cable {i}: mode 2 holds the counter at 200");
        assert!(slide.iter().all(|&t| without[t].owner.is_none()));
        assert!((d_with - 6.5).abs() < 0.3 && d_without < 5.0, "cable {i}: the region's 6.5 ({d_with}) against the follow camera's own ({d_without})");
        assert!(off_with > 1.0, "cable {i}: the camera stays off the cable ({off_with})");
        owners.push(owner);
        assert_eq!(run(&lv, at, yaw, 600, true, jump), with, "cable {i}: deterministic");
    }
    owners.sort();
    assert_eq!(owners, [10, 11, 12], "one region per cable");
}

/// Novalis's region (record 1: cuboid 82, mode 0, priority 4, the facing check, pitch 23°): Ratchet standing in it
/// with the camera looking along +y (within 80° of the record's `rot.z` = 90°) → the region takes the camera and
/// tilts it down (the scripted pitch replaces the stick's); turned the other way (the camera 180° off) it leaves at
/// once. The hero digest's lake and moves runs walk through this cuboid (ticks 58..135 and 180..292 / 612..): the
/// digest harness loads no records, so the digest does not show it (docs/plan/player_controller.md §15).
#[test]
fn novalis_region_tilts_the_camera() {
    let Some(lv) = level_data(1) else { eprintln!("skipped: no extracted/"); return };
    let r = lv.cams[1].pvar.as_deref().and_then(rc_formats::cameras::RegionTweak::parse).unwrap();
    assert_eq!((lv.cams[1].record.class, r.header.cuboid, r.mode, r.facing, r.pitch), (17, 82, 0, 1, 23.0));
    let c = lv.vols.cuboids[82].matrix[3];
    let hit = coll_line(&lv.mesh, [c[0], c[1], c[2] + 4.0], [c[0], c[1], c[2] - 20.0], QueryFlags::NONE).expect("ground in cuboid 82");
    let at = [hit.point[0], hit.point[1], hit.point[2] + 0.05];
    let idle = |_: u32| PadInput::neutral();
    let yaw = std::f32::consts::FRAC_PI_2;
    let with = run(&lv, at, yaw, 240, true, idle);
    let without = run(&lv, at, yaw, 240, false, idle);
    let pitch = |f: [f32; 3]| f[2].asin().to_degrees();
    let (a, b) = (with.last().unwrap(), without.last().unwrap());
    eprintln!("cuboid 82 at {at:?}: camera pitch with the records {:.1}°, without {:.1}°; owner {:?}, counter {}", pitch(a.fwd), pitch(b.fwd), a.owner, a.counter);
    assert_eq!((a.owner, a.counter), (Some(1), 200));
    assert!(pitch(a.fwd) < pitch(b.fwd) - 5.0, "tilted down");
    let away = run(&lv, at, -yaw, 60, true, idle);
    assert!(away.iter().all(|r| r.owner.is_none() && r.counter == 0), "facing away: never taken");
    assert_eq!(run(&lv, at, yaw, 240, true, idle), with, "deterministic");
}

/// The camera classes the port runs on every level (`CameraPorts::from_overlays`: the level's `lvl.camvtbl` rows
/// against the reference levels' code): class 17 on the 14 levels that have it, class 7 (the Swingshot camera) on
/// all 19, class 23 (the placed view) on the 11 and class 18 (the moby focus) on the 7 that have it.
#[test]
fn ported_camera_classes_every_level() {
    if crate::common::overlay(1).is_none() { eprintln!("skipped: no extracted/"); return }
    for n in 0..19u32 {
        let Some(t) = crate::common::overlay(n) else { return };
        let p = CameraPorts::from_overlays(&t, &crate::common::overlay);
        assert!(p.swing, "level {n}: class 7 is level 01's code");
        let has = |c: i32| t.camvtbl().iter().any(|e| e.class == c);
        assert_eq!(p.region, has(17), "level {n}");
        assert_eq!(p.placed, has(23), "level {n}: class 23 is level 00's code");
        assert_eq!(p.focus, has(18), "level {n}: class 18 is level 02's code");
        assert_eq!(p.flyby, has(19), "level {n}: class 19 is level 10's code");
        assert_eq!(p.race, has(20) && has(21), "level {n}: classes 20 / 21 are level 14's code");
        assert_eq!(p.giant, has(22), "level {n}: class 22 is level 15's code");
    }
}
