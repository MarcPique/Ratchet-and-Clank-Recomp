//! The creature units freed by W3 lane 2 (docs/plan/creatures.md §13): Pokitaru's 1246 (level 11), Aridia's 580 with
//! its nests 668 and 612 (level 02), Pokitaru's 1231 (level 11). The level harness is `creature_classes`'s.

use rc_formats::{gameplay, moby_spawn};

/// The pvars of the created instances and the class-table slots (survey).
#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_w3_creature_pvars() {
    for (level, oc) in [(11u32, 1246i16), (2, 580), (2, 668), (2, 612), (11, 1231), (11, 1297)] {
        let Some(gp) = rc_formats::test_data::gameplay(level) else { eprintln!("skipped: no extracted/"); return };
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
        let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
        let mut n = 0;
        for (i, m) in inst.iter().enumerate() {
            if m.o_class as i16 != oc || !spawned[i] { continue; }
            n += 1;
            let p = usize::try_from(m.pvar_index).ok().and_then(|k| pvars.get(k)).and_then(|b| b.as_deref()).unwrap_or(&[]);
            let w = |o: usize| if p.len() >= o + 4 { i32::from_le_bytes(p[o..o + 4].try_into().unwrap()) } else { -99 };
            if oc == 1246 {
                let r = w(0x254);
                let rc = usize::try_from(r).ok().and_then(|r| inst.get(r)).map(|m| m.o_class).unwrap_or(-1);
                eprintln!("1246 inst {i} pos {:?} grp {} 230 {} 234 {} 238 {} 23c {} ride {} (class {rc})", m.position, m.group, w(0x230), w(0x234), w(0x238), f32::from_bits(w(0x23c) as u32), r);
            }
            if n > 2 { continue; }
            eprintln!("L{level:02} class {oc} inst {i} pos {:?} rot {:?} scale {} group {} upd {} pvars {}", m.position, m.rotation, m.scale, m.group, m.update_distance, p.len());
            for (k, c) in p.chunks(16).enumerate() {
                let w: Vec<String> = c.chunks(4).map(|b| { let u = u32::from_le_bytes([b[0], b[1], b.get(2).copied().unwrap_or(0), b.get(3).copied().unwrap_or(0)]); let f = f32::from_bits(u); if f.is_finite() && f.abs() > 1e-4 && f.abs() < 1e6 { format!("{u:08x}({f})") } else { format!("{u:08x}") } }).collect();
                if w.iter().all(|s| s == "00000000") { continue; }
                eprintln!("  +{:03x}: {}", k * 16, w.join(" "));
            }
        }
        eprintln!("L{level:02} class {oc}: {n} created");
        let Ok(b) = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")) else { return };
        let ov = rc_formats::level_overlay::LevelOverlay::parse(&b).unwrap();
        for e in ov.vtbl().iter().filter(|e| e.o_class as i16 == oc) {
            let slots: Vec<String> = (0..6).map(|k| format!("{:#x}", ov.u32(e.w8 + 4 * k).unwrap_or(0))).collect();
            eprintln!("L{level:02} class {oc}: update {:#x} table {:#x} slots {}", e.update, e.w8, slots.join(" "));
        }
    }
}

/// The level words the units read (`$gp` data and tables), as hex and f32 (survey).
#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_w3_words() {
    for (level, addr, n) in [(11u32, 0x16_22c0u32, 0x50usize), (11, 0x16_1f00, 0x10), (2, 0x16_1900, 0x100), (11, 0x16_1a00, 0x100)] {
        let Ok(b) = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")) else { eprintln!("skipped"); return };
        let ov = rc_formats::level_overlay::LevelOverlay::parse(&b).unwrap();
        for k in (0..n).step_by(16) {
            let a = addr + k as u32;
            let w: Vec<String> = (0..4).map(|j| ov.u32(a + 4 * j).map_or("--------".into(), |u| { let f = f32::from_bits(u); if f.is_finite() && f.abs() > 1e-4 && f.abs() < 1e6 { format!("{u:08x}({f})") } else { format!("{u:08x}") } })).collect();
            eprintln!("L{level:02} {a:#x}: {}", w.join(" "));
        }
    }
}

use crate::creature_classes::{hero_at, load, wrench, Lv};
use rc_game::hero::Hero;
use rc_game::moby_runtime::MobyId;
use rc_game::moby_update::classes::units::{self, pokitaru_biter as pb};
use rc_game::moby_update::classes::ClassUpdate;
use rc_game::moby_update::creature::react;
use rc_game::moby_update::services::pvar as p;
use rc_game::ps2v::Pf;
use std::collections::HashMap;

// ---------------------------------------------------------------------------------------------------
// Harness bits

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap();
    ClassUpdate::Unit(i as u16)
}

/// Level `level` after its load pass with Ratchet at `at`.
fn level_at(level: u32, at: [f32; 3]) -> Option<(Lv, Hero)> {
    let mut lv = load(level)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    Some((lv, hero))
}

/// The Pokitaru boats 1075 held still: their own update (`units::pokitaru_boat`, ported in batch 5) off, so a test that
/// drives a boat's state and position by hand (or counts the biters' own particles) is not moved by the boat's sailing
/// and wake.
fn hold_boats(lv: &mut Lv) {
    for i in lv.of_class(1075) {
        let m = &mut lv.table.mobys[i];
        m.update_fn = None;
        m.mode |= rc_game::moby_runtime::mode::NO_UPDATE;
    }
}

/// `n` ticks with `ids` marked drawn before each (as MobyProc would for mobys on screen), recording each one's states.
fn run(lv: &mut Lv, hero: &Hero, ids: &[MobyId], n: usize) -> Vec<Vec<u8>> {
    let mut seen = vec![Vec::new(); ids.len()];
    for _ in 0..n {
        for &i in ids { if lv.table.mobys[i].state < 0x80 { lv.table.mobys[i].visible = 1; } }
        lv.tick(hero);
        for (k, &i) in ids.iter().enumerate() {
            let s = lv.table.mobys[i].state;
            if seen[k].last() != Some(&s) { seen[k].push(s); }
        }
    }
    seen
}

fn hits_from(lv: &Lv, target: MobyId, class: i16) -> Vec<rc_game::moby_update::services::HitRecord> {
    lv.hits_on(target).into_iter().filter(|h| h.attacker.is_some_and(|a| lv.table.mobys[a].o_class == class)).collect()
}

fn parts(lv: &Lv, ty: u8) -> u64 { lv.svc.fx.part_spawns.get(&ty).copied().unwrap_or(0) }

/// (unit, class, the levels whose table runs it, created instances on them).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[("U375 1246", 1246, &[11], 126), ("U95 580", 580, &[2], 82), ("U101 668", 668, &[2], 7), ("U96 612", 612, &[2], 15), ("U373 1231", 1231, &[11], 7), ("U373 1297", 1297, &[11], 0)];

