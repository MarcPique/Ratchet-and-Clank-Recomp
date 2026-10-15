//! `prepare`: builds the Tier 1 engine cache `<data>/cache/v1/` from the Tier 0 archive in the data folder, with the
//! engine's own cache code (`rc_data::cache`: layout, stamp, lump checks). Never needs the disc image. `extract`
//! runs it as its last stage; the launcher can also run it alone ("Rebuild cache").
//!
//! v1 content: every level's `core_data`, `gameplay_ntsc` and HUD banks, WAD-decompressed, plus `stamp.toml`
//! (docs/plan/launcher_extractor.md "Tier 1 as built"). A fresh cache is only checked: lumps whose trailer, payload
//! hash and source hash still match are kept, so a re-run is quick. A stale one (other cache or converter version,
//! other extraction) is emptied and rebuilt. Other `cache/v<N>` folders are removed.

use crate::workers::{self, Ctx};
use crate::{mib, Code, Emit, Error, Event, Stage};
use rc_data::cache::{self, Cache, Ensured, Status};
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// Free space asked for per byte of a lump not yet built: decompressed lumps are 1.7–2.8 times their WAD source
/// (measured over the 19 levels), so 3 is safe.
pub const SPACE_FACTOR: u64 = 3;

/// Totals of a `prepare` run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prepared {
    /// Lumps in the cache after the run.
    pub lumps: u64,
    /// Lumps decompressed and written by this run.
    pub built: u64,
    /// Lumps that were valid and kept.
    pub up_to_date: u64,
    /// Bytes of the Tier 0 sources.
    pub source_bytes: u64,
    /// Decompressed bytes of all lumps.
    pub lump_bytes: u64,
    /// Bytes of `cache/v1` on disk (lumps with trailers, stamp).
    pub disk_bytes: u64,
}

/// Maps a cache error to the contract's codes: unreadable Tier 0 = 10, a Tier 0 lump that does not decompress = 40
/// (the archive is damaged), a failed write = 30 (31 when the disk is full).
fn cache_error(e: rc_data::Error) -> Error {
    match e {
        rc_data::Error::Write { path, err } => workers::write_error(&path, err),
        e @ rc_data::Error::Source { .. } => Error::new(Code::CannotRead, e.to_string()),
        e @ rc_data::Error::Wad { .. } => Error::new(Code::VerifyFailed, format!("{e}; run `ratchet_clank-extract verify` and re-extract")),
    }
}

/// Builds or refreshes `<out>/cache/v1/` from the Tier 0 archive in `out`.
pub fn prepare(out: &Path, threads: usize, cancel: Option<&AtomicBool>, emit: Emit) -> Result<Prepared, Error> {
    if !out.join("toc.bin").is_file() {
        return Err(Error::new(Code::CannotRead, format!("{} is not a Ratchet & Clank Recomp data folder (no toc.bin); extract the disc first", out.display())));
    }
    let sources = match cache::v1_sources(out) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(Error::new(Code::CannotRead, format!("cannot list {}: {e}", out.join("levels").display()))),
    };
    let sizes: Vec<u64> = sources
        .iter()
        .map(|r| std::fs::metadata(out.join(r)).map(|m| m.len()))
        .collect::<Result<_, _>>()
        .map_err(|e| Error::new(Code::CannotRead, format!("cannot read the archive in {}: {e}", out.display())))?;
    let total: u64 = sizes.iter().sum();

    for old in cache::other_versions(out) {
        std::fs::remove_dir_all(&old).map_err(|e| workers::write_error(&old, e))?;
        emit(Event::Info(format!("removed the engine cache of another version ({})", old.display())));
    }
    let c = Cache::new(out);
    let writer = format!("ratchet_clank-extract {}", crate::EXTRACTOR_VERSION);
    match c.status().map_err(cache_error)? {
        Status::Fresh => emit(Event::Info(format!("checking the engine cache in {} ({} lumps)", c.dir().display(), sources.len()))),
        Status::Missing => {
            emit(Event::Info(format!("building the engine cache in {} ({} lumps)", c.dir().display(), sources.len())));
            c.reset(&writer).map_err(cache_error)?;
        }
        Status::Stale(why) => {
            emit(Event::Info(format!("the engine cache in {} is stale ({why}); rebuilding it", c.dir().display())));
            c.reset(&writer).map_err(cache_error)?;
        }
    }
    let stale = c.remove_partials().map_err(cache_error)?;
    if stale > 0 { emit(Event::Info(format!("removed {stale} unfinished cache file(s) of an interrupted run"))); }

    let need: u64 = sources.iter().zip(&sizes).filter(|(r, _)| !c.lump_path(r).is_file()).map(|(_, s)| s * SPACE_FACTOR).sum::<u64>();
    if need > 0 {
        let need = need + crate::extract::SPACE_MARGIN;
        match crate::space::available_bytes(out) {
            Some(free) if free < need => {
                return Err(Error::new(Code::NoSpace, format!(
                    "not enough free space at {} for the engine cache: {:.0} MiB needed, {:.0} MiB available", out.display(), mib(need), mib(free))));
            }
            Some(_) => {}
            None => emit(Event::Info("free disk space unknown on this platform; not checked".into())),
        }
    }

    let results = workers::run(
        sources.len(), total, Stage::Prepare, threads, cancel, emit,
        |i| sources[i].clone(),
        || Ok(()),
        |_: &mut (), i, ctx: &Ctx| {
            let r = c.ensure_lump(&sources[i]).map_err(cache_error)?;
            ctx.add(sizes[i]);
            Ok(r)
        },
    )?;
    let mut p = Prepared { lumps: results.len() as u64, source_bytes: total, ..Prepared::default() };
    for r in results {
        match r {
            Ensured::Built(n) => { p.built += 1; p.lump_bytes += n; }
            Ensured::UpToDate(n) => { p.up_to_date += 1; p.lump_bytes += n; }
        }
    }
    p.disk_bytes = c.disk_bytes();
    emit(Event::Info(format!(
        "engine cache: {} lumps ({} built, {} kept), {:.1} MiB in {}",
        p.lumps, p.built, p.up_to_date, mib(p.disk_bytes), c.dir().display()
    )));
    Ok(p)
}
