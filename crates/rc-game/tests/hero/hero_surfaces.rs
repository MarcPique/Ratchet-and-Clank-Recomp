//! Package P1 (docs/plan/hero_states.md §3): the surface ids each level's collision uses, against the surface
//! rules that level's own `HeroSurfaceReaction` compiles (`rc_game::hero::surface::LEVEL_RULES`). Skipped when
//! `extracted/` (the extracted game data) is absent.

use rc_formats::moby_anim::{parse_sequence, MobyAnimClass, MobySequence};
use rc_formats::{collision, level};
use rc_game::collision_query::{coll_line, QueryFlags};
use rc_game::hero::anim::RatchetAnim;
use rc_game::hero::surface::{rules, LEVEL_RULES};
use rc_game::moby_runtime::{ClassInfo, Moby, MobyTable};
use rc_game::pad::{button, PadInput};
use rc_game::tick::{Game, GameOptions, TickHooks};
use std::collections::BTreeMap;

/// Surface id → face count (world mesh), surface id → the classes whose collision uses it.
type Uses = (BTreeMap<u8, usize>, BTreeMap<u8, Vec<i32>>);

/// The surface ids of level `n`'s world mesh and moby class collision blobs.
fn surfaces(n: usize) -> Option<Uses> {
    let dir = rc_formats::test_data::root().join(format!("levels/{n:02}"));
    let data = rc_formats::test_data::core_data(n as u32)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let core = level::parse_level_core(&idx, data.len()).ok()?;
    let mesh = collision::parse_collision(&core, &data).ok()?;
    let mut world = BTreeMap::new();
    for c in &mesh.cells {
        for f in &c.faces { *world.entry(f.surface & 0x1f).or_insert(0) += 1; }
    }
    let mut mobys: BTreeMap<u8, Vec<i32>> = BTreeMap::new();
    for (class, blob) in rc_formats::moby_collision::parse_level(&core, &data).ok()? {
        for f in &blob.faces {
            let v = mobys.entry(f[3] & 0x1f).or_default();
            if !v.contains(&class) { v.push(class); }
        }
    }
    Some((world, mobys))
}


/// Every surface id a level's collision uses is handled by that level's own reaction, or by no level's (0xa,
/// 0x1f: no rule anywhere): the superset reaction cannot change a level through data it does not have.
#[test]
fn every_level_handles_its_own_surfaces() {
    let any: u16 = LEVEL_RULES.iter().fold(0, |m, r| m | r.handled);
    for n in 0..19 {
        let Some((world, mobys)) = surfaces(n) else { continue };
        let r = rules(n as i32);
        for id in world.keys().chain(mobys.keys()) {
            let id = *id as i16;
            assert!(r.handles(id) || any & (1u16.checked_shl(id as u32).unwrap_or(0)) == 0, "level {n:02}: surface {id:#x} is handled elsewhere but not here");
        }
    }
}

struct Lv {
    mesh: collision::Collision,
    ratchet: MobyAnimClass,
    death_z: f32,
}

fn level_data(n: usize) -> Option<Lv> {
    let dir = rc_formats::test_data::root().join(format!("levels/{n:02}"));
    let data = rc_formats::test_data::core_data(n as u32)?;
    let idx = std::fs::read(dir.join("core_index.bin")).ok()?;
    let settings = rc_formats::test_data::gameplay_section(n as u32, "level_settings")?;
    let core = level::parse_level_core(&idx, data.len()).ok()?;
    let mesh = collision::parse_collision(&core, &data).ok()?;
    let death_z = f32::from_le_bytes(settings[0x28..0x2c].try_into().unwrap());
    let blob = rc_formats::test_data::core_block(n as u32, "moby_class/0000")?;
    let class = rc_formats::moby::parse_moby_class(&blob).ok()?;
    let seqs: Vec<Option<MobySequence>> = (0..256)
        .map(|i| rc_formats::test_data::core_block(n as u32, &format!("ratchet_seq/{i:03}")).and_then(|b| parse_sequence(&b, 0).ok()))
        .collect();
    Some(Lv { mesh, ratchet: MobyAnimClass::new(&class, seqs), death_z })
}