/// Every unit's classes resolve to the unit on its levels (and to nothing else), with the census's instance counts;
/// the reaction tables.
#[test]
fn w3_creature_units_resolve_on_their_levels() {
    let Some(ov1) = crate::common::overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut created: HashMap<(&str, i16), usize> = HashMap::new();
    for level in 0..19u32 {
        let pt = crate::common::ports(level, &[]).unwrap();
        let gp = rc_formats::test_data::gameplay(level).unwrap();
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        for &(name, oc, levels, _) in EXPECTED {
            let u = unit(name);
            if levels.contains(&level) {
                assert_eq!(pt.get(oc), Some(u), "level {level:02} class {oc}");
                let n = inst.iter().zip(&tests).filter(|(m, t)| m.o_class as i16 == oc && t.spawn).count();
                *created.entry((name, oc)).or_default() += n;
            } else if pt.in_table(oc) {
                assert_ne!(pt.get(oc), Some(u), "level {level:02} class {oc}");
            }
        }
        let t = react::tables_from_overlays(&crate::common::overlay(level).unwrap(), &ov1, &crate::common::overlay);
        assert_eq!(t.get(&1246) == Some(&react::Table::Pokitaru1246), level == 11, "level {level:02}: {:?}", t.get(&1246));
        assert_eq!(t.get(&580) == Some(&react::Table::Aridia580), level == 2, "level {level:02}: {:?}", t.get(&580));
        if level == 2 { assert_eq!(t.get(&668), None, "the nests keep the default table"); }
    }
    for &(name, oc, _, n) in EXPECTED { assert_eq!(created.get(&(name, oc)).copied().unwrap_or(0), n, "{name} class {oc}"); }
}

// ---------------------------------------------------------------------------------------------------
// U375: Pokitaru's biters 1246 (level 11)

/// The biters after the load pass and a quiet run (survey).
#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_1246_level() {
    let Some((mut lv, hero)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped"); return };
    let ids = lv.of_class(1246);
    let seen = run(&mut lv, &hero, &ids, 600);
    for (k, &i) in ids.iter().enumerate() {
        let m = &lv.table.mobys[i];
        eprintln!("#{i} state {} pos {:?} area {} mode {} boat {} seq {} states {:?}", m.state, m.position, p::i32(&m.pvars, pb::pv::AREA), p::i32(&m.pvars, pb::pv::MODE), p::i32(&m.pvars, pb::pv::BOAT), m.anim.seq_b, seen[k]);
    }
    for b in [440usize, 441, 453] { let m = &lv.table.mobys[b]; eprintln!("boat #{b} class {} state {} pos {:?}", m.o_class, m.state, m.position); }
}

/// Code words of a level overlay (`RC_DUMP=level:addr:words`), for a disassembly by hand (survey).
#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_w3_code() {
    let Ok(spec) = std::env::var("RC_DUMP") else { return };
    for s in spec.split(',') {
        let v: Vec<&str> = s.split(':').collect();
        let level: u32 = v[0].parse().unwrap();
        let addr = u32::from_str_radix(v[1], 16).unwrap();
        let n: u32 = v[2].parse().unwrap();
        let Ok(b) = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")) else { return };
        let ov = rc_formats::level_overlay::LevelOverlay::parse(&b).unwrap();
        for k in 0..n { if let Some(u) = ov.u32(addr + 4 * k) { eprintln!("W {:08x} {u:08x}", addr + 4 * k); } }
    }
}

/// A point inside area path `path` (a 2-D polygon, `region::point_in_polygon`) near `near` (a 0.5 grid over its
/// bounding box), with `near`'s z; `min` away from `near` at least. None for an empty path or a bounding box that is
/// not finite or wider than 1000 on a side (the grid is capped at 2000 × 2000 points).
fn inside(lv: &mut Lv, path: usize, near: [f32; 4], min: f32) -> Option<[f32; 3]> {
    let pts: Vec<[f32; 4]> = lv.svc.splines.get(path)?.iter().map(|q| q.map(f32::from_bits)).collect();
    if pts.is_empty() { return None; }
    let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for q in &pts {
        x0 = x0.min(q[0]);
        x1 = x1.max(q[0]);
        y0 = y0.min(q[1]);
        y1 = y1.max(q[1]);
    }
    if ![x0, x1, y0, y1].iter().all(|v| v.is_finite()) || 1000.0 < x1 - x0 || 1000.0 < y1 - y0 { return None; }
    let hero = hero_at([0.0, 0.0, -100.0]);
    let w = lv.world(&hero);
    let mut best: Option<([f32; 3], f32)> = None;
    for i in 0..2000 {
        let x = x0 + 0.5 * i as f32;
        if x1 < x { break; }
        for j in 0..2000 {
            let y = y0 + 0.5 * j as f32;
            if y1 < y { break; }
            if rc_game::moby_update::creature::region::point_in_polygon(&w, path, [x, y, near[2], 1.0]) {
                let d = (x - near[0]).powi(2) + (y - near[1]).powi(2);
                if d >= min * min && best.is_none_or(|b| d < b.1) { best = Some(([x, y, near[2]], d)); }
            }
        }
    }
    best.map(|b| b.0)
}

/// Beach biter #499 (area path 0) and Ratchet inside its area 3 away: it turns to him (4, the side offset), walks
/// up (key times 8..22 step), bites (5: a sphere of 0.6 at +0.333 every tick: hit records on his moby with flags 1,
/// damage 1, type 0 / 1, class 1246, pushing); the other two of the area come too.
#[test]
fn beach_biters_go_for_ratchet_and_bite() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let near = lv.table.mobys[499].position;
    let at = inside(&mut lv, 0, near, 3.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    let seen = run(&mut lv, &hero, &[499, 500, 501], 600);
    eprintln!("states {seen:?}");
    assert!(seen.iter().all(|s| s.first() == Some(&pb::st::GO)), "the first tick sees him: {seen:?}");
    assert!(seen.iter().any(|s| s.contains(&pb::st::BITE)), "{seen:?}");
    let hits = hits_from(&lv, lv.hero_idx, 1246);
    eprintln!("bites: {}", hits.len());
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|h| h.flags == 1 && h.damage == Pf::ONE && (h.b28, h.b29, h.h2a) == (0, 1, 1246)), "{hits:?}");
    let side = p::ff(&lv.table.mobys[499].pvars, pb::pv::SIDE).abs();
    assert!(side <= 30.0f32.to_radians() + 1e-6);
    let hp = hero.pos.map(|x| x.to_f32());
    let q = lv.table.mobys[499].position;
    assert!(((q[0] - hp[0]).powi(2) + (q[1] - hp[1]).powi(2)).sqrt() < 12.0, "walked up to him");
}

/// The same run twice gives the same states and positions (the rand draws at fixed points).
#[test]
fn beach_biter_run_is_deterministic() {
    let go = || -> Option<(Vec<Vec<u8>>, [f32; 4])> {
        let (mut lv, _) = level_at(11, [0.0, 0.0, -100.0])?;
        let near = lv.table.mobys[499].position;
        let at = inside(&mut lv, 0, near, 3.0)?;
        let hero = hero_at([at[0], at[1], near[2]]);
        let s = run(&mut lv, &hero, &[499, 500, 501], 300);
        Some((s, lv.table.mobys[499].position))
    };
    let (Some(a), Some(b)) = (go(), go()) else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(a, b);
}

