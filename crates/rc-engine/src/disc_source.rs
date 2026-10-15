//! Where the engine's game data comes from: one **data folder**, the Tier 0 archive `ratchet_clank-extract` writes
//! (`docs/plan/launcher_extractor.md` §4.1; the development `extracted/` tree has the same layout). The engine
//! never opens a disc image: the disc reader (`rc_formats::disc`) is the extractor's alone.
//!
//! Every request uses the archive's relative file name (`levels/NN/core_data.bin`, `boot/SCUS_971.99`, `toc.bin`,
//! `levels/NN/{music,speech,scene}/…`, `global/spaceships/NNN.bin`, `global/save_game.bin`).
//!
//! **Data root** ([`startup`], once per process; launcher contract `docs/plan/launcher_contract.md`):
//! 1. `--data-dir <dir>` (or `--data-dir=<dir>`), how the launcher starts the game;
//! 2. `RC_DATA_DIR`, equivalent;
//! 3. `RC_EXTRACTED` (development override);
//! 4. an adjacent `Data` directory (portable Windows install);
//! 5. `<workspace>/extracted` (development default).
//!
//! The folder must exist and hold `toc.bin`. `<data>/extract-info.json` (written last by the extractor) must be
//! present for 1 and 2, and its `data_format` must equal [`DATA_FORMAT`]; the development tree may lack it (one
//! warning). A failure prints one `error:` line naming the folder and the fix (re-extract via the launcher) and
//! exits with [`EXIT_NO_DATA`] or [`EXIT_DATA_FORMAT`], before any window opens.
//!
//! `--version-json` prints [`version_json`] and exits before anything else (no window, no data access).

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The data-folder layout this runtime reads (launcher contract `data_format`; `extract-info.json`).
pub const DATA_FORMAT: u64 = 1;
/// Exit code: bad command line (`--data-dir` without a value).
pub const EXIT_USAGE: i32 = 2;
/// Exit code: the data folder is missing, is not a data folder, or its extraction is incomplete.
pub const EXIT_NO_DATA: i32 = 3;
/// Exit code: the data folder was written for another `data_format` (or its `extract-info.json` is unreadable).
pub const EXIT_DATA_FORMAT: i32 = 4;

