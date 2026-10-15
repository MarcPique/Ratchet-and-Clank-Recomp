//! `cargo xtask sweep [--limit <size>] [--dry-run]`: keeps `target/` under a size limit (default 30 GB) with
//! cargo-sweep (`cargo install cargo-sweep --locked`), which removes the least recently used build units first.
//!
//! cargo-sweep's `--maxsize` (0.8.0): a bare number is MiB; `B`, `MB`, `GB`, `TB` are decimal and `KiB`… `TiB` binary.
//! This command always passes the exact byte count (`<n>B`). It counts file bytes (`len`, not disk blocks) of the whole
//! target folder, and ranks each unit (one `.fingerprint/<name>-<hash>` folder with its `deps/`, `build/` and profile
//! files) by the newest access time (atime) of its fingerprint files, which cargo reads on every build that uses the
//! unit. It never removes `incremental/` (not tracked by fingerprints) or files without a unit hash (the final
//! `target/debug/ratchet_clank`, `libbevy_dylib.dylib`); `cargo clean` is the full reset.
//!
//! Safety: it refuses while any other `cargo`, `rustc` or `cargo-nextest` process runs (it must never delete files
//! under a running build); the `cargo` that runs this xtask (an ancestor process) does not count. Without cargo-sweep
//! it prints the install line and exits 0. `test-full` runs it at the end and only warns when it fails.

use crate::{cargo, repo_root};
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

/// The default limit: 30 GB (decimal, as cargo-sweep's `GB`).
pub const DEFAULT_LIMIT: u64 = 30_000_000_000;

/// The process names that mean a build or test run may be using `target/`.
const BUSY_NAMES: &[&str] = &["cargo", "rustc", "cargo-nextest"];

const INSTALL: &str = "cargo install cargo-sweep --locked";

#[derive(Debug, PartialEq, Eq)]
pub struct Opts {
    pub limit: u64,
    pub dry_run: bool,
}

impl Default for Opts {
    fn default() -> Self { Opts { limit: DEFAULT_LIMIT, dry_run: false } }
}

fn parse(argv: &[OsString]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        let a = a.to_str().ok_or_else(|| format!("unexpected argument {a:?}"))?;
        if a == "--dry-run" || a == "-n" {
            o.dry_run = true;
        } else if a == "--limit" {
            let v = it.next().and_then(|v| v.to_str()).ok_or("--limit needs a size, e.g. --limit 30GB")?;
            o.limit = parse_size(v)?;
        } else if let Some(v) = a.strip_prefix("--limit=") {
            o.limit = parse_size(v)?;
        } else {
            return Err(format!("unknown argument {a}"));
        }
    }
    Ok(o)
}

/// A size such as `30GB`, `20G`, `1.5TB`, `500MB`, `512MiB` in bytes. `K`/`KB`, `M`/`MB`, `G`/`GB`, `T`/`TB` are
/// decimal (as cargo-sweep reads them), `KiB`…`TiB` binary, `B` bytes; case-insensitive, a space before the unit is
/// allowed. A bare number is refused (cargo-sweep would read it as MiB, `du -h` as bytes: ambiguous).
pub fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (num, unit) = (&s[..split], s[split..].trim());
    let bad = || format!("bad size {s:?}: give a number and a unit, e.g. 30GB, 20G, 500MB");
    let n: f64 = num.parse().map_err(|_| bad())?;
    let mult: u64 = match unit.to_ascii_lowercase().as_str() {
        "b" => 1,
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        "t" | "tb" => 1_000_000_000_000,
        "kib" => 1 << 10,
        "mib" => 1 << 20,
        "gib" => 1 << 30,
        "tib" => 1 << 40,
        _ => return Err(bad()),
    };
    let bytes = n * mult as f64;
    if !(bytes.is_finite() && bytes >= 1.0 && bytes < u64::MAX as f64) { return Err(bad()); }
    Ok(bytes.round() as u64)
}

/// cargo-sweep's `--maxsize` value for a limit in bytes: the exact byte count (`<n>B`; a bare number would be MiB).
pub fn maxsize_arg(bytes: u64) -> String { format!("{bytes}B") }

/// Bytes as decimal GB/MB (the units of `--limit`).
pub fn human(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 { format!("{:.2} GB", b / 1e9) } else if b >= 1e6 { format!("{:.1} MB", b / 1e6) } else { format!("{bytes} B") }
}

