//! End-to-end check of the tfrag-lighting truth test on synthetic EE images built from the real
//! level data (no emulator needed). Skipped when `extracted/` is absent.

use rc_trace::ee::{EeImage, Savestate, P2S_EE_MEMORY, P2S_VERSION};
use rc_trace::tfrag_light_cmp::{self as tlc, LevelInputs};
use rc_trace::zip::{self, METHOD_STORE, METHOD_ZSTD};

fn level(n: u32) -> Option<LevelInputs> {
    let root = rc_trace::default_extracted();
    root.join("toc.bin").exists().then(|| LevelInputs::load(&root, n).unwrap())
}

fn run(img: &EeImage, lvl: &LevelInputs) -> (tlc::Location, tlc::Report) {
    let loc = tlc::locate(img, lvl).unwrap();
    let bank = tlc::locate_bank(img, lvl).expect("bank");
    assert!(bank.equals_disc);
    let r = tlc::compare(img, lvl, &loc, &lvl.bank, Some(&bank.points)).unwrap();
    (loc, r)
}

#[test]
fn locator_and_compare_on_synthetic_novalis() {
    let Some(lvl) = level(1) else { return };
    let base = 0x0053_7c40; // arbitrary, not the game's
    let block = base + lvl.block_offset as u32;

    // 1. Our own lit bytes in RAM: everything must match.
    let img = tlc::synthesize(&lvl, base, true);
    let (loc, r) = run(&img, &lvl);
    assert_eq!(loc.table, block + lvl.table_offset as u32);
    assert_eq!(loc.headers_matching, lvl.tfrags.len());
    assert_eq!(loc.relocated, lvl.tfrags.len());
    assert_eq!(loc.core_ptr_check.map(|c| c.3), Some(true));
    assert!(r.all_equal(), "{} of {} vertices differ", r.diffs.len(), r.vertices);
    assert_eq!(r.tfrags_equal, lvl.tfrags.len());
    assert!(r.blended.vertices > 0 && r.single.vertices > 0);
    eprintln!("lit: {} tfrags, {} vertices ({} blended), all equal", r.tfrags, r.vertices, r.blended.vertices);

    // 2. One corrupted byte: exactly one mismatch, at the right place, off by the right amount.
    let mut img2 = EeImage::new(img.ram.clone(), "corrupt");
    let (ti, vi) = (lvl.tfrags.len() / 2, 3);
    let addr = loc.data[ti] + lvl.tfrags[ti].header.rgba_ofs as u32 + 4 * vi as u32 + 1;
    img2.ram[addr as usize] = img2.ram[addr as usize].wrapping_sub(1);
    let (_, r2) = run(&img2, &lvl);
    assert_eq!(r2.diffs.len(), 1);
    assert_eq!((r2.diffs[0].tfrag, r2.diffs[0].vertex), (ti, vi));
    assert_eq!(r2.hist[1][8 + 1], 1); // ours - ram = +1 on green

    // 3. The disc placeholder bytes (the pass "did not run"): mismatches reported, RAM == disc everywhere.
    let unlit = tlc::synthesize(&lvl, base, false);
    let (_, r3) = run(&unlit, &lvl);
    assert!(!r3.all_equal());
    assert_eq!(r3.ram_equals_disc, r3.vertices);
    assert!(r3.vertices_equal * 10 < r3.vertices, "lighting should change most vertices ({} of {} unchanged)", r3.vertices_equal, r3.vertices);
    eprintln!("unlit: {} of {} vertices equal ({} bytes of {})", r3.vertices_equal, r3.vertices, r3.bytes_equal, r3.bytes);

    // 4. Without the level global (zeroed), the search alone still finds the table.
    let mut img4 = EeImage::new(img.ram.clone(), "no global");
    img4.ram[0x17_4294..0x17_4298].fill(0);
    let loc4 = tlc::locate(&img4, &lvl).unwrap();
    assert_eq!(loc4.table, loc.table);
    assert_eq!(loc4.core_ptr_check.map(|c| c.3), Some(false));
}

/// The same lit image packed as a PCSX2 v2.8.2-style savestate (stored version id, zstd RAM).
#[test]
fn compare_through_a_savestate_file() {
    let Some(lvl) = level(1) else { return };
    let img = tlc::synthesize(&lvl, 0x0040_0000, true);
    let mut ver = (0x9a59u32 << 16).to_le_bytes().to_vec();
    ver.extend_from_slice(&[0; 32]);
    let z = zip::build(&[(P2S_VERSION, &ver, METHOD_STORE), (P2S_EE_MEMORY, &img.ram, METHOD_ZSTD)]);
    let dir = std::env::temp_dir().join(format!("rc-trace-synth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A savestate is a zip; Savestate::open does not look at the name (synthetic, in the temp dir).
    let p = dir.join("synthetic_savestate.zip");
    std::fs::write(&p, z).unwrap();
    let ee = Savestate::open(&p).unwrap().ee().unwrap();
    assert!(ee.ram == img.ram);
    let (_, r) = run(&ee, &lvl);
    assert!(r.all_equal());
    std::fs::remove_dir_all(&dir).ok();
}
