//! `ratchet_clank-extract`: identifies the user's own Ratchet & Clank disc image and writes the Tier 0 archive
//! (every lump the ToC and the filesystem reference, byte-identical to the disc, in the `extracted/` layout; every file
//! is checked against the committed size/SHA-1 table `data/<serial>.tsv`), so the disc image is never needed again. `prepare` then builds the Tier 1 engine cache
//! from that archive (`rc_data::cache`), and `extract` runs it as its last stage. `export` writes the optional
//! Tier 2 exports (PNG, WAV, glTF, JSON) from the archive ([`export`]).
//!
//! Interface: `docs/plan/launcher_contract.md` (CLI, JSON lines, exit codes, `extract-info.json`).
//! Design: `docs/plan/launcher_extractor.md`. Dependency-free by decision (in-crate SHA-1 and JSON).

pub mod archive;
pub mod build_db;
pub mod export;
pub mod extract;
pub mod identify;
pub mod json;
pub mod prepare;
/// SHA-1, shared with the loader snapshots (it lives in rc-formats).
pub use rc_formats::sha1;
pub mod space;
pub mod verify;
mod workers;

use json::Value;
use std::fmt;

/// `ratchet_clank-extract` version, recorded as `extractor_version` in `extract-info.json`.
pub const EXTRACTOR_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Version of the data folder layout (`data_format` in the manifest and `extract-info.json`).
pub const DATA_FORMAT: u32 = 1;
/// Written last by a successful `extract`; its absence marks an incomplete data folder.
pub const INFO_FILE: &str = "extract-info.json";
/// Suffix of a file still being written. Renamed away only once complete and hash-checked.
pub const PARTIAL_SUFFIX: &str = ".partial";
/// Default number of copy/hash workers.
pub const DEFAULT_THREADS: usize = 4;

/// Error codes = process exit codes (contract "Codes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    CannotRead = 10,
    NotIso = 11,
    NotRatchet = 20,
    Unsupported = 21,
    WriteFailed = 30,
    NoSpace = 31,
    VerifyFailed = 40,
    Internal = 99,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub message: String,
}

impl Error {
    pub fn new(code: Code, message: impl Into<String>) -> Self { Error { code, message: message.into() } }
    pub fn exit_code(&self) -> i32 { self.code as i32 }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "error {}: {}", self.code as i32, self.message) }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Identify,
    Copy,
    Verify,
    Prepare,
    Export,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self { Stage::Identify => "identify", Stage::Copy => "copy", Stage::Verify => "verify", Stage::Prepare => "prepare", Stage::Export => "export" }
    }
}

/// What the disc is (the contract's `disc` line plus additive fields).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscInfo {
    pub serial: String,
    pub region: String,
    pub version: String,
    pub supported: bool,
    pub game: build_db::Game,
    pub title: String,
    pub elf_sha1: String,
}

/// A progress, info or disc line. `error` and `done` are the terminal lines, written by the binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Progress { stage: Stage, done: u64, total: u64, file: String },
    Info(String),
    Disc(DiscInfo),
}

impl Event {
    pub fn to_json(&self) -> String {
        match self {
            Event::Progress { stage, done, total, file } => json::object(&[
                ("type", "progress".into()),
                ("stage", stage.name().into()),
                ("done", (*done).into()),
                ("total", (*total).into()),
                ("file", file.as_str().into()),
            ]),
            Event::Info(m) => json::object(&[("type", "info".into()), ("message", m.as_str().into())]),
            Event::Disc(d) => json::object(&[
                ("type", "disc".into()),
                ("serial", d.serial.as_str().into()),
                ("region", d.region.as_str().into()),
                ("version", d.version.as_str().into()),
                ("supported", d.supported.into()),
                ("game", d.game.id().into()),
                ("title", d.title.as_str().into()),
                ("elf_sha1", d.elf_sha1.as_str().into()),
            ]),
        }
    }
}

pub fn error_json(e: &Error) -> String {
    json::object(&[("type", "error".into()), ("code", Value::Num(e.code as i64)), ("message", e.message.as_str().into())])
}

pub fn done_json(elapsed_ms: u64) -> String { json::object(&[("type", "done".into()), ("elapsed_ms", elapsed_ms.into())]) }

/// Receives events. Called on the thread that started the operation only.
pub type Emit<'a> = &'a mut dyn FnMut(Event);

/// Totals of an `extract` or `verify` run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub files: u64,
    pub bytes: u64,
}

pub fn mib(b: u64) -> f64 { b as f64 / (1u64 << 20) as f64 }