/// One process: (pid, parent pid, executable name).
pub type Proc = (u32, u32, String);

/// The processes of `ps -axo pid=,ppid=,comm=` (on macOS `comm` is the executable path: the name is its last part).
fn processes() -> Result<Vec<Proc>, String> {
    let out = Command::new("ps").args(["-axo", "pid=,ppid=,comm="]).output().map_err(|e| format!("cannot run ps: {e}"))?;
    if !out.status.success() { return Err("ps failed".into()); }
    Ok(parse_ps(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_ps(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.trim_start().splitn(2, char::is_whitespace);
            let pid = f.next()?.parse().ok()?;
            let rest = f.next()?.trim_start();
            let mut g = rest.splitn(2, char::is_whitespace);
            let ppid = g.next()?.parse().ok()?;
            let comm = g.next()?.trim();
            let name = comm.rsplit('/').next().unwrap_or(comm);
            Some((pid, ppid, name.to_string()))
        })
        .collect()
}

/// The build processes that block a sweep: those named in [`BUSY_NAMES`], except `me` and its ancestors (the
/// `cargo run` of the xtask alias, and a rustup proxy above it).
pub fn busy(procs: &[Proc], me: u32) -> Vec<(u32, String)> {
    let mut ancestors = vec![me];
    let mut cur = me;
    while let Some(&(_, ppid, _)) = procs.iter().find(|p| p.0 == cur) {
        if ppid == 0 || ancestors.contains(&ppid) { break; }
        ancestors.push(ppid);
        cur = ppid;
    }
    procs.iter().filter(|p| BUSY_NAMES.contains(&p.2.as_str()) && !ancestors.contains(&p.0)).map(|p| (p.0, p.2.clone())).collect()
}

/// Whether `cargo sweep` answers.
fn installed() -> bool {
    cargo().args(["sweep", "--version"]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// What the preflight decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    Go,
    /// cargo-sweep is not installed: print the install line, exit 0.
    NotInstalled,
    /// Build processes run: refuse, exit 1.
    Busy(Vec<(u32, String)>),
    /// The process list could not be read: refuse (a build might run), exit 1.
    Unknown(String),
}

pub fn gate(installed: bool, procs: Result<Vec<Proc>, String>, me: u32) -> Gate {
    if !installed { return Gate::NotInstalled; }
    match procs {
        Err(e) => Gate::Unknown(e),
        Ok(p) => {
            let b = busy(&p, me);
            if b.is_empty() { Gate::Go } else { Gate::Busy(b) }
        }
    }
}

/// The file bytes under `dir` (symlinks not followed), as cargo-sweep counts them.
pub fn dir_bytes(dir: &Path) -> u64 {
    let Ok(md) = std::fs::symlink_metadata(dir) else { return 0 };
    if !md.is_dir() { return if md.is_file() { md.len() } else { 0 }; }
    std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| dir_bytes(&e.path())).sum()).unwrap_or(0)
}

/// The units named in cargo-sweep's `-v` lines for removed `.fingerprint/<name>-<hash>` folders: the names, sorted,
/// deduplicated (a crate built with two feature sets or profiles is one name).
pub fn removed_units(log: &str) -> (usize, Vec<String>) {
    let mut paths = 0;
    let mut names: Vec<String> = Vec::new();
    for l in log.lines() {
        if !(l.contains("Would remove: ") || l.contains("Successfully removed: ")) { continue; }
        paths += 1;
        let p = l.rsplit(": ").next().unwrap_or("").trim().trim_matches('"');
        let p = Path::new(p);
        if p.parent().and_then(|d| d.file_name()).is_some_and(|n| n == ".fingerprint") {
            let f = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let name = f.rsplit_once('-').map_or(f.as_str(), |(n, _)| n).to_string();
            if !names.contains(&name) { names.push(name); }
        }
    }
    names.sort();
    (paths, names)
}

