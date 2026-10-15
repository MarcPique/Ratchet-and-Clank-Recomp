//! The ported classes whose particle types were ported for them (G-PRT-001 / G-PRT-007, docs/plan/particles.md "Update
//! types, 2026-09-29"): their records now live their game lives in `UpdateParts` instead of dying unported on their
//! first update. Run by `cargo xtask test-job particles`. Skipped when `extracted/` is absent. The level harness is
//! `creature_classes`'s; `UpdateParts` runs after each moby pass, as in the game state update 0x2a4080.

use crate::creature_classes::{hero_at, load, Lv};
use rc_game::hero::Hero;
use rc_game::moby_update::classes::units::flying_biter as fb;
use rc_game::moby_update::services::pvar as p;
use rc_game::particles::{rec, type68};

fn tick(lv: &mut Lv, hero: &Hero) {
    lv.tick(hero);
    lv.particles.update_parts(&mut lv.rng);
}

/// Level 13's flying biters (group 8, #18–#23) carry the type-68 ribbon while drawn: every flyer's record lives
/// (none killed unported), sits on the flyer's joint list 0 with its tail 0.6–1.0 behind, flickers in width, draws as
/// the additive textured ribbon (kind 3); a knocked flyer lets it go (killed by its owner, not by the type).
#[test]
fn flying_biter_ribbons_live() {
    let Some(mut lv) = load(13) else { eprintln!("skipped: no extracted/"); return };
    let hero = hero_at([508.0, 400.0, 300.0]);
    lv.load_pass(&hero);
    let mut widths = std::collections::BTreeSet::new();
    let mut held = 0;
    for _ in 0..240 {
        tick(&mut lv, &hero);
        for i in 18..=23usize {
            let h = p::i32(&lv.table.mobys[i].pvars, fb::pv::PART);
            if h == 0 { continue; }
            held += 1;
            let j = lv.world(&hero).joint_point(i, 0);
            let r = &lv.particles.pool.recs[(h - 1) as usize];
            assert_eq!((r[0], r[1] & 0x83, r[3]), (type68::TYPE, 3, 0x48), "#{i}: a live additive ribbon");
            assert_eq!(type68::joint_of(r), Some((i, 0)));
            let e1 = rec::v3(r, 0x10);
            assert!((0..3).all(|k| (e1[k] - j[k]).abs() < 1e-4), "#{i}: end 1 {e1:?} on the joint {j:?}");
            let e2 = rec::v3(r, 0x20);
            let l = ((e2[0] - e1[0]).powi(2) + (e2[1] - e1[1]).powi(2) + (e2[2] - e1[2]).powi(2)).sqrt();
            assert!((0.59..=1.01).contains(&l), "#{i}: tail {l}");
            widths.insert(rec::ff(r, 0x1c).to_bits());
        }
    }
    assert!(held > 240, "the flyers held their ribbons ({held})");
    assert!(widths.len() > 50, "the width flickers");
    assert_eq!(lv.particles.stats.unported_kills[type68::TYPE as usize], 0);
    assert!(lv.particles.live_by_type()[type68::TYPE as usize] >= 1);
}

/// The flyer driver's spark flag (+0x120 bit 0: the driver family's other classes, G-CLS-015; no Novalis Blarg flyer
/// has it, so it is set here on each 660): two type-10 sparks per engine joint every 7th tick; they live and fade (25
/// updates each) instead of dying unported, drifting away from the joint.
#[test]
fn flyer_driver_sparks_live() {
    let Some(mut lv) = load(1) else { eprintln!("skipped: no extracted/"); return };
    let flyers = lv.of_class(660);
    assert!(flyers.iter().all(|&i| lv.table.mobys[i].pvars[0x120] & 1 == 0), "no Novalis flyer has the flag");
    for &i in &flyers {
        let f = &mut lv.table.mobys[i].pvars[0x120];
        if *f & 0x1e != 0 { *f |= 1; }
    }
    let hero = hero_at([0.0, 0.0, 0.0]);
    lv.load_pass(&hero);
    let mut peak = 0;
    for _ in 0..120 {
        tick(&mut lv, &hero);
        peak = peak.max(lv.particles.live_by_type()[10]);
    }
    let made = lv.svc.fx.part_spawns.get(&10).copied().unwrap_or(0);
    eprintln!("flyers {flyers:?}: type-10 sparks made {made}, peak alive {peak}");
    assert!(made > 0 && peak > 0);
    assert!(peak as u64 <= made);
    assert_eq!(lv.particles.stats.unported_kills[10], 0);
}