/// No target, but drawn within 32 of the camera: it grazes (3) about home (a point 1–3 from it) and idles again (2).
#[test]
fn a_beach_biter_grazes_near_the_camera() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let near = lv.table.mobys[499].position;
    let hero = hero_at([near[0] + 25.0, near[1], near[2]]);
    let home = p::v4f(&lv.table.mobys[499].pvars, pb::pv::HOME);
    let mut far = 0.0f32;
    let mut seen = Vec::new();
    for _ in 0..1200 {
        let s = run(&mut lv, &hero, &[499], 1);
        if seen.last() != s[0].last() { seen.push(s[0][0]); }
        let g = p::v4f(&lv.table.mobys[499].pvars, pb::pv::GRAZE);
        if lv.table.mobys[499].state == pb::st::GRAZE { far = far.max(((g[0] - home[0]).powi(2) + (g[1] - home[1]).powi(2)).sqrt()); }
    }
    eprintln!("states {seen:?}, graze point up to {far} from home");
    assert!(seen.contains(&pb::st::GRAZE) && seen.iter().filter(|&&s| s == pb::st::IDLE).count() >= 2, "{seen:?}");
    assert!((1.0..=3.0).contains(&far));
    assert!(hits_from(&lv, lv.hero_idx, 1246).is_empty());
}

/// Weapons: a wrench hit knocks it back (9: flash 0x78, the flight 6·dt out / 6·dt up, health 2 → 1) and it walks
/// on after landing on its beach (2); the second kills it (0xb: untargetable, flash 0xf0, `SetDeathBits`: the save
/// bit), the flight ends in the small explosion (its light and sparks) and the moby is deleted.
#[test]
fn weapons_knock_back_and_kill_a_biter() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let near = lv.table.mobys[499].position;
    let hero = hero_at([near[0] + 25.0, near[1], near[2]]);
    for _ in 0..5 { lv.tick(&hero); }
    let h = lv.hero_idx;
    lv.hit(&hero, 499, &wrench(h, [1.0, 0.0]));
    lv.tick(&hero);
    let m = &lv.table.mobys[499];
    assert_eq!(m.state, pb::st::KNOCKED);
    assert_eq!(m.pvars[pb::pv::FLASH + 7], 0x78);
    assert_eq!(p::ff(&m.pvars, pb::pv::D), 1.0);
    let mut seen = vec![pb::st::KNOCKED];
    for _ in 0..300 {
        let s = run(&mut lv, &hero, &[499], 1);
        if seen.last() != s[0].last() { seen.push(s[0][0]); }
        if lv.table.mobys[499].state != pb::st::KNOCKED { break; }
    }
    eprintln!("after the knock {seen:?} at {:?}", lv.table.mobys[499].position);
    assert_eq!(lv.table.mobys[499].state, pb::st::IDLE, "landed on its beach");
    for _ in 0..60 { lv.tick(&hero); }
    let before = parts(&lv, 23) + parts(&lv, 53) + parts(&lv, 8) + lv.svc.fx.part_spawns.values().sum::<u64>();
    lv.hit(&hero, 499, &wrench(h, [1.0, 0.0]));
    lv.tick(&hero);
    let m = &lv.table.mobys[499];
    assert_eq!(m.state, pb::st::DYING);
    assert_eq!(m.mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    assert_eq!(m.pvars[pb::pv::FLASH + 7], 0xf0);
    let sid = m.spawn_id;
    assert!(lv.svc.save.death.contains(&(11, sid)));
    for _ in 0..300 {
        lv.tick(&hero);
        if lv.table.mobys[499].state >= 0x80 { break; }
    }
    assert!(lv.table.mobys[499].state >= 0x80, "deleted");
    let after = parts(&lv, 23) + parts(&lv, 53) + parts(&lv, 8) + lv.svc.fx.part_spawns.values().sum::<u64>();
    assert!(after > before, "the explosion's particles");
}

/// Group 57's boarders wait for boat #440 (class 1075, not ported: its state is set by hand here) hidden, without
/// collision (0xc); the boat's state past 1 shows them. With Ratchet in their area and the boat alongside (in its
/// frame x below the boarder's line +0x258 and above −3.1) one jumps (0xd: blend 1, the lob, anim speed
/// `ticks(60)`·4·dt / distance, facing the jump), lands (anim speed 1, 0xe) and goes for him (0xf).
#[test]
fn boarders_wait_hidden_and_jump_aboard() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let (a, boat) = (551usize, 440usize);
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.visible, m.mode & 1, m.has_collision), (pb::st::WAIT_BOAT, 0, 1, false));
    let near = m.position;
    let at = inside(&mut lv, 13, near, 2.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    // The boarders wait out at sea; the boat brings them to the beach (area 13): put this one 6 off Ratchet there.
    lv.table.mobys[a].position = [at[0] + 6.0, at[1], near[2], 0.0];
    hold_boats(&mut lv);
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[a].mode & 1, 1, "hidden while the boat is idle");
    lv.table.mobys[boat].state = 2;
    let p = lv.table.mobys[a].position;
    // (Its hull is not there to land on: the boat's origin well below, so the sea test does not fire.)
    lv.table.mobys[boat].position = [p[0], p[1] + 3.0, p[2] - 30.0, 1.0];
    lv.table.mobys[boat].rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    p::set_ff(&mut lv.table.mobys[a].pvars, pb::pv::BOARD_X, 3.0);
    let mut seen = vec![pb::st::WAIT_BOAT];
    let mut speed = None;
    lv.table.mobys[boat].rows[3] = [0.0, 0.0, 0.0, 1.0];
    for _ in 0..400 {
        // The boat's own update is held (hold_boats): keep it where the test put it.
        lv.table.mobys[boat].state = 2;
        lv.table.mobys[boat].position = [p[0], p[1] + 3.0, p[2] - 30.0, 1.0];
        lv.tick(&hero);
        let m = &lv.table.mobys[a];
        if seen.last() != Some(&m.state) {
            seen.push(m.state);
            if m.state == pb::st::BOARD { speed = Some(m.anim.speed); }
        }
        if seen.contains(&pb::st::ABOARD_GO) { break; }
    }
    let m = &lv.table.mobys[a];
    eprintln!("boarder states {seen:?}, jump anim speed {speed:?}; kind {} moby {} pos {:?} hero {:?} boat {:?} line {}", p::i32(&m.pvars, pb::pv::TGT_KIND), p::i32(&m.pvars, pb::pv::TGT_MOBY), m.position, at, lv.table.mobys[boat].position, p::ff(&m.pvars, pb::pv::BOARD_X));
    let m = &lv.table.mobys[a];
    assert_eq!((m.visible, m.mode & 1), (1, 0), "shown");
    assert!(m.has_collision);
    assert!(seen.starts_with(&[pb::st::WAIT_BOAT, pb::st::BOARD, pb::st::ABOARD]), "{seen:?}");
    assert!(speed.is_some_and(|s| s > 0.0 && s != 1.0));
    assert_eq!(m.anim.speed, 1.0);
}

