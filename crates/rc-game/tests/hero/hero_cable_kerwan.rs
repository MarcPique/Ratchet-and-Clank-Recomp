//! The cable slide 0x74 on Kerwan's three cables (level 3, the only level whose data has cables: the grind paths of
//! a level whose overlay has the cable contact `0x20d330`; docs/plan/hero_states.md "Cable slide"), headless on the
//! level's world mesh: jump up under a cable (✕, with □ for the longer reach), the hands catch it, the slide to the
//! end, the drop off the end. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, MobyAnimClass, MobySequence};
use rc_formats::volumes::GrindPath;
use rc_formats::{collision, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_game::hero::anim::{AnimCtl, RatchetAnim};
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};

const KERWAN: u32 = 3;

struct Lv {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    death_z: f32,
    paths: Vec<GrindPath>,
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
    let paths = rc_formats::volumes::parse_grind_paths(&g).ok()?;
    Some(Lv { mesh, ratchet: MobyAnimClass::new(&class, seqs), death_z, paths })
}

/// One tick's record.
#[derive(Clone, Debug, PartialEq)]
struct Rec {
    state: i32,
    pos: [f32; 3],
    seq: u8,
    speed: f32,
}

/// Ratchet on level `n` at `at` facing `yaw`, run with `input(t)`.
fn ride(lv: &Lv, n: u32, at: [f32; 3], yaw: f32, ticks: u32, input: impl Fn(u32) -> PadInput) -> Vec<Rec> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), lv.death_z);
    g.finish_load();
    g.hero.idle.level = n as i32;
    g.grind_paths = std::sync::Arc::new(lv.paths.clone());
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        let b = input(t).bytes();
        let seq = {
            g.tick(Some(&b), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
            anim.ctl(&lv.ratchet).view().seq_b
        };
        out.push(Rec { state: g.hero.state, pos: g.hero.position(), seq, speed: g.hero.boots.cable_speed * 60.0 });
    }
    out
}

fn states(recs: &[Rec]) -> Vec<(u32, i32)> {
    let mut v: Vec<(u32, i32)> = Vec::new();
    for (t, r) in recs.iter().enumerate() {
        if v.last().map(|x| x.1) != Some(r.state) { v.push((t as u32, r.state)); }
    }
    v
}

/// The ground under a point of cable `i`, `d` along it from its start, and the cable's yaw.
fn under(lv: &Lv, i: usize, d: f32) -> ([f32; 3], f32) {
    let p = &lv.paths[i].points;
    let (a, b) = (p[0], p[1]);
    let dir = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
    let q = [a[0] + dir[0] * d / l, a[1] + dir[1] * d / l, a[2] + dir[2] * d / l];
    let hit = coll_line(&lv.mesh, [q[0], q[1], q[2] - 0.1], [q[0], q[1], q[2] - 8.0], QueryFlags::NONE).expect("ground under the cable");
    (hit.point, dir[1].atan2(dir[0]))
}

