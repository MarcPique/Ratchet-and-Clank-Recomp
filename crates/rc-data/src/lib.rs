//! `rc-data`: the engine's access to decompressed game data, backed by the Tier 1 engine cache
//! (docs/plan/launcher_extractor.md §5.2, "Tier 1 as built").
//!
//! The engine asks for a lump by level and name ([`level_core_data`], [`level_gameplay`], [`hud_bank`], or any WAD
//! lump by its Tier 0 path with [`wad_lump`]) and gets an `Arc<[u8]>` holding exactly what
//! `rc_formats::wad::decompress` returns for the Tier 0 file. Each lump is produced once per process and then shared:
//! 1. from memory, if this process already has it;
//! 2. else from `<data>/cache/v1/wad/…` ([`cache`]), checked (trailer, length, XXH64, source length);
//! 3. else decompressed from Tier 0 and written to the cache (one log line), so the next start reads it. This is the
//!    lazy build: a missing cache is created, a stale one (stamp mismatch) emptied first, a corrupt lump replaced.
//!    When the cache cannot be written (read-only folder, disk full), the lump is kept in memory only and the
//!    process stops trying to write (one log line). `RC_CACHE=0` skips the disk cache entirely.
//!
//! `RC_PERF_LOG=1` prints one line per request (where it came from, bytes, ms); [`stats`] counts them.
//!
//! Why a crate and not an engine module: `ratchet_clank-extract prepare` builds the same cache with the same code (layout,
//! stamp, lump checks), and a crate without Bevy keeps the extractor free of the engine and its tests fast.

pub mod cache;
pub mod xxh64;

pub use cache::{Cache, Error};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Counters of one [`Lumps`] store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Lump requests.
    pub requests: u64,
    /// Served from this process's memory.
    pub memory_hits: u64,
    /// Read from the disk cache.
    pub cache_reads: u64,
    /// WAD decompressions from Tier 0 (lazy builds and in-memory fallbacks).
    pub decompressions: u64,
    /// Lumps written to the disk cache.
    pub cache_writes: u64,
    /// Cached lumps found corrupt or stale and replaced.
    pub rebuilds: u64,
}

/// How lumps of one data folder are produced.
enum Backing {
    /// Through the disk cache.
    Cache(Cache),
    /// Decompressed in memory each process (`RC_CACHE=0`, or the cache cannot be written).
    Memory,
}

#[derive(Default)]
struct Inner {
    roots: HashMap<PathBuf, Backing>,
    lumps: HashMap<(PathBuf, String), Arc<[u8]>>,
    stats: Stats,
}

/// A store of decompressed lumps, each produced once. The engine uses the process-wide one ([`global`]); tests
/// make their own.
pub struct Lumps {
    inner: Mutex<Inner>,
    /// A fixed cache folder for every data folder (tests); `None` = `<data>/cache/v1`.
    cache_dir: Option<PathBuf>,
    /// Use the disk cache (`RC_CACHE` is not `0`).
    use_cache: bool,
    /// Print every request (`RC_PERF_LOG`).
    log_requests: bool,
    /// The stamp's `written_by`.
    writer: String,
}

impl Default for Lumps {
    fn default() -> Self { Lumps::new() }
}

