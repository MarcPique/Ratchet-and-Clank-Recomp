//! Cache life cycle on a synthetic data folder (no disc bytes): lazy build, once per process, stale stamps, corrupt
//! lumps, an unwritable cache, and `prepare`'s keep-or-rebuild step.

use rc_data::cache::{self, Cache, Ensured, LumpState, Stamp, Status, TRAILER_LEN};
use rc_data::{Lumps, Stats};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A WAD stream: literal `a b c d` (`a` = `first`), then a little match that repeats `d` 7 more times.
fn wad(first: u8) -> Vec<u8> {
    let mut v = b"WAD\0\0\0\0TEST\0\0\0\0\0".to_vec();
    v.extend_from_slice(&[0x01, first, b'b', b'c', b'd', 0xC0, 0x00]);
    let size = v.len() as u32;
    v[3..7].copy_from_slice(&size.to_le_bytes());
    v
}

fn expected(first: u8) -> Vec<u8> {
    let mut v = vec![first, b'b', b'c'];
    v.extend([b'd'; 8]);
    v
}

/// A fresh data folder with level 01 (core data, gameplay, HUD bank 0) and an `extract-info.json`.
fn data_folder(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rc-data-lifecycle-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("levels/01")).unwrap();
    std::fs::write(d.join("toc.bin"), [0u8; 4]).unwrap();
    std::fs::write(d.join(cache::core_data_rel(1)), wad(b'A')).unwrap();
    std::fs::write(d.join(cache::gameplay_rel(1)), wad(b'G')).unwrap();
    std::fs::write(d.join(cache::hud_bank_rel(1, 0)), wad(b'H')).unwrap();
    info(&d, "1");
    d
}

