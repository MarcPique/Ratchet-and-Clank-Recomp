//! Every moby class shadow block of all 19 levels parses (`rc_formats::moby_shadow`), with the record counts and
//! the invariants the shadow system relies on pinned (docs/plan/shadows.md §2). Skipped when `extracted/` is absent.

use rc_formats::moby_shadow::{self, ShadowPrim};
use std::collections::BTreeSet;

#[test]
fn every_level_shadow_block_parses() {
    let (mut entries, mut spheres, mut capsules, mut levels) = (0usize, 0usize, 0usize, 0usize);
    let mut classes = BTreeSet::new();
    let mut seg_counts = BTreeSet::new();
    let mut most = 0usize;
    for lv in 0..19u32 {
        let Some(c) = rc_formats::test_data::core(lv) else { continue };
        levels += 1;
        for (oc, joints, blk) in moby_shadow::parse_level(&c.core, &c.data).unwrap_or_else(|e| panic!("level {lv}: {e}")) {
            entries += 1;
            classes.insert(oc);
            most = most.max(blk.prims.len());
            assert!(!blk.prims.is_empty(), "level {lv} class {oc}: empty block");
            for p in &blk.prims {
                match p {
                    ShadowPrim::Sphere { .. } => spheres += 1,
                    ShadowPrim::Capsule { .. } => capsules += 1,
                }
                for j in p.joints() { assert!(j < joints as u32, "level {lv} class {oc}: joint {j} of {joints}"); }
                for s in p.segments() {
                    assert!(s == -10 || s == 0 || (4..=16).contains(&s), "level {lv} class {oc}: segments {s}");
                    seg_counts.insert(s);
                }
                // A sphere outline needs at least one point (the game loops `segments` times, never 0).
                if let ShadowPrim::Sphere { segments, .. } = p { assert!(*segments > 0, "level {lv} class {oc}: sphere segments"); }
            }
        }
    }
    if levels == 0 { eprintln!("skipped: no extracted/"); return; }
    eprintln!("{levels} levels: {entries} class entries ({} classes), {capsules} capsules, {spheres} spheres, at most {most} records, counts {seg_counts:?}", classes.len());
    if levels == 19 {
        assert_eq!((entries, classes.len()), (154, 79));
        assert_eq!((capsules, spheres), (1668, 45));
        assert_eq!(most, 21, "Ratchet");
    }
    // Novalis casters (docs/plan/shadows.md §2): Ratchet, the thin-capsule class 10, the trooper, amoeboids, critters,
    // 634, the talking NPC and 811; no crate, bolt or pickup class.
    if let Some(c) = rc_formats::test_data::core(1) {
        let casters: Vec<i32> = moby_shadow::parse_level(&c.core, &c.data).unwrap().into_iter().map(|(oc, _, _)| oc).collect();
        let want = [0, 10, 459, 572, 577, 634, 774, 811, 865, 866];
        for oc in want { assert!(casters.contains(&oc), "class {oc} casts on Novalis: {casters:?}"); }
        for oc in [500, 501, 502, 505, 511] { assert!(!casters.contains(&oc), "crate class {oc} has no shadow block"); }
    }
}
