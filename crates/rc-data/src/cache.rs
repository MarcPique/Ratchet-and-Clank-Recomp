//! The Tier 1 engine cache on disk, version 1: every WAD lump the engine reads, decompressed once by the same
//! golden-tested `rc_formats::wad::decompress` the engine used to call on every load. Nothing is converted, so the
//! cached bytes are exactly `wad::decompress(Tier 0 file)` (round-trip test `tests/data/roundtrip.rs`, all 19 levels).
//!
//! Layout under the data folder (docs/plan/launcher_extractor.md §6.2 and "Tier 1 as built"):
//! ```text
//! <data>/cache/v1/stamp.toml                       cache version, converter version per kind, Tier 0 manifest hash
//! <data>/cache/v1/wad/levels/NN/core_data.lump     decompressed levels/NN/core_data.bin
//! <data>/cache/v1/wad/levels/NN/gameplay_ntsc.lump decompressed levels/NN/gameplay_ntsc.bin
//! <data>/cache/v1/wad/levels/NN/hud_bank_B.lump    decompressed levels/NN/hud_bank_B.bin (B = 0..4)
//! ```
//! A kind's files mirror the Tier 0 path of their source under `<kind>/`, with the extension `.lump`.
//!
//! **Lump file** = the decompressed bytes as they are (offset 0 = offset 0 of the decompressed lump, so tools and a
//! future memory map can use it directly), then a 48-byte little-endian trailer ([`Trailer`]): magic `RCWLUMP1`, kind
//! version, reserved 0, payload length, payload XXH64, source length, source XXH64. A reader checks the magic, the
//! kind version, the length, the payload hash (corruption) and the Tier 0 source length (a swapped source); the
//! source hash is checked only by `prepare`, which reads the sources anyway.
//!
//! **Stamp** (`stamp.toml`, `key = value` lines): `cache_version`, `game`, `tier0_manifest` (XXH64 of
//! `extract-info.json`, or `none` in a development tree without one), one `kind.<name> = <version>` line per kind,
//! and the informational `written_by`. The cache is stale when any of the first four differ from this build's;
//! a stale cache is emptied and rebuilt (eagerly by `ratchet_clank-extract prepare`, lazily by the engine).
//!
//! Writes are atomic: `<file>.<pid>.partial`, then a rename. A killed writer leaves at most a `.partial` file
//! (removed by the next `prepare`) and never a complete-looking wrong lump.

use crate::xxh64::xxh64;
use rc_formats::wad;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The cache layout version: the `v<N>` folder name and the stamp's `cache_version`.
pub const CACHE_VERSION: u32 = 1;
/// The stamp file inside `cache/v<N>/`.
pub const STAMP_FILE: &str = "stamp.toml";
/// Kind: a WAD lump decompressed with `rc_formats::wad::decompress`, unchanged.
pub const WAD_KIND: &str = "wad";
/// Converter version of [`WAD_KIND`]. Bump it when the bytes a `.lump` holds change for the same source.
pub const WAD_KIND_VERSION: u32 = 1;
/// Every kind this build writes, with its converter version (the stamp's `kind.<name>` lines).
pub const KINDS: &[(&str, u32)] = &[(WAD_KIND, WAD_KIND_VERSION)];
/// Extension of a cached lump.
pub const LUMP_EXT: &str = "lump";
/// Size of the trailer after a lump's payload.
pub const TRAILER_LEN: usize = 48;
/// First 8 bytes of the trailer.
pub const TRAILER_MAGIC: [u8; 8] = *b"RCWLUMP1";
/// Suffix of a cache file still being written.
pub const PARTIAL_SUFFIX: &str = ".partial";
/// The extractor's completion record in the data folder (launcher contract); its hash is the Tier 0 manifest hash.
pub const INFO_FILE: &str = "extract-info.json";
/// The game this cache belongs to (the stamp's `game`).
pub const GAME: &str = "rac1";

/// `<root>/cache/v<CACHE_VERSION>`: where the cache of the data folder `root` lives.
pub fn cache_dir(root: &Path) -> PathBuf { root.join("cache").join(format!("v{CACHE_VERSION}")) }