/// At most five of a group board at once (`0x316090`: class 1246 in 0xd..0x11); a boat in state 5 takes its hidden
/// boarders away (deleted, no explosion); a boarder that sinks below its boat's z + 0.5 blows up (`0x2742a8`).
#[test]
fn boarding_limits_and_the_boats_end() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let (a, boat) = (551usize, 440usize);
    let near = lv.table.mobys[a].position;
    let at = inside(&mut lv, 13, near, 2.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    lv.table.mobys[a].position = [at[0] + 6.0, at[1], near[2], 0.0];
    let group: Vec<MobyId> = lv.of_class(1246).into_iter().filter(|&i| i != a && lv.table.mobys[i].group == 57).collect();
    assert!(group.len() >= 5);
    // The boat leaves (state 5) while they are hidden: taken away (deleted), without an explosion.
    let b = group[6];
    hold_boats(&mut lv);
    let before = lv.svc.fx.part_spawns.values().sum::<u64>();
    lv.table.mobys[boat].state = 5;
    lv.tick(&hero);
    assert!(lv.table.mobys[b].state >= 0x80, "taken away");
    assert_eq!(lv.svc.fx.part_spawns.values().sum::<u64>(), before, "without an explosion");
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { return };
    hold_boats(&mut lv);
    lv.table.mobys[a].position = [at[0] + 6.0, at[1], near[2], 0.0];
    let p = lv.table.mobys[a].position;
    let place = |lv: &mut Lv| {
        lv.table.mobys[boat].state = 2;
        lv.table.mobys[boat].position = [p[0], p[1] + 3.0, p[2] - 10.0, 1.0];
        lv.table.mobys[boat].rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
    };
    place(&mut lv);
    p::set_ff(&mut lv.table.mobys[a].pvars, pb::pv::BOARD_X, 3.0);
    for &g in &group[..5] { lv.table.mobys[g].state = pb::st::BOARD; lv.table.mobys[g].position[2] += 50.0; }
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[a].state, pb::st::WAIT_BOAT, "five boarding: it waits");
    for &g in &group[..5] { lv.table.mobys[g].state = pb::st::WAIT_BOAT; }
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[a].state, pb::st::BOARD, "four: it jumps");
    let before = lv.svc.fx.part_spawns.values().sum::<u64>();
    // A boarder aboard, below the boat: the explosion.
    let c = group[7];
    let q = lv.table.mobys[c].position;
    lv.table.mobys[c].state = pb::st::ABOARD;
    lv.table.mobys[boat].state = 2;
    lv.table.mobys[boat].position = [q[0], q[1], q[2] + 1.0, 1.0];
    lv.tick(&hero);
    assert!(lv.table.mobys[c].state >= 0x80);
    assert!(lv.svc.fx.part_spawns.values().sum::<u64>() > before, "the small explosion");
}

/// The Suck Cannon (1246's table, level11 0x21c038): taken while it walks (state 2 → held 0x14, the state saved in
/// +0xbc), refused in other states (3: nothing changes); let go, the carried update ends and it returns to the
/// saved state (record state 0).
#[test]
fn the_suck_cannon_takes_a_walking_biter() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (499usize, 500usize);
    let near = lv.table.mobys[a].position;
    let hero = hero_at([near[0] + 25.0, near[1], near[2]]);
    lv.tick(&hero);
    lv.table.mobys[b].state = pb::st::GRAZE;
    let mouth = [near[0] + 3.0, near[1], near[2] + 1.0, 1.0];
    {
        let mut w = lv.world(&hero);
        assert_eq!(react::table(&w, a), Some(react::Table::Pokitaru1246));
        assert_eq!(react::record(&w, a), Some(pb::pv::SUCK));
        assert_eq!(w.m(a).state, pb::st::IDLE);
        assert!(react::slot_start(&mut w, a, mouth) != 0);
        assert_eq!((w.m(a).state, w.m(a).cmd), (pb::st::HELD, pb::st::IDLE));
        assert_eq!(react::slot_start(&mut w, b, mouth), 0, "refused while grazing");
        assert_eq!(w.m(b).state, pb::st::GRAZE);
        rc_game::moby_update::creature::set_pi16(&mut w, a, pb::pv::SUCK + react::rec::STATE, 1);
        react::slot_let_go(&mut w, a);
        assert_eq!(w.m(a).state, pb::st::HELD, "slot +0x0c only lets go");
    }
    let mut back = None;
    for k in 0..600 {
        lv.tick(&hero);
        if lv.table.mobys[a].state != pb::st::HELD { back = Some(k); break; }
    }
    assert!(back.is_some(), "the carried update never finished");
    assert_eq!(lv.table.mobys[a].state, pb::st::IDLE);
    assert_eq!(p::i16(&lv.table.mobys[a].pvars, pb::pv::SUCK + react::rec::STATE), 0);
}

/// A knocked biter that went into the sea (the knock record's water count) blows three bubbles a tick (type 34, the
/// pop level 222.5); one that lands in the water (below 222) is deleted.
#[test]
fn a_knocked_biter_in_the_sea_bubbles_and_is_lost() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 499usize;
    let near = lv.table.mobys[a].position;
    let hero = hero_at([near[0] + 25.0, near[1], near[2]]);
    // The boats' wake (type 34 too) is not the biter's: held.
    hold_boats(&mut lv);
    lv.tick(&hero);
    let h = lv.hero_idx;
    lv.hit(&hero, a, &wrench(h, [1.0, 0.0]));
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[a].state, pb::st::KNOCKED);
    p::set_ff(&mut lv.table.mobys[a].pvars, pb::pv::K + 0x5c, 1.0);
    let b0 = parts(&lv, 34);
    lv.tick(&hero);
    assert_eq!(parts(&lv, 34) - b0, 3);
    // Land it outside its area (another beach's path as its area): lost.
    p::set_i32(&mut lv.table.mobys[a].pvars, pb::pv::AREA, 13);
    let mut gone = false;
    for _ in 0..300 {
        lv.tick(&hero);
        let m = &lv.table.mobys[a];
        if m.state != pb::st::KNOCKED { gone = m.state >= 0x80; break; }
    }
    assert!(gone, "state {}", lv.table.mobys[a].state);
}

