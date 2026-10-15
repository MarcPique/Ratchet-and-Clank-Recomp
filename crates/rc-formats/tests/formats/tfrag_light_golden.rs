//! Checks the assumptions `tfrag_light` makes about the retail data (docs/plan/tfrag_lighting.md).
//! Skipped when `extracted/` is absent.

use rc_formats::tfrag_light::{self, LightBank, NormalTable, VertexLight, BOOT_NORMAL_TABLE_VADDR};
use rc_formats::{level, tfrag, wad, Buf};
use std::path::PathBuf;

fn extracted() -> Option<PathBuf> {
    let root = rc_formats::test_data::root();
    root.join("toc.bin").exists().then_some(root)
}

#[test]
fn normal_table_boot_copy_matches_level01_overlay() {
    let Some(root) = extracted() else { return; };
    let boot = NormalTable::from_elf(&std::fs::read(root.join("boot/SCUS_971.99")).unwrap(), BOOT_NORMAL_TABLE_VADDR).unwrap();
    // level01's LightTfrags (0x2a8e40) DMAs from 0x166500, in the overlay's own copy of the table.
    let sections = rc_formats::font::parse_overlay_sections(&std::fs::read(root.join("levels/01/overlay.bin")).unwrap()).unwrap();
    let bytes = rc_formats::font::read_overlay(&sections, 0x0016_6500, 256 * 8).expect("normal table in the overlay");
    let ov = NormalTable(std::array::from_fn(|i| std::array::from_fn(|k| u32::from_le_bytes(bytes[8 * i + 4 * k..][..4].try_into().unwrap()))));
    assert_eq!(boot, ov);
    assert_eq!(boot.0[0], [1.0f32.to_bits(), 0]);
    assert_eq!(boot.0[64][1], 1.0f32.to_bits());
}

#[test]
fn light_records_match_tfrag_layout_in_every_level() {
    let Some(root) = extracted() else { return; };
    let table = NormalTable::from_elf(&std::fs::read(root.join("boot/SCUS_971.99")).unwrap(), BOOT_NORMAL_TABLE_VADDR).unwrap();
    for i in 0..19 {
        let dir = root.join(format!("levels/{i:02}"));
        let gameplay = wad::decompress(&std::fs::read(dir.join("gameplay_ntsc.bin")).unwrap()).unwrap();
        let bank: LightBank = tfrag_light::parse_light_bank(&gameplay).unwrap();
        assert!(bank.count >= 1 && bank.count <= 12, "level {i}: {} lights", bank.count);
        let core_data = wad::decompress(&std::fs::read(dir.join("core_data.bin")).unwrap()).unwrap();
        let core = level::parse_level_core(&std::fs::read(dir.join("core_index.bin")).unwrap(), core_data.len()).unwrap();
        let block = tfrag::tfrag_block(&core, &core_data).unwrap();
        let table_offset = Buf(block).i32(0).unwrap() as usize;
        let tfrags = tfrag::parse_tfrags(block).unwrap();
        let (mut sum, mut n) = ([0u64; 3], 0u64);
        for (ti, t) in tfrags.iter().enumerate() {
            let h = t.header;
            // `lb 0x3c` / `lb 0x29` are loop counters: must be positive as signed bytes.
            assert!(h.vert_count > 0 && h.vert_count < 128 && h.rgba_size < 128, "level {i} tfrag {ti}");
            assert!(t.rgba.len() >= h.vert_count as usize && t.lights.len() == h.vert_count as usize);
            assert_eq!(h.dir_lights_one, 0xff, "level {i} tfrag {ti}: per-vertex light select expected");
            assert_eq!(h.point_lights, 0xffff);
            let base = table_offset.wrapping_add(h.data as isize as usize);
            // The light-block origin quadword is the tfrag origin the point-light pass adds.
            let q: [i32; 4] = Buf(block).pod(base + h.light_ofs as usize, "origin").unwrap();
            assert_eq!(q[..3], t.origin[..3], "level {i} tfrag {ti}: light origin != tfrag origin");
            for (vi, rec) in t.lights.iter().enumerate() {
                let l = VertexLight::from_record(rec);
                // +0 is the byte offset of position `vi` in the tfrag data (used by the point-light pass).
                let p: [i16; 3] = Buf(block).pod(base + l.pos_ofs as usize, "position").unwrap();
                let e = t.positions[vi];
                assert_eq!(p, [e.x, e.y, e.z], "level {i} tfrag {ti} vertex {vi}: pos_ofs does not address position {vi}");
                assert!(l.color & 0x8000 != 0, "base colour alpha bit");
                for s in [l.select & 0xf, if l.select & 0xff00 != 0 { l.select >> 4 & 0xf } else { 0 }] {
                    assert!((s as usize) < bank.count || s == 0xf, "level {i} tfrag {ti}: select {:#x}", l.select);
                }
            }
            let lit = tfrag_light::light_tfrag(t, &bank, &table, None);
            for c in &lit[..h.vert_count as usize] {
                assert_eq!(c.a, 0x80);
                sum[0] += c.r as u64; sum[1] += c.g as u64; sum[2] += c.b as u64; n += 1;
            }
        }
        eprintln!("level {i:02}: {} light sets, mean lit rgb {:.1?}", bank.count, sum.map(|s| s as f64 / n as f64));
    }
}
