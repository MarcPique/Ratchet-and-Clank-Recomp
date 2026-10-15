//! `extract`: identify, then copy every Tier 0 file from the image into the data folder, hashing each file while
//! it is copied and checking it against the build's table, then write `extract-info.json`, then build the Tier 1
//! engine cache (`prepare`, stage `prepare`).
//!
//! Crash safety (the launcher cancels by killing the process): `extract-info.json` is removed first and written
//! last; every file is written as `<name>.partial` and renamed only when complete and hash-checked.

use crate::archive::{self, Job};
use crate::build_db::{self, Build, Game, TableEntry};
use crate::identify::{self, Identified};
use crate::workers::{self, Ctx, CHUNK};
use crate::{json, mib, space, Code, Emit, Error, Event, Stage, Summary, DATA_FORMAT, EXTRACTOR_VERSION, INFO_FILE, PARTIAL_SUFFIX};
use rc_formats::disc::Disc;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// Free space required on top of the bytes to write.
pub const SPACE_MARGIN: u64 = 64 << 20;

pub struct Options<'a> {
    /// Skip the PAL copies (`archive::is_pal_copy`).
    pub ntsc_only: bool,
    pub threads: usize,
    /// Cooperative cancel for library callers (the binary is cancelled by being killed).
    pub cancel: Option<&'a AtomicBool>,
}

impl Default for Options<'_> {
    fn default() -> Self { Options { ntsc_only: false, threads: crate::DEFAULT_THREADS, cancel: None } }
}

pub(crate) fn partial_path(dest: &Path) -> PathBuf {
    let mut s = OsString::from(dest.as_os_str());
    s.push(PARTIAL_SUFFIX);
    PathBuf::from(s)
}

fn read_error(iso: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => Error::new(Code::CannotRead, format!("{} ends early (truncated image)", iso.display())),
        _ => Error::new(Code::CannotRead, format!("cannot read {}: {e}", iso.display())),
    }
}

/// Deletes `*.partial` files left under `dir` by a killed run.
fn remove_stale_partials(dir: &Path) -> Result<usize, Error> {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                Ok(_) if p.to_string_lossy().ends_with(PARTIAL_SUFFIX) => {
                    fs::remove_file(&p).map_err(|e| workers::write_error(&p, e))?;
                    n += 1;
                }
                _ => {}
            }
        }
    }
    Ok(n)
}

/// The table of a supported build.
pub(crate) fn build_table(b: &Build) -> Result<Vec<TableEntry>, Error> {
    let text = b.table.ok_or_else(|| Error::new(Code::Internal, format!("build {} v{} has no file table", b.serial, b.version)))?;
    build_db::parse_table(text).map_err(|e| Error::new(Code::Internal, format!("built-in table for {}: {e}", b.serial)))
}

/// Copies the jobs from the image to `out`. Returns the problems (size/hash mismatches), empty on success.
fn copy_files(iso_path: &Path, out: &Path, jobs: &[Job], threads: usize, cancel: Option<&AtomicBool>, emit: Emit) -> Result<Vec<String>, Error> {
    let total = jobs.iter().map(|j| j.bytes).sum();
    let results = workers::run(
        jobs.len(), total, Stage::Copy, threads, cancel, emit,
        |i| jobs[i].path.clone(),
        || Ok((File::open(iso_path).map_err(|e| Error::new(Code::CannotRead, format!("cannot open {}: {e}", iso_path.display())))?, vec![0u8; CHUNK])),
        |st: &mut (File, Vec<u8>), i, ctx: &Ctx| {
            let (iso, buf) = st;
            let job = &jobs[i];
            let dest = out.join(&job.path);
            let part = partial_path(&dest);
            if let Some(dir) = dest.parent() { fs::create_dir_all(dir).map_err(|e| workers::write_error(dir, e))?; }
            let mut f = File::create(&part).map_err(|e| workers::write_error(&part, e))?;
            let copied = iso.seek(SeekFrom::Start(job.offset)).map_err(read_error(iso_path))
                .and_then(|_| workers::stream(iso, job.bytes, buf, ctx, read_error(iso_path), |b| workers::write_all(&mut f, &part, b)));
            drop(f);
            let got = match copied {
                Ok(h) => h,
                Err(e) => { let _ = fs::remove_file(&part); return Err(e); }
            };
            if job.sha1.is_some_and(|want| want != got) {
                let _ = fs::remove_file(&part);
                return Ok(Some(format!("{}: SHA-1 {} differs from the table", job.path, crate::sha1::hex(&got))));
            }
            fs::rename(&part, &dest).map_err(|e| workers::write_error(&dest, e))?;
            Ok(None)
        },
    )?;
    Ok(results.into_iter().flatten().collect())
}

/// Reports up to 50 problems as info lines and returns the code-40 error.
pub(crate) fn fail_with(problems: &[String], what: &str, emit: Emit) -> Error {
    for p in problems.iter().take(50) { emit(Event::Info(p.clone())); }
    if problems.len() > 50 { emit(Event::Info(format!("… and {} more", problems.len() - 50))); }
    Error::new(Code::VerifyFailed, format!("{} {what}", problems.len()))
}