fn env_on(name: &str) -> bool { std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0") }

fn mib(n: usize) -> f64 { n as f64 / (1u64 << 20) as f64 }

impl Lumps {
    /// A store using `<data>/cache/v1`, `RC_CACHE` and `RC_PERF_LOG` from the environment.
    pub fn new() -> Lumps {
        Lumps {
            inner: Mutex::default(),
            cache_dir: None,
            use_cache: std::env::var_os("RC_CACHE").is_none_or(|v| v != "0"),
            log_requests: env_on("RC_PERF_LOG"),
            writer: format!("rc-data {} (lazy build)", env!("CARGO_PKG_VERSION")),
        }
    }

    /// A store whose cache lives in `dir` (for every data folder), or none at all (`None`: memory only).
    pub fn with_cache_dir(dir: Option<&Path>) -> Lumps {
        Lumps { cache_dir: dir.map(Path::to_path_buf), use_cache: dir.is_some(), log_requests: false, ..Lumps::new() }
    }

    pub fn stats(&self) -> Stats { self.lock().stats }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> { self.inner.lock().unwrap_or_else(|e| e.into_inner()) }

    /// Opens (and if needed creates or empties) the cache of `root`.
    fn open(&self, root: &Path) -> Backing {
        if !self.use_cache {
            if self.cache_dir.is_none() { println!("rc-data: RC_CACHE=0: decompressing game data in memory, no engine cache"); }
            return Backing::Memory;
        }
        let c = match &self.cache_dir { Some(d) => Cache::with_dir(root, d), None => Cache::new(root) };
        let why = match c.status() {
            Ok(cache::Status::Fresh) => return Backing::Cache(c),
            Ok(cache::Status::Missing) => format!("no engine cache in {} yet", c.dir().display()),
            Ok(cache::Status::Stale(why)) => format!("the engine cache in {} is stale ({why})", c.dir().display()),
            Err(e) => {
                eprintln!("rc-data: cannot check the engine cache ({e}); decompressing in memory");
                return Backing::Memory;
            }
        };
        match c.reset(&self.writer) {
            Ok(()) => {
                println!("rc-data: {why}; building it as lumps are first used (ratchet_clank-extract prepare builds it all at once)");
                Backing::Cache(c)
            }
            Err(e) => {
                eprintln!("rc-data: {why}, and it cannot be created ({e}); decompressing in memory");
                Backing::Memory
            }
        }
    }

    /// The decompressed WAD lump of the Tier 0 file `rel` (e.g. `levels/01/core_data.bin`) in the data folder `root`.
    pub fn get(&self, root: &Path, rel: &str) -> Result<Arc<[u8]>, Error> {
        let t0 = Instant::now();
        let mut g = self.lock();
        let Inner { roots, lumps, stats } = &mut *g;
        stats.requests += 1;
        let key = (root.to_path_buf(), rel.to_string());
        if let Some(a) = lumps.get(&key) {
            stats.memory_hits += 1;
            if self.log_requests { println!("rc-data: {rel}: memory ({:.1} MiB)", mib(a.len())); }
            return Ok(a.clone());
        }
        let backing = roots.entry(key.0.clone()).or_insert_with(|| self.open(root));
        let (data, how) = match backing {
            Backing::Memory => {
                let (data, _) = cache::decompress_tier0(root, rel)?;
                stats.decompressions += 1;
                (data, "decompressed in memory")
            }
            Backing::Cache(c) => match c.read_lump(rel) {
                Ok(data) => {
                    stats.cache_reads += 1;
                    (data, "engine cache")
                }
                Err(state) => {
                    if state != cache::LumpState::Missing {
                        println!("rc-data: cached {rel} is {state}; rebuilding it");
                        stats.rebuilds += 1;
                    }
                    let b = c.build_lump(rel)?;
                    stats.decompressions += 1;
                    match b.write {
                        Ok(w) => {
                            stats.cache_writes += 1;
                            println!(
                                "rc-data: cached {rel} ({:.1} MiB; decompressed in {:.1} ms, written in {:.1} ms)",
                                mib(b.data.len()), b.decompress.as_secs_f64() * 1e3, w.as_secs_f64() * 1e3
                            );
                        }
                        Err(e) => {
                            eprintln!("rc-data: cannot write the engine cache ({e}); decompressing in memory from now on");
                            *backing = Backing::Memory;
                        }
                    }
                    (b.data, "decompressed from Tier 0")
                }
            },
        };
        let a: Arc<[u8]> = Arc::from(data);
        if self.log_requests { println!("rc-data: {rel}: {how} ({:.1} MiB, {:.1} ms)", mib(a.len()), t0.elapsed().as_secs_f64() * 1e3); }
        lumps.insert(key, a.clone());
        Ok(a)
    }

    /// Drops the lumps this store holds for `root` (they stay alive where still referenced). For a future level
    /// change; the engine loads one level per process today.
    pub fn forget(&self, root: &Path) { self.lock().lumps.retain(|(r, _), _| r != root); }

    /// Drops the lumps of level `index` (`levels/NN/…`) this store holds for `root` (still alive where referenced): the
    /// engine's runtime level change frees the level it left, as the game reuses the level's memory.
    pub fn forget_level(&self, root: &Path, index: u32) {
        let prefix = format!("levels/{index:02}/");
        self.lock().lumps.retain(|(r, rel), _| r != root || !rel.starts_with(&prefix));
    }
}

static GLOBAL: OnceLock<Lumps> = OnceLock::new();

/// The process-wide store the engine uses.
pub fn global() -> &'static Lumps { GLOBAL.get_or_init(Lumps::new) }

/// Counters of the process-wide store.
pub fn stats() -> Stats { global().stats() }

/// Any WAD lump by its Tier 0 path, from the process-wide store.
pub fn wad_lump(root: &Path, rel: &str) -> Result<Arc<[u8]>, Error> { global().get(root, rel) }

/// `levels/NN/core_data.bin`, decompressed.
pub fn level_core_data(root: &Path, index: u32) -> Result<Arc<[u8]>, Error> { wad_lump(root, &cache::core_data_rel(index)) }

/// `levels/NN/gameplay_ntsc.bin`, decompressed.
pub fn level_gameplay(root: &Path, index: u32) -> Result<Arc<[u8]>, Error> { wad_lump(root, &cache::gameplay_rel(index)) }

/// `levels/NN/hud_bank_B.bin`, decompressed.
pub fn hud_bank(root: &Path, index: u32, bank: usize) -> Result<Arc<[u8]>, Error> { wad_lump(root, &cache::hud_bank_rel(index, bank)) }