/// Tier 0 path of a level's core data (WAD-compressed).
pub fn core_data_rel(index: u32) -> String { format!("levels/{index:02}/core_data.bin") }
/// Tier 0 path of a level's NTSC gameplay file (WAD-compressed).
pub fn gameplay_rel(index: u32) -> String { format!("levels/{index:02}/gameplay_ntsc.bin") }
/// Tier 0 path of a level's HUD bank `bank` (WAD-compressed; `rc_formats::hud::BANKS` banks).
pub fn hud_bank_rel(index: u32, bank: usize) -> String { format!("levels/{index:02}/hud_bank_{bank}.bin") }

/// The Tier 0 files v1 caches, in `root`'s `levels/NN/` folders (only those present, in path order): each level's
/// core data, NTSC gameplay file and HUD banks. These are the lumps the engine decompressed on every load.
pub fn v1_sources(root: &Path) -> io::Result<Vec<String>> {
    let mut levels: Vec<u32> = fs::read_dir(root.join("levels"))?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().filter(|n| n.len() == 2).and_then(|n| n.parse().ok()))
        .collect();
    levels.sort_unstable();
    let mut out = Vec::new();
    for l in levels {
        let names = [core_data_rel(l), gameplay_rel(l)].into_iter().chain((0..rc_formats::hud::BANKS).map(|b| hud_bank_rel(l, b)));
        out.extend(names.filter(|rel| root.join(rel).is_file()));
    }
    Ok(out)
}

/// Why a cache operation failed.
#[derive(Debug)]
pub enum Error {
    /// A Tier 0 file (or `extract-info.json`) exists but cannot be read, or a source is missing.
    Source { path: PathBuf, err: io::Error },
    /// A Tier 0 source is not a valid WAD stream: the archive is damaged (`ratchet_clank-extract verify` names it).
    Wad { path: PathBuf, err: rc_formats::buf::FormatError },
    /// Creating, writing, renaming or deleting a cache file failed.
    Write { path: PathBuf, err: io::Error },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Source { path, err } => write!(f, "cannot read {}: {err}", path.display()),
            Error::Wad { path, err } => write!(f, "{} is not a valid WAD stream ({err}); the game data is damaged", path.display()),
            Error::Write { path, err } => write!(f, "cannot write {}: {err}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Source { err, .. } | Error::Write { err, .. } => Some(err),
            Error::Wad { err, .. } => Some(err),
        }
    }
}

