//! A small std-only worker pool for copying and hashing files, with progress reported on the calling thread.

use crate::sha1::Sha1;
use crate::{Code, Emit, Error, Event, Stage};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

/// Read/write chunk size per worker.
pub(crate) const CHUNK: usize = 4 << 20;

/// Shared state a job sees: progress counter and the stop flag (another worker failed, or the caller cancelled).
pub(crate) struct Ctx<'a> {
    done: AtomicU64,
    stop: AtomicBool,
    cancel: Option<&'a AtomicBool>,
}

impl Ctx<'_> {
    pub(crate) fn add(&self, n: u64) { self.done.fetch_add(n, Relaxed); }
    pub(crate) fn should_stop(&self) -> bool { self.stop.load(Relaxed) || self.cancel.is_some_and(|c| c.load(Relaxed)) }
    pub(crate) fn stopped() -> Error { Error::new(Code::Internal, "cancelled") }
}

/// Runs `work(state, i, ctx)` for `i in 0..n` on up to `threads` workers, each with its own `init()` state.
/// Emits `progress` lines for `stage` about every 100 ms (the `file` is `name(i)` of the job started last) and a
/// final line with `done == total`. The first error stops all workers and is returned.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run<S, R: Send>(
    n: usize,
    total: u64,
    stage: Stage,
    threads: usize,
    cancel: Option<&AtomicBool>,
    emit: Emit,
    name: impl Fn(usize) -> String + Sync,
    init: impl Fn() -> Result<S, Error> + Sync,
    work: impl Fn(&mut S, usize, &Ctx) -> Result<R, Error> + Sync,
) -> Result<Vec<R>, Error> {
    let ctx = Ctx { done: AtomicU64::new(0), stop: AtomicBool::new(false), cancel };
    let next = AtomicUsize::new(0);
    let current = Mutex::new(String::new());
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    let first_err: Mutex<Option<Error>> = Mutex::new(None);
    emit(Event::Progress { stage, done: 0, total, file: String::new() });
    std::thread::scope(|s| {
        let (tx, rx) = mpsc::channel::<()>();
        for _ in 0..threads.clamp(1, n.max(1)) {
            let (tx, ctx, next, current, results, first_err, name, init, work) = (tx.clone(), &ctx, &next, &current, &results, &first_err, &name, &init, &work);
            s.spawn(move || {
                let r = (|| -> Result<(), Error> {
                    let mut st = init()?;
                    loop {
                        if ctx.should_stop() { return Err(Ctx::stopped()); }
                        let i = next.fetch_add(1, Relaxed);
                        if i >= n { return Ok(()); }
                        *current.lock().unwrap_or_else(|e| e.into_inner()) = name(i);
                        let r = work(&mut st, i, ctx)?;
                        results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
                    }
                })();
                if let Err(e) = r {
                    ctx.stop.store(true, Relaxed);
                    first_err.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert(e);
                }
                drop(tx);
            });
        }
        drop(tx);
        while let Err(mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(Duration::from_millis(100)) {
            let file = current.lock().unwrap_or_else(|e| e.into_inner()).clone();
            emit(Event::Progress { stage, done: ctx.done.load(Relaxed).min(total), total, file });
        }
    });
    if let Some(e) = first_err.into_inner().unwrap_or_else(|e| e.into_inner()) { return Err(e); }
    let file = current.into_inner().unwrap_or_else(|e| e.into_inner());
    emit(Event::Progress { stage, done: total, total, file });
    Ok(results.into_inner().unwrap_or_else(|e| e.into_inner()).into_iter().map(|r| r.expect("every job ran")).collect())
}

/// Streams `len` bytes from `src` through SHA-1 into `sink`, counting progress. `read_err`/`sink` map I/O errors.
pub(crate) fn stream(
    src: &mut impl Read,
    len: u64,
    buf: &mut [u8],
    ctx: &Ctx,
    read_err: impl Fn(std::io::Error) -> Error,
    mut sink: impl FnMut(&[u8]) -> Result<(), Error>,
) -> Result<[u8; 20], Error> {
    let mut h = Sha1::new();
    let mut left = len;
    while left > 0 {
        if ctx.should_stop() { return Err(Ctx::stopped()); }
        let n = left.min(buf.len() as u64) as usize;
        src.read_exact(&mut buf[..n]).map_err(&read_err)?;
        h.update(&buf[..n]);
        sink(&buf[..n])?;
        ctx.add(n as u64);
        left -= n as u64;
    }
    Ok(h.finish())
}

/// Maps a write-side I/O error: disk full → 31, anything else → 30.
pub(crate) fn write_error(what: &std::path::Path, e: std::io::Error) -> Error {
    if e.kind() == std::io::ErrorKind::StorageFull {
        Error::new(Code::NoSpace, format!("disk full while writing {}", what.display()))
    } else {
        Error::new(Code::WriteFailed, format!("cannot write {}: {e}", what.display()))
    }
}

pub(crate) fn write_all(f: &mut std::fs::File, path: &std::path::Path, b: &[u8]) -> Result<(), Error> { f.write_all(b).map_err(|e| write_error(path, e)) }
