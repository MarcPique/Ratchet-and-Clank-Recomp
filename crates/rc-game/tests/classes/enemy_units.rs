//! The enemy class units of W2 lane 1 (docs/plan/creatures.md §10, gaps.md G-ENM-010 / G-ENM-001): each unit resolves
//! to its port on its levels (the census's instance counts) and runs headless on one of them — movement / AI, its
//! attack on Ratchet, a weapon hit through the hit records and the resolver, its death with drops, and every side
//! effect its coverage table marks ported. Skipped when `extracted/` is absent. The level harness is
//! `creature_classes`'s.

use crate::creature_classes::{hero_at, load, wrench, Lv};
use rc_formats::{gameplay, moby_spawn};
use rc_game::hero::Hero;
use rc_game::moby_runtime::{mode, MobyId};
use rc_game::moby_update::classes::units::{self, wave_gate as wg, area_stalker as ast, buzz_bomb as bb, flying_biter as fb, hover_zapper as hz, pack_biter as pb, rolling_mine as rm};
use rc_game::moby_update::classes::{ClassUpdate, LevelPorts};
use rc_game::moby_update::creature::react;
use rc_game::moby_update::services::pvar as p;
use std::collections::HashMap;

fn ports(level: u32) -> Option<LevelPorts> { crate::common::ports(level, &[]) }

fn unit(name: &str) -> ClassUpdate {
    let i = units::PORTS.iter().position(|u| u.unit == name).unwrap_or_else(|| panic!("no unit {name}"));
    ClassUpdate::Unit(i as u16)
}

/// (unit row, class, the levels whose table runs it, created instances on them).
const EXPECTED: &[(&str, i16, &[u32], usize)] = &[("U553 mine", 568, &[18], 20), ("U301", 193, &[9, 15], 93), ("U268 252", 252, &[8, 14], 77), ("U407 63", 63, &[13], 58), ("U300 52", 52, &[9, 16], 46), ("U521 1445", 1445, &[16], 24), ("U426 1271", 1271, &[13], 1)];

/// Every unit's classes resolve to the unit on its levels (and nowhere else), with the census's instance counts.
#[test]
fn enemy_units_resolve_on_their_levels() {
    let Some(_) = crate::common::overlay(1) else { eprintln!("skipped: no extracted/"); return };
    let mut created: HashMap<(&str, i16), usize> = HashMap::new();
    for level in 0..19u32 {
        let pt = ports(level).unwrap();
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
    }
    eprintln!("created instances now ported: {created:?}");
    for &(name, oc, _, n) in EXPECTED { assert_eq!(created[&(name, oc)], n, "{name} class {oc}"); }
}

/// The reaction tables of the ported units are found by code identity on their levels only.
#[test]
fn enemy_reaction_tables_resolve() {
    let Some(ov1) = crate::common::overlay(1) else { eprintln!("skipped"); return };
    for level in 0..19u32 {
        let t = react::tables_from_overlays(&crate::common::overlay(level).unwrap(), &ov1, &crate::common::overlay);
        assert_eq!(t.get(&568) == Some(&react::Table::RollingMine568), level == 18, "level {level:02}: {:?}", t.get(&568));
        assert_eq!(t.get(&193) == Some(&react::Table::Held7), [9, 15].contains(&level), "level {level:02}: {:?}", t.get(&193));
        assert_eq!(t.get(&252) == Some(&react::Table::Hover252), [8, 14].contains(&level), "level {level:02}: {:?}", t.get(&252));
        assert_eq!(t.get(&1445) == Some(&react::Table::Held7), level == 16, "level {level:02}: {:?}", t.get(&1445));
        assert_eq!(t.get(&63) == Some(&react::Table::Critter), level == 13, "level {level:02}: {:?}", t.get(&63));
        let mine: Vec<_> = t.iter().filter(|(c, _)| [193, 1445, 63, 252, 568, 577, 572, 866, 270, 749].contains(*c)).collect();
        eprintln!("L{level:02}: {mine:?}");
    }
}

// ---------------------------------------------------------------------------------------------------
// U553: 568, the rolling mines of level 18 (the boss 1422's pool)

/// Level 18 with its mines parked by their first update; Ratchet at `at`.
fn mines(at: [f32; 3]) -> Option<(Lv, Hero, Vec<MobyId>)> {
    let mut lv = load(18)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    let ids = lv.table.mobys.iter().enumerate().filter(|(_, m)| m.o_class == 568).map(|(i, _)| i).collect();
    Some((lv, hero, ids))
}

/// What level18 `0x2d5cf8` (the boss 1422's code, not ported) leaves in a mine it throws: visible, targetable, with
/// collision, state 1 at `from` with the throw velocity `vel` (a tick), the landing height, the fuse and yaw.
fn throw(lv: &mut Lv, id: MobyId, from: [f32; 3], vel: [f32; 3], land_z: f32, fuse: i32) {
    let has = lv.classes.classes.get(&568).is_some_and(|c| c.0.has_collision);
    let m = &mut lv.table.mobys[id];
    m.update_dist = 0xff;
    m.draw_dist = 0xff;
    m.state = 1;
    m.mode = (m.mode & 0xffbe) | mode::TARGETABLE;
    m.visible = 1;
    m.cmd = 0;
    m.has_collision = has;
    m.position = [from[0], from[1], from[2], 1.0];
    m.rotation[0] = 0.0;
    m.rotation[1] = -2.5;
    m.rotation[2] = vel[1].atan2(vel[0]);
    p::set_v4f(&mut m.pvars, rm::pv::LAND, [from[0], from[1], land_z, 1.0]);
    p::set_v4f(&mut m.pvars, rm::pv::THROW, [vel[0], vel[1], vel[2], 0.0]);
    p::set_i32(&mut m.pvars, rm::pv::FUSE, fuse);
    p::set_i32(&mut m.pvars, rm::pv::TOUCHED, 0);
}

fn parked(lv: &Lv, id: MobyId) -> bool {
    let m = &lv.table.mobys[id];
    m.state == rm::st::PARKED && !m.has_collision && m.mode & mode::HIDDEN != 0 && m.mode & mode::TARGETABLE == 0
}

/// A mine near Ratchet's feet on level 18, the spot the tests throw it from.
fn spot(lv: &Lv, id: MobyId) -> [f32; 3] { let q = lv.table.mobys[id].position; [q[0], q[1], q[2]] }

