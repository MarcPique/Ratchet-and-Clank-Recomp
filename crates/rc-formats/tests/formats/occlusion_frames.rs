//! Per-frame occlusion masks on Novalis (level 01) as the engine builds them (docs/plan/occlusion_culling.md
//! §2): 200 seeded camera positions, half uniform in the tfrag bounds and half jittered inside random grid
//! cells, fed through one `OcclusionState` in sequence (so the neighbour and previous-mask paths run too).
//! Skipped when `extracted/` is not present (the data never ships with the repo).

use rc_formats::{gameplay, level, occlusion::*, tfrag, tie, wad};

/// Visible tfrags per populated cell on level 01 (min, max).
const NOVALIS_TFRAGS_PER_CELL: (usize, usize) = (5, 522);

#[test]
fn novalis_frame_masks() {
    let dir = rc_formats::test_data::root().join("levels/01");
    if !dir.join("core_data.bin").exists() { eprintln!("skipped: no extracted/levels/01"); return; }
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap();
    let data = wad::decompress(&read("core_data.bin")).unwrap();
    let core = level::parse_level_core(&read("core_index.bin"), data.len()).unwrap();
    let grid = parse_occlusion(&core, &data).unwrap();
    let gp = wad::decompress(&read("gameplay_ntsc.bin")).unwrap();
    let maps = parse_gameplay_occlusion_mappings(&gp).unwrap().expect("Novalis has occlusion mappings");
    let tfrags = tfrag::parse_level_tfrags(&core, &data).unwrap();
    let heads: Vec<_> = tfrags.iter().map(|t| t.header).collect();
    let ties: Vec<i32> = tie::parse_tie_instances(&gp).unwrap().iter().map(|t| t.occlusion_index).collect();
    let mobys = gameplay::parse_moby_instances(&gp).unwrap();
    let lo = resolve_level_occlusion(Some(&maps), &heads, &ties, &mobys);

    // Tfrag bounds from the bounding spheres (raw units / 1024), as the engine frames the level.
    let (mut lo_b, mut hi_b) = ([f32::MAX; 3], [f32::MIN; 3]);
    for h in &heads {
        let s = h.bsphere.map(|v| v / 1024.0);
        for k in 0..3 { lo_b[k] = lo_b[k].min(s[k] - s[3]); hi_b[k] = hi_b[k].max(s[k] + s[3]); }
    }

    let mut seed = 0x9e37_79b9u32;
    let mut rnd = move || { seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5; (seed >> 8) as f32 / (1u32 << 24) as f32 };
    let mut state = OcclusionState::default();
    let (mut hits, mut counts) = (0usize, (usize::MAX, 0usize));
    for i in 0..200 {
        let cam = if i % 2 == 0 {
            std::array::from_fn(|k| lo_b[k] + rnd() * (hi_b[k] - lo_b[k]))
        } else {
            let c = grid.cells[(rnd() * grid.cells.len() as f32) as usize % grid.cells.len()];
            [c.x, c.y, c.z].map(|v| v as f32 * CELL_SIZE + 0.01 + rnd() * (CELL_SIZE - 0.02))
        };
        let mask = *state.update(Some(&grid), OcclusionMode::Active, OcclusionFallback::Neighbours, false, cam);
        assert!(mask[0x7f] & 0x80 != 0, "camera {cam:?}: bit 1023 not set");
        assert!(OcclBits::ALWAYS.visible(&mask));
        if let Some(cell) = grid.cell_for(cam) {
            hits += 1;
            assert_eq!(mask, grid.frame_mask(cell), "camera {cam:?}: in-grid mask differs from the cell's");
            assert_eq!(state.previous, PreviousMask::Grid(cell.mask));
            let n = lo.visible_tfrags(&mask).count();
            assert!((NOVALIS_TFRAGS_PER_CELL.0..=NOVALIS_TFRAGS_PER_CELL.1).contains(&n), "camera {cam:?}: {n} visible tfrags");
            counts = (counts.0.min(n), counts.1.max(n));
        }
    }
    assert!(hits >= 100, "only {hits} cameras inside the grid");
    eprintln!("occlusion frames (Novalis): 200 cameras, {hits} inside the grid, visible tfrags {}..={}", counts.0, counts.1);
}