/// Full extraction into `out` (the launcher's `games/rac1/data/`).
pub fn extract(iso_path: &Path, out: &Path, builds: &[Build], opts: &Options, emit: Emit) -> Result<(Identified, Summary), Error> {
    let (id, iso) = identify::identify(iso_path, builds, emit)?;
    let build = &builds[id.build.expect("supported build")];
    let table = build_table(build)?;
    let disc = Disc::new(iso).map_err(|e| Error::new(Code::CannotRead, format!("cannot read the disc's table of contents: {e}")))?;
    let jobs = archive::with_table(archive::disc_jobs(&disc)?, &table, opts.ntsc_only)?;
    drop(disc);
    let summary = Summary { files: jobs.len() as u64, bytes: jobs.iter().map(|j| j.bytes).sum() };

    fs::create_dir_all(out).map_err(|e| workers::write_error(out, e))?;
    let info = out.join(INFO_FILE);
    if info.exists() { fs::remove_file(&info).map_err(|e| workers::write_error(&info, e))?; }
    let stale = remove_stale_partials(out)?;
    if stale > 0 { emit(Event::Info(format!("removed {stale} unfinished file(s) of an interrupted extraction"))); }

    // Existing files are replaced in place, so only the growth needs free space.
    let need: u64 = jobs.iter().map(|j| j.bytes.saturating_sub(fs::metadata(out.join(&j.path)).map(|m| m.len()).unwrap_or(0))).sum::<u64>() + SPACE_MARGIN;
    match space::available_bytes(out) {
        Some(free) if free < need => {
            return Err(Error::new(Code::NoSpace, format!(
                "not enough free space at {}: {:.0} MiB needed, {:.0} MiB available", out.display(), mib(need), mib(free))));
        }
        Some(_) => {}
        None => emit(Event::Info("free disk space unknown on this platform; not checked".into())),
    }
    emit(Event::Info(format!(
        "extracting {} files ({:.1} MiB{}) to {}",
        summary.files, mib(summary.bytes), if opts.ntsc_only { ", PAL copies skipped" } else { "" }, out.display())));

    let problems = copy_files(iso_path, out, &jobs, opts.threads, opts.cancel, emit)?;
    if !problems.is_empty() { return Err(fail_with(&problems, "file(s) differ from the reference: the disc image is damaged; re-dump the disc", emit)); }

    let text = json::object(&[
        ("disc", id.serial.as_str().into()),
        ("data_format", DATA_FORMAT.into()),
        ("extractor_version", EXTRACTOR_VERSION.into()),
        ("ntsc_only", opts.ntsc_only.into()),
        ("files", summary.files.into()),
        ("bytes", summary.bytes.into()),
    ]) + "\n";
    let tmp = partial_path(&info);
    fs::write(&tmp, text).map_err(|e| workers::write_error(&tmp, e))?;
    fs::rename(&tmp, &info).map_err(|e| workers::write_error(&info, e))?;

    // Tier 1 after the Tier 0 archive is complete (its stamp hashes `extract-info.json`). The cache is optional and
    // rebuilt from Tier 0 at any time (the game builds missing lumps itself), so a failure here does not fail the
    // extraction: one info line says so. Cancelling still cancels.
    if let Err(e) = crate::prepare::prepare(out, opts.threads, opts.cancel, emit) {
        if opts.cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) { return Err(e); }
        emit(Event::Info(format!(
            "the engine cache was not built ({}); the game builds it on first start, or run `ratchet_clank-extract prepare --out {}`",
            e.message, out.display())));
    }
    Ok((id, summary))
}

/// `table`: hashes every archive file straight from an RC1 image (any RC1 build whose ToC parses; nothing is
/// written) and returns the rows of its size/SHA-1 table.
pub fn table(iso_path: &Path, builds: &[Build], threads: usize, emit: Emit) -> Result<(Identified, Vec<TableEntry>), Error> {
    let (id, iso) = identify::probe(iso_path, builds, emit)?;
    if id.game != Game::Rac1 { identify::require_supported(&id, builds, emit)?; }
    let disc = Disc::new(iso).map_err(|e| Error::new(Code::CannotRead, format!("cannot read the disc's table of contents: {e}")))?;
    let jobs = archive::disc_jobs(&disc)?;
    drop(disc);
    let total = jobs.iter().map(|j| j.bytes).sum();
    let hashes = workers::run(
        jobs.len(), total, Stage::Copy, threads, None, emit,
        |i| jobs[i].path.clone(),
        || Ok((File::open(iso_path).map_err(|e| Error::new(Code::CannotRead, format!("cannot open {}: {e}", iso_path.display())))?, vec![0u8; CHUNK])),
        |st: &mut (File, Vec<u8>), i, ctx: &Ctx| {
            let (iso, buf) = st;
            iso.seek(SeekFrom::Start(jobs[i].offset)).map_err(read_error(iso_path))?;
            workers::stream(iso, jobs[i].bytes, buf, ctx, read_error(iso_path), |_| Ok(()))
        },
    )?;
    Ok((id, jobs.into_iter().zip(hashes).map(|(j, sha1)| TableEntry { path: j.path, size: j.bytes, sha1 }).collect()))
}