/// The swimmer #569 (mode 2, path 51, its +0x254 the tide water 1158 #453): the boarders' sea test runs in its state
/// 0x13 too, and the path starts below the tide + 0.5: it blows up on its first tick after the init (the game's
/// code). Without that moby it swims its path loop (the nodes advance, z at most the tide − 1) and, at a key, goes
/// through sequences 4 → 5 (Ratchet within 6) → 6 and bites him within 4 (a sphere of 0.55 at +0.5).
#[test]
fn the_swimmer_blows_up_under_the_tide_or_swims_and_bites() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 569usize;
    let hero = hero_at([0.0, 0.0, -100.0]);
    assert_eq!(lv.table.mobys[a].state, pb::st::SWIM, "the load pass ran its init");
    run(&mut lv, &hero, &[a], 1);
    assert!(lv.table.mobys[a].state >= 0x80, "blown up: {}", lv.table.mobys[a].state);
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { return };
    let _ = hero;
    // A fresh level: take the swimmer back to its init without the tide moby.
    let path = p::i32(&lv.table.mobys[a].pvars, pb::pv::PATH) as usize;
    let pts: Vec<[f32; 4]> = lv.svc.splines[path].iter().map(|q| q.map(f32::from_bits)).collect();
    let m = &mut lv.table.mobys[a];
    p::set_i32(&mut m.pvars, pb::pv::BOAT, -1);
    let hp = pts[1];
    let hero = hero_at([hp[0], hp[1], hp[2]]);
    let mut nodes = Vec::new();
    for _ in 0..1500 {
        lv.tick(&hero);
        let m = &lv.table.mobys[a];
        assert_eq!(m.state, pb::st::SWIM);
        let n = p::i32(&m.pvars, pb::pv::NODE);
        if nodes.last() != Some(&n) { nodes.push(n); }
    }
    let hits = hits_from(&lv, lv.hero_idx, 1246);
    eprintln!("swim nodes {nodes:?} of {}, bites {}", pts.len(), hits.len());
    assert!(nodes.len() >= 2);
    assert!(!hits.is_empty() && hits.iter().all(|h| h.flags == 1 && h.damage == Pf::ONE));
}

// ---------------------------------------------------------------------------------------------------
// U95 / U101: Aridia's sand sharks 580 and their nests 668 (level 02)

use rc_game::moby_update::classes::units::aridia_sandshark as ss;

/// The sharks and nests after the load pass and a quiet run (survey).
#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_580_level() {
    let Some((mut lv, hero)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped"); return };
    let ids = lv.of_class(580);
    let nests = lv.of_class(668);
    let seen = run(&mut lv, &hero, &ids, 300);
    for (k, &i) in ids.iter().enumerate() {
        let m = &lv.table.mobys[i];
        let q = &m.pvars;
        let w = |o: usize| p::i32(q, o);
        eprintln!("#{i} grp {} st {:?} pos {:?} area {} lair {} amb {} {} ambm {} wait {} ctr {} speed {} range {} side {} sign {} alert {}", m.group, seen[k], m.position, w(ss::pv::AREA), w(ss::pv::LAIR), w(ss::pv::AMBUSH_A), w(ss::pv::AMBUSH_B), w(ss::pv::AMBUSH_MOBY), w(ss::pv::WAIT_MOBY), w(ss::pv::COUNTER), p::ff(q, ss::pv::SPEED), p::ff(q, ss::pv::RANGE), p::ff(q, ss::pv::SIDE_DEG), w(ss::pv::SIDE_SIGN), p::i16(q, ss::pv::ALERT));
    }
    for &i in &nests {
        let m = &lv.table.mobys[i];
        eprintln!("nest #{i} grp {} st {} pos {:?} period {} ctr {} b4 {} mission {}", m.group, m.state, m.position, p::i32(&m.pvars, ss::nv::PERIOD), p::i32(&m.pvars, ss::nv::COUNTER), m.b4, m.mission);
    }
    { let c = 0x299usize; let m = &lv.table.mobys[c]; eprintln!("counter #{c} class {} pvars {} 14c {} 150 {}", m.o_class, m.pvars.len(), if m.pvars.len() >= 0x154 { p::i32(&m.pvars, 0x14c) } else { -1 }, if m.pvars.len() >= 0x154 { p::i32(&m.pvars, 0x150) } else { -1 }); }
}

fn cuboid_centre(lv: &Lv, i: i32) -> [f32; 3] { lv.svc.volumes.shape(rc_formats::volumes::ShapeKind::Cuboid, i).unwrap().centre() }
fn counter_word(lv: &Lv, o: usize) -> i32 { p::i32(&lv.table.mobys[0x299].pvars, o) }

/// Group 5 (#450..#460, area 7, nest #554) with Ratchet inside the area: after the dive (0x12 → 0x17) the sharks
/// come for him under the sand (0x16: fin puffs), surface near him (0x13: class sound 4, dive puffs) and bite
/// (6: a sphere of 0.5 ahead of the nose at key 13: hit records on his moby with flags 1, damage 1, class 580).
#[test]
fn sand_sharks_come_up_and_bite_ratchet() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let near = lv.table.mobys[450].position;
    let at = inside(&mut lv, 7, near, 4.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    let ids: Vec<MobyId> = (450..=460).collect();
    let seen = run(&mut lv, &hero, &ids, 1200);
    eprintln!("states {:?}", &seen[..4]);
    let all: Vec<u8> = seen.iter().flatten().copied().collect();
    for s in [ss::st::DIVE_HOME, ss::st::UNDER_HOME, ss::st::UNDER_CHASE, ss::st::UP_CHASE, ss::st::CHASE, ss::st::BITE] { assert!(all.contains(&s), "{s:#x} in {seen:?}"); }
    let hits = hits_from(&lv, lv.hero_idx, 580);
    eprintln!("bites {}, sounds 4: {}, type-2 puffs {}", hits.len(), lv.svc.sounds.iter().filter(|e| e.index == 4 && ids.contains(&e.moby)).count(), parts(&lv, 2));
    assert!(!hits.is_empty() && hits.iter().all(|h| h.flags == 1 && h.damage == Pf::ONE && (h.b28, h.b29, h.h2a) == (0, 1, 580)));
    assert!(lv.svc.sounds.iter().any(|e| e.index == 4 && ids.contains(&e.moby)));
    assert!(parts(&lv, 2) > 0);
}

#[test]
fn sand_shark_run_is_deterministic() {
    let go = || -> Option<(Vec<Vec<u8>>, [f32; 4])> {
        let (mut lv, _) = level_at(2, [0.0, 0.0, -100.0])?;
        let near = lv.table.mobys[450].position;
        let at = inside(&mut lv, 7, near, 4.0)?;
        let hero = hero_at([at[0], at[1], near[2]]);
        let s = run(&mut lv, &hero, &[450, 451, 452], 600);
        Some((s, lv.table.mobys[450].position))
    };
    let (Some(a), Some(b)) = (go(), go()) else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(a, b);
}