fn info(d: &Path, format: &str) {
    std::fs::write(d.join("extract-info.json"),
        format!(r#"{{"disc":"SCUS_971.99","data_format":{format},"extractor_version":"0.1.0","ntsc_only":false,"files":3,"bytes":1}}"#)).unwrap();
}

/// A named stamp edit.
type StampEdit = (&'static str, Box<dyn Fn(&str) -> String>);
/// A named lump-file damage.
type Damage = (&'static str, Box<dyn Fn(&mut Vec<u8>)>);

fn stats(requests: u64, memory_hits: u64, cache_reads: u64, decompressions: u64, cache_writes: u64, rebuilds: u64) -> Stats {
    Stats { requests, memory_hits, cache_reads, decompressions, cache_writes, rebuilds }
}

#[test]
fn lazy_build_then_once_per_process_then_read_from_disk() {
    let d = data_folder("lazy");
    let c = Cache::new(&d);
    assert_eq!(c.status().unwrap(), Status::Missing);
    assert_eq!(cache::v1_sources(&d).unwrap(), ["levels/01/core_data.bin", "levels/01/gameplay_ntsc.bin", "levels/01/hud_bank_0.bin"]);

    // First process: builds the cache lazily, decompressing each lump once.
    let s = Lumps::with_cache_dir(Some(c.dir()));
    let a = s.get(&d, &cache::core_data_rel(1)).unwrap();
    let b = s.get(&d, &cache::core_data_rel(1)).unwrap();
    assert_eq!(&a[..], expected(b'A'));
    assert!(Arc::ptr_eq(&a, &b), "the second request shares the first one's bytes");
    assert_eq!(&s.get(&d, &cache::gameplay_rel(1)).unwrap()[..], expected(b'G'));
    assert_eq!(s.stats(), stats(3, 1, 0, 2, 2, 0));
    assert_eq!(c.status().unwrap(), Status::Fresh);
    let stamp = Stamp::parse(&std::fs::read_to_string(c.stamp_path()).unwrap()).unwrap();
    assert_eq!(stamp.kinds, [("wad".to_string(), cache::WAD_KIND_VERSION)]);
    assert!(stamp.tier0_manifest.starts_with("extract-info.json:xxh64:"), "{}", stamp.tier0_manifest);
    let file = std::fs::read(c.lump_path(&cache::core_data_rel(1))).unwrap();
    assert_eq!(&file[..file.len() - TRAILER_LEN], expected(b'A'), "a lump is the raw decompressed bytes, then the trailer");

    // Second process: reads from disk, decompresses nothing.
    let s = Lumps::with_cache_dir(Some(c.dir()));
    assert_eq!(&s.get(&d, &cache::core_data_rel(1)).unwrap()[..], expected(b'A'));
    assert_eq!(&s.get(&d, &cache::gameplay_rel(1)).unwrap()[..], expected(b'G'));
    assert_eq!(&s.get(&d, &cache::gameplay_rel(1)).unwrap()[..], expected(b'G'));
    assert_eq!(s.stats(), stats(3, 1, 2, 0, 0, 0));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn stale_stamp_triggers_a_rebuild() {
    let d = data_folder("stale");
    let c = Cache::new(&d);
    let rel = cache::core_data_rel(1);
    Lumps::with_cache_dir(Some(c.dir())).get(&d, &rel).unwrap();
    let fresh = std::fs::read_to_string(c.stamp_path()).unwrap();

    // Each stamp field that must match: an older converter, another cache version, another extraction.
    let edits: [StampEdit; 3] = [
        ("wad converter version", Box::new(|t: &str| t.replace("kind.wad = 1", "kind.wad = 0"))),
        ("cache version", Box::new(|t: &str| t.replace("cache_version = 1", "cache_version = 7"))),
        ("unreadable stamp", Box::new(|_: &str| "garbage".to_string())),
    ];
    for (why, edit) in edits {
        std::fs::write(c.stamp_path(), edit(&fresh)).unwrap();
        // A marker in the old lump proves it is not served again.
        std::fs::write(c.lump_path(&rel), b"old lump").unwrap();
        match c.status().unwrap() {
            Status::Stale(r) => assert!(r.contains(why), "{r}"),
            other => panic!("{why}: {other:?}"),
        }
        let s = Lumps::with_cache_dir(Some(c.dir()));
        assert_eq!(&s.get(&d, &rel).unwrap()[..], expected(b'A'));
        assert_eq!(s.stats(), stats(1, 0, 0, 1, 1, 0), "{why}: the stale cache was emptied, then the lump rebuilt");
        assert_eq!(c.status().unwrap(), Status::Fresh, "{why}");
        assert_eq!(c.read_lump(&rel).unwrap(), expected(b'A'), "{why}");
    }

    // A re-extraction with another result (here: another data format) makes the whole cache stale.
    info(&d, "2");
    assert!(matches!(c.status().unwrap(), Status::Stale(r) if r.contains("another extraction")));
    let s = Lumps::with_cache_dir(Some(c.dir()));
    s.get(&d, &rel).unwrap();
    assert_eq!(s.stats().decompressions, 1);
    assert_eq!(c.status().unwrap(), Status::Fresh);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn corrupt_lump_is_detected_and_rebuilt() {
    let d = data_folder("corrupt");
    let c = Cache::new(&d);
    let rel = cache::gameplay_rel(1);
    Lumps::with_cache_dir(Some(c.dir())).get(&d, &rel).unwrap();
    let good = std::fs::read(c.lump_path(&rel)).unwrap();

    let damage: [Damage; 5] = [
        ("payload hash mismatch", Box::new(|f: &mut Vec<u8>| f[2] ^= 0x40)),
        ("no lump trailer", Box::new(|f: &mut Vec<u8>| f.truncate(f.len() - 1))),
        ("no lump trailer", Box::new(|f: &mut Vec<u8>| f.truncate(3))),
        ("payload bytes", Box::new(|f: &mut Vec<u8>| { f.insert(0, 0); })),
        ("no lump trailer", Box::new(|f: &mut Vec<u8>| { let n = f.len(); f[n - TRAILER_LEN] = b'X'; })),
    ];
    for (why, hurt) in damage {
        let mut f = good.clone();
        hurt(&mut f);
        std::fs::write(c.lump_path(&rel), &f).unwrap();
        match c.read_lump(&rel) {
            Err(LumpState::Corrupt(r)) => assert!(r.contains(why), "{why}: {r}"),
            other => panic!("{why}: {other:?}"),
        }
        let s = Lumps::with_cache_dir(Some(c.dir()));
        assert_eq!(&s.get(&d, &rel).unwrap()[..], expected(b'G'), "{why}");
        assert_eq!(s.stats(), stats(1, 0, 0, 1, 1, 1), "{why}");
        assert_eq!(std::fs::read(c.lump_path(&rel)).unwrap(), good, "{why}: rewritten");
    }

    // A lump of another converter version, or built from another source, is stale, not corrupt; also rebuilt.
    let mut f = good.clone();
    let n = f.len();
    f[n - TRAILER_LEN + 8] = 9;
    std::fs::write(c.lump_path(&rel), &f).unwrap();
    assert!(matches!(c.read_lump(&rel), Err(LumpState::Stale(r)) if r.contains("converter version 9")));
    std::fs::write(c.lump_path(&rel), &good).unwrap();
    std::fs::write(d.join(&rel), [wad(b'g'), vec![0; 4]].concat()).unwrap();
    assert!(matches!(c.read_lump(&rel), Err(LumpState::Stale(r)) if r.contains("source is")));
    let s = Lumps::with_cache_dir(Some(c.dir()));
    assert_eq!(&s.get(&d, &rel).unwrap()[..], expected(b'g'));
    assert_eq!(s.stats().rebuilds, 1);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn unwritable_cache_falls_back_to_memory() {
    let d = data_folder("readonly");
    // `cache` is a file, so `cache/v1` cannot be created.
    std::fs::write(d.join("cache"), b"not a folder").unwrap();
    let s = Lumps::with_cache_dir(Some(&d.join("cache/v1")));
    assert_eq!(&s.get(&d, &cache::core_data_rel(1)).unwrap()[..], expected(b'A'));
    assert_eq!(&s.get(&d, &cache::core_data_rel(1)).unwrap()[..], expected(b'A'));
    assert_eq!(&s.get(&d, &cache::hud_bank_rel(1, 0)).unwrap()[..], expected(b'H'));
    assert_eq!(s.stats(), stats(3, 1, 0, 2, 0, 0));

    // Memory only (`RC_CACHE=0`): nothing is written.
    let e = data_folder("memory");
    let s = Lumps::with_cache_dir(None);
    assert_eq!(&s.get(&e, &cache::gameplay_rel(1)).unwrap()[..], expected(b'G'));
    assert!(!e.join("cache").exists());
    // A missing or damaged Tier 0 lump is an error, not a panic.
    assert!(matches!(s.get(&e, "levels/02/core_data.bin"), Err(rc_data::Error::Source { .. })));
    std::fs::write(e.join("levels/01/core_data.bin"), b"WAD garbage").unwrap();
    assert!(matches!(s.get(&e, &cache::core_data_rel(1)), Err(rc_data::Error::Wad { .. })));
    for p in [d, e] { std::fs::remove_dir_all(p).unwrap(); }
}

#[test]
fn ensure_keeps_good_lumps_and_rebuilds_the_rest() {
    let d = data_folder("ensure");
    let c = Cache::new(&d);
    c.reset("test").unwrap();
    let rels = cache::v1_sources(&d).unwrap();
    for r in &rels { assert_eq!(c.ensure_lump(r).unwrap(), Ensured::Built(11)); }
    for r in &rels { assert_eq!(c.ensure_lump(r).unwrap(), Ensured::UpToDate(11)); }
    // Same length, other bytes: the source hash catches it.
    std::fs::write(d.join(&rels[0]), wad(b'Z')).unwrap();
    assert_eq!(c.ensure_lump(&rels[0]).unwrap(), Ensured::Built(11));
    assert_eq!(c.read_lump(&rels[0]).unwrap(), expected(b'Z'));
    // Leftovers of a killed writer are removed.
    std::fs::write(c.dir().join("wad/levels/01/core_data.lump.123.partial"), b"x").unwrap();
    assert_eq!(c.remove_partials().unwrap(), 1);
    // Older cache versions are found for removal; this one and non-version folders are not.
    for v in ["v0", "v12", "vx", "tmp"] { std::fs::create_dir_all(d.join("cache").join(v)).unwrap(); }
    assert_eq!(cache::other_versions(&d), [d.join("cache/v0"), d.join("cache/v12")]);
    std::fs::remove_dir_all(&d).unwrap();
}