#[test]
fn rolling_mines_park_on_their_first_update() {
    let Some((lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    assert_eq!(ids.len(), 20);
    for &id in &ids {
        assert!(parked(&lv, id), "mine {id}: {:?}", lv.table.mobys[id].state);
        assert_eq!(p::ff(&lv.table.mobys[id].pvars, 0x30), 0.05);
    }
}

/// Thrown: it falls to the landing height, rolls out along its heading, then rolls to Ratchet within 6.
#[test]
fn a_thrown_mine_falls_rolls_out_and_rolls_to_ratchet() {
    let Some((mut lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let id = ids[0];
    let s = spot(&lv, id);
    let ground = s[2];
    // Ratchet 4 units east of where the roll-out ends (it rolls 2.5·dt·… ≈ 1.25 east).
    let hero = hero_at([s[0] + 5.0, s[1], ground]);
    throw(&mut lv, id, [s[0], s[1], ground + 2.0], [0.0, 0.0, 0.05], ground, 10_000);
    let mut states = Vec::new();
    let mut top = ground;
    for _ in 0..240 {
        lv.tick(&hero);
        let m = &lv.table.mobys[id];
        top = top.max(m.position[2]);
        if states.last() != Some(&m.state) { states.push(m.state); }
    }
    assert_eq!(states, vec![1, 2, 3], "fall, roll-out, chase");
    assert!(ground + 2.0 < top, "it went up first: {top}");
    let m = &lv.table.mobys[id];
    let d0 = 5.0f32;
    let d = ((m.position[0] - (s[0] + 5.0)).powi(2) + (m.position[1] - s[1]).powi(2)).sqrt();
    assert!(d < d0 - 1.5, "rolled toward Ratchet: {d}");
    assert!(m.rotation[1] != -2.5, "rolled");
    assert_eq!(m.anim.seq_b, 2, "sequence 1 after the roll-out, then 2 once it wrapped: {}", m.anim.seq_b);
    assert!(lv.svc.sounds.iter().all(|e| e.index != 1), "no blast yet");
}

/// A weapon hit (the wrench's record) sets it off: the small blast (class sound 1, streaks and sparks, no damage),
/// then it is parked again.
#[test]
fn a_weapon_hit_sets_the_mine_off() {
    let Some((mut lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let id = ids[1];
    let s = spot(&lv, id);
    let hero = hero_at([s[0] + 30.0, s[1], s[2]]);
    throw(&mut lv, id, [s[0], s[1], s[2] + 0.1], [0.0, 0.0, 0.0], s[2], 10_000);
    for _ in 0..90 { lv.tick(&hero); }
    assert_eq!(lv.table.mobys[id].state, rm::st::CHASE);
    let h = lv.hero_idx;
    lv.hit(&hero, id, &wrench(h, [1.0, 0.0]));
    lv.tick(&hero);
    assert!(parked(&lv, id));
    assert_eq!(lv.svc.sounds.iter().filter(|e| e.moby == id && e.index == 1).count(), 1, "the blast's class sound 1");
    assert_eq!(lv.svc.fx.part_spawns.get(&15).copied().unwrap_or(0), 5, "5 streaks");
    assert!(lv.hits_on(h).is_empty(), "the small blast has no damage sphere");
    assert!(lv.svc.camera_shakes.is_empty());
}

/// The fuse runs out: the small blast.
#[test]
fn the_fuse_sets_the_mine_off() {
    let Some((mut lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let id = ids[2];
    let s = spot(&lv, id);
    let hero = hero_at([s[0] + 30.0, s[1], s[2]]);
    throw(&mut lv, id, [s[0], s[1], s[2] + 0.1], [0.0, 0.0, 0.0], s[2], 60);
    let mut at = None;
    for t in 0..120 {
        lv.tick(&hero);
        if at.is_none() && parked(&lv, id) { at = Some(t); }
    }
    assert_eq!(at, Some(59), "the 60th tick: the fuse ran out");
    assert_eq!(lv.svc.sounds.iter().filter(|e| e.moby == id && e.index == 1).count(), 1);
}

/// Ratchet within 2 of a falling mine sets it off (small); a mine that rolls into a decoy blows up large: its 1.5
/// damage sphere hits Ratchet's moby (flags 0x810001, damage 1, type 2 / 1, class 568) and shakes the camera.
#[test]
fn ratchet_near_a_falling_mine_and_a_decoy_contact() {
    let Some((mut lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let id = ids[3];
    let s = spot(&lv, id);
    let hero = hero_at([s[0] + 1.0, s[1], s[2]]);
    throw(&mut lv, id, [s[0], s[1], s[2] + 3.0], [0.0, 0.0, 0.0], s[2], 10_000);
    lv.tick(&hero);
    assert!(parked(&lv, id), "Ratchet within 2 of the falling mine");
    assert!(lv.hits_on(lv.hero_idx).is_empty());

    // A decoy (the Decoy Glove's 0xcb in state 3) 5 units east, Ratchet beside the mine's path short of it.
    let id = ids[4];
    let s = spot(&lv, id);
    let hero = hero_at([s[0] + 4.0, s[1] + 0.8, s[2]]);
    let decoy = { let mut w = lv.world(&hero); w.create_moby(0xcb).unwrap() };
    {
        let d = &mut lv.table.mobys[decoy];
        d.state = 3;
        d.mode |= mode::NO_UPDATE;
        d.position = [s[0] + 5.0, s[1], s[2], 1.0];
    }
    // Landing above the step east of the pool spot (floor 103.16 vs the spot's 102.9): the chase's gravity
    // (−10·dt² a tick, `0x221460` keeps it) would otherwise sink a mine rolled into the step.
    throw(&mut lv, id, [s[0], s[1], s[2] + 0.8], [0.0, 0.0, 0.0], s[2] + 0.8, 10_000);
    let mut blew = None;
    for t in 0..400 {
        lv.tick(&hero);
        if blew.is_none() && parked(&lv, id) { blew = Some(t); break; }
    }
    assert!(blew.is_some(), "the decoy contact");
    let hits = lv.hits_on(lv.hero_idx);
    eprintln!("blew at {blew:?}, mine at {:?}, hits on Ratchet {hits:?}", lv.table.mobys[id].position);
    assert_eq!(hits.len(), 1);
    assert_eq!((hits[0].flags, hits[0].damage.to_f32(), hits[0].b28, hits[0].b29, hits[0].h2a), (0x81_0001, 1.0, 2, 1, 568));
    assert!(!lv.svc.camera_shakes.is_empty(), "the large blast shakes the camera");
    assert_eq!(lv.svc.fx.part_spawns.get(&15).copied().unwrap_or(0), 5 + 20, "the small blast's 5 streaks, the large one's 20");
}

/// The Suck Cannon: 568's own table (level18 0x2d6108..): taken only while rolling (state 3 → held 4, the state
/// saved in +0xbc, slot +0x00 returns 2), refused while falling (record state 0); let go, the carried update brings it
/// back to 3 (record state 0); slot +0x14 parks it instead of deleting it.
#[test]
fn the_suck_cannon_takes_a_rolling_mine() {
    let Some((mut lv, _, ids)) = mines([0.0, 0.0, -100.0]) else { eprintln!("skipped: no extracted/"); return };
    let (a, b) = (ids[5], ids[6]);
    let s = spot(&lv, a);
    let hero = hero_at([s[0] + 30.0, s[1], s[2]]);
    throw(&mut lv, a, [s[0], s[1], s[2] + 0.1], [0.0, 0.0, 0.0], s[2], 10_000);
    for _ in 0..90 { lv.tick(&hero); }
    let sb = spot(&lv, b);
    throw(&mut lv, b, [sb[0], sb[1], sb[2] + 50.0], [0.0, 0.0, 0.0], sb[2], 10_000);
    lv.tick(&hero);
    let mouth = [s[0] + 3.0, s[1], s[2] + 1.0, 1.0];
    {
        let mut w = lv.world(&hero);
        assert_eq!(react::table(&w, a), Some(react::Table::RollingMine568));
        assert_eq!(react::record(&w, a), Some(rm::pv::RECORD));
        assert_eq!(w.m(a).state, rm::st::CHASE);
        assert_eq!(react::slot_start(&mut w, a, mouth), 2);
        assert_eq!((w.m(a).state, w.m(a).cmd), (rm::st::HELD, rm::st::CHASE));
        assert_eq!(w.m(b).state, rm::st::FALL);
        rc_game::moby_update::creature::set_pi16(&mut w, b, rm::pv::RECORD + react::rec::STATE, 5);
        assert_eq!(react::slot_start(&mut w, b, mouth), 0, "refused while falling");
        assert_eq!((w.m(b).state, rc_game::moby_update::creature::pi16(&w, b, rm::pv::RECORD + react::rec::STATE)), (rm::st::FALL, 0));
        // Let go (record state 1 → falls): the state stays held.
        rc_game::moby_update::creature::set_pi16(&mut w, a, rm::pv::RECORD + react::rec::STATE, 1);
        react::slot_let_go(&mut w, a);
        assert_eq!(w.m(a).state, rm::st::HELD);
    }
    let mut back = None;
    for k in 0..600 {
        lv.tick(&hero);
        if lv.table.mobys[a].state != rm::st::HELD { back = Some(k); break; }
    }
    assert!(back.is_some(), "the carried update never finished");
    assert_eq!(lv.table.mobys[a].state, rm::st::CHASE);
    assert_eq!(p::i16(&lv.table.mobys[a].pvars, rm::pv::RECORD + react::rec::STATE), 0);
    {
        let mut w = lv.world(&hero);
        react::slot_delete(&mut w, a);
    }
    assert!(lv.table.mobys[a].state == rm::st::PARKED && lv.table.mobys[a].visible == 0 && lv.table.mobys[a].mode & mode::HIDDEN != 0);
}

/// The pvars of the created instances of `RC_SURVEY=level:class,…` (survey; words, floats where plausible).
#[test]
#[ignore = "survey: prints, asserts nothing; RC_SURVEY=9:193 … --ignored --nocapture"]
fn survey_enemy_pvars() {
    let Ok(spec) = std::env::var("RC_SURVEY") else { return };
    for item in spec.split(',') {
        let (l, c) = item.split_once(':').unwrap();
        let (level, oc): (u32, i16) = (l.parse().unwrap(), c.parse().unwrap());
        let Some(gp) = rc_formats::test_data::gameplay(level) else { eprintln!("skipped: no extracted/"); return };
        let inst = gameplay::parse_moby_instances(&gp).unwrap();
        let tests = moby_spawn::loader_spawns(&inst, &mut moby_spawn::SpawnSave::default());
        let spawned: Vec<bool> = tests.iter().map(|t| t.spawn).collect();
        let pvars = gameplay::parse_pvars_spawned(&gp, &spawned).unwrap();
        let mut n = 0;
        let mut distinct: std::collections::BTreeMap<usize, std::collections::BTreeSet<u32>> = Default::default();
        for (i, m) in inst.iter().enumerate() {
            if m.o_class as i16 != oc || !spawned[i] { continue; }
            n += 1;
            let pv = usize::try_from(m.pvar_index).ok().and_then(|k| pvars.get(k)).and_then(|b| b.as_deref()).unwrap_or(&[]);
            for (k, c) in pv.chunks(4).enumerate() {
                if c.len() == 4 { distinct.entry(k * 4).or_default().insert(u32::from_le_bytes(c.try_into().unwrap())); }
            }
            if n <= 2 { eprintln!("L{level:02} {oc} inst {i} pos {:?} rot {:?} scale {} group {} pvars {} bytes", m.position, m.rotation, m.scale, m.group, pv.len()); }
        }
        eprintln!("L{level:02} class {oc}: {n} created; words that vary or are nonzero:");
        for (o, set) in distinct {
            if set.len() == 1 && set.contains(&0) { continue; }
            let v: Vec<String> = set.iter().take(6).map(|&u| { let f = f32::from_bits(u); if f.is_finite() && f.abs() > 1e-4 && f.abs() < 1e6 { format!("{u:08x}({f})") } else { format!("{u:08x}") } }).collect();
            eprintln!("  +{o:03x}: {}{}", v.join(" "), if set.len() > 6 { " …" } else { "" });
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// U301: 193, the pack biters of levels 09 / 15

/// Level 09 with Ratchet at `at` after the load pass and one tick (the biters' init).
fn biters(level: u32, at: [f32; 3]) -> Option<(Lv, Hero, Vec<MobyId>)> {
    let mut lv = load(level)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    lv.tick(&hero);
    let ids = lv.of_class(193);
    Some((lv, hero, ids))
}

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_biters() {
    let Some((lv, _, ids)) = biters(9, [0.0, 0.0, -100.0]) else { eprintln!("skipped"); return };
    for &id in ids.iter().take(60) {
        let m = &lv.table.mobys[id];
        eprintln!("#{id} state {} pos {:?} group {} path {} giant {}", m.state, m.position, m.group, p::i32(&m.pvars, pb::pv::PATH), p::i32(&m.pvars, pb::pv::GIANT_ONLY));
    }
}

/// The group-3 pack of level 09 (#51–#56 near (284, 184, 36.3)) with Ratchet `d` units north of #53: every state the
/// biters pass through, and the hit records on Ratchet's moby after `n` ticks.
fn pack_run(d: f32, n: usize) -> Option<(Lv, Hero, Vec<std::collections::BTreeSet<u8>>)> {
    let (mut lv, _, ids) = biters(9, [284.6, 182.7 + d, 36.3])?;
    let hero = hero_at([284.6, 182.7 + d, 36.3]);
    let pack: Vec<MobyId> = ids.iter().copied().filter(|&i| lv.table.mobys[i].group == 3).collect();
    let mut seen = vec![std::collections::BTreeSet::new(); pack.len()];
    for _ in 0..n {
        lv.tick(&hero);
        for (k, &i) in pack.iter().enumerate() { seen[k].insert(lv.table.mobys[i].state); }
    }
    Some((lv, hero, seen))
}

/// Ratchet 6 units from the pack: they face him, charge and bite (a hit record on his moby: flags 1, damage 1, the
/// push 0.2 along the biter's heading) and stay on the ground.
#[test]
fn a_biter_pack_charges_and_bites_ratchet() {
    let Some((lv, _, seen)) = pack_run(6.0, 600) else { eprintln!("skipped: no extracted/"); return };
    eprintln!("states: {seen:?}");
    let hits = lv.hits_on(lv.hero_idx);
    eprintln!("hits on Ratchet: {}", hits.len());
    assert!(seen.iter().any(|s| s.contains(&pb::st::GRAZE)));
    assert!(seen.iter().any(|s| s.contains(&pb::st::FACE) && s.contains(&pb::st::CHARGE) && s.contains(&pb::st::BITE)));
    assert!(!hits.is_empty(), "a bite");
    for h in &hits {
        assert_eq!((h.flags, h.damage.to_f32()), (1, 1.0));
        let dir = h.dir.map(|x| x.to_f32());
        assert!(((dir[0] * dir[0] + dir[1] * dir[1]).sqrt() - 0.2).abs() < 1e-4, "{dir:?}");
        assert_eq!(lv.table.mobys[h.attacker.unwrap()].o_class, 193);
    }
}

#[test]
fn biter_pack_run_is_deterministic() {
    let Some((a, _, sa)) = pack_run(6.0, 300) else { eprintln!("skipped"); return };
    let (b, _, sb) = pack_run(6.0, 300).unwrap();
    assert_eq!(sa, sb);
    let pos = |lv: &Lv| lv.of_class(193).iter().map(|&i| lv.table.mobys[i].position.map(f32::to_bits)).collect::<Vec<_>>();
    assert_eq!(pos(&a), pos(&b));
    assert_eq!(a.hits_on(a.hero_idx).len(), b.hits_on(b.hero_idx).len());
}

/// A weaker hit (damage 0.5, the resolver's record) knocks a biter back (state 6, flash 0xfa, its cooldown 60) and
/// calls its pack (+0xbc = 1); it lands (9) and charges again (3). The wrench (damage 1) kills it: the death flight
/// (99, untargetable, no collision, flash 0x78), `SetDeathBits`' bolts and save bit, then the death explosion (class
/// sound 6, three spark pairs) and the delete.
#[test]
fn weapons_knock_back_and_kill_a_biter() {
    let Some((mut lv, hero, ids)) = biters(9, [284.6, 170.0, 36.3]) else { eprintln!("skipped: no extracted/"); return };
    let pack: Vec<MobyId> = ids.iter().copied().filter(|&i| lv.table.mobys[i].group == 3).collect();
    let (a, b) = (pack[0], pack[1]);
    for _ in 0..5 { lv.tick(&hero); }
    let h = lv.hero_idx;
    let mut t = wrench(h, [0.0, 1.0]);
    t.damage = rc_game::ps2v::Pf::f(0.5);
    lv.hit(&hero, a, &t);
    lv.tick(&hero);
    {
        let m = &lv.table.mobys[a];
        assert_eq!(m.state, pb::st::KNOCKED);
        assert_eq!(p::ff(&m.pvars, pb::pv::D), 0.5);
        assert!(p::i16(&m.pvars, pb::pv::COOLDOWN) > 50);
        assert!(pack.iter().all(|&i| lv.table.mobys[i].cmd == 1 || i == a || lv.table.mobys[i].state == pb::st::CHARGE), "the pack is called");
    }
    let mut states = vec![];
    for _ in 0..240 {
        lv.tick(&hero);
        let s = lv.table.mobys[a].state;
        if states.last() != Some(&s) { states.push(s); }
    }
    eprintln!("after the knock: {states:?}");
    assert!(states.starts_with(&[pb::st::KNOCKED, pb::st::RECOVER, pb::st::CHARGE]) || states.starts_with(&[pb::st::RECOVER, pb::st::CHARGE]));
    // The wrench kills b.
    let bolts0: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    lv.hit(&hero, b, &wrench(h, [0.0, 1.0]));
    lv.tick(&hero);
    {
        let m = &lv.table.mobys[b];
        assert_eq!(m.state, pb::st::DYING);
        assert!(m.mode & mode::TARGETABLE == 0 && !m.has_collision);
    }
    assert!(lv.svc.save.death.contains(&(9, lv.table.mobys[b].spawn_id)), "the death bit");
    let bolts: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    eprintln!("b4 {}: bolts {bolts0} -> {bolts}", lv.table.mobys[b].b4);
    if lv.table.mobys[b].b4 > 0 { assert!(bolts > bolts0, "SetDeathBits' bolts"); }
    let mut gone = false;
    for _ in 0..200 {
        lv.tick(&hero);
        if lv.table.mobys[b].state >= 0x80 { gone = true; break; }
    }
    assert!(gone, "the death flight ends");
    assert!(lv.svc.sounds.iter().any(|e| e.moby == b && e.index == 6), "the death explosion's class sound 6");
}

/// The Suck Cannon: 193's table is the shape `react::HELD7` on 09 and 15; its init stores its own sequence table in
/// the record (+0xd0 → `react::SEQS_193`: pulled 7, held 2); let go, the carried update returns it to grazing (1,
/// record state 0).
#[test]
fn the_suck_cannon_takes_a_biter() {
    let Some((mut lv, hero, ids)) = biters(9, [284.6, 170.0, 36.3]) else { eprintln!("skipped: no extracted/"); return };
    let a = ids.iter().copied().find(|&i| lv.table.mobys[i].group == 3).unwrap();
    {
        let mut w = lv.world(&hero);
        assert_eq!(react::table(&w, a), Some(react::Table::Held7));
        assert_eq!(react::record(&w, a), Some(pb::pv::SUCK));
        assert_eq!(rc_game::moby_update::creature::pi32(&w, a, pb::pv::SUCK + react::rec::SEQS), react::seq_table_id(react::SEQS_193));
        rc_game::moby_update::creature::set_pi16(&mut w, a, pb::pv::SUCK + react::rec::STATE, 5);
        assert_eq!(react::take(&mut w, a, [284.6, 172.0, 37.0, 1.0]), 2);
        assert_eq!(w.m(a).state, pb::st::HELD);
        assert_eq!(w.m(a).anim.seq_b, 7, "the pulled sequence of 193's table");
        rc_game::moby_update::creature::set_pi16(&mut w, a, pb::pv::SUCK + react::rec::STATE, 1);
        react::slot_let_go(&mut w, a);
    }
    let mut back = None;
    for k in 0..600 {
        lv.tick(&hero);
        if lv.table.mobys[a].state != pb::st::HELD { back = Some(k); break; }
    }
    assert!(back.is_some());
    assert_eq!(lv.table.mobys[a].state, pb::st::GRAZE);
    assert_eq!(p::i16(&lv.table.mobys[a].pvars, pb::pv::SUCK + react::rec::STATE), 0);
}

/// The Taunter's lure (+0x38) alerts it for `ticks(240)`: the range goes from 12 to 20.
#[test]
fn the_taunter_alerts_a_biter() {
    let Some((mut lv, hero, ids)) = biters(9, [284.6, 170.0, 36.3]) else { eprintln!("skipped: no extracted/"); return };
    let a = ids.iter().copied().find(|&i| lv.table.mobys[i].group == 3).unwrap();
    assert_eq!(p::ff(&lv.table.mobys[a].pvars, pb::pv::RANGE), 12.0);
    p::set_i32(&mut lv.table.mobys[a].pvars, pb::pv::LURE, 1);
    lv.tick(&hero);
    let m = &lv.table.mobys[a];
    assert_eq!((p::ff(&m.pvars, pb::pv::RANGE), p::i16(&m.pvars, pb::pv::ALERT), p::i32(&m.pvars, pb::pv::LURE)), (20.0, 239, 0));
}

/// Quartu's Giant Clank-sized pack (+0x228) stays hidden, untargetable and without collision while Ratchet is on foot
/// (the body 0x1413f4 is never 2 in the port yet: G-HERO-005); the others run. Some Quartu biters have a path.
#[test]
fn quartu_giant_clank_biters_stay_hidden_on_foot() {
    let Some(mut lv) = load(15) else { eprintln!("skipped: no extracted/"); return };
    let ids = lv.of_class(193);
    let giant: Vec<MobyId> = ids.iter().copied().filter(|&i| p::i32(&lv.table.mobys[i].pvars, pb::pv::GIANT_ONLY) != 0).collect();
    let pathed = ids.iter().filter(|&&i| p::i32(&lv.table.mobys[i].pvars, pb::pv::PATH) != -1).count();
    eprintln!("L15: {} biters, {} giant-only, {} with a path", ids.len(), giant.len(), pathed);
    assert!(!giant.is_empty() && pathed > 0);
    let g = lv.table.mobys[giant[0]].position;
    let hero = hero_at([g[0] + 3.0, g[1], g[2]]);
    lv.load_pass(&hero);
    for _ in 0..3 { lv.tick(&hero); }
    let m = &lv.table.mobys[giant[0]];
    assert!(m.mode & mode::HIDDEN != 0 && m.mode & mode::TARGETABLE == 0 && !m.has_collision && m.update_dist == 0xff);
    assert_eq!(m.state, 0, "never past its init");
}

/// Standing on ground surface 1 kills a biter (the death explosion, class sound 6, deleted).
#[test]
fn a_biter_on_the_deadly_floor_dies() {
    let Some((mut lv, hero, ids)) = biters(9, [284.6, 170.0, 36.3]) else { eprintln!("skipped: no extracted/"); return };
    // The nearest ground of surface 1 to the pack (a scan of the level's collision).
    let mut spot = None;
    {
        let w = lv.world(&hero);
        'scan: for r in 0..120 {
            for dx in -r..=r {
                for dy in [-r, r] {
                    for (x, y) in [(dx, dy), (dy, dx)] {
                        let p = [284.0 + x as f32, 184.0 + y as f32, 80.0, 1.0];
                        let g = rc_game::moby_update::creature::ground::ground(&w, p, 0.5, 0);
                        if g.hit && g.surface == 1 { spot = Some([p[0], p[1], g.z]); break 'scan; }
                    }
                }
            }
        }
    }
    let Some(s) = spot else { eprintln!("no surface 1 on level 09"); return };
    eprintln!("surface 1 at {s:?}");
    let a = ids.iter().copied().find(|&i| lv.table.mobys[i].group == 3).unwrap();
    let hero = hero_at([s[0] + 5.0, s[1], s[2]]);
    lv.table.mobys[a].position = [s[0], s[1], s[2], 1.0];
    lv.tick(&hero);
    assert!(lv.table.mobys[a].state >= 0x80, "deleted");
    assert!(lv.svc.sounds.iter().any(|e| e.moby == a && e.index == 6));
}

// ---------------------------------------------------------------------------------------------------
// U268: 252, the hover zappers of levels 08 / 14

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_zappers() {
    let Some(mut lv) = load(8) else { eprintln!("skipped"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero);
    lv.tick(&hero);
    for id in lv.of_class(252) {
        let m = &lv.table.mobys[id];
        eprintln!("#{id} state {} pos {:?} area {} home {:?} grounded {}", m.state, m.position, p::i32(&m.pvars, hz::pv::AREA), p::v4f(&m.pvars, hz::pv::HOME), p::i16(&m.pvars, hz::pv::GROUNDED));
    }
}

/// Level 08's zappers #214 / #215 (area path 64, home near (268, 196, 35.2)) with Ratchet at `at`: the per-tick states
/// of #214, the arcs drawn (quads per tick of its registrations), the level after. The charge sound ends after
/// `sound_len` ticks (the harness's sink keeps a slot alive until cleared).
fn zapper_run(at: [f32; 3], ticks: usize, sound_len: usize) -> Option<(Vec<u8>, Vec<usize>, Lv, Hero)> {
    let mut lv = load(8)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    let (mut states, mut arcs) = (Vec::new(), Vec::new());
    let mut charge_at = None;
    for t in 0..ticks {
        lv.tick(&hero);
        let s = lv.table.mobys[214].state;
        if s == hz::st::CHARGE && charge_at.is_none() { charge_at = Some(t); }
        if charge_at.is_some_and(|c| t >= c + sound_len) { lv.sink.slots.clear(); }
        states.push(s);
        let arc_row = units::PORTS.iter().position(|u| u.unit == "U268 252 arc").unwrap() as u16;
        let n = lv.svc.draw_callbacks.list1.iter().filter(|(cb, id)| *id == 214 && *cb == rc_game::moby_update::classes::draw_callbacks::Callback::UnitQuads(arc_row)).map(|_| units::fx_quads(&lv.table, &lv.svc, arc_row, 214).map_or(0, |q| q.quads.len())).sum();
        arcs.push(n);
    }
    Some((states, arcs, lv, hero))
}

fn runs(v: &[u8]) -> Vec<u8> { let mut r: Vec<u8> = Vec::new(); for &s in v { if r.last() != Some(&s) { r.push(s); } } r }

/// Ratchet near the zappers: #214 chases him, charges (class sound 0), zaps: the arc grows over keys 9..14 (six quads
/// a tick, FX 0xe, additive) and at key 14 his moby takes a hit (flags 1, damage 1, push 0.2 along its heading); then
/// it heads home. Its glow fades toward red while charging / zapping and is drawn on list 2 at joint 3.
#[test]
fn a_zapper_chases_charges_and_zaps_ratchet() {
    let Some((states, arcs, lv, _)) = zapper_run([270.0, 194.0, 35.2], 400, 40) else { eprintln!("skipped: no extracted/"); return };
    let r = runs(&states);
    eprintln!("states {r:?}; arc ticks {}", arcs.iter().filter(|&&n| n > 0).count());
    assert!(r.windows(3).any(|w| w == [hz::st::CHASE, hz::st::CHARGE, hz::st::ZAP]), "{r:?}");
    assert!(arcs.contains(&6));
    let hits: Vec<_> = lv.hits_on(lv.hero_idx).into_iter().filter(|h| h.attacker.is_some_and(|a| lv.table.mobys[a].o_class == 252)).collect();
    eprintln!("hits on Ratchet: {}", hits.len());
    assert!(!hits.is_empty());
    for h in &hits {
        assert_eq!((h.flags, h.damage.to_f32()), (1, 1.0));
        let d = h.dir.map(|x| x.to_f32());
        assert!(((d[0] * d[0] + d[1] * d[1]).sqrt() - 0.2).abs() < 1e-4);
    }
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 214 && e.index == 0), "the charge sound");
    let m = &lv.table.mobys[214];
    let glow_row = units::PORTS.iter().position(|u| u.unit == "U268 252 glow").unwrap() as u16;
    let q = units::glow_quads(&lv.table, &lv.svc, glow_row, 214);
    assert_eq!(q.len(), 1);
    assert_eq!((q[0].size, q[0].pull, q[0].rgba), (hz::GLOW_SIZE, hz::GLOW_PULL, (m.glow & 0xff_ffff) | 0x4000_0000));
    assert!(lv.svc.draw_callbacks.list2.iter().any(|(_, id)| *id == 214));
}

#[test]
fn zapper_run_is_deterministic() {
    let Some((a, aa, la, _)) = zapper_run([270.0, 194.0, 35.2], 240, 40) else { eprintln!("skipped"); return };
    let (b, ab, lb, _) = zapper_run([270.0, 194.0, 35.2], 240, 40).unwrap();
    assert_eq!((a, aa), (b, ab));
    assert_eq!(la.table.mobys[214].position.map(f32::to_bits), lb.table.mobys[214].position.map(f32::to_bits));
}

/// Without a target it bobs over home (lift 4 on the ground: home z + 4 ± 0.5) and drifts within 3 of home.
#[test]
fn an_idle_zapper_bobs_and_drifts_near_home() {
    let Some((states, _, lv, _)) = zapper_run([268.0, 226.0, 35.2], 600, 0) else { eprintln!("skipped: no extracted/"); return };
    let r = runs(&states);
    assert!(r.contains(&hz::st::DRIFT), "{r:?}");
    let m = &lv.table.mobys[214];
    let h = p::v4f(&m.pvars, hz::pv::HOME);
    let dz = m.position[2] - (h[2] + 4.0);
    let dxy = ((m.position[0] - h[0]).powi(2) + (m.position[1] - h[1]).powi(2)).sqrt();
    eprintln!("#214 z − (home + 4) = {dz}, xy from home {dxy}");
    assert!(dz.abs() < 0.8 && dxy < 4.0);
}

/// Any weapon hit the resolver passes kills it: `SetDeathBits` (the save bit and bolts), class sound 1, the beam
/// explosion (5 streaks), deleted.
#[test]
fn the_wrench_kills_a_zapper() {
    let Some(mut lv) = load(8) else { eprintln!("skipped: no extracted/"); return };
    let hero = hero_at([268.0, 226.0, 35.2]);
    lv.load_pass(&hero);
    lv.tick(&hero);
    let bolts0: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    let h = lv.hero_idx;
    lv.hit(&hero, 214, &wrench(h, [1.0, 0.0]));
    lv.tick(&hero);
    assert!(lv.table.mobys[214].state >= 0x80, "deleted");
    assert!(lv.svc.save.death.contains(&(8, lv.table.mobys[214].spawn_id)));
    let bolts: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    eprintln!("b4 {}: bolts {bolts0} -> {bolts}", lv.table.mobys[214].b4);
    if lv.table.mobys[214].b4 > 0 { assert!(bolts > bolts0); }
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 214 && e.index == 1));
    assert_eq!(lv.svc.fx.part_spawns.get(&15).copied().unwrap_or(0), 5);
}

/// The Suck Cannon: 252's table (held 5); let go (its own slot +0x0c) it goes home (3, lift 4, amplitude 0.5,
/// record state 0).
#[test]
fn the_suck_cannon_takes_a_zapper() {
    let Some(mut lv) = load(8) else { eprintln!("skipped: no extracted/"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero);
    lv.tick(&hero);
    let mut w = lv.world(&hero);
    assert_eq!(react::table(&w, 214), Some(react::Table::Hover252));
    rc_game::moby_update::creature::set_pi16(&mut w, 214, hz::pv::SUCK + react::rec::STATE, 5);
    assert_eq!(react::take(&mut w, 214, [268.0, 197.0, 36.0, 1.0]), 2);
    assert_eq!(w.m(214).state, hz::st::HELD);
    react::slot_let_go(&mut w, 214);
    assert_eq!(w.m(214).state, hz::st::HOME);
    assert_eq!((rc_game::moby_update::creature::pf(&w, 214, hz::pv::LIFT), rc_game::moby_update::creature::pi16(&w, 214, hz::pv::SUCK + react::rec::STATE)), (4.0, 0));
}

// ---------------------------------------------------------------------------------------------------
// U407: 63, the flying biters of level 13

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_flyers() {
    let Some(mut lv) = load(13) else { eprintln!("skipped"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero);
    for id in lv.of_class(63) {
        let m = &lv.table.mobys[id];
        eprintln!("#{id} pos {:?} group {} on_foot {} flight {} path {} cuboid {} range {} alert {}", m.position, m.group, p::i16(&m.pvars, fb::pv::ON_FOOT), p::i32(&m.pvars, fb::pv::FLIGHT), p::i32(&m.pvars, fb::pv::PATH), p::i32(&m.pvars, fb::pv::CUBOID), p::ff(&m.pvars, fb::pv::RANGE), p::ff(&m.pvars, fb::pv::ALERT_RANGE));
    }
}

/// Level 13's flyers of group 8 (#18–#23, hovering 6 over (508, 455, 300)) with Ratchet at `at`: the states each
/// passes through, and the level after.
fn flyer_run(at: [f32; 3], ticks: usize) -> Option<(Lv, Hero, Vec<Vec<u8>>)> {
    let mut lv = load(13)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    let ids: Vec<MobyId> = (18..=23).collect();
    let mut seen = vec![Vec::new(); ids.len()];
    for _ in 0..ticks {
        lv.tick(&hero);
        for (k, &i) in ids.iter().enumerate() {
            let s = lv.table.mobys[i].state;
            if seen[k].last() != Some(&s) { seen[k].push(s); }
        }
    }
    Some((lv, hero, seen))
}

/// No target: they hover and fly to random points near home (0xe ⇄ 0xf), above the ground, carrying the type-68
/// particle while drawn.
#[test]
fn flyers_hover_and_roam_near_home() {
    let Some((lv, _, seen)) = flyer_run([508.0, 400.0, 300.0], 600) else { eprintln!("skipped: no extracted/"); return };
    eprintln!("states {seen:?}");
    assert!(seen.iter().all(|s| s.iter().all(|&x| x == fb::st::HOVER || x == fb::st::FLY_TO)));
    assert!(seen.iter().any(|s| s.contains(&fb::st::FLY_TO)));
    for i in 18..=23usize {
        let m = &lv.table.mobys[i];
        let h = p::v4f(&m.pvars, fb::pv::HOME);
        assert!(((m.position[0] - h[0]).powi(2) + (m.position[1] - h[1]).powi(2)).sqrt() < 4.5 && m.position[2] > h[2] + 4.0, "#{i} {:?} home {h:?}", m.position);
    }
}

/// Ratchet below them: they land (0x11), charge (3) and bite (4): hit records on his moby (flags 1, damage 1).
#[test]
fn flyers_land_charge_and_bite_ratchet() {
    let Some((lv, _, seen)) = flyer_run([508.0, 452.0, 300.0], 900) else { eprintln!("skipped: no extracted/"); return };
    eprintln!("states {seen:?}");
    assert!(seen.iter().any(|s| s.windows(2).any(|w| w == [fb::st::LAND, fb::st::CHARGE])));
    assert!(seen.iter().any(|s| s.contains(&fb::st::BITE)));
    let hits: Vec<_> = lv.hits_on(lv.hero_idx).into_iter().filter(|h| h.attacker.is_some_and(|a| lv.table.mobys[a].o_class == 63)).collect();
    eprintln!("hits on Ratchet: {}", hits.len());
    assert!(!hits.is_empty() && hits.iter().all(|h| h.flags == 1 && h.damage.to_f32() == 1.0));
}

#[test]
fn flyer_run_is_deterministic() {
    let Some((a, _, sa)) = flyer_run([508.0, 452.0, 300.0], 400) else { eprintln!("skipped"); return };
    let (b, _, sb) = flyer_run([508.0, 452.0, 300.0], 400).unwrap();
    assert_eq!(sa, sb);
    for i in 18..=23usize { assert_eq!(a.table.mobys[i].position.map(f32::to_bits), b.table.mobys[i].position.map(f32::to_bits)); }
}

/// A weaker hit knocks a flyer back (6: flash 0xfa, cooldown 60, the particle off, the pack called); it lands and
/// charges (3). The wrench kills another: the death flight (99), `SetDeathBits` (save bit, bolts), then the death
/// explosion (class sound 4) and the delete.
#[test]
fn weapons_knock_back_and_kill_a_flyer() {
    let Some((mut lv, hero, _)) = flyer_run([508.0, 400.0, 300.0], 5) else { eprintln!("skipped: no extracted/"); return };
    let h = lv.hero_idx;
    let mut t = wrench(h, [0.0, 1.0]);
    t.damage = rc_game::ps2v::Pf::f(0.5);
    lv.hit(&hero, 18, &t);
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[18].state, fb::st::KNOCKED);
    assert_eq!(p::i32(&lv.table.mobys[18].pvars, fb::pv::PART), 0, "the particle off");
    assert!((19..=23).all(|i| lv.table.mobys[i].cmd == 1 || lv.table.mobys[i].state != fb::st::HOVER), "the pack is called");
    let mut states = vec![];
    for _ in 0..300 {
        lv.tick(&hero);
        let s = lv.table.mobys[18].state;
        if states.last() != Some(&s) { states.push(s); }
        if s >= 0x80 { break; }
    }
    eprintln!("after the knock: {states:?}");
    assert!(states.contains(&fb::st::CHARGE) || states.iter().any(|&s| s >= 0x80), "{states:?}");
    let bolts0: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    lv.hit(&hero, 20, &wrench(h, [0.0, 1.0]));
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[20].state, fb::st::DYING);
    assert!(lv.svc.save.death.contains(&(13, lv.table.mobys[20].spawn_id)));
    let bolts: usize = (13..=16).map(|c| lv.of_class(c).len()).sum();
    eprintln!("b4 {}: bolts {bolts0} -> {bolts}", lv.table.mobys[20].b4);
    if lv.table.mobys[20].b4 > 0 { assert!(bolts > bolts0); }
    let mut gone = false;
    for _ in 0..300 { lv.tick(&hero); if lv.table.mobys[20].state >= 0x80 { gone = true; break; } }
    assert!(gone);
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 20 && e.index == 4), "the death explosion's sound 4");
}

/// The Suck Cannon: 63's table is the level-01 critter shape; its own sequence table (pulled 7); let go, the carried
/// update returns it to hovering (0xe). The Taunter alerts it: range +0x200 (32).
#[test]
fn the_suck_cannon_and_the_taunter_act_on_a_flyer() {
    let Some((mut lv, hero, _)) = flyer_run([508.0, 400.0, 300.0], 5) else { eprintln!("skipped: no extracted/"); return };
    {
        let mut w = lv.world(&hero);
        assert_eq!(react::table(&w, 19), Some(react::Table::Critter));
        rc_game::moby_update::creature::set_pi16(&mut w, 19, fb::pv::SUCK + react::rec::STATE, 5);
        assert_eq!(react::take(&mut w, 19, [508.0, 402.0, 301.0, 1.0]), 2);
        assert_eq!((w.m(19).state, w.m(19).anim.seq_b), (fb::st::HELD, 7));
        rc_game::moby_update::creature::set_pi16(&mut w, 19, fb::pv::SUCK + react::rec::STATE, 1);
        react::slot_let_go(&mut w, 19);
    }
    let mut back = false;
    for _ in 0..600 { lv.tick(&hero); if lv.table.mobys[19].state != fb::st::HELD { back = true; break; } }
    assert!(back);
    assert_eq!(lv.table.mobys[19].state, fb::st::HOVER);
    p::set_i32(&mut lv.table.mobys[21].pvars, fb::pv::LURE, 1);
    lv.tick(&hero);
    assert_eq!(p::ff(&lv.table.mobys[21].pvars, fb::pv::RANGE), 32.0);
}

/// Knocked below z 5 (a Gemlik pit): `SetDeathBits` and deleted, no explosion.
#[test]
fn a_flyer_knocked_into_a_pit_dies() {
    let Some((mut lv, hero, _)) = flyer_run([508.0, 400.0, 300.0], 5) else { eprintln!("skipped: no extracted/"); return };
    lv.table.mobys[22].state = fb::st::KNOCKED;
    lv.table.mobys[22].position[2] = 4.0;
    lv.tick(&hero);
    assert!(lv.table.mobys[22].state >= 0x80);
    assert!(lv.svc.save.death.contains(&(13, lv.table.mobys[22].spawn_id)));
    assert!(!lv.svc.sounds.iter().any(|e| e.moby == 22 && e.index == 4), "no explosion");
}

// ---------------------------------------------------------------------------------------------------
// U300: 52, the buzz bombs of levels 09 / 16

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_buzz_bombs() {
    let Some(mut lv) = load(9) else { eprintln!("skipped"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero);
    for id in lv.of_class(52) {
        let m = &lv.table.mobys[id];
        eprintln!("#{id} pos {:?} group {} area {}", m.position, m.group, p::i32(&m.pvars, bb::pv::AREA));
    }
}

/// Level 09's group 12 (#24–#29 near (241, 272, 41)) with Ratchet at `at`: the states of #26, the level after.
fn buzz_run(at: [f32; 3], ticks: usize) -> Option<(Vec<u8>, Lv, Hero)> {
    let mut lv = load(9)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    // The load pass runs every update once (INIT → IDLE), so the state before the first tick is recorded too.
    let mut states = vec![lv.table.mobys[26].state];
    for _ in 0..ticks {
        lv.tick(&hero);
        let s = lv.table.mobys[26].state;
        if states.last() != Some(&s) { states.push(s); }
    }
    Some((states, lv, hero))
}

/// Ratchet in their area: they call the group, buzz (class sound 1, looping), chase, wind up and circle with the
/// fuse (180), blink, and blow up: the beam explosion's 2-unit damage sphere hits Ratchet (flags 0x810001, damage 1,
/// class 52), class sound 0, `SetDeathBits`, deleted.
#[test]
fn buzz_bombs_chase_ratchet_and_blow_up() {
    let Some((states, lv, _)) = buzz_run([241.0, 275.0, 41.0], 420) else { eprintln!("skipped: no extracted/"); return };
    eprintln!("#26 states {states:?}");
    assert!(states.starts_with(&[bb::st::IDLE, bb::st::CHASE, bb::st::WIND_UP, bb::st::CIRCLE]), "{states:?}");
    assert!(lv.table.mobys[26].state >= 0x80, "blown up");
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 26 && e.index == 1 && e.flags == 4), "the buzz");
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 26 && e.index == 0), "the blast's sound");
    assert!(lv.svc.save.death.contains(&(9, lv.table.mobys[26].spawn_id)));
    let hits: Vec<_> = lv.hits_on(lv.hero_idx).into_iter().filter(|h| h.flags == 0x81_0001 && h.h2a == 52).collect();
    eprintln!("blast hits on Ratchet: {}", hits.len());
    assert!(!hits.is_empty() && hits.iter().all(|h| h.damage.to_f32() == 1.0));
}

#[test]
fn buzz_run_is_deterministic() {
    let Some((a, la, _)) = buzz_run([241.0, 275.0, 41.0], 200) else { eprintln!("skipped"); return };
    let (b, lb, _) = buzz_run([241.0, 275.0, 41.0], 200).unwrap();
    assert_eq!(a, b);
    for i in 24..=29usize { assert_eq!(la.table.mobys[i].position.map(f32::to_bits), lb.table.mobys[i].position.map(f32::to_bits)); }
}

/// Idle, it hovers 0.5 over the ground; the wrench knocks it back (5; class sound 8, the group called) three times
/// (health 3) and the third kills it: the death flight (6), `SetDeathBits`, the death explosion (sound 0), deleted.
#[test]
fn the_wrench_knocks_back_and_kills_a_buzz_bomb() {
    let Some((_, mut lv, hero)) = buzz_run([241.0, 258.0, 41.0], 60) else { eprintln!("skipped: no extracted/"); return };
    let m = &lv.table.mobys[26];
    assert_eq!(m.state, bb::st::IDLE);
    {
        let w = lv.world(&hero);
        let g = rc_game::moby_update::creature::ground::ground(&w, w.m(26).position, 0.5, 0).z;
        assert!((w.m(26).position[2] - (g + 0.5)).abs() < 0.05, "hovers 0.5 up: {} vs {g}", w.m(26).position[2]);
    }
    let h = lv.hero_idx;
    let mut seen = vec![];
    for k in 0..3 {
        // Past the resolver's per-kind cooldown between the hits.
        for _ in 0..40 { lv.tick(&hero); }
        lv.hit(&hero, 26, &wrench(h, [0.0, 1.0]));
        lv.tick(&hero);
        seen.push(lv.table.mobys[26].state);
        eprintln!("hit {k}: state {} health {}", lv.table.mobys[26].state, p::ff(&lv.table.mobys[26].pvars, bb::pv::D));
    }
    assert_eq!(seen, vec![bb::st::KNOCKED, bb::st::KNOCKED, bb::st::DYING]);
    assert!(lv.svc.sounds.iter().filter(|e| e.moby == 26 && e.index == 8).count() == 3, "the hurt sound each hit");
    assert!(lv.svc.save.death.contains(&(9, lv.table.mobys[26].spawn_id)));
    let mut gone = false;
    for _ in 0..200 { lv.tick(&hero); if lv.table.mobys[26].state >= 0x80 { gone = true; break; } }
    assert!(gone);
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 26 && e.index == 0), "the death explosion's sound 0");
}

/// The Taunter's lure alerts it (`randf(180, 240)` ticks): its search reaches 24.
#[test]
fn the_taunter_alerts_a_buzz_bomb() {
    let Some((_, mut lv, hero)) = buzz_run([241.0, 258.0, 41.0], 10) else { eprintln!("skipped: no extracted/"); return };
    p::set_i32(&mut lv.table.mobys[27].pvars, bb::pv::LURE, 1);
    lv.tick(&hero);
    let a = p::i32(&lv.table.mobys[27].pvars, bb::pv::ALERT);
    assert!((179..=240).contains(&a), "{a}");
    // The range follows the alert on the ticks whose `randi(4)` is 0 (the shared rand stream: other classes' draws move
    // which ticks those are), so wait for one within the alert's 180..240 ticks.
    for _ in 0..60 {
        lv.tick(&hero);
        if p::ff(&lv.table.mobys[27].pvars, bb::pv::RANGE) == 24.0 { break; }
    }
    assert_eq!(p::ff(&lv.table.mobys[27].pvars, bb::pv::RANGE), 24.0);
}

// ---------------------------------------------------------------------------------------------------
// U521: 1445, the area stalkers of level 16

#[test]
#[ignore = "survey: prints, asserts nothing; run with --ignored --nocapture"]
fn survey_stalkers() {
    let Some(mut lv) = load(16) else { eprintln!("skipped"); return };
    let hero = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero);
    for id in lv.of_class(1445) {
        let m = &lv.table.mobys[id];
        eprintln!("#{id} pos {:?} group {} area {} offset {}", m.position, m.group, p::i32(&m.pvars, ast::pv::AREA), p::ff(&m.pvars, ast::pv::OFFSET));
    }
}

/// Level 16's stalkers #1327–#1332 (area 25, around (183, 264, 120)) with Ratchet at `at`: #1329's states, the level.
fn stalker_run(at: [f32; 3], ticks: usize) -> Option<(Vec<u8>, Lv, Hero)> {
    let mut lv = load(16)?;
    let hero = hero_at(at);
    lv.load_pass(&hero);
    // The load pass runs every update once (INIT → IDLE), so the state before the first tick is recorded too.
    let mut states = vec![lv.table.mobys[26].state];
    for _ in 0..ticks {
        lv.tick(&hero);
        let s = lv.table.mobys[1329].state;
        if states.last() != Some(&s) { states.push(s); }
    }
    Some((states, lv, hero))
}

/// Ratchet in their area: they walk to him and bite (the joint hit: flags 1, damage 1, class 1445).
#[test]
fn stalkers_walk_to_ratchet_and_bite() {
    let Some((states, lv, _)) = stalker_run([185.0, 264.0, 120.0], 400) else { eprintln!("skipped: no extracted/"); return };
    eprintln!("#1329 states {states:?}");
    assert!(states.windows(2).any(|w| w == [ast::st::WALK, ast::st::BITE]), "{states:?}");
    let hits: Vec<_> = lv.hits_on(lv.hero_idx).into_iter().filter(|h| h.attacker.is_some_and(|a| lv.table.mobys[a].o_class == 1445)).collect();
    eprintln!("bites on Ratchet: {}", hits.len());
    assert!(!hits.is_empty() && hits.iter().all(|h| h.flags == 1 && h.damage.to_f32() == 1.0));
}

#[test]
fn stalker_run_is_deterministic() {
    let Some((a, la, _)) = stalker_run([185.0, 264.0, 120.0], 200) else { eprintln!("skipped"); return };
    let (b, lb, _) = stalker_run([185.0, 264.0, 120.0], 200).unwrap();
    assert_eq!(a, b);
    for i in 1327..=1332usize { assert_eq!(la.table.mobys[i].position.map(f32::to_bits), lb.table.mobys[i].position.map(f32::to_bits)); }
}

/// Any weapon hit kills it (no health accounting): the death flight (8), then the death explosion (sound 6),
/// `SetDeathBits` (the save bit, bolts), deleted. The Suck Cannon: `Held7` with its own (193-equal) sequences.
#[test]
fn a_hit_kills_a_stalker_and_the_suck_cannon_takes_one() {
    let Some((_, mut lv, hero)) = stalker_run([183.0, 245.0, 120.0], 5) else { eprintln!("skipped: no extracted/"); return };
    let h = lv.hero_idx;
    let mut t = wrench(h, [0.0, 1.0]);
    t.damage = rc_game::ps2v::Pf::f(0.1);
    lv.hit(&hero, 1328, &t);
    lv.tick(&hero);
    assert_eq!(lv.table.mobys[1328].state, ast::st::DYING);
    assert!(lv.table.mobys[1328].mode & mode::TARGETABLE == 0);
    let mut gone = false;
    for _ in 0..200 { lv.tick(&hero); if lv.table.mobys[1328].state >= 0x80 { gone = true; break; } }
    assert!(gone);
    assert!(lv.svc.sounds.iter().any(|e| e.moby == 1328 && e.index == 6));
    assert!(lv.svc.save.death.contains(&(16, lv.table.mobys[1328].spawn_id)));
    let mut w = lv.world(&hero);
    assert_eq!(react::table(&w, 1330), Some(react::Table::Held7));
    rc_game::moby_update::creature::set_pi16(&mut w, 1330, ast::pv::SUCK + react::rec::STATE, 5);
    assert_eq!(react::take(&mut w, 1330, [183.0, 262.0, 121.0, 1.0]), 2);
    assert_eq!((w.m(1330).state, w.m(1330).anim.seq_b), (ast::st::HELD, 7));
}

// ---------------------------------------------------------------------------------------------------
// U426: 1271, level 13's wave gate

/// While any of its groups has a live member it waits; once they are gone (Ratchet inside its cuboid) it takes the
/// state +0x20 and the command byte +0x24 of its pvars.
#[test]
fn the_wave_gate_opens_when_its_groups_are_gone() {
    let Some(mut lv) = load(13) else { eprintln!("skipped: no extracted/"); return };
    let hero0 = hero_at([0.0, 0.0, -100.0]);
    lv.load_pass(&hero0);
    let g = lv.of_class(1271)[0];
    let pvw = |lv: &Lv, o: usize| p::i32(&lv.table.mobys[g].pvars, o);
    let groups: Vec<i32> = (0..8).map(|k| pvw(&lv, 4 * k)).take_while(|&x| x != -1).collect();
    let (st, cmd, cub) = (pvw(&lv, 0x20), pvw(&lv, 0x24), pvw(&lv, 0x28));
    eprintln!("gate #{g} at {:?}: groups {groups:?}, state {st}, cmd {cmd}, cuboid {cub}", lv.table.mobys[g].position);
    // Ratchet at the gate (inside its cuboid when it has one: the cuboid's centre).
    let at = if cub != -1 { let c = lv.svc.volumes.cuboids[cub as usize].matrix; [c[3][0], c[3][1], c[3][2]] } else { let q = lv.table.mobys[g].position; [q[0], q[1], q[2]] };
    let hero = hero_at(at);
    lv.tick(&hero);
    let before = (lv.table.mobys[g].state, lv.table.mobys[g].cmd);
    {
        let w = lv.world(&hero);
        assert!(groups.iter().any(|&gr| wg::alive(&w, gr, -1) > 0), "a live group");
    }
    // Every member of its groups gone.
    for &gr in &groups {
        let list: Vec<u16> = lv.svc.groups.lists[gr as usize].clone().unwrap_or_default();
        for e in list { let m = (e & 0x7fff) as usize; if m != g { lv.table.mobys[m].state = 0xfd; } }
    }
    lv.tick(&hero);
    let after = (lv.table.mobys[g].state, lv.table.mobys[g].cmd);
    eprintln!("state / cmd {before:?} -> {after:?}");
    if st != -1 { assert_eq!(after.0, st as u8); }
    if cmd != -1 { assert_eq!(after.1, cmd as u8); }
    assert_ne!(before, after);
}