/// A wrench hit kills a shark (health 1): the group is alerted (`randf(240, 300)`), the death flight (0x18: flash
/// 0x78, untargetable, the counter moby #665's +0x14c − 1), `SetDeathBits` (the save bit), the landing's sand
/// splash (27 type-2 puffs) and the body's dust (44 type-23 puffs); it waits in the nest (0x1a: 10 under home, no
/// collision), and nest #554 launches it again (0x1b: the launch puffs and 0xd's flight, 0x17; the counter + 1).
#[test]
fn a_killed_shark_is_relaunched_by_its_nest() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 450usize;
    let near = lv.table.mobys[a].position;
    let hero = hero_at([near[0] + 40.0, near[1], near[2]]);
    run(&mut lv, &hero, &[a], 30);
    let n0 = counter_word(&lv, 0x14c);
    let h = lv.hero_idx;
    let (d23, d2) = (parts(&lv, 23), parts(&lv, 2));
    lv.hit(&hero, a, &wrench(h, [1.0, 0.0]));
    run(&mut lv, &hero, &[a], 1);
    let m = &lv.table.mobys[a];
    assert_eq!(m.state, ss::st::DYING);
    assert_eq!(m.pvars[ss::pv::FLASH + 7], 0x78);
    assert_eq!(m.mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    assert_eq!(counter_word(&lv, 0x14c), n0 - 1);
    assert!((451..=460).all(|i| p::i16(&lv.table.mobys[i].pvars, ss::pv::ALERT) > 200), "the group is alerted");
    let sid = m.spawn_id;
    let mut seen = vec![ss::st::DYING];
    for _ in 0..900 {
        let s = run(&mut lv, &hero, &[a], 1);
        if seen.last() != s[0].last() { seen.push(s[0][0]); }
        if seen.contains(&ss::st::UNDER_HOME) { break; }
    }
    eprintln!("after the kill {seen:?}; dust {} splash {}", parts(&lv, 23) - d23, parts(&lv, 2) - d2);
    assert!(lv.svc.save.death.contains(&(2, sid)));
    assert!(parts(&lv, 23) - d23 >= 44, "the dust");
    assert!(parts(&lv, 2) - d2 >= 27, "the splash");
    assert!(seen.starts_with(&[ss::st::DYING, ss::st::NESTED, ss::st::LAUNCHED, ss::st::UNDER_HOME]), "{seen:?}");
    assert_eq!(counter_word(&lv, 0x14c), n0, "the nest counted it back");
    assert_eq!(lv.table.mobys[a].parent, Some(554));
}

/// The ambush (#465: cuboids 36 / 5, the nest #558 as its spring point): hidden under the sand 10 up (0xc) until
/// Ratchet enters a cuboid, then it leaps from the nest toward its home (0xd: spin, the lob) and dives in (0x17).
#[test]
fn the_ambush_springs_from_the_nest() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 465usize;
    assert_eq!((lv.table.mobys[a].state, lv.table.mobys[a].mode & 1), (ss::st::AMBUSH, 1));
    let c36 = cuboid_centre(&lv, 36);
    let hero = hero_at(c36);
    let mut seen = vec![ss::st::AMBUSH];
    let mut from = None;
    for _ in 0..600 {
        let s = run(&mut lv, &hero, &[a], 1);
        if seen.last() != s[0].last() {
            seen.push(s[0][0]);
            if s[0][0] == ss::st::FLIGHT { from = Some(lv.table.mobys[a].position); }
        }
        if seen.contains(&ss::st::UNDER_HOME) { break; }
    }
    eprintln!("ambush {seen:?} from {from:?}");
    assert!(seen.starts_with(&[ss::st::AMBUSH, ss::st::FLIGHT, ss::st::UNDER_HOME]), "{seen:?}");
    let n = lv.table.mobys[558].position;
    let f = from.unwrap();
    assert!((f[0] - n[0]).abs() < 0.5 && (f[1] - n[1]).abs() < 0.5, "from the nest's position");
    assert!(lv.table.mobys[a].mode & rc_game::moby_runtime::mode::TARGETABLE != 0 && lv.table.mobys[a].has_collision);
}

/// The waiting sharks (#430: +0x258 = #723) start (0 → the init) once that moby reaches state 5: shown, targetable,
/// with collision; a lair shark (#428: cuboid 32) comes for a target in its lair (3 → 5).
#[test]
fn waiting_and_lair_sharks_start() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    assert_eq!(lv.table.mobys[430].state, ss::st::WAIT);
    run(&mut lv, &hero, &[430], 5);
    assert_eq!(lv.table.mobys[430].state, ss::st::WAIT);
    lv.table.mobys[723].state = 5;
    run(&mut lv, &hero, &[430], 1);
    let m = &lv.table.mobys[430];
    eprintln!("#430 after its moby: state {} mode {:#x}", m.state, m.mode);
    assert_ne!(m.state, ss::st::WAIT);
    assert_eq!(m.mode & 0x41, 0);
    assert!(m.has_collision);
    let c32 = cuboid_centre(&lv, 32);
    assert_eq!(lv.table.mobys[428].state, ss::st::LAIR);
    let hero = hero_at(c32);
    let s = run(&mut lv, &hero, &[428], 60);
    eprintln!("lair #428 {s:?}");
    assert!(s[0].contains(&ss::st::CHASE));
}

/// The nest #554 (health 3): a wrench hit knocks it (3: flash 0x78, blend 2) and it settles (1); the third blows it
/// up (4: untargetable, flash 0xfa, `SetDeathBits`): the beam explosion, the pieces 1764 ×2 and 1765 ×2, the counter
/// moby's +0x150 − 1, deleted.
#[test]
fn a_nest_is_knocked_and_destroyed() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let n = 554usize;
    let near = lv.table.mobys[n].position;
    let hero = hero_at([near[0] + 40.0, near[1], near[2]]);
    run(&mut lv, &hero, &[n], 5);
    let h = lv.hero_idx;
    let c0 = counter_word(&lv, 0x150);
    for k in 0..2 {
        lv.hit(&hero, n, &wrench(h, [1.0, 0.0]));
        run(&mut lv, &hero, &[n], 1);
        assert_eq!(lv.table.mobys[n].state, ss::nst::KNOCKED, "hit {k}");
        assert_eq!(lv.table.mobys[n].pvars[ss::nv::FLASH + 7], 0x78);
        let s = run(&mut lv, &hero, &[n], 200);
        assert_eq!(lv.table.mobys[n].state, ss::nst::IDLE, "{s:?}");
    }
    let pieces = |lv: &Lv| lv.table.mobys.iter().filter(|m| (m.o_class == 1764 || m.o_class == 1765) && m.state < 0x80).count();
    let p0 = pieces(&lv);
    lv.hit(&hero, n, &wrench(h, [1.0, 0.0]));
    run(&mut lv, &hero, &[n], 1);
    let m = &lv.table.mobys[n];
    assert_eq!((m.state, m.pvars[ss::nv::FLASH + 7]), (ss::nst::DYING, 0xfa));
    assert_eq!(m.mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    assert!(lv.svc.save.death.contains(&(2, m.spawn_id)));
    let mut pk = 0;
    for _ in 0..300 {
        run(&mut lv, &hero, &[n], 1);
        pk = pk.max(pieces(&lv));
        if lv.table.mobys[n].state >= 0x80 { break; }
    }
    assert!(lv.table.mobys[n].state >= 0x80);
    assert_eq!(pk - p0, 4, "four pieces");
    assert_eq!(counter_word(&lv, 0x150), c0 - 1);
}