/// Runs the sweep; `true` when it swept, had nothing to do, or cargo-sweep is missing; `false` when it refused or
/// failed.
pub fn run(o: &Opts) -> bool {
    let have = installed();
    // The process list only matters when cargo-sweep is there.
    let procs = if have { processes() } else { Ok(Vec::new()) };
    match gate(have, procs, std::process::id()) {
        Gate::NotInstalled => {
            eprintln!("xtask: cargo-sweep is not installed, so target/ is not swept. Install it once with:\n  {INSTALL}");
            return true;
        }
        Gate::Unknown(e) => {
            eprintln!("xtask: sweep refused: cannot list processes ({e}), so a build might be running");
            return false;
        }
        Gate::Busy(b) => {
            let list: Vec<String> = b.iter().map(|(pid, n)| format!("{n} ({pid})")).collect();
            eprintln!("xtask: sweep refused: a build or test run is active: {}. Run it again when they have finished.", list.join(", "));
            return false;
        }
        Gate::Go => {}
    }
    let target = repo_root().join("target");
    let before = dir_bytes(&target);
    let verb = if o.dry_run { "would remove" } else { "removed" };
    eprintln!("xtask: target/ holds {} (file bytes); limit {}", human(before), human(o.limit));
    let mut c = cargo();
    c.args(["sweep", "-v", "--maxsize", &maxsize_arg(o.limit)]).current_dir(repo_root());
    if o.dry_run { c.arg("--dry-run"); }
    eprintln!("xtask: cargo sweep --maxsize {}{}", maxsize_arg(o.limit), if o.dry_run { " --dry-run" } else { "" });
    let out = match c.output() {
        Ok(out) => out,
        Err(e) => { eprintln!("xtask: cannot run cargo sweep: {e}"); return false; }
    };
    let log = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    // cargo-sweep's summary lines (its -v debug lines are only parsed).
    for l in log.lines().filter(|l| l.starts_with("[INFO]") || l.starts_with("[WARN]") || l.starts_with("[ERROR]") || l.starts_with("Error")) {
        eprintln!("  {l}");
    }
    if !out.status.success() { eprintln!("xtask: cargo sweep failed ({})", out.status); return false; }
    let (paths, names) = removed_units(&log);
    if paths == 0 {
        eprintln!("xtask: {verb} nothing: target/ is under the limit");
    } else {
        let shown: Vec<&str> = names.iter().take(25).map(String::as_str).collect();
        let more = if names.len() > shown.len() { format!(", … ({} more)", names.len() - shown.len()) } else { String::new() };
        eprintln!("xtask: {verb} {paths} files/folders of {} build units: {}{more}", names.len(), shown.join(", "));
    }
    if !o.dry_run {
        let after = dir_bytes(&target);
        eprintln!("xtask: target/ {} -> {} (removed {})", human(before), human(after), human(before.saturating_sub(after)));
    }
    true
}