fn write_err(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ { move |err| Error::Write { path: path.to_path_buf(), err } }

/// `tier0_manifest` for `root`: `extract-info.json:xxh64:<16 hex>` of the extractor's completion record (its
/// disc, data format, extractor version, NTSC-only flag, file and byte counts; the file contents themselves are fixed
/// by the committed SHA-1 table for that disc), or `none` for a development tree without one.
pub fn tier0_manifest(root: &Path) -> Result<String, Error> {
    let p = root.join(INFO_FILE);
    match fs::read(&p) {
        Ok(b) => Ok(format!("{INFO_FILE}:xxh64:{:016x}", xxh64(&b))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok("none".into()),
        Err(err) => Err(Error::Source { path: p, err }),
    }
}

/// Reads the Tier 0 file `rel` of the data folder `root` and WAD-decompresses it: `(decompressed, source)`. The one
/// place Tier 1 v1 turns Tier 0 bytes into lump bytes.
pub fn decompress_tier0(root: &Path, rel: &str) -> Result<(Vec<u8>, Vec<u8>), Error> {
    let path = root.join(rel);
    let source = fs::read(&path).map_err(|err| Error::Source { path: path.clone(), err })?;
    let data = wad::decompress(&source).map_err(|err| Error::Wad { path, err })?;
    Ok((data, source))
}

/// The contents of `stamp.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub cache_version: u32,
    pub game: String,
    pub tier0_manifest: String,
    /// `(kind, converter version)`, in [`KINDS`] order when written by this build.
    pub kinds: Vec<(String, u32)>,
    /// Who wrote it (informational, not compared).
    pub written_by: String,
}

impl Stamp {
    /// The stamp this build writes for the data folder `root`.
    pub fn current(root: &Path, written_by: &str) -> Result<Stamp, Error> {
        Ok(Stamp {
            cache_version: CACHE_VERSION,
            game: GAME.into(),
            tier0_manifest: tier0_manifest(root)?,
            kinds: KINDS.iter().map(|&(k, v)| (k.to_string(), v)).collect(),
            written_by: written_by.into(),
        })
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from(
            "# Ratchet & Clank Recomp Tier 1 engine cache (docs/plan/launcher_extractor.md). Rebuilt from the Tier 0 archive when stale;\n\
             # delete this folder at any time.\n",
        );
        s += &format!("cache_version = {}\ngame = \"{}\"\ntier0_manifest = \"{}\"\n", self.cache_version, self.game, self.tier0_manifest);
        for (k, v) in &self.kinds { s += &format!("kind.{k} = {v}\n"); }
        s += &format!("written_by = \"{}\"\n", self.written_by);
        s
    }

    /// Parses [`Stamp::to_text`] output (`key = value` lines, `#` comments; values are integers or `"strings"`
    /// without escapes). `None` if a required key is missing or a line is malformed.
    pub fn parse(text: &str) -> Option<Stamp> {
        let (mut cache_version, mut game, mut tier0_manifest, mut kinds, mut written_by) = (None, None, None, Vec::new(), String::new());
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let (k, v) = line.split_once('=')?;
            let (k, v) = (k.trim(), v.trim());
            let s = || v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).filter(|v| !v.contains('"')).map(str::to_string);
            match k {
                "cache_version" => cache_version = Some(v.parse().ok()?),
                "game" => game = Some(s()?),
                "tier0_manifest" => tier0_manifest = Some(s()?),
                "written_by" => written_by = s()?,
                _ => {
                    let kind = k.strip_prefix("kind.").filter(|k| !k.is_empty())?;
                    kinds.push((kind.to_string(), v.parse().ok()?));
                }
            }
        }
        Some(Stamp { cache_version: cache_version?, game: game?, tier0_manifest: tier0_manifest?, kinds, written_by })
    }

    /// Why a cache with stamp `self` cannot be used by a build that wants `want`; `None` when it can.
    pub fn stale_reason(&self, want: &Stamp) -> Option<String> {
        if self.cache_version != want.cache_version {
            return Some(format!("cache version {} (this build: {})", self.cache_version, want.cache_version));
        }
        if self.game != want.game { return Some(format!("game {:?} (this build: {:?})", self.game, want.game)); }
        if self.tier0_manifest != want.tier0_manifest {
            return Some(format!("built from another extraction ({} vs {})", self.tier0_manifest, want.tier0_manifest));
        }
        for (k, v) in &want.kinds {
            match self.kinds.iter().find(|(sk, _)| sk == k) {
                Some((_, sv)) if sv == v => {}
                Some((_, sv)) => return Some(format!("{k} converter version {sv} (this build: {v})")),
                None => return Some(format!("no {k} lumps (this build writes version {v})")),
            }
        }
        None
    }
}

/// State of the cache folder as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// No stamp: never built (or deleted).
    Missing,
    /// The stamp matches this build and this extraction.
    Fresh,
    /// The stamp is unreadable or does not match (the reason).
    Stale(String),
}

/// A lump file's trailer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trailer {
    pub kind_version: u32,
    pub payload_len: u64,
    pub payload_xxh64: u64,
    pub source_len: u64,
    pub source_xxh64: u64,
}

impl Trailer {
    pub fn to_bytes(&self) -> [u8; TRAILER_LEN] {
        let mut b = [0u8; TRAILER_LEN];
        b[0..8].copy_from_slice(&TRAILER_MAGIC);
        b[8..12].copy_from_slice(&self.kind_version.to_le_bytes());
        // 12..16 reserved, zero.
        b[16..24].copy_from_slice(&self.payload_len.to_le_bytes());
        b[24..32].copy_from_slice(&self.payload_xxh64.to_le_bytes());
        b[32..40].copy_from_slice(&self.source_len.to_le_bytes());
        b[40..48].copy_from_slice(&self.source_xxh64.to_le_bytes());
        b
    }