/// Ratchet alone on level `n`'s world mesh at `at`, facing `yaw`, run with `input(t)`: (state, position, fade
/// requested 0x141401) per tick.
fn drop_in(lv: &Lv, n: i32, at: [f32; 3], yaw: f32, ticks: u32, input: impl Fn(u32) -> PadInput) -> Vec<(i32, [f32; 3], u8)> {
    let class = ClassInfo { scale: 1.0, ..Default::default() };
    let mut m = Moby::init_instance(0, 0, Some(&class));
    m.position = [at[0], at[1], at[2], 1.0];
    m.rotation = [0.0, 0.0, yaw, 0.0];
    let mut g = Game::new(&lv.mesh, MobyTable::new(vec![m], 16), 0, GameOptions::default(), lv.death_z);
    g.finish_load();
    g.hero.idle.level = n;
    let mut anim = RatchetAnim::new(&lv.ratchet);
    let mut mobys = |_: &mut MobyTable, _: &rc_game::hero::Hero, _: &mut rc_game::rng::Rng, _: &rc_game::follow_camera::CameraView, _: &collision::Collision, _: u64| {};
    let mut parts = |_: &rc_game::hero::Hero, _: &rc_game::follow_camera::CameraView, _: &mut rc_game::rng::Rng, _: u64| {};
    let mut hooks = TickHooks { mobys: &mut mobys, particles: &mut parts, world: None };
    let mut out = Vec::new();
    for t in 0..ticks {
        let b = input(t).bytes();
        g.tick(Some(&b), &lv.mesh, &mut anim.ctl(&lv.ratchet), &mut hooks);
        out.push((g.hero.state, g.hero.position(), g.hero.fell_out));
    }
    out
}

fn states(recs: &[(i32, [f32; 3], u8)]) -> Vec<(u32, i32)> {
    let mut v: Vec<(u32, i32)> = Vec::new();
    for (t, r) in recs.iter().enumerate() {
        if v.last().map(|x| x.1) != Some(r.0) { v.push((t as u32, r.0)); }
    }
    v
}

/// A point on a face of surface `id` in level `n`'s mesh whose top is clear (a line from 3 above hits it).
fn find_surface(mesh: &collision::Collision, id: u8) -> Option<[f32; 3]> {
    for c in &mesh.cells {
        for f in &c.faces {
            if f.surface & 0x1f != id { continue; }
            let v = f.v.map(|k| c.vertices[k as usize]);
            let p = [(v[0][0] + v[1][0] + v[2][0]) / 3.0, (v[0][1] + v[1][1] + v[2][1]) / 3.0, (v[0][2] + v[1][2] + v[2][2]) / 3.0];
            if let Some(h) = coll_line(mesh, [p[0], p[1], p[2] + 3.0], [p[0], p[1], p[2] - 0.5], QueryFlags(2)) {
                if h.surface_id() == id as i32 && (h.point[2] - p[2]).abs() < 0.01 && 0.99 < h.normal[2] / (h.normal[0].hypot(h.normal[1]).hypot(h.normal[2])) {
                    return Some(p);
                }
            }
        }
    }
    None
}


/// Aridia (level 2) quicksand, surface 3: dropped onto it Ratchet sinks in (0x68), ✕ jumps him out (0x69, the
/// jump system, h = depth + 1.7), he falls back in (0x68, the second try) and without ✕ sinks to the death fade.
/// Aridia's own capsule and `LandEta` pass through surface 3 (flags 0x324, `rc_game::hero::surface::PASS_SURFACE`),
/// so nothing holds him on the face: the n-th try sinks at 1 u/s to (n + 1)/2 below the top, then at 0.25 u/s,
/// and the fade comes 1.2 below the top (L00 0x22bf00), here before the 170-tick limit.
#[test]
fn aridia_quicksand_sink_jump_out_and_fade() {
    let Some(lv) = level_data(2) else { eprintln!("skipped: no extracted/levels/02"); return };
    let p = find_surface(&lv.mesh, 3).expect("a clear surface-3 face on level 2");
    let x = |t: u32| if (60..62).contains(&t) { PadInput::neutral().press(button::CROSS) } else { PadInput::neutral() };
    let recs = drop_in(&lv, 2, [p[0], p[1], p[2] + 1.5], 0.0, 300, x);
    let s = states(&recs);
    let depth = |t: usize| p[2] - recs[t].1[2];
    eprintln!("quicksand at {p:?}: states {s:?}");
    assert_eq!(s.iter().map(|x| x.1).collect::<Vec<_>>(), vec![0x68, 0x69, 0x68], "{s:?}");
    assert_eq!(s[1].0, 60, "the jump out on the ✕ press (after 15 ticks)");
    let top = recs[70..110].iter().map(|r| r.1[2]).fold(f32::MIN, f32::max);
    assert!(top > p[2] + 1.0, "the jump out rises above the sand: {top} vs {}", p[2]);
    // The first try (n = 0): past 0.5 deep before the ✕, then 0.25 u/s (the capsule no longer stops him 0.25 in).
    assert!(depth(55) > 0.5, "sunk to {} by tick 55", depth(55));
    assert!((depth(58) - depth(56) - 2.0 * 0.25 / 60.0).abs() < 1e-3, "0.25 u/s past 0.5: {}", depth(58) - depth(56));
    // The second try (n = 1): 1 u/s to 1.0 deep, then 0.25 u/s.
    let back = s[2].0 as usize;
    let one = (back..300).find(|&t| depth(t) > 1.0).expect("1.0 deep");
    assert!((depth(one - 2) - depth(one - 4) - 2.0 / 60.0).abs() < 1e-3, "1 u/s above 1.0");
    assert!((depth(one + 4) - depth(one + 2) - 2.0 * 0.25 / 60.0).abs() < 1e-3, "0.25 u/s below 1.0");
    // The fade 1.2 below the top, before the 170-tick limit.
    let fade = recs.iter().position(|r| r.2 != 0).expect("the death fade");
    assert!(fade > back && fade < back + 170, "fade at {fade}, second try from {back}");
    assert!((depth(fade - 1) - 1.2).abs() < 0.01, "faded {} deep", depth(fade - 1));
    // Level 1 (Novalis) compiles no rule for surface 3: the same face is plain ground there.
    let recs = drop_in(&lv, 1, [p[0], p[1], p[2] + 1.5], 0.0, 60, |_| PadInput::neutral());
    assert!(recs.iter().all(|r| r.0 != 0x68), "{:?}", states(&recs));
}