/// Each cable: stand on the platform under its start (1 along it, the cable 2.2..2.9 overhead), ✕: the rising jump's
/// hands catch it (7 → 0x74), the grab anim 0x73 then the slide 0x66, the speed up to 14 u/s, ✕ / □ mashed on the
/// way do nothing (no jump-off), past the end the fall 6 and the landing. Twice: identical ticks.
#[test]
fn kerwan_cables_jump_catch_slide_drop() {
    let Some(lv) = level_data(KERWAN) else { return };
    assert_eq!(lv.paths.len(), 3, "Kerwan's cables");
    for i in 0..3 {
        let (g, yaw) = under(&lv, i, 1.0);
        let input = |t: u32| match t {
            30 => PadInput::neutral().press(button::CROSS),
            t if t > 60 && t % 9 == 0 => PadInput::neutral().press(if t % 2 == 0 { button::CROSS } else { button::SQUARE }),
            _ => PadInput::neutral(),
        };
        let recs = ride(&lv, KERWAN, [g[0], g[1], g[2] + 0.05], yaw, 900, input);
        let s = states(&recs);
        let slide: Vec<&Rec> = recs.iter().filter(|r| r.state == 0x74).collect();
        let top = slide.iter().map(|r| r.speed).fold(0.0f32, f32::max);
        let off = recs.iter().position(|r| r.state == 6).expect("off the end");
        let end = lv.paths[i].points[1];
        let d_end = ((recs[off].pos[0] - end[0]).powi(2) + (recs[off].pos[1] - end[1]).powi(2)).sqrt();
        eprintln!("cable {i}: ground {g:?} yaw {yaw}: {s:x?}; {} ticks on it, top {top} u/s, off {d_end} past the end", slide.len());
        let seq: Vec<i32> = s.iter().map(|x| x.1).collect();
        // (The mashing goes on after the landing: jumps.)
        assert_eq!(seq[..5], [0, 7, 0x74, 6, 0], "cable {i}: {s:x?}");
        let seqs: Vec<u8> = slide.iter().map(|r| r.seq).collect();
        let first66 = seqs.iter().position(|&q| q == 0x66).expect("the slide anim 0x66");
        assert!(seqs[..first66].iter().all(|&q| q == 0x73) && seqs[first66..].iter().all(|&q| q == 0x66), "cable {i}: anims");
        assert!((top - 14.0).abs() < 1e-3, "cable {i}: top speed {top}");
        assert!(d_end < 1.5, "cable {i}: let go {d_end} from the end");
        assert_eq!(ride(&lv, KERWAN, [g[0], g[1], g[2] + 0.05], yaw, 900, input), recs, "cable {i}: deterministic");
    }
}

/// Kerwan's own path search (`0x205830`, `rc_game::hero::boots::FIXED_REACH_LEVELS`) reaches 1.7 across in every
/// state; level00's `0x20cd08` (the other cable levels' code) only 0.9 in the jump. Standing 1.25 beside a cable,
/// facing along it, the rising jump's hands catch it on Kerwan (7 → 0x74); with level 4's search the same jump
/// misses (7, then the landing). Headless on Kerwan's data both times.
#[test]
fn kerwan_cable_reach_is_its_own() {
    let Some(lv) = level_data(KERWAN) else { return };
    let jump = |t: u32| if t == 30 { PadInput::neutral().press(button::CROSS) } else { PadInput::neutral() };
    let mut checked = 0;
    for i in 0..3 {
        let p = &lv.paths[i].points;
        let (a, b) = (p[0], p[1]);
        let yaw = (b[1] - a[1]).atan2(b[0] - a[0]);
        let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        for side in [1.0f32, -1.0] {
            // 1.5 along the cable, 1.25 to its side; the ground under that point, the cable 2.2..2.9 above it.
            let q = [a[0] + (b[0] - a[0]) * 1.5 / l, a[1] + (b[1] - a[1]) * 1.5 / l, a[2] + (b[2] - a[2]) * 1.5 / l];
            let s = [q[0] - yaw.sin() * 1.25 * side, q[1] + yaw.cos() * 1.25 * side];
            let Some(hit) = coll_line(&lv.mesh, [s[0], s[1], q[2] - 0.1], [s[0], s[1], q[2] - 8.0], QueryFlags::NONE) else { continue };
            if !(2.2..2.9).contains(&(q[2] - hit.point[2])) || hit.normal[2] < 0.99 { continue; }
            let at = [hit.point[0], hit.point[1], hit.point[2] + 0.05];
            let own = states(&ride(&lv, KERWAN, at, yaw, 120, jump));
            let l00 = states(&ride(&lv, 4, at, yaw, 120, jump));
            eprintln!("cable {i} side {side}: Kerwan {own:x?}, level 4's search {l00:x?}");
            assert_eq!(own.iter().map(|x| x.1).take(3).collect::<Vec<_>>(), [0, 7, 0x74], "cable {i}: Kerwan catches it");
            assert!(l00.iter().all(|x| x.1 != 0x74), "cable {i}: level00's reach misses it");
            checked += 1;
        }
    }
    assert!(checked > 0, "a flat floor 1.25 beside a cable");
}
