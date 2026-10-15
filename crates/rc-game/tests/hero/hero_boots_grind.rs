//! Package P5 (docs/plan/hero_states.md §3): the spline follower on every level's grind paths, and the grind on
//! real rails — Oltanis (level 14: three parallel rails, grind, grind jump, rail switch) and Kalebo III (level 16:
//! the long grind-rail course) — headless, on the level's world mesh. Skipped when `extracted/` is absent.

use rc_formats::moby_anim::{parse_sequence, MobyAnimClass, MobySequence};
use rc_formats::volumes::GrindPath;
use rc_formats::{collision, level};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::boots::GRIND_BOOTS;
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::spline;
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::path::PathBuf;

fn dir(n: usize) -> PathBuf { rc_formats::test_data::root().join(format!("levels/{n:02}")) }

fn grind_paths(n: usize) -> Option<Vec<GrindPath>> {
    let g = rc_formats::test_data::gameplay(n as u32)?;
    Some(rc_formats::volumes::parse_grind_paths(&g).expect("grind paths"))
}

/// The data the spline follower relies on, on every level: w is the chord to the next point (the last point's to
/// the first); the nearest point of each vertex is itself; advancing by a segment's w from its start lands on the
/// next point.
#[test]
fn spline_follower_on_every_levels_grind_paths() {
    let mut total = 0;
    for n in 0..19 {
        let Some(paths) = grind_paths(n) else { continue };
        for (i, p) in paths.iter().enumerate() {
            let pts = &p.points;
            let m = pts.len();
            total += 1;
            for k in 0..m {
                let a = [pts[k][0], pts[k][1], pts[k][2]];
                let b = pts[(k + 1) % m];
                let chord = spline::dist3(a, [b[0], b[1], b[2]]);
                assert!((pts[k][3] - chord).abs() < 1e-3 * chord.max(1.0), "level {n} path {i} point {k}: w {} chord {chord}", pts[k][3]);
            }
            let closed = p.flag != 0;
            for k in (0..m).step_by((m / 7).max(1)) {
                let a = [pts[k][0], pts[k][1], pts[k][2]];
                let (q, _) = spline::nearest(pts, closed, 999.0, 2.0, 2.5, a).expect("a vertex is on its spline");
                assert!(spline::dist3(q, a) < 1e-3, "level {n} path {i}: nearest of vertex {k} is {q:?}, not {a:?}");
                if k + 1 < m {
                    let mut c = spline::Cursor { seg: k as i32, t: 0.0 };
                    let (r, _) = spline::advance(pts, closed, pts[k][3], &mut c);
                    let b = [pts[k + 1][0], pts[k + 1][1], pts[k + 1][2]];
                    assert!(spline::dist3(r, b) < 1e-2, "level {n} path {i}: advance from {k} gives {r:?}, not {b:?}");
                }
            }
        }
    }
    eprintln!("{total} grind paths checked");
}

struct Lv {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    death_z: f32,
    paths: Vec<GrindPath>,
}

fn level_data(n: usize) -> Option<Lv> {
    let d = dir(n);
    let data = rc_formats::test_data::core_data(n as u32)?;
    let idx = std::fs::read(d.join("core_index.bin")).ok()?;
    let settings = rc_formats::test_data::gameplay_section(n as u32, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).ok()?;
    let mesh = collision::parse_collision(&core, &data).ok()?;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(n as u32, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).ok()?;
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(n as u32, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    Some(Lv { mesh, ratchet: MobyAnimClass::new(&class, seqs), death_z, paths: grind_paths(n)? })
}

/// One tick's record: state, position, rail, stick used.
type Rec = (i32, [f32; 3], Option<usize>);

/// Ratchet with the Grind Boots on level `n` at `at` facing `yaw`, run with `input(t, camera yaw)`.
fn ride(lv: &Lv, n: i32, at: [f32; 3], yaw: f32, ticks: u32, input: impl Fn(u32, f32) -> PadInput) -> Vec<Rec> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), lv.death_z);
    g.finish_load();
    g.hero.idle.level = n;
    g.hero.grant_items(&[GRIND_BOOTS]);
    g.grind_paths = std::sync::Arc::new(lv.paths.clone());
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        let b = input(t, g.camera.out.yaw().to_f32()).bytes();
        g.tick(Some(&b), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        out.push((g.hero.state, g.hero.position(), g.hero.boots.rail));
    }
    out
}

fn states(recs: &[Rec]) -> Vec<(u32, i32)> {
    let mut v: Vec<(u32, i32)> = Vec::new();
    for (t, r) in recs.iter().enumerate() {
        if v.last().map(|x| x.1) != Some(r.0) { v.push((t as u32, r.0)); }
    }
    v
}

/// The stick that points the hero at world yaw `y` with the camera at `cam`.
fn stick_to(y: f32, cam: f32) -> PadInput {
    let a = y - cam;
    PadInput::neutral().stick(-a.sin(), -a.cos())
}