    /// The trailer in the last [`TRAILER_LEN`] bytes of `file`; `None` without the magic or with nonzero reserved bytes.
    pub fn from_file_end(file: &[u8]) -> Option<Trailer> {
        let b = file.get(file.len().checked_sub(TRAILER_LEN)?..)?;
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        (b[0..8] == TRAILER_MAGIC && u32_at(12) == 0).then(|| Trailer {
            kind_version: u32_at(8),
            payload_len: u64_at(16),
            payload_xxh64: u64_at(24),
            source_len: u64_at(32),
            source_xxh64: u64_at(40),
        })
    }
}

/// Why a cached lump cannot be served.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LumpState {
    /// Not built yet.
    Missing,
    /// Damaged: truncated, bad trailer, or a payload hash mismatch (the reason).
    Corrupt(String),
    /// Written by another converter version, or its Tier 0 source changed (the reason).
    Stale(String),
}

impl fmt::Display for LumpState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LumpState::Missing => write!(f, "missing"),
            LumpState::Corrupt(why) => write!(f, "corrupt ({why})"),
            LumpState::Stale(why) => write!(f, "stale ({why})"),
        }
    }
}

/// One lump decompressed from Tier 0 by [`Cache::build_lump`].
#[derive(Debug)]
pub struct Built {
    /// The decompressed bytes.
    pub data: Vec<u8>,
    pub decompress: Duration,
    /// The cache write: how long it took, or why it failed (the bytes are still good).
    pub write: Result<Duration, Error>,
}

/// What [`Cache::ensure_lump`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ensured {
    /// The lump was valid and matches its source: kept. Payload bytes.
    UpToDate(u64),
    /// The lump was (re)built. Payload bytes.
    Built(u64),
}

/// The cache of one data folder.
#[derive(Clone, Debug)]
pub struct Cache {
    root: PathBuf,
    dir: PathBuf,
}

impl Cache {
    /// The cache of the data folder `root`, in `<root>/cache/v1`.
    pub fn new(root: &Path) -> Cache { Cache::with_dir(root, &cache_dir(root)) }

    /// The cache of `root` kept in `dir` instead (tests; never the development `extracted/` tree).
    pub fn with_dir(root: &Path, dir: &Path) -> Cache { Cache { root: root.to_path_buf(), dir: dir.to_path_buf() } }

    /// The Tier 0 data folder.
    pub fn root(&self) -> &Path { &self.root }
    /// The cache folder (`cache/v1`).
    pub fn dir(&self) -> &Path { &self.dir }
    pub fn stamp_path(&self) -> PathBuf { self.dir.join(STAMP_FILE) }

    /// Where the lump of the Tier 0 file `rel` is cached: `<dir>/wad/<rel with extension .lump>`.
    pub fn lump_path(&self, rel: &str) -> PathBuf { self.dir.join(WAD_KIND).join(rel).with_extension(LUMP_EXT) }

