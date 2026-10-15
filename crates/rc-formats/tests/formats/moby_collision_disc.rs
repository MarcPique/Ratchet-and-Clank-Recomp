//! Every moby class collision blob of all 19 levels parses (`rc_formats::moby_collision`), with the
//! per-kind counts pinned. Skipped when `extracted/` is absent.

use rc_formats::{level, moby_collision};

#[test]
fn every_level_moby_collision_blob_parses() {
    let root = rc_formats::test_data::root().join("levels");
    let (mut blobs, mut kinds, mut verts, mut faces, mut posed) = (0usize, [0usize; 5], 0usize, 0usize, 0usize);
    let mut levels = 0;
    for lv in 0..19 {
        let dir = root.join(format!("{lv:02}"));
        let (Ok(data), Ok(idx)) = (std::fs::read(dir.join("core_data.bin")), std::fs::read(dir.join("core_index.bin"))) else { continue };
        let data = rc_formats::wad::decompress(&data).unwrap();
        levels += 1;
        let core = level::parse_level_core(&idx, data.len()).unwrap();
        for (oc, c) in moby_collision::parse_level(&core, &data).unwrap_or_else(|e| panic!("level {lv}: {e}")) {
            blobs += 1;
            verts += c.vertices.len();
            faces += c.faces.len();
            if c.joint_counts != [0, 0] { posed += 1; }
            for p in &c.prims {
                let k = p.kind();
                assert!((1..=4).contains(&k), "level {lv} class {oc}: kind {k}");
                kinds[k as usize] += 1;
                assert_eq!(p.byte1(), 4, "level {lv} class {oc}: byte 1");
                if k == 2 || k == 4 {
                    // Every mask bit a joint primitive carries selects a non-zero joint count above its joints.
                    for bit in 0..2 {
                        if p.mask() & (1 << bit) == 0 { continue; }
                        let n = c.joint_counts[bit] as i32;
                        let js = if k == 2 { vec![p.word4()] } else { p.joints().iter().map(|&j| j as i32).collect() };
                        assert!(js.iter().all(|&j| (0..n).contains(&j)), "level {lv} class {oc}: joints {js:?} of {n}");
                    }
                }
            }
        }
    }
    if levels == 0 { eprintln!("skipped: no extracted/"); return; }
    eprintln!("{levels} levels: {blobs} blobs ({posed} posed), kinds 1-4 {:?}, {verts} vertices, {faces} faces", &kinds[1..]);
    if levels == 19 {
        assert_eq!(blobs, 1064);
        assert_eq!(kinds[1..], [464, 31, 350, 302]);
        assert_eq!((verts, faces, posed), (23369, 38359, 104));
    }
}