/// A point `d` along path `p` from its start, and the path's yaw there.
fn along(p: &GrindPath, d: f32) -> ([f32; 3], f32) {
    let mut c = spline::Cursor::default();
    let (q, _) = spline::advance(&p.points, p.flag != 0, d, &mut c);
    let (yaw, _) = spline::heading(&p.points, p.flag != 0, c, q, 1).unwrap();
    (q, yaw)
}

/// Oltanis (level 14): rails 2, 3 and 4 run side by side from the mount at (157, 265, 51). Dropped onto the middle
/// rail (2) Ratchet grinds along it; ✕ is the grind jump and he lands back on it; ✕ with the stick toward rail 3
/// is the rail switch and he ends up grinding on rail 3 or 4 (whichever lies that way).
#[test]
fn oltanis_grind_jump_and_rail_switch() {
    let Some(lv) = level_data(14) else { eprintln!("skipped: no extracted/levels/14"); return };
    let rail = &lv.paths[2];
    let (start, yaw) = along(rail, 3.0);
    let at = [start[0], start[1], start[2] + 0.3];
    // Grind for 150 ticks, jump at 150.
    let recs = ride(&lv, 14, at, yaw, 260, |t, _| if t == 150 { PadInput::neutral().press(button::CROSS) } else { PadInput::neutral() });
    let s = states(&recs);
    eprintln!("Oltanis grind + jump: {s:x?}, rail at 149 {:?}, pos at 149 {:?}", recs[149].2, recs[149].1);
    // The rail's own collision is a pit surface (0xc) just under the spline: dropped onto it he takes the pit
    // fall 0x79 → 6 for a few ticks, then the fall catches the rail.
    let on = s.iter().find(|x| x.1 == 0x28).expect("onto the rail").0 as usize;
    assert!(on < 20, "{s:x?}");
    assert!(recs[on..150].iter().all(|r| r.0 == 0x28 && r.2 == Some(2)), "grinding rail 2 until the jump: {s:x?}");
    let run = spline::dist3(recs[149].1, recs[on + 5].1);
    assert!(run > 15.0, "grinds {run} in 144 ticks");
    let jump = s.iter().position(|x| x.1 == 0x29).expect("the grind jump");
    assert_eq!(s[jump].0, 150);
    assert_eq!(s.get(jump + 1).map(|x| x.1), Some(0x28), "lands back on the rail: {s:x?}");
    let top = recs[150..s[jump + 1].0 as usize].iter().map(|r| r.1[2] - recs[149].1[2]).fold(f32::MIN, f32::max);
    assert!(top > 1.0, "jump height {top}");
    assert_eq!(recs.last().unwrap().2, Some(2));

    // The rail switch: toward rail 3's side of rail 2 (the yaw from rail 2 to rail 3 at the start).
    let side_pt = rail_point_near(&lv.paths[3], start);
    let side = (side_pt[1] - start[1]).atan2(side_pt[0] - start[0]);
    let recs = ride(&lv, 14, at, yaw, 320, |t, cam| {
        let s = if (140..200).contains(&t) { stick_to(side, cam) } else { PadInput::neutral() };
        if t == 150 { s.press(button::CROSS) } else { s }
    });
    let s = states(&recs);
    let rails: Vec<Option<usize>> = recs.iter().map(|r| r.2).fold(Vec::new(), |mut v, x| { if v.last() != Some(&x) { v.push(x); } v });
    eprintln!("Oltanis rail switch: states {s:x?}, rails {rails:?}, end {:?}", recs.last().unwrap().1);
    assert!(s.iter().any(|x| x.1 == 0x2a), "a rail switch: {s:x?}");
    let end = recs.last().unwrap();
    assert_eq!(end.0, 0x28, "grinding at the end: {s:x?}");
    assert!(matches!(end.2, Some(3) | Some(4)), "on another rail: {rails:?}");
    let pts = &lv.paths[end.2.unwrap()].points;
    let (q, _) = spline::nearest(pts, false, 999.0, 2.0, 2.5, end.1).unwrap();
    assert!(spline::dist3(q, end.1) < 0.05, "on the rail: {:?} vs {q:?}", end.1);
}

fn rail_point_near(p: &GrindPath, at: [f32; 3]) -> [f32; 3] { spline::nearest(&p.points, p.flag != 0, 999.0, 2.0, 0.0, at).unwrap().0 }