    /// Reads the stamp and compares it with this build's.
    pub fn status(&self) -> Result<Status, Error> {
        let text = match fs::read_to_string(self.stamp_path()) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Status::Missing),
            Err(e) => return Ok(Status::Stale(format!("unreadable stamp: {e}"))),
        };
        let want = Stamp::current(&self.root, "")?;
        Ok(match Stamp::parse(&text) {
            None => Status::Stale("unreadable stamp".into()),
            Some(s) => s.stale_reason(&want).map_or(Status::Fresh, Status::Stale),
        })
    }

    /// Empties the cache (every lump of every kind) and writes a fresh stamp. Lumps are then built on demand
    /// ([`Cache::build_lump`], [`Cache::ensure_lump`]).
    pub fn reset(&self, written_by: &str) -> Result<(), Error> {
        let stamp = Stamp::current(&self.root, written_by)?;
        let sp = self.stamp_path();
        match fs::remove_file(&sp) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(write_err(&sp)(e)),
        }
        for (kind, _) in KINDS {
            let d = self.dir.join(kind);
            if d.exists() { fs::remove_dir_all(&d).map_err(write_err(&d))?; }
        }
        fs::create_dir_all(&self.dir).map_err(write_err(&self.dir))?;
        write_atomic(&sp, &[stamp.to_text().as_bytes()])
    }

    /// The cached lump of the Tier 0 file `rel`, checked (trailer, length, payload hash, kind version, source
    /// length). The payload only, without the trailer.
    pub fn read_lump(&self, rel: &str) -> Result<Vec<u8>, LumpState> {
        let mut file = match fs::read(self.lump_path(rel)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(LumpState::Missing),
            Err(e) => return Err(LumpState::Corrupt(format!("unreadable: {e}"))),
        };
        let t = check_lump(&file)?;
        let source_len = fs::metadata(self.root.join(rel)).map(|m| m.len()).map_err(|e| LumpState::Stale(format!("source: {e}")))?;
        if t.source_len != source_len {
            return Err(LumpState::Stale(format!("source is {source_len} bytes, the lump was built from {}", t.source_len)));
        }
        file.truncate(t.payload_len as usize);
        Ok(file)
    }

    /// Reads the Tier 0 file `rel` and decompresses it: `(decompressed, source)`.
    pub fn decompress_source(&self, rel: &str) -> Result<(Vec<u8>, Vec<u8>), Error> { decompress_tier0(&self.root, rel) }

    /// Writes the lump of `rel` (atomically) from its decompressed bytes and its Tier 0 source.
    pub fn write_lump(&self, rel: &str, data: &[u8], source: &[u8]) -> Result<(), Error> {
        let t = Trailer {
            kind_version: WAD_KIND_VERSION,
            payload_len: data.len() as u64,
            payload_xxh64: xxh64(data),
            source_len: source.len() as u64,
            source_xxh64: xxh64(source),
        };
        let path = self.lump_path(rel);
        if let Some(d) = path.parent() { fs::create_dir_all(d).map_err(write_err(d))?; }
        write_atomic(&path, &[data, &t.to_bytes()])
    }

    /// Decompresses `rel` from Tier 0 and writes its lump. A write failure is reported in [`Built::write`], not as
    /// an error: the decompressed bytes are good either way.
    pub fn build_lump(&self, rel: &str) -> Result<Built, Error> {
        let t0 = Instant::now();
        let (data, source) = self.decompress_source(rel)?;
        let decompress = t0.elapsed();
        let t0 = Instant::now();
        let write = self.write_lump(rel, &data, &source).map(|()| t0.elapsed());
        Ok(Built { data, decompress, write })
    }

    /// `prepare`'s step: keeps the lump of `rel` if it is valid and was built from exactly this source (length and
    /// XXH64), else rebuilds it. Write failures are errors here.
    pub fn ensure_lump(&self, rel: &str) -> Result<Ensured, Error> {
        let path = self.root.join(rel);
        let source = fs::read(&path).map_err(|err| Error::Source { path: path.clone(), err })?;
        if let Ok(file) = fs::read(self.lump_path(rel)) {
            if let Ok(t) = check_lump(&file) {
                if t.source_len == source.len() as u64 && t.source_xxh64 == xxh64(&source) { return Ok(Ensured::UpToDate(t.payload_len)); }
            }
        }
        let data = wad::decompress(&source).map_err(|err| Error::Wad { path, err })?;
        self.write_lump(rel, &data, &source)?;
        Ok(Ensured::Built(data.len() as u64))
    }

    /// Deletes `*.partial` files a killed writer left in the cache folder. Returns how many.
    pub fn remove_partials(&self) -> Result<usize, Error> {
        let mut n = 0;
        let mut stack = vec![self.dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                match e.file_type() {
                    Ok(t) if t.is_dir() => stack.push(p),
                    Ok(_) if p.to_string_lossy().ends_with(PARTIAL_SUFFIX) => {
                        fs::remove_file(&p).map_err(write_err(&p))?;
                        n += 1;
                    }
                    _ => {}
                }
            }
        }
        Ok(n)
    }

    /// Total bytes of the files in the cache folder.
    pub fn disk_bytes(&self) -> u64 {
        let mut n = 0;
        let mut stack = vec![self.dir.clone()];
        while let Some(d) = stack.pop() {
            let Ok(rd) = fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                match e.metadata() {
                    Ok(m) if m.is_dir() => stack.push(e.path()),
                    Ok(m) => n += m.len(),
                    Err(_) => {}
                }
            }
        }
        n
    }
}

