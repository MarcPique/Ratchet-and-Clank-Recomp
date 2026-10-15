//! Ground-truth harness: read the PS2's EE memory (PCSX2 savestates or the PINE socket) and
//! compare it with what our reimplementation computes. Doc: docs/plan/trace_harness.md,
//! tools/trace/README.md, docs/workflows/pcsx2.md.
//!
//! A dev tool (never ships). Where things live:
//! * reads game data from `extracted/` ([`default_extracted`]) and never writes there ([`ensure_not_extracted`]);
//! * writes generated output (EE dumps, reports, CSVs) to `work/trace/` ([`trace_out_dir`]);
//! * reads the user's personal material from `~/PS2/ratchet1/` ([`personal_dir`]): savestates in `savestates/`,
//!   recordings in `traces/` (`record` writes new recordings there, the one personal write).

pub mod class_census;
pub mod disc_check;
pub mod ee;
pub mod hero_analysis;
pub mod hero_record;
pub mod hero_replay;
pub mod hero_trace;
pub mod level_peek;
pub mod mips;
pub mod novalis_spawn;
pub mod overlay_diff;
pub mod pine;
pub mod port_sim;
pub mod spawn_facts;
pub mod tfrag_light_cmp;
pub mod tie_shrub_cmp;
pub mod zip;

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// Default `extracted/` directory (the workspace's, git-ignored); override with `--extracted` or `RC_EXTRACTED`.
pub fn default_extracted() -> PathBuf {
    let p = rc_formats::test_data::root();
    p.canonicalize().unwrap_or(p)
}

/// The repository root (this crate lives at `tools/trace`).
pub fn repo_root() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    p.canonicalize().unwrap_or(p)
}

fn env_dir(var: &str) -> Option<PathBuf> { std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from) }

/// Generated dev output: `RC_WORK`, else `<repo>/work` (git-ignored, rebuildable).
pub fn work_dir() -> PathBuf { env_dir("RC_WORK").unwrap_or_else(|| repo_root().join("work")) }

/// This tool's output folder, `work/trace/`.
pub fn trace_out_dir() -> PathBuf { work_dir().join("trace") }

/// The user's personal material (ISO, savestates, recordings): `RC_PERSONAL`, else `~/PS2/ratchet1`.
pub fn personal_dir() -> PathBuf {
    env_dir("RC_PERSONAL").unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join("PS2/ratchet1"))
}

/// Kept savestates: `~/PS2/ratchet1/savestates/`.
pub fn savestates_dir() -> PathBuf { personal_dir().join("savestates") }

/// Hero recordings: `~/PS2/ratchet1/traces/`.
pub fn recordings_dir() -> PathBuf { personal_dir().join("traces") }

/// The committed Novalis spawn fixture (`distill-spawn` writes it, `tests/trace/novalis_spawn.rs` reads it).
pub fn spawn_fixture() -> PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/novalis_spawn.tsv") }

/// Refuses an output path inside the game-data tree: this tool never writes into `extracted/`.
pub fn ensure_not_extracted(out: &Path) -> Result<()> {
    let abs = |p: &Path| -> PathBuf {
        let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(p) };
        // Canonicalise the longest existing ancestor (the file itself may not exist yet).
        let mut base = p.clone();
        let mut rest = Vec::new();
        while !base.exists() {
            match (base.file_name().map(|n| n.to_owned()), base.parent().map(Path::to_path_buf)) {
                (Some(n), Some(parent)) => { rest.push(n); base = parent; }
                _ => break,
            }
        }
        let mut c = base.canonicalize().unwrap_or(base);
        for n in rest.into_iter().rev() { c.push(n); }
        c
    };
    let ex = abs(&default_extracted());
    if abs(out).starts_with(&ex) {
        bail!("{} is inside the game-data folder {}; rc-trace writes generated output under {} only", out.display(), ex.display(), trace_out_dir().display());
    }
    Ok(())
}

/// Creates `out`'s parent and writes it, after [`ensure_not_extracted`].
pub fn write_output(out: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    ensure_not_extracted(out)?;
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) { std::fs::create_dir_all(d)?; }
    std::fs::write(out, bytes)?;
    Ok(())
}
