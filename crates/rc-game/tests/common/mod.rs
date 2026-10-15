//! Helpers shared by rc-game's integration binaries (`#[path = "../common/mod.rs"] mod common;` in a group's
//! `main.rs`). Per-process caches of the parsed level overlays and of the per-level [`LevelPorts`]:
//! `LevelPorts::from_overlays` parses the reference overlay of every port it matches, so an uncached build re-read
//! and re-parsed up to 19 overlays per level (about 1.1 s). The cached values are the same parse of the same files,
//! handed out as clones; a test sees exactly what it built before (docs/workflows/testing.md §5).
//!
//! Under `cargo test` the caches live for the whole binary; under nextest (one process per test) they still save
//! the repeated parses inside one test.

#![allow(dead_code)]

use rc_formats::level_overlay::LevelOverlay;
use rc_game::moby_update::classes::LevelPorts;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, OnceLock};

/// A per-process memo: each key's value is computed once; a thread asking for a key that another thread is
/// computing waits for it. A computation that panics leaves the key empty, so the next caller panics the same way.
pub struct Memo<K, V>(OnceLock<Mutex<HashMap<K, Arc<OnceLock<V>>>>>);

impl<K: Eq + Hash, V: Clone> Memo<K, V> {
    pub const fn new() -> Self { Memo(OnceLock::new()) }

    pub fn get(&self, key: K, compute: impl FnOnce() -> V) -> V {
        let cell = self.0.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default().clone();
        cell.get_or_init(compute).clone()
    }
}

/// The level's parsed `overlay.bin` (None when `extracted/` lacks it; a parse error panics, as the per-file helpers
/// did).
pub fn overlay(level: u32) -> Option<Arc<LevelOverlay>> {
    static CACHE: Memo<u32, Option<Arc<LevelOverlay>>> = Memo::new();
    CACHE.get(level, || {
        let b = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")).ok()?;
        Some(Arc::new(LevelOverlay::parse(&b).unwrap()))
    })
}

/// `LevelPorts::from_overlays(overlay(level), overlay, external)` (None when the level's overlay is missing).
pub fn ports(level: u32, external: &[u32]) -> Option<LevelPorts> {
    static CACHE: Memo<(u32, Vec<u32>), Option<LevelPorts>> = Memo::new();
    CACHE.get((level, external.to_vec()), || Some(LevelPorts::from_overlays(&*overlay(level)?, &overlay, external)))
}

/// The level's `LevelWaterData` (None when `extracted/` lacks the overlay or the gameplay file; a load error panics
/// with the level, as the per-file helpers did).
pub fn water_data(level: u32) -> Option<rc_game::water::world::LevelWaterData> {
    static CACHE: Memo<u32, Option<rc_game::water::world::LevelWaterData>> = Memo::new();
    CACHE.get(level, || {
        let bytes = std::fs::read(rc_formats::test_data::level_dir(level).join("overlay.bin")).ok()?;
        let target = LevelOverlay::parse(&bytes).unwrap();
        let gp = rc_formats::test_data::gameplay(level)?;
        Some(rc_game::water::world::LevelWaterData::load(&bytes, &target, &overlay, &gp).unwrap_or_else(|e| panic!("level {level:02} water data: {e}")))
    })
}
