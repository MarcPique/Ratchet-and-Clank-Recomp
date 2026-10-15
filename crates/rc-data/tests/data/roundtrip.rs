//! `parse(Tier 0) == load(Tier 1)` on the real data: for every lump v1 caches in all 19 levels, the bytes the
//! engine gets through the cache (built lazily, then read back by a fresh store, then by `prepare`'s check) equal
//! `rc_formats::wad::decompress` of the Tier 0 file (whose output rc-formats' golden test checks against the committed
//! snapshot hashes). Skips without the development tree (`RC_EXTRACTED`, else `<workspace>/extracted`); the cache goes to a temp
//! folder, never into that tree.

use rc_data::cache::{self, Cache, Ensured};
use rc_data::Lumps;
use rc_formats::wad;
use std::time::{Duration, Instant};

#[test]
fn every_cached_lump_equals_fresh_decompression_for_all_19_levels() {
    let root = rc_formats::test_data::root();
    if !root.join("levels/01/core_data.bin").is_file() {
        eprintln!("skipping: no development data tree at {}", root.display());
        return;
    }
    let dir = std::env::temp_dir().join(format!("rc-data-roundtrip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sources = cache::v1_sources(&root).unwrap();
    let (mut levels, mut lumps, mut bytes) = (0, 0, 0u64);
    let (mut t_decompress, mut t_cached) = (Duration::ZERO, Duration::ZERO);
    for level in 0..19u32 {
        let prefix = format!("levels/{level:02}/");
        let rels: Vec<&String> = sources.iter().filter(|r| r.starts_with(&prefix)).collect();
        assert!(rels.len() >= 3, "level {level:02}: {rels:?}");
        assert!(rels.contains(&&cache::core_data_rel(level)) && rels.contains(&&cache::gameplay_rel(level)));
        levels += 1;
        let c = Cache::with_dir(&root, &dir);
        let first = Lumps::with_cache_dir(Some(&dir));
        let second = Lumps::with_cache_dir(Some(&dir));
        for rel in &rels {
            let t0 = Instant::now();
            let fresh = wad::decompress(&std::fs::read(root.join(rel)).unwrap()).unwrap();
            t_decompress += t0.elapsed();
            assert_eq!(&first.get(&root, rel).unwrap()[..], &fresh[..], "{rel}: lazily built");
            let t0 = Instant::now();
            let cached = second.get(&root, rel).unwrap();
            t_cached += t0.elapsed();
            assert_eq!(&cached[..], &fresh[..], "{rel}: read back from the cache");
            assert_eq!(c.read_lump(rel).unwrap(), fresh, "{rel}: read_lump");
            assert_eq!(c.ensure_lump(rel).unwrap(), Ensured::UpToDate(fresh.len() as u64), "{rel}: prepare keeps it");
            lumps += 1;
            bytes += fresh.len() as u64;
        }
        assert_eq!(second.stats().cache_reads, rels.len() as u64, "level {level:02}: nothing decompressed on the second read");
        assert_eq!(second.stats().decompressions, 0);
        // One level at a time keeps the temp folder small.
        std::fs::remove_dir_all(&dir).unwrap();
    }
    assert_eq!(levels, 19);
    eprintln!(
        "{levels} levels, {lumps} lumps, {:.1} MiB decompressed; \
         decompress {:.0} ms total vs cached read + check {:.0} ms total",
        bytes as f64 / (1u64 << 20) as f64, t_decompress.as_secs_f64() * 1e3, t_cached.as_secs_f64() * 1e3
    );
}