/// `cargo xtask sweep [--limit <size>] [--dry-run]`.
pub fn main(argv: &[OsString]) -> ExitCode {
    let o = match parse(argv) {
        Ok(o) => o,
        Err(e) => { eprintln!("error: {e}\n\n{}", crate::HELP); return ExitCode::from(2); }
    };
    if run(&o) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> { v.iter().map(OsString::from).collect() }

    #[test]
    fn parses_sizes_and_units() {
        assert_eq!(parse_size("30GB"), Ok(30_000_000_000));
        assert_eq!(parse_size("20G"), Ok(20_000_000_000));
        assert_eq!(parse_size("20g"), Ok(20_000_000_000));
        assert_eq!(parse_size("1.5TB"), Ok(1_500_000_000_000));
        assert_eq!(parse_size("500MB"), Ok(500_000_000));
        assert_eq!(parse_size("500 M"), Ok(500_000_000));
        assert_eq!(parse_size("2GiB"), Ok(2 << 30));
        assert_eq!(parse_size("512mib"), Ok(512 << 20));
        assert_eq!(parse_size("1000B"), Ok(1000));
        for bad in ["30", "", "GB", "30XB", "-1GB", "0GB", "1..2GB"] { assert!(parse_size(bad).is_err(), "{bad}"); }
    }

    /// cargo-sweep reads a bare number as MiB, so the limit is always passed as bytes.
    #[test]
    fn passes_exact_bytes_to_cargo_sweep() {
        assert_eq!(maxsize_arg(DEFAULT_LIMIT), "30000000000B");
        assert_eq!(maxsize_arg(parse_size("20G").unwrap()), "20000000000B");
        assert_eq!(human(DEFAULT_LIMIT), "30.00 GB");
        assert_eq!(human(12_345_678), "12.3 MB");
    }

    #[test]
    fn parses_flags() {
        assert_eq!(parse(&[]), Ok(Opts { limit: 30_000_000_000, dry_run: false }));
        assert_eq!(parse(&os(&["--limit", "20G", "--dry-run"])), Ok(Opts { limit: 20_000_000_000, dry_run: true }));
        assert_eq!(parse(&os(&["--limit=10GB"])), Ok(Opts { limit: 10_000_000_000, dry_run: false }));
        assert!(parse(&os(&["--limit"])).is_err());
        assert!(parse(&os(&["--limit", "30"])).is_err());
        assert!(parse(&os(&["--bogus"])).is_err());
    }

    #[test]
    fn parses_ps_lines() {
        let t = "    1     0 /sbin/launchd\n  501   400 /Users/x/.rustup/toolchains/stable/bin/cargo\n 7 1 /Applications/My App.app/Contents/MacOS/My App\n";
        assert_eq!(parse_ps(t), [(1, 0, "launchd".into()), (501, 400, "cargo".into()), (7, 1, "My App".into())]);
    }

    fn table() -> Vec<Proc> {
        vec![
            (1, 0, "launchd".into()),
            (100, 1, "zsh".into()),
            (200, 100, "cargo".into()), // rustup proxy
            (201, 200, "cargo".into()), // toolchain cargo: `cargo run -p xtask`
            (202, 201, "xtask".into()), // us
        ]
    }

    /// Our own `cargo run` (and the rustup proxy above it) never blocks.
    #[test]
    fn own_cargo_does_not_count() {
        assert!(busy(&table(), 202).is_empty());
        assert_eq!(gate(true, Ok(table()), 202), Gate::Go);
    }

    /// Another cargo, a rustc or nextest anywhere else blocks; a name that merely contains `cargo` does not.
    #[test]
    fn a_running_build_blocks() {
        let mut t = table();
        t.extend([(300, 100, "cargo".into()), (301, 300, "rustc".into()), (302, 1, "cargo-nextest".into()), (303, 1, "cargo-sweep".into())]);
        assert_eq!(busy(&t, 202), [(300, "cargo".to_string()), (301, "rustc".to_string()), (302, "cargo-nextest".to_string())]);
        // A rustc under our own cargo (e.g. test-full's last step still running) blocks too.
        t.push((400, 201, "rustc".into()));
        assert!(busy(&t, 202).contains(&(400, "rustc".to_string())));
        assert!(matches!(gate(true, Ok(t), 202), Gate::Busy(b) if b.len() == 4));
        assert!(matches!(gate(true, Err("no ps".into()), 202), Gate::Unknown(_)));
    }

    /// Without cargo-sweep nothing else is looked at, and [`run`] reports success (exit 0).
    #[test]
    fn missing_tool_is_not_an_error() {
        let mut t = table();
        t.push((300, 1, "rustc".into()));
        assert_eq!(gate(false, Ok(t), 202), Gate::NotInstalled);
        assert_eq!(gate(false, Err("no ps".into()), 202), Gate::NotInstalled);
    }

    #[test]
    fn names_the_removed_units() {
        let log = "[DEBUG] Would remove: \"/r/target/debug/deps/libquote-43017b3b5d4894e1.rlib\"\n\
                   [DEBUG] Would remove: \"/r/target/debug/.fingerprint/quote-43017b3b5d4894e1\"\n\
                   [DEBUG] Would remove: \"/r/target/debug/.fingerprint/rc-game-02a48906b39368ba\"\n\
                   [DEBUG] Successfully removed: \"/r/target/debug/.fingerprint/quote-1111111111111111\"\n\
                   [INFO] Would clean: 5.47 GiB from \"/r/target\"\n";
        assert_eq!(removed_units(log), (4, vec!["quote".to_string(), "rc-game".to_string()]));
        assert_eq!(removed_units("[INFO] Would clean: nothing from \"/r/target\"\n"), (0, vec![]));
    }

    #[test]
    fn counts_file_bytes() {
        let d = std::env::temp_dir().join(format!("xtask-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("x"), [0u8; 10]).unwrap();
        std::fs::write(d.join("a/b/y"), [0u8; 32]).unwrap();
        assert_eq!(dir_bytes(&d), 42);
        assert_eq!(dir_bytes(&d.join("missing")), 0);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