/// Kalebo III (level 16): the long rail 1 (2800 points) from the mount at (115, 264, 133); Ratchet grinds it for
/// 20 seconds without ✕: he stays on it, and where the course's obstacles block him (the grind is meant to be
/// jumped there) he takes the grind hurt 0x42 (one damage each) and rides on; two runs are identical.
#[test]
fn kalebo_long_grind() {
    let Some(lv) = level_data(16) else { eprintln!("skipped: no extracted/levels/16"); return };
    let rail = &lv.paths[1];
    let (start, yaw) = along(rail, 2.0);
    let at = [start[0], start[1], start[2] + 0.3];
    let recs = ride(&lv, 16, at, yaw, 1200, |_, _| PadInput::neutral());
    let s = states(&recs);
    let dist: f32 = recs.windows(2).map(|w| spline::dist3(w[0].1, w[1].1)).sum();
    eprintln!("Kalebo grind: {s:x?}, {dist} units in 1200 ticks, end {:?}", recs.last().unwrap().1);
    let on = s.iter().find(|x| x.1 == 0x28).expect("onto the rail").0 as usize;
    assert!(on < 40, "{s:x?}");
    assert!(recs[on..].iter().all(|r| matches!(r.0, 0x28 | 0x42) && r.2 == Some(1)), "on rail 1 to the end: {s:x?}");
    assert!(dist > 120.0, "{dist}");
    let again = ride(&lv, 16, at, yaw, 1200, |_, _| PadInput::neutral());
    assert!(recs.iter().zip(&again).all(|(a, b)| a == b), "deterministic");
}

/// Per tick on the rail: state, record 0's angles (x: the displacement's pitch, y: the body lean into the rail's bend
/// ahead), records 3 / 1 / 2's z (the gun lean) and Ratchet's sequence B.
type PoseRec = (i32, [f32; 3], [f32; 3], u8);

/// [`ride`] recording the grind's joint records (L00 0x21ce0c..0x21d1b0, `hero::boots`'s `grind_lean`), with `gun`: a
/// gun in the hand slot's id (0x140408; definition +0x18 ≠ 0).
fn ride_pose(lv: &Lv, n: i32, at: [f32; 3], yaw: f32, ticks: u32, gun: bool) -> Vec<PoseRec> {
    use rc_game::hero::idle::joint::{HEAD, NECK, REC0, REC2};
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), lv.death_z);
    g.finish_load();
    g.hero.idle.level = n;
    g.hero.grant_items(&[GRIND_BOOTS]);
    g.grind_paths = std::sync::Arc::new(lv.paths.clone());
    if gun {
        let mut defs = vec![rc_game::hero::items::WeaponDef::default(); 40];
        defs[15].w18 = 1;
        g.hero.weapons.defs = defs;
        g.hero.items.slot.id = 15;
    }
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for _ in 0..ticks {
        g.tick(Some(&PadInput::neutral().bytes()), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        let j = &g.hero.idle.joints;
        out.push((g.hero.state, j[REC0].cur, [j[HEAD].cur[2], j[NECK].cur[2], j[REC2].cur[2]], anim.state.seq_b));
    }
    out
}

/// Kalebo III's long rail: record 0 x follows the rail's pitch from the start, the body lean y only after 90 ticks on
/// the rail (then into the bends: both signs over the course); without a gun records 1..3 keep no z. With a gun in
/// hand on sequence 0x31: record 3 z → −47° (+ the aim lean 0x13f920, 0 here), records 1 / 2 z → 42°.
#[test]
fn kalebo_grind_leans() {
    let Some(lv) = level_data(16) else { eprintln!("skipped: no extracted/levels/16"); return };
    let rail = &lv.paths[1];
    let (start, yaw) = along(rail, 2.0);
    let at = [start[0], start[1], start[2] + 0.3];
    let recs = ride_pose(&lv, 16, at, yaw, 900, false);
    let on = recs.iter().position(|r| r.0 == 0x28).expect("onto the rail");
    assert!(recs[on..].iter().any(|r| 0.005 < r.1[0].abs()), "the pitch lean");
    assert!(recs[on..on + 80].iter().all(|r| r.1[1].abs() < 0.005), "no body lean in the first 90 ticks");
    let late = &recs[on + 100..];
    assert!(late.iter().any(|r| 0.02 < r.1[1]) && late.iter().any(|r| r.1[1] < -0.02), "the lean into both ways' bends");
    assert!(late.iter().all(|r| r.1[1].abs() <= std::f32::consts::FRAC_PI_4 + 0.1));
    assert!(recs.iter().all(|r| r.2 == [0.0; 3]), "no gun lean");
    let gun = ride_pose(&lv, 16, at, yaw, 300, true);
    // The end of the first 60-tick stretch on sequence 0x31 (the springs settled).
    let k = (60..gun.len()).find(|&k| gun[k - 60..=k].iter().all(|r| r.0 == 0x28 && r.3 == 0x31)).expect("60 ticks grinding on 0x31");
    let last = &gun[k];
    assert!((last.2[0] - f32::from_bits(0xbf51_ff7e)).abs() < 0.03, "{:?}", last.2);
    assert!((last.2[1] - f32::from_bits(0x3f3b_a866)).abs() < 0.01 && (last.2[2] - last.2[1]).abs() < 0.02, "{:?}", last.2);
}
