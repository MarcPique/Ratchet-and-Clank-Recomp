//! Survey of the classes a Bomb Glove explosion creates on Novalis (`cargo xtask test-job --test classes --filter explosion_survey::
//! --ignored --nocapture`): class header scale, mode bits, packet counts, textures, sequences. Skipped without
//! `extracted/`.

#[test]
#[ignore]
fn explosion_classes_survey() {
    let Some(data) = rc_formats::test_data::core_data(1) else { return };
    let dir = rc_formats::test_data::root().join("levels/01");
    let Ok(idx) = std::fs::read(dir.join("core_index.bin")) else { return };
    let core = rc_formats::level::parse_level_core(&idx, data.len()).unwrap();
    let classes = rc_formats::moby::parse_level_mobys(&core, &data).unwrap();
    for oc in [121, 122, 1192, 0x70, 639, 1747, 1748, 1749] {
        let Some(c) = classes.iter().find(|c| c.o_class == oc) else { println!("class {oc}: not on Novalis"); continue };
        let h = &c.class.header;
        let tex: std::collections::BTreeSet<i32> = c.class.high_lod.iter().flat_map(|s| s.triangles.iter().map(|t| t.texture)).collect();
        let tris: usize = c.class.high_lod.iter().map(|s| s.triangles.len()).sum();
        let alphas: std::collections::BTreeSet<u8> = c.class.high_lod.iter().flat_map(|s| s.vertices.iter().map(|v| v.skin.count)).collect();
        println!(
            "class {oc}: scale {} mode bits {:#x} (bits2 {:#x}) high {} low {} metal {} joints {} seqs {} lod_trans {} bsphere {:?} glow {:#x}; {tris} tris, textures {tex:?}, table {:?}, skin counts {alphas:?}",
            h.scale, h.mode_bits, h.mode_bits2, h.high_lod_count, h.low_lod_count, h.metal_count, h.joint_count, h.sequence_count, h.lod_trans, h.bsphere, h.glow_rgba,
            tex.iter().map(|&t| if t >= 0 { c.texture_table_index(t).map(|i| i as i64).unwrap_or(-9) } else { t as i64 }).collect::<Vec<_>>()
        );
    }
}