/// The launcher contract's `--version-json` line.
pub fn version_json() -> String {
    format!(r#"{{"name":"ratchet_clank","version":"{}","game":"rac1","data_format":{DATA_FORMAT}}}"#, env!("CARGO_PKG_VERSION"))
}

/// Which rule chose the data root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin { Arg, Env, Portable, Dev }

impl Origin {
    fn describe(self) -> &'static str {
        match self { Origin::Arg => "--data-dir", Origin::Env => "RC_DATA_DIR", Origin::Portable => "adjacent Data folder", Origin::Dev => "development default" }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Args { VersionJson, Run { data_dir: Option<PathBuf>, ignored: Vec<String> } }

/// The runtime's own arguments. Unknown arguments are returned for one warning line (the OS or a future launcher
/// may pass some), not rejected.
fn parse_args(args: impl IntoIterator<Item = OsString>) -> std::result::Result<Args, String> {
    let (mut data_dir, mut ignored) = (None, Vec::new());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let s = a.to_string_lossy();
        if s == "--version-json" { return Ok(Args::VersionJson); }
        if s == "--data-dir" {
            let v = it.next().filter(|v| !v.is_empty()).ok_or("usage: --data-dir <folder> needs a folder")?;
            data_dir = Some(PathBuf::from(v));
        } else if let Some(v) = s.strip_prefix("--data-dir=") {
            if v.is_empty() { return Err("usage: --data-dir=<folder> needs a folder".into()); }
            data_dir = Some(PathBuf::from(v));
        } else {
            ignored.push(s.into_owned());
        }
    }
    Ok(Args::Run { data_dir, ignored })
}

fn nonempty(v: Option<OsString>) -> Option<PathBuf> { v.filter(|v| !v.is_empty()).map(PathBuf::from) }

/// The resolution order (module docs), with the environment passed in.
fn resolve(arg: Option<PathBuf>, env_data_dir: Option<OsString>, env_extracted: Option<OsString>, portable: Option<PathBuf>) -> (PathBuf, Origin) {
    if let Some(p) = arg { return (p, Origin::Arg); }
    if let Some(p) = nonempty(env_data_dir) { return (p, Origin::Env); }
    if let Some(p) = nonempty(env_extracted) { return (p, Origin::Dev); }
    if let Some(p) = portable.filter(|p| p.is_dir()) { return (p, Origin::Portable); }
    (dev_extracted(), Origin::Dev)
}

fn portable_data() -> Option<PathBuf> { Some(std::env::current_exe().ok()?.parent()?.join("Data")) }

/// `<workspace>/extracted` (resolved from this crate's manifest dir at compile time, so `cargo dev` works from any
/// cwd).
fn dev_extracted() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted") }

/// A data-folder check failure: the process exit code and the message.
#[derive(Debug)]
struct DataError { code: i32, message: String }

/// The unsigned integer value of `"key"` in a flat JSON object (enough for `extract-info.json`).
fn json_uint(text: &str, key: &str) -> Option<u64> {
    let rest = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The string value of `"key"` in a flat JSON object (no escapes needed for the fields read here).
fn json_str<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = &text[text.find(&format!("\"{key}\""))? + key.len() + 2..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// Checks the data folder; `Ok` carries the one line to print (a summary, or the development-tree warning).
fn check(root: &Path, origin: Origin) -> std::result::Result<String, DataError> {
    let fix = "Extract your disc with the Ratchet & Clank Recomp launcher (or `ratchet_clank-extract extract --iso <image> --out <folder>`) \
               and start the game from it, or pass --data-dir <folder>.";
    let no_data = |what: String| DataError { code: EXIT_NO_DATA, message: format!("{what} {fix}") };
    if !root.is_dir() {
        return Err(no_data(format!("the game data folder {} ({}) does not exist.", root.display(), origin.describe())));
    }
    if !root.join("toc.bin").is_file() {
        return Err(no_data(format!("{} ({}) is not a Ratchet & Clank Recomp game data folder (no toc.bin).", root.display(), origin.describe())));
    }
    let info_path = root.join("extract-info.json");
    let info = match std::fs::read_to_string(&info_path) {
        Ok(t) => t,
        Err(_) if origin == Origin::Dev => {
            return Ok(format!("warning: no extract-info.json in {} (development tree; data format not checked)", root.display()));
        }
        Err(_) => {
            return Err(no_data(format!(
                "the extraction in {} ({}) is incomplete (no extract-info.json).",
                root.display(), origin.describe()
            )));
        }
    };
    match json_uint(&info, "data_format") {
        Some(DATA_FORMAT) => Ok(format!(
            "data: {} ({}; disc {}, data_format {DATA_FORMAT}, extractor {}, {} files)",
            root.display(), origin.describe(), json_str(&info, "disc").unwrap_or("?"),
            json_str(&info, "extractor_version").unwrap_or("?"), json_uint(&info, "files").map_or("?".into(), |n| n.to_string())
        )),
        found => Err(DataError {
            code: EXIT_DATA_FORMAT,
            message: format!(
                "the game data in {} has data format {}, but this Ratchet & Clank Recomp build needs data format {DATA_FORMAT}. \
                 Re-extract your disc via the Ratchet & Clank Recomp launcher (or ratchet_clank-extract from this build).",
                root.display(), found.map_or("<unreadable>".into(), |n| n.to_string())
            ),
        }),
    }
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The process start (`main`, first line): handles `--version-json`, resolves and checks the data root, and
/// remembers it for [`data_root`]. Prints and exits on any failure; never returns without a usable folder.
pub fn startup() -> PathBuf {
    let args = match parse_args(std::env::args_os().skip(1)) {
        Ok(a) => a,
        Err(e) => { crate::user_error::report(&e); std::process::exit(EXIT_USAGE) }
    };
    let (arg, ignored) = match args {
        Args::VersionJson => { println!("{}", version_json()); std::process::exit(0) }
        Args::Run { data_dir, ignored } => (data_dir, ignored),
    };
    if !ignored.is_empty() { eprintln!("warning: ignoring unknown arguments {ignored:?}"); }
    for var in ["RC_ISO", "RC_SOURCE"] {
        if std::env::var_os(var).is_some_and(|v| !v.is_empty()) {
            println!("note: {var} is ignored; the engine reads only the game data folder (disc images: ratchet_clank-extract)");
        }
    }
    let (root, origin) = resolve(arg, std::env::var_os("RC_DATA_DIR"), std::env::var_os("RC_EXTRACTED"), portable_data());
    match check(&root, origin) {
        Ok(line) => println!("{line}"),
        Err(e) => { crate::user_error::report(&e.message); std::process::exit(e.code) }
    }
    ROOT.get_or_init(|| root).clone()
}

/// The data root chosen by [`startup`]; before it (unit tests), the same order without `--data-dir` or checks.
pub fn data_root() -> PathBuf {
    ROOT.get().cloned().unwrap_or_else(|| {
        // Unit tests only (the game sets ROOT at startup).
        resolve(None, std::env::var_os("RC_DATA_DIR"), std::env::var_os("RC_EXTRACTED"), portable_data()).0
    })
}

/// The bytes of `rel` (a path relative to the data root, as the extractor names it).
pub fn read(root: &Path, rel: &str) -> Result<Vec<u8>> {
    let p = root.join(rel);
    std::fs::read(&p).with_context(|| format!("reading {}", p.display()))
}

/// `levels/NN/<name>`: `core_index.bin`, `core_data.bin` (WAD-compressed), `gs_ram.bin`, `gameplay_ntsc.bin`, ...
pub fn level_file(root: &Path, index: u32, name: &str) -> Result<Vec<u8>> { read(root, &format!("levels/{index:02}/{name}")) }

/// `read` for a full path under `root` (e.g. `root.join("boot/SCUS_971.99")`).
pub fn read_path(root: &Path, p: &Path) -> Result<Vec<u8>> {
    match p.strip_prefix(root) {
        Ok(rel) => read(root, &rel.to_string_lossy().replace('\\', "/")),
        Err(_) => std::fs::read(p).with_context(|| format!("reading {}", p.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> { v.iter().map(OsString::from).collect() }

    #[test]
    fn version_json_is_the_contract_line() {
        let v = version_json();
        assert_eq!(v, format!(r#"{{"name":"ratchet_clank","version":"{}","game":"rac1","data_format":1}}"#, env!("CARGO_PKG_VERSION")));
        assert_eq!(json_uint(&v, "data_format"), Some(1));
        assert_eq!(json_str(&v, "game"), Some("rac1"));
    }

    #[test]
    fn arguments() {
        assert_eq!(parse_args(os(&[])), Ok(Args::Run { data_dir: None, ignored: vec![] }));
        assert_eq!(parse_args(os(&["--data-dir", "/d"])), Ok(Args::Run { data_dir: Some("/d".into()), ignored: vec![] }));
        assert_eq!(parse_args(os(&["--data-dir=/a b", "-psn_0_1"])),
            Ok(Args::Run { data_dir: Some("/a b".into()), ignored: vec!["-psn_0_1".into()] }));
        assert_eq!(parse_args(os(&["--data-dir", "/d", "--version-json"])), Ok(Args::VersionJson));
        assert!(parse_args(os(&["--data-dir"])).is_err());
        assert!(parse_args(os(&["--data-dir="])).is_err());
    }

    #[test]
    fn resolution_order() {
        let e = |s: &str| Some(OsString::from(s));
        let portable = temp("portable");
        assert_eq!(resolve(Some("/arg".into()), e("/env"), e("/ex"), Some(portable.clone())), ("/arg".into(), Origin::Arg));
        assert_eq!(resolve(None, e("/env"), e("/ex"), Some(portable.clone())), ("/env".into(), Origin::Env));
        assert_eq!(resolve(None, e(""), e("/ex"), Some(portable.clone())), ("/ex".into(), Origin::Dev));
        assert_eq!(resolve(None, None, None, Some(portable.clone())), (portable.clone(), Origin::Portable));
        assert_eq!(resolve(None, None, None, None), (dev_extracted(), Origin::Dev));
        std::fs::remove_dir_all(portable).unwrap();
    }

    /// A fresh empty folder under the system temp dir.
    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ratchet_clank-disc-source-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn folder_checks() {
        let d = temp("checks");
        let missing = d.join("nope");
        assert_eq!(check(&missing, Origin::Arg).unwrap_err().code, EXIT_NO_DATA);
        assert_eq!(check(&missing, Origin::Dev).unwrap_err().code, EXIT_NO_DATA);
        assert_eq!(check(&d, Origin::Env).unwrap_err().code, EXIT_NO_DATA, "no toc.bin");
        std::fs::write(d.join("toc.bin"), [0u8; 4]).unwrap();
        // No extract-info.json: an incomplete extraction, except in the development tree.
        let e = check(&d, Origin::Arg).unwrap_err();
        assert!(e.code == EXIT_NO_DATA && e.message.contains("incomplete"), "{}", e.message);
        assert!(check(&d, Origin::Dev).unwrap().starts_with("warning:"));
        let info = |f: &str| std::fs::write(d.join("extract-info.json"),
            format!(r#"{{"disc":"SCUS_971.99","data_format":{f},"extractor_version":"0.1.0","ntsc_only":false,"files":2937,"bytes":1}}"#)).unwrap();
        info("1");
        let ok = check(&d, Origin::Arg).unwrap();
        assert!(ok.contains("SCUS_971.99") && ok.contains("2937 files"), "{ok}");
        info("2");
        for o in [Origin::Arg, Origin::Env, Origin::Dev] {
            let e = check(&d, o).unwrap_err();
            assert!(e.code == EXIT_DATA_FORMAT && e.message.contains("data format 2") && e.message.contains("launcher"), "{}", e.message);
        }
        std::fs::write(d.join("extract-info.json"), "garbage").unwrap();
        assert_eq!(check(&d, Origin::Arg).unwrap_err().code, EXIT_DATA_FORMAT);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn reads_are_relative_to_the_root() {
        let d = temp("read");
        std::fs::create_dir_all(d.join("levels/01")).unwrap();
        std::fs::write(d.join("levels/01/gs_ram.bin"), b"abc").unwrap();
        assert_eq!(level_file(&d, 1, "gs_ram.bin").unwrap(), b"abc");
        assert_eq!(read_path(&d, &d.join("levels/01/gs_ram.bin")).unwrap(), b"abc");
        assert!(read(&d, "toc.bin").is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }
}
