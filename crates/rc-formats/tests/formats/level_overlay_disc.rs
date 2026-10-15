//! `level_overlay` on the 19 level overlays: the section layout and class tables, and the relocator against
//! copies known from the Ghidra cluster table (`tools/ghidra/names/clusters.tsv`). Skipped without `extracted/`.

use rc_formats::level_overlay::{LevelOverlay, Relocation};

fn overlay(level: u32) -> Option<LevelOverlay> {
    let b = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")).ok()?;
    Some(LevelOverlay::parse(&b).unwrap())
}

#[test]
fn every_overlay_has_the_seven_sections_and_a_class_table() {
    let Some(_) = overlay(1) else { eprintln!("skipped: no extracted/"); return };
    for level in 0..19 {
        let ov = overlay(level).unwrap();
        assert_eq!(ov.sections.len(), 7, "level {level:02}");
        assert_eq!(ov.sections[0].dest, 0x15ef00, "level {level:02} .lit");
        assert_eq!(ov.sections[1].kind, 8, "level {level:02} .bss");
        let vt = ov.vtbl();
        assert!(vt.len() >= 90, "level {level:02}: {} classes", vt.len());
        // The bolt classes are on every level, with one update function in `.text`.
        let bolts: Vec<u32> = vt.iter().filter(|e| (13..=16).contains(&e.o_class)).map(|e| e.update).collect();
        assert_eq!(bolts.len(), 4, "level {level:02}");
        assert!(bolts.iter().all(|&a| a == bolts[0] && ov.code(a, 1).is_some()), "level {level:02}");
    }
}

#[test]
fn relocation_finds_level13_copies_of_level01_code_and_data() {
    let (Some(l01), Some(l13)) = (overlay(1), overlay(13)) else { eprintln!("skipped: no extracted/"); return };
    let r = Relocation::new(&l01, &l13);
    assert!(!r.is_identity());
    // (level01, level13) from the cluster table; the bomb 0x2c3300 is a hash miss there (a `move` of a `lui`).
    for (a, b) in [(0x2bb758u32, 0x2b2038u32), (0x30cd18, 0x30c9e0), (0x3098f0, 0x3055a0), (0x2ea178, 0x2f0fb8), (0x2c3300, 0x2c61b8), (0x300de0, 0x2fb830)] {
        assert_eq!(r.func(a), Some(b), "function {a:#x}");
        assert!(r.same_code(a, b));
    }
    // Novalis-only code has no copy.
    assert_eq!(r.func(0x2f4428), None, "Blarg flyer update");
    // The frame's corner joint-list ids (`fun_0023a318`'s `lui`/`addiu` pair).
    assert_eq!(r.data(0x161fe0), Some(0x161e08));
    // Identity on the reference itself.
    let same = Relocation::new(&l01, &l01);
    assert!(same.is_identity());
    assert_eq!(same.data(0x1b2a08), Some(0x1b2a08));
}