/// Checks a whole lump file (trailer, kind version, length, payload hash); returns its trailer.
fn check_lump(file: &[u8]) -> Result<Trailer, LumpState> {
    let t = Trailer::from_file_end(file).ok_or_else(|| LumpState::Corrupt(format!("no lump trailer ({} bytes)", file.len())))?;
    if t.kind_version != WAD_KIND_VERSION {
        return Err(LumpState::Stale(format!("{WAD_KIND} converter version {} (this build: {WAD_KIND_VERSION})", t.kind_version)));
    }
    if t.payload_len != (file.len() - TRAILER_LEN) as u64 {
        return Err(LumpState::Corrupt(format!("{} payload bytes, the trailer says {}", file.len() - TRAILER_LEN, t.payload_len)));
    }
    if xxh64(&file[..file.len() - TRAILER_LEN]) != t.payload_xxh64 { return Err(LumpState::Corrupt("payload hash mismatch".into())); }
    Ok(t)
}

/// Writes `parts` to `<path>.<pid>.partial`, then renames it over `path`.
fn write_atomic(path: &Path, parts: &[&[u8]]) -> Result<(), Error> {
    use std::io::Write;
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}{PARTIAL_SUFFIX}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let r = (|| {
        let mut f = fs::File::create(&tmp)?;
        for p in parts { f.write_all(p)?; }
        Ok(())
    })();
    if let Err(e) = r {
        let _ = fs::remove_file(&tmp);
        return Err(write_err(&tmp)(e));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        write_err(path)(e)
    })
}

/// `cache/v<N>` folders of other cache versions in `root` (`prepare` removes them).
pub fn other_versions(root: &Path) -> Vec<PathBuf> {
    let current = format!("v{CACHE_VERSION}");
    let Ok(rd) = fs::read_dir(root.join("cache")) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n != current && n.strip_prefix('v').is_some_and(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
        })
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_round_trips_and_compares() {
        let s = Stamp {
            cache_version: 1,
            game: "rac1".into(),
            tier0_manifest: "extract-info.json:xxh64:0123456789abcdef".into(),
            kinds: vec![("wad".into(), 1)],
            written_by: "ratchet_clank-extract 0.1.0".into(),
        };
        let text = s.to_text();
        assert!(text.contains("kind.wad = 1\n") && text.contains("cache_version = 1\n"), "{text}");
        assert_eq!(Stamp::parse(&text), Some(s.clone()));
        let mut want = s.clone();
        want.written_by = "other".into();
        assert_eq!(s.stale_reason(&want), None);
        want.kinds[0].1 = 2;
        assert!(s.stale_reason(&want).unwrap().contains("wad converter version 1"));
        want.kinds.push(("tex".into(), 1));
        want.kinds[0].1 = 1;
        assert!(s.stale_reason(&want).unwrap().contains("no tex lumps"));
        let mut want = s.clone();
        want.tier0_manifest = "none".into();
        assert!(s.stale_reason(&want).unwrap().contains("another extraction"));
        want.cache_version = 2;
        assert!(s.stale_reason(&want).unwrap().contains("cache version"));
        for bad in ["", "cache_version = x\ngame = \"a\"\ntier0_manifest = \"b\"", "game = \"a\"\ntier0_manifest = \"b\"", "junk"] {
            assert_eq!(Stamp::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn trailer_round_trips() {
        let t = Trailer { kind_version: 1, payload_len: 23_908_288, payload_xxh64: 0x0123_4567_89ab_cdef, source_len: 14_022_297, source_xxh64: u64::MAX };
        let mut file = vec![7u8; 5];
        file.extend(t.to_bytes());
        assert_eq!(Trailer::from_file_end(&file), Some(t));
        file[5] ^= 1;
        assert_eq!(Trailer::from_file_end(&file), None);
        assert_eq!(Trailer::from_file_end(&[0u8; TRAILER_LEN - 1]), None);
    }

    #[test]
    fn paths() {
        let c = Cache::new(Path::new("/data"));
        assert_eq!(c.dir(), Path::new("/data/cache/v1"));
        assert_eq!(c.lump_path(&core_data_rel(1)), Path::new("/data/cache/v1/wad/levels/01/core_data.lump"));
        assert_eq!(c.lump_path(&hud_bank_rel(18, 4)), Path::new("/data/cache/v1/wad/levels/18/hud_bank_4.lump"));
        assert_eq!(gameplay_rel(5), "levels/05/gameplay_ntsc.bin");
    }
}