/// Levels 6 and 0xe compile their capsule and `LandEta` with flags 0xd24 (`rc_game::hero::surface::PASS_SURFACE`:
/// Ratchet passes through the deadly liquid, surface 0xd), but neither level's collision (world mesh or moby
/// classes) has a surface-0xd face, so their constant changes nothing their data can show (docs/plan/
/// level_generalisation.md H2). Aridia's (3) is the visible one (above).
#[test]
fn deadly_liquid_pass_levels_have_no_such_faces() {
    for n in [6usize, 14] {
        let Some((world, mobys)) = surfaces(n) else { continue };
        assert!(!world.contains_key(&0xd) && !mobys.contains_key(&0xd), "level {n}: surface 0xd in {world:?} / {mobys:?}");
    }
}

/// A slippery floor (surface 7, levels 12 and 14): walking onto it is the slide 0x2f; Novalis' rules ignore 7.
#[test]
fn slippery_floor_is_the_slide() {
    for n in [12usize, 14] {
        let Some(lv) = level_data(n) else { continue };
        let p = find_surface(&lv.mesh, 7).expect("a clear surface-7 face");
        let stick = |t: u32| if (40..90).contains(&t) { PadInput::neutral().stick(1.0, 0.0) } else { PadInput::neutral() };
        let recs = drop_in(&lv, n as i32, [p[0], p[1], p[2] + 0.5], 0.0, 100, stick);
        let s = states(&recs);
        eprintln!("level {n}: surface 7 at {p:?}: states {s:?}");
        assert_eq!(s[0].1, 0, "idle on the flat ice");
        assert_eq!(s[1], (41, 0x2f), "the walk becomes the slide");
        // Still sliding after the stick is released (the slide keeps its speed; no stop within 10 ticks).
        assert!(recs[90..100].iter().all(|r| r.0 == 0x2f));
        let recs = drop_in(&lv, 1, [p[0], p[1], p[2] + 0.5], 0.0, 100, stick);
        assert!(recs.iter().all(|r| r.0 != 0x2f), "level 1 has no slippery rule");
    }
}

/// Novalis' flow chutes (surface 4; the flow class 679 near them is not ported): standing on one is the sinking
/// floor 0x31 (group 0x10); without the flow's push and its 0x13f530 hold nothing moves him.
#[test]
fn novalis_flow_surface_is_the_sinking_floor() {
    let Some(lv) = level_data(1) else { eprintln!("skipped: no extracted/levels/01"); return };
    let p = find_surface(&lv.mesh, 4).expect("a clear surface-4 face on Novalis");
    let recs = drop_in(&lv, 1, [p[0], p[1], p[2] + 0.5], 0.0, 60, |_| PadInput::neutral());
    assert!(recs.iter().all(|r| r.0 == 0x31), "{:?}", states(&recs));
}