/// The Suck Cannon (580's table, level02 0x1fb6dc): taken (held 9); swallowed: the counter moby's +0x14c − 1; the
/// delete slot sends it back to a nest (0x1a). The Morph-o-Ray's keep (+0x2e = 2) does the same through the tick.
#[test]
fn the_suck_cannon_and_the_morph_ray_on_a_shark() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (450usize, 451usize);
    let near = lv.table.mobys[a].position;
    let hero = hero_at([near[0] + 40.0, near[1], near[2]]);
    run(&mut lv, &hero, &[a, b], 30);
    let n0 = counter_word(&lv, 0x14c);
    let mouth = [near[0] + 3.0, near[1], near[2] + 1.0, 1.0];
    {
        let mut w = lv.world(&hero);
        assert_eq!(react::table(&w, a), Some(react::Table::Aridia580));
        assert!(react::slot_start(&mut w, a, mouth) != 0);
        assert_eq!(w.m(a).state, ss::st::HELD);
        rc_game::moby_update::creature::set_pi16(&mut w, a, ss::pv::SUCK + react::rec::STATE, 3);
        assert!(react::slot_swallow(&mut w, a, mouth) != 0);
        react::slot_delete(&mut w, a);
        assert_eq!(w.m(a).state, ss::st::NESTED);
    }
    assert_eq!(counter_word(&lv, 0x14c), n0 - 1);
    p::set_u8(&mut lv.table.mobys[b].pvars, ss::pv::KEEP, 2);
    run(&mut lv, &hero, &[b], 1);
    assert_eq!(lv.table.mobys[b].state, ss::st::NESTED);
    assert_eq!(counter_word(&lv, 0x14c), n0 - 2);
}

// ---------------------------------------------------------------------------------------------------
// U96: Aridia's flame-throwing sentries 612 (level 02)

use rc_game::moby_update::classes::units::aridia_flamer as af;

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_612_level() {
    let Some((mut lv, hero)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped"); return };
    let ids = lv.of_class(612);
    let seen = run(&mut lv, &hero, &ids, 120);
    for (k, &i) in ids.iter().enumerate() {
        let m = &lv.table.mobys[i];
        let q = &m.pvars;
        eprintln!("#{i} st {:?} pos {:?} guard {} wake {} post {} wcub {} mode {}", seen[k], m.position, p::ff(q, af::pv::GUARD_R), p::ff(q, af::pv::WAKE_R), p::i32(q, af::pv::POST), p::i32(q, af::pv::WAKE_CUBOID), p::i32(q, af::pv::MODE));
    }
}

/// Sentry #516 asleep (9, mode 3; post cuboid 12, wake radius 20, guard radius 18) with Ratchet 5 from its post: it
/// smokes while asleep (type 21 every 4th tick), wakes (2), rises onto the ground at the post (3 → 4), walks there
/// (5), faces him (7 → 0xb) and flames (0xc: type-12 flames and glow puffs, the smoke sparks of type 25; the kept
/// flames' hits on his moby: flags 0x10001, damage 1, type 5 / 1, class 612).
#[test]
fn a_sentry_wakes_and_flames_ratchet() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 516usize;
    let post = cuboid_centre(&lv, 12);
    let s21 = parts(&lv, 21);
    // Ratchet away first (beyond the wake radius): it sleeps and smokes.
    let far = hero_at([post[0] + 30.0, post[1], post[2]]);
    let mut seen = run(&mut lv, &far, &[a], 40);
    assert_eq!(seen[0], vec![af::st::ASLEEP_B]);
    assert!(parts(&lv, 21) - s21 >= 9, "a smoke puff every 4th tick");
    let hero = hero_at([post[0] + 3.0, post[1], post[2]]);
    // With the particle update after each tick (the engine's order): the kept flames fly out to him.
    for _ in 0..1500 {
        let s = run(&mut lv, &hero, &[a], 1);
        if seen[0].last() != s[0].last() { seen[0].push(s[0][0]); }
        lv.particles.update_parts(&mut lv.rng);
    }
    eprintln!("sentry {seen:?}; smoke {} flames {} sparks {}", parts(&lv, 21) - s21, parts(&lv, 12), parts(&lv, 25));
    for s in [af::st::ASLEEP_B, af::st::WAKE, af::st::RISE, af::st::LANDED, af::st::TO_POST, af::st::GUARD, af::st::AIM, af::st::FLAME] {
        assert!(seen[0].contains(&s), "{s:#x} in {seen:?}");
    }
    assert!(parts(&lv, 12) > 0 && parts(&lv, 25) > 0);
    let hits = hits_from(&lv, lv.hero_idx, 612);
    eprintln!("flame hits {}", hits.len());
    assert!(!hits.is_empty() && hits.iter().all(|h| h.flags == 0x1_0001 && h.damage == Pf::ONE && (h.b28, h.b29, h.h2a) == (5, 1, 612)), "{hits:?}");
}

#[test]
fn sentry_run_is_deterministic() {
    let go = || -> Option<(Vec<Vec<u8>>, [f32; 4], usize)> {
        let (mut lv, _) = level_at(2, [0.0, 0.0, -100.0])?;
        let post = cuboid_centre(&lv, 12);
        let hero = hero_at([post[0] + 3.0, post[1], post[2]]);
        let s = run(&mut lv, &hero, &[516], 900);
        let n = hits_from(&lv, lv.hero_idx, 612).len();
        Some((s, lv.table.mobys[516].position, n))
    };
    let (Some(a), Some(b)) = (go(), go()) else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(a, b);
}

/// Weapons on sentry #515 (health 3): a hit under 1 flashes it (100) without a knockback; a wrench hit (1) knocks it
/// away from Ratchet (0x10: flash 0x78, 6·dt / 6·dt) and it guards again (7); another sentry's flame does nothing; the
/// last hit sends it flying (0x11: flash 0xfa, untargetable) and at the end blows it up (`SetDeathBits`, the beam
/// explosion and the five pieces 1751, 1752, 1753, 1769, 1922).
#[test]
fn weapons_knock_back_and_blow_up_a_sentry() {
    let Some((mut lv, _)) = level_at(2, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 515usize;
    let q = lv.table.mobys[a].position;
    let hero = hero_at([q[0] + 40.0, q[1], q[2]]);
    run(&mut lv, &hero, &[a], 5);
    let h = lv.hero_idx;
    let mut weak = wrench(h, [1.0, 0.0]);
    weak.damage = Pf::f(0.5);
    lv.hit(&hero, a, &weak);
    run(&mut lv, &hero, &[a], 1);
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.pvars[af::pv::FLASH + 7]), (af::st::GUARD, 100));
    assert_eq!(p::ff(&m.pvars, af::pv::D), 2.5);
    run(&mut lv, &hero, &[a], 90);
    let mut own = wrench(516, [1.0, 0.0]);
    own.h1a = 612;
    lv.hit(&hero, a, &own);
    run(&mut lv, &hero, &[a], 1);
    assert_eq!(p::ff(&lv.table.mobys[a].pvars, af::pv::D), 2.5, "its own class's flames do nothing");
    run(&mut lv, &hero, &[a], 90);
    lv.hit(&hero, a, &wrench(h, [1.0, 0.0]));
    run(&mut lv, &hero, &[a], 1);
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.pvars[af::pv::FLASH + 7]), (af::st::KNOCKED, 0x78));
    let s = run(&mut lv, &hero, &[a], 300);
    assert_eq!(lv.table.mobys[a].state, af::st::GUARD, "{s:?}");
    let pieces = |lv: &Lv| lv.table.mobys.iter().filter(|m| [1751, 1752, 1753, 1769, 1922].contains(&m.o_class) && m.state < 0x80).count();
    let p0 = pieces(&lv);
    run(&mut lv, &hero, &[a], 90);
    let mut strong = wrench(h, [1.0, 0.0]);
    strong.damage = Pf::f(2.0);
    lv.hit(&hero, a, &strong);
    run(&mut lv, &hero, &[a], 1);
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.pvars[af::pv::FLASH + 7]), (af::st::DYING, 0xfa));
    assert_eq!(m.mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    let sid = m.spawn_id;
    assert!(!lv.svc.save.death.contains(&(2, sid)), "not before the flight ends");
    let mut pk = 0;
    for _ in 0..300 {
        run(&mut lv, &hero, &[a], 1);
        pk = pk.max(pieces(&lv));
        if lv.table.mobys[a].state >= 0x80 { break; }
    }
    assert!(lv.table.mobys[a].state >= 0x80);
    assert!(lv.svc.save.death.contains(&(2, sid)), "SetDeathBits at the end of the flight");
    assert_eq!(pk - p0, 5);
}

// ---------------------------------------------------------------------------------------------------
// U373: Pokitaru's ball throwers 1231 and their ball 1297 (level 11)

use rc_game::moby_update::classes::units::pokitaru_thrower as pt;

fn balls(lv: &Lv) -> Vec<MobyId> { lv.of_class(1297) }

/// Thrower #491 (area path 23) with Ratchet inside its area 6 away: stand (3) → wind up (4: at key 15 the ball 1297
/// appears in its hand with 50 converging sparkles, type 2) → throw (5: at key 33 the ball flies off at 30 u/s along
/// its facing; the ball's glow, four type-59 sprites a tick, and its trail) → recover (6) → 3. The ball blows up on
/// contact (the beam explosion's damage sphere: a hit record on Ratchet's moby, damage 2) and is deleted.
#[test]
fn a_thrower_throws_a_ball_at_ratchet() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 491usize;
    let area = p::i32(&lv.table.mobys[a].pvars, pt::pv::AREA) as usize;
    let near = lv.table.mobys[a].position;
    let at = inside(&mut lv, area, near, 6.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    let (s2, s59) = (parts(&lv, 2), parts(&lv, 59));
    let mut seen = Vec::new();
    let mut held = false;
    let mut flew = false;
    let mut blew = false;
    for _ in 0..900 {
        let s = run(&mut lv, &hero, &[a], 1);
        if seen.last() != s[0].last() { seen.push(s[0][0]); }
        for b in balls(&lv) {
            match lv.table.mobys[b].state {
                pt::bst::HELD => held = true,
                pt::bst::FLYING => flew = true,
                _ => {}
            }
        }
        if flew && balls(&lv).is_empty() { blew = true; }
        if blew { break; }
    }
    let hits: Vec<_> = lv.hits_on(lv.hero_idx).into_iter().filter(|h| h.attacker.is_some_and(|x| lv.table.mobys[x].o_class == 1297)).collect();
    eprintln!("thrower {seen:?}; held {held} flew {flew} blew {blew}; sparkles/trail {} glow {}; ball hits {hits:?}", parts(&lv, 2) - s2, parts(&lv, 59) - s59);
    for s in [pt::st::STAND, pt::st::WIND, pt::st::THROW] { assert!(seen.contains(&s), "{s} in {seen:?}"); }
    assert!(held && flew && blew);
    assert!(parts(&lv, 2) - s2 >= 50, "the sparkles");
    assert!(parts(&lv, 59) - s59 >= 4);
    assert!(hits.iter().any(|h| h.damage == Pf::f(2.0)), "the blast hit him");
}

#[test]
fn thrower_run_is_deterministic() {
    let go = || -> Option<(Vec<Vec<u8>>, [f32; 4], usize)> {
        let (mut lv, _) = level_at(11, [0.0, 0.0, -100.0])?;
        let area = p::i32(&lv.table.mobys[491].pvars, pt::pv::AREA) as usize;
        let near = lv.table.mobys[491].position;
        let at = inside(&mut lv, area, near, 6.0)?;
        let hero = hero_at([at[0], at[1], near[2]]);
        let s = run(&mut lv, &hero, &[491], 400);
        Some((s, lv.table.mobys[491].position, balls(&lv).len()))
    };
    let (Some(a), Some(b)) = (go(), go()) else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(a, b);
}

/// A wrench hit while it holds a ball: the ball is dropped (deleted), the knockback (7: flash 0x78, 4·dt / 4·dt) and
/// back to 3 on its area; two more hits kill it (8: flash 0xf0, untargetable, `SetDeathBits`; the explosion, deleted).
#[test]
fn weapons_knock_back_and_kill_a_thrower() {
    let Some((mut lv, _)) = level_at(11, [0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let a = 491usize;
    let area = p::i32(&lv.table.mobys[a].pvars, pt::pv::AREA) as usize;
    let near = lv.table.mobys[a].position;
    let at = inside(&mut lv, area, near, 6.0).unwrap();
    let hero = hero_at([at[0], at[1], near[2]]);
    for _ in 0..900 {
        run(&mut lv, &hero, &[a], 1);
        if p::i32(&lv.table.mobys[a].pvars, pt::pv::BALL) != 0 { break; }
    }
    let b = (p::i32(&lv.table.mobys[a].pvars, pt::pv::BALL) - 1) as usize;
    assert_eq!(lv.table.mobys[b].o_class, 1297);
    let h = lv.hero_idx;
    lv.hit(&hero, a, &wrench(h, [1.0, 0.0]));
    run(&mut lv, &hero, &[a], 1);
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.pvars[pt::pv::FLASH + 7]), (pt::st::KNOCKED, 0x78));
    assert!(lv.table.mobys[b].state >= 0x80, "the ball dropped");
    assert_eq!(p::i32(&m.pvars, pt::pv::BALL), 0);
    let s = run(&mut lv, &hero, &[a], 300);
    eprintln!("after the knock {s:?}");
    for _ in 0..2 {
        run(&mut lv, &hero, &[a], 90);
        lv.hit(&hero, a, &wrench(h, [1.0, 0.0]));
        run(&mut lv, &hero, &[a], 1);
        if lv.table.mobys[a].state == pt::st::DYING { break; }
    }
    let m = &lv.table.mobys[a];
    assert_eq!((m.state, m.pvars[pt::pv::FLASH + 7]), (pt::st::DYING, 0xf0));
    assert_eq!(m.mode & rc_game::moby_runtime::mode::TARGETABLE, 0);
    assert!(lv.svc.save.death.contains(&(11, m.spawn_id)));
    for _ in 0..300 {
        run(&mut lv, &hero, &[a], 1);
        if lv.table.mobys[a].state >= 0x80 { break; }
    }
    assert!(lv.table.mobys[a].state >= 0x80);
}
