//! The test tiers of docs/workflows/testing.md §2, one command each:
//!
//! - `test-quick [crate]`: every crate's unit tests (`--lib --bins`, about 5 s), or one crate's.
//! - `test-job <area…>`: the per-job tier (§2.1): the unit tests, the integration binaries of each named area ([`AREAS`],
//!   the area map of §3), and for the area `shared` the shared-code guard set (the NO_IDLE hero digest compared with
//!   the baseline, and the all-levels smoke). `test-job --test <binary>` runs one integration binary alone.
//! - Targeting options of `test-quick` and `test-job` ([`Sel`]): `--filter <name>` (a test-name substring; `--exact`
//!   for a full path), `--ignored` (only the ignored tests), `--nocapture`; each is translated for the runner.
//! - `test-full`: the full suite (§2.2), then the digest compared with the baseline (never rewritten), then
//!   `cargo xtask sweep` (a failure there only warns).
//! - `digest-baseline`: the only command that writes the baseline, for a human who has decided a digest change is
//!   intended (or a job with none, before its first edit); it prints what changed.
//!
//! All of them set `RC_AUDIO=0`, run from the repo root and use cargo-nextest when it is installed (`cargo nextest`
//! answers), else `cargo test`. Both runners get `--workspace --features rc-engine/dev` spelled out ([`NEXTEST`],
//! [`CARGO_TEST`]; there is no Cargo test alias), so they share the dev (dynamic) Bevy build. These commands are the
//! only supported way to run tests (docs/workflows/testing.md §1).

use crate::{cargo, repo_root};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

/// Part of an area: a whole integration binary, or the tests of one whose names start with one of the prefixes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// (package, integration binary).
    Bin(&'static str, &'static str),
    /// (package, integration binary, test-name prefixes: a module `name::`, or one test's full path).
    Tests(&'static str, &'static str, &'static [&'static str]),
}

/// An area of testing.md §3: its name, its aliases, and its integration tests. The unit tests of every area run in
/// every `test-job` (they are not split by area). `shared` is the shared-code guard set (§2.1 step 3); its digest
/// comparison is added by [`job`].
pub struct Area {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub parts: &'static [Part],
}

use Part::{Bin, Tests};

pub const AREAS: &[Area] = &[
    Area { name: "hero", aliases: &[], parts: &[Bin("rc-game", "hero")] },
    Area { name: "weapons", aliases: &[], parts: &[Bin("rc-game", "weapons")] },
    Area { name: "classes", aliases: &["creatures", "mobys"], parts: &[Bin("rc-game", "classes")] },
    Area { name: "world", aliases: &["levels", "collision", "water"], parts: &[Bin("rc-game", "world")] },
    Area { name: "ui", aliases: &["menus", "hud", "map", "save", "vendor"], parts: &[Bin("rc-game", "ui")] },
    Area {
        name: "audio",
        aliases: &[],
        parts: &[
            Tests("rc-game", "ui", &["reverb_conformance::", "sound_conformance::"]),
            Tests("rc-game", "weapons", &["hero_guns_novalis::novalis_ryno_salvo_sounds_mix_without_overflow"]),
        ],
    },
    Area { name: "formats", aliases: &[], parts: &[Bin("rc-formats", "formats")] },
    Area { name: "data", aliases: &[], parts: &[Bin("rc-data", "data")] },
    Area { name: "extract", aliases: &[], parts: &[Bin("rc-extract", "extract")] },
    Area { name: "video", aliases: &["movies"], parts: &[Bin("rc-video", "movies")] },
    Area { name: "engine", aliases: &["render", "input"], parts: &[] },
    Area { name: "particles", aliases: &["fx"], parts: &[Tests("rc-game", "classes", &["particle_consumers::"])] },
    Area { name: "trace", aliases: &[], parts: &[Bin("rc-trace", "trace")] },
    Area { name: "repo", aliases: &["guards", "layout"], parts: &[Bin("repo-checks", "guards")] },
    Area { name: "shared", aliases: &[], parts: &[Tests("rc-game", "world", &["all_levels_smoke::"])] },
];

/// The area named `name` (or one of its aliases).
pub fn area(name: &str) -> Option<&'static Area> { AREAS.iter().find(|a| a.name == name || a.aliases.contains(&name)) }

/// The runner: nextest when installed, else `cargo test`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runner { Nextest, CargoTest }

/// Which tests of the selected binaries run, and how: the same meaning under both runners.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Sel {
    /// `--filter <name>` (repeatable, any matches): a test-name substring, e.g. `hero_novalis::` or `novalis_hero_digest`.
    pub filters: Vec<String>,
    /// `--exact`: the filters are full test paths (`<module>::<test_fn>`).
    pub exact: bool,
    /// `--ignored`: only the ignored tests (surveys, printers).
    pub ignored: bool,
    /// `--nocapture`: show the tests' output.
    pub nocapture: bool,
}

impl Sel {
    /// A nextest filterset for the name filters (`None` without any).
    fn nextest_expr(&self) -> Option<String> {
        if self.filters.is_empty() { return None; }
        let eq = if self.exact { "=" } else { "" };
        Some(self.filters.iter().map(|f| format!("test({eq}{f})")).collect::<Vec<_>>().join(" | "))
    }
    /// The nextest flags other than the filterset.
    fn nextest_flags(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.ignored { v.extend(["--run-ignored", "only"].map(String::from)); }
        if self.nocapture { v.push("--no-capture".into()); }
        v
    }
    /// The libtest arguments (after `--`) under `cargo test`; `prefixes` are a filtered part's own filters, replaced by
    /// the name filters when there are any (libtest ORs its filters, so the two cannot be intersected).
    fn libtest_args(&self, prefixes: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = if self.filters.is_empty() { prefixes.iter().map(|s| s.to_string()).collect() } else { self.filters.clone() };
        if self.exact && !self.filters.is_empty() { v.push("--exact".into()); }
        if self.ignored { v.push("--ignored".into()); }
        if self.nocapture { v.push("--nocapture".into()); }
        v
    }
    fn is_empty(&self) -> bool { *self == Sel::default() }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Opts {
    /// `--cargo-test`: use `cargo test` even when nextest is installed.
    cargo_test: bool,
    /// The positional arguments (the crate of `test-quick`, the areas of `test-job`).
    names: Vec<String>,
    /// `--test <binary>` (repeatable, `test-job` only): integration binaries to run on their own.
    tests: Vec<String>,
    /// The targeting options.
    sel: Sel,
    /// After `--`: passed to the runner as is.
    extra: Vec<OsString>,
}

fn parse(argv: &[OsString]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = argv.iter();
    while let Some(a) = it.next() {
        let mut value = |opt: &str| -> Result<String, String> {
            match it.next().and_then(|v| v.to_str()) {
                Some(v) if !v.is_empty() && !v.starts_with('-') => Ok(v.to_string()),
                _ => Err(format!("{opt} needs a value")),
            }
        };
        match a.to_str() {
            Some("--") => { o.extra = it.cloned().collect(); break; }
            Some("--cargo-test") => o.cargo_test = true,
            Some("--test") => o.tests.push(value("--test")?),
            Some("--filter") => o.sel.filters.push(value("--filter")?),
            Some("--exact") => o.sel.exact = true,
            Some("--ignored") => o.sel.ignored = true,
            Some("--nocapture" | "--no-capture") => o.sel.nocapture = true,
            Some(s) if s.starts_with('-') => return Err(format!("unknown option {s}")),
            Some(s) => o.names.push(s.to_string()),
            None => return Err(format!("unexpected argument {a:?}")),
        }
    }
    if o.sel.exact && o.sel.filters.is_empty() { return Err("--exact needs --filter <module>::<test_fn>".into()); }
    Ok(o)
}

/// The integration binaries on disk: `crates/*/tests/<bin>/main.rs` or `…/tests/<bin>.rs` (also under `tools/`).
pub fn integration_bins() -> Vec<String> {
    let mut out = Vec::new();
    for top in ["crates", "tools"] {
        let Ok(pkgs) = std::fs::read_dir(repo_root().join(top)) else { continue };
        for pkg in pkgs.flatten() {
            let Ok(ents) = std::fs::read_dir(pkg.path().join("tests")) else { continue };
            for e in ents.flatten() {
                let (path, name) = (e.path(), e.file_name().to_string_lossy().into_owned());
                if path.join("main.rs").is_file() { out.push(name); } else if let Some(stem) = name.strip_suffix(".rs") { if path.is_file() { out.push(stem.to_string()); } }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn runner(o: &Opts) -> Runner {
    if o.cargo_test { return Runner::CargoTest; }
    let ok = cargo().args(["nextest", "--version"]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
    if ok { Runner::Nextest } else { Runner::CargoTest }
}

/// One command to run: its arguments to `cargo` and extra environment.
#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    pub args: Vec<String>,
    pub env: Vec<(&'static str, String)>,
}

fn step(args: &[&str]) -> Step { Step { args: args.iter().map(|s| s.to_string()).collect(), env: Vec::new() } }

/// `cargo nextest run` over the workspace with the dev Bevy features (shares the `cargo dev` build).
pub const NEXTEST: &[&str] = &["nextest", "run", "--workspace", "--features", "rc-engine/dev"];
/// `cargo test` with the same flags: the `--cargo-test` fallback (explicit flags, no Cargo alias).
pub const CARGO_TEST: &[&str] = &["test", "--workspace", "--features", "rc-engine/dev"];

/// `-E <base> & (<name filters>)`, or either alone; nothing when both are absent.
fn push_expr(s: &mut Step, base: Option<String>, sel: &Sel) {
    let e = match (base, sel.nextest_expr()) {
        (Some(b), Some(n)) => format!("({b}) & ({n})"),
        (Some(e), None) | (None, Some(e)) => e,
        (None, None) => return,
    };
    s.args.extend(["-E".to_string(), e]);
}

/// A `cargo test` step: `CARGO_TEST`, the target flags, then `-- <libtest args>` when there are any.
fn cargo_test_step(targets: &[String], libtest: Vec<String>) -> Step {
    let mut s = step(CARGO_TEST);
    s.args.extend(targets.iter().cloned());
    if !libtest.is_empty() { s.args.push("--".into()); s.args.extend(libtest); }
    s
}

/// A nextest filterset matching the parts.
fn filterset(parts: &[Part]) -> String {
    let id = |p: &str, b: &str| format!("binary_id({p}::{b})");
    parts
        .iter()
        .map(|p| match p {
            Bin(pkg, bin) => id(pkg, bin),
            Tests(pkg, bin, pre) => {
                let t: Vec<String> = pre.iter().map(|s| format!("test(/^{}/)", s.replace('.', "\\."))).collect();
                format!("({} & ({}))", id(pkg, bin), t.join(" | "))
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

const UNIT: [&str; 2] = ["--lib", "--bins"];

/// The unit tests of every crate, or of one (`krate`: a package name, `-` or `_` alike).
fn quick_steps(r: Runner, krate: Option<&str>, sel: &Sel) -> Vec<Step> {
    match r {
        Runner::Nextest => {
            let mut s = step(NEXTEST);
            s.args.extend(UNIT.map(String::from));
            push_expr(&mut s, krate.map(|k| format!("package({})", k.replace('_', "-"))), sel);
            s.args.extend(sel.nextest_flags());
            vec![s]
        }
        // `cargo test -p <crate>` would resolve other features and rebuild (testing.md §1); the unit tests of every
        // crate take about 5 s, so without nextest they all run.
        Runner::CargoTest => vec![cargo_test_step(&UNIT.map(String::from), sel.libtest_args(&[]))],
    }
}

/// The unit tests and the integration parts of the areas (duplicates dropped), without the digest, plus the whole
/// binaries `only` (`--test`; with no areas, just those binaries and no unit tests).
fn job_steps(r: Runner, areas: &[&Area], only: &[String], sel: &Sel) -> Vec<Step> {
    let mut parts: Vec<Part> = Vec::new();
    for a in areas { for p in a.parts { if !parts.contains(p) { parts.push(*p); } } }
    // The `--test` binaries not already a whole part of an area.
    let mut extra_bins: Vec<&str> = Vec::new();
    for b in only { if !extra_bins.contains(&b.as_str()) && !parts.iter().any(|p| matches!(p, Bin(_, x) if x == b)) { extra_bins.push(b); } }
    // A whole binary (an area's or a `--test` one) covers any filtered part of it.
    let whole: Vec<&str> = parts.iter().filter_map(|p| match p { Bin(_, b) => Some(*b), _ => None }).chain(extra_bins.iter().copied()).collect();
    parts.retain(|p| !matches!(p, Tests(_, b, _) if whole.contains(b)));
    let mut bins: Vec<&str> = Vec::new();
    for p in &parts { let b = match p { Bin(_, b) | Tests(_, b, _) => *b }; if !bins.contains(&b) { bins.push(b); } }
    for b in &extra_bins { if !bins.contains(b) { bins.push(b); } }
    let test = |b: &str| ["--test".to_string(), b.to_string()];
    match r {
        Runner::Nextest => {
            let mut s = step(NEXTEST);
            let mut base: Vec<String> = Vec::new();
            if !areas.is_empty() {
                s.args.extend(UNIT.map(String::from));
                base.push("kind(lib) | kind(bin)".into());
                if !parts.is_empty() { base.push(filterset(&parts)); }
                // `--test` alone selects whole binaries; next to the areas' filterset they need their own term.
                base.extend(extra_bins.iter().map(|b| format!("binary(={b})")));
            }
            for b in &bins { s.args.extend(test(b)); }
            push_expr(&mut s, (!base.is_empty()).then(|| base.join(" | ")), sel);
            s.args.extend(sel.nextest_flags());
            vec![s]
        }
        Runner::CargoTest => {
            let mut out = if areas.is_empty() { Vec::new() } else { quick_steps(r, None, sel) };
            let targets: Vec<String> = whole.iter().flat_map(|b| test(b)).collect();
            if !targets.is_empty() { out.push(cargo_test_step(&targets, sel.libtest_args(&[]))); }
            for p in &parts {
                if let Tests(_, b, pre) = p { out.push(cargo_test_step(&test(b), sel.libtest_args(pre))); }
            }
            out
        }
    }
}

/// The full suite. There are no doctests (checked 2026-09-29: every lib's `Doc-tests` runs 0), and nextest does not
/// run doctests; add a `cargo test --workspace --features rc-engine/dev --doc` step here when the first one appears.
fn full_steps(r: Runner) -> Vec<Step> {
    match r {
        Runner::Nextest => vec![step(&[NEXTEST, &["--no-fail-fast"]].concat())],
        Runner::CargoTest => vec![step(&[CARGO_TEST, &["--no-fail-fast"]].concat())],
    }
}

fn results_dir() -> PathBuf { repo_root().join("work/test-results") }
/// The digest baseline (git-ignored; written only by `digest-baseline`).
pub fn baseline_path() -> PathBuf { results_dir().join("hero_digest_no_idle.txt") }
fn job_digest_path() -> PathBuf { results_dir().join("digest_job.txt") }

/// Ratchet's NO_IDLE digest into `out` (the test runs in the package folder, so the path is absolute).
fn digest_step(out: &std::path::Path) -> Step {
    let mut s = step(&[CARGO_TEST, &["-q", "--test", "hero", "--", "--exact", "hero_novalis::novalis_hero_digest"]].concat());
    s.env = vec![("RC_HERO_DIGEST", out.display().to_string()), ("RC_HERO_DIGEST_NO_IDLE", "1".to_string())];
    s
}

fn run_step(s: &Step, extra: &[OsString]) -> bool {
    let mut c = cargo();
    c.args(&s.args).current_dir(repo_root()).env("RC_AUDIO", "0");
    // Extra arguments go to the test binaries: after the `--` a step already has, else after a new one.
    if !extra.is_empty() {
        if !s.args.iter().any(|a| a == "--") && s.args.first().is_some_and(|a| a != "nextest") { c.arg("--"); }
        c.args(extra);
    }
    for (k, v) in &s.env { c.env(k, v); }
    let shown: Vec<String> = s.env.iter().map(|(k, v)| format!("{k}={v}")).chain(["RC_AUDIO=0".to_string(), "cargo".to_string()]).chain(s.args.iter().cloned()).collect();
    eprintln!("xtask: {}", shown.join(" "));
    match c.status() {
        Ok(st) => st.success(),
        Err(e) => { eprintln!("error: cannot run cargo: {e}"); false }
    }
}

fn run_all(steps: &[Step], extra: &[OsString]) -> bool {
    // Every step runs (a failing unit test does not hide an area's failures); the result is their conjunction.
    let mut ok = true;
    for s in steps { ok &= run_step(s, extra); }
    ok
}

fn done(ok: bool) -> ExitCode { if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE } }

fn opts(argv: &[OsString]) -> Result<Opts, ExitCode> {
    parse(argv).map_err(|e| { eprintln!("error: {e}\n\n{}", crate::HELP); ExitCode::from(2) })
}

/// `cargo xtask test-quick [crate] [targeting options] [--cargo-test] [-- <runner args>]`.
pub fn quick(argv: &[OsString]) -> ExitCode {
    let o = match opts(argv) { Ok(o) => o, Err(c) => return c };
    if o.names.len() > 1 { eprintln!("error: test-quick takes at most one crate"); return ExitCode::from(2); }
    if !o.tests.is_empty() { eprintln!("error: --test is an option of test-job (`cargo xtask test-job --test <binary>`)"); return ExitCode::from(2); }
    let r = runner(&o);
    if r == Runner::CargoTest && !o.names.is_empty() { eprintln!("xtask: no nextest: running every crate's unit tests (a single crate would rebuild)"); }
    let steps = quick_steps(r, o.names.first().map(String::as_str), &o.sel);
    let ok = run_all(&steps, &o.extra);
    done(ok)
}

/// `cargo xtask test-job <area…> [--test <binary>]… [targeting options] [--cargo-test] [-- <runner args>]`.
pub fn job(argv: &[OsString]) -> ExitCode {
    let o = match opts(argv) { Ok(o) => o, Err(c) => return c };
    if o.names.is_empty() && o.tests.is_empty() { eprintln!("error: test-job needs an area ({}) or --test <binary>", area_names()); return ExitCode::from(2); }
    let known = integration_bins();
    for t in &o.tests {
        if !known.contains(t) { eprintln!("error: unknown integration binary {t:?}; binaries: {}", known.join(", ")); return ExitCode::from(2); }
    }
    let mut areas = Vec::new();
    for n in &o.names {
        match area(n) {
            Some(a) => areas.push(a),
            None => { eprintln!("error: unknown area {n:?}; areas: {}", area_names()); return ExitCode::from(2); }
        }
    }
    let r = runner(&o);
    let steps = job_steps(r, &areas, &o.tests, &o.sel);
    let mut ok = run_all(&steps, &o.extra);
    // A targeted run (a name filter, only the ignored tests) is not the guard set: the digest runs only in a plain job.
    if areas.iter().any(|a| a.name == "shared") && o.sel.filters.is_empty() && !o.sel.ignored {
        ok = digest_check() && ok;
    }
    done(ok)
}

/// Runs the digest into `work/test-results/digest_job.txt` and compares it with the baseline.
fn digest_check() -> bool {
    let (base, now) = (baseline_path(), job_digest_path());
    if !base.exists() {
        eprintln!("xtask: no digest baseline at {}: take one before the first edit with `cargo xtask digest-baseline`", base.display());
        return false;
    }
    if std::fs::create_dir_all(results_dir()).is_err() || !run_step(&digest_step(&now), &[]) { return false; }
    let same = matches!((std::fs::read(&base), std::fs::read(&now)), (Ok(a), Ok(b)) if a == b);
    if same { eprintln!("xtask: hero digest identical to the baseline"); } else { eprintln!("xtask: HERO DIGEST DIFFERS: {} vs {}", now.display(), base.display()); }
    same
}

/// `cargo xtask digest-baseline`: writes the digest baseline, and prints what changed against the old one.
pub fn baseline(argv: &[OsString]) -> ExitCode {
    if !argv.is_empty() { eprintln!("error: digest-baseline takes no arguments"); return ExitCode::from(2); }
    let old = std::fs::read_to_string(baseline_path()).ok();
    let ok = std::fs::create_dir_all(results_dir()).is_ok() && run_step(&digest_step(&baseline_path()), &[]);
    if !ok { return done(false); }
    let new = std::fs::read_to_string(baseline_path()).unwrap_or_default();
    match old {
        None => eprintln!("xtask: wrote the first baseline {}", baseline_path().display()),
        Some(o) if o == new => eprintln!("xtask: baseline unchanged ({})", baseline_path().display()),
        Some(o) => {
            let d = line_diff(&o, &new);
            eprintln!("xtask: BASELINE CHANGED ({}): {} line(s) differ; the first ones:", baseline_path().display(), d.len());
            for l in d.iter().take(20) { eprintln!("{l}"); }
        }
    }
    done(true)
}

/// The lines that differ between two texts, position by position (`-` old, `+` new, with the line number).
fn line_diff(old: &str, new: &str) -> Vec<String> {
    let (a, b): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let mut out = Vec::new();
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i), b.get(i));
        if x == y { continue; }
        if let Some(x) = x { out.push(format!("  {:>6} - {x}", i + 1)); }
        if let Some(y) = y { out.push(format!("  {:>6} + {y}", i + 1)); }
    }
    out
}

/// `cargo xtask test-full [--cargo-test] [-- <runner args>]`: the full suite, then the digest compared with the baseline
/// (a mismatch fails; the baseline is never rewritten here), then the target/ sweep (a failure only warns).
pub fn full(argv: &[OsString]) -> ExitCode {
    let o = match opts(argv) { Ok(o) => o, Err(c) => return c };
    if !o.names.is_empty() { eprintln!("error: test-full takes no areas"); return ExitCode::from(2); }
    if !o.tests.is_empty() || !o.sel.is_empty() { eprintln!("error: test-full runs everything; for a targeted run use test-job / test-quick"); return ExitCode::from(2); }
    let steps = full_steps(runner(&o));
    let ok = run_all(&steps, &o.extra);
    let digest = digest_check();
    // Keep target/ under the limit; a refused or failed sweep never fails the test run.
    if !crate::sweep::run(&crate::sweep::Opts::default()) { eprintln!("xtask: warning: the target/ sweep did not run (above); the test result stands"); }
    done(ok && digest)
}

fn area_names() -> String { AREAS.iter().map(|a| a.name).collect::<Vec<_>>().join(", ") }

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> { v.iter().map(OsString::from).collect() }

    #[test]
    fn parses_names_flags_and_extra() {
        let o = parse(&os(&["hero", "--cargo-test", "ui", "--", "--nocapture", "x"])).unwrap();
        assert_eq!(o, Opts { cargo_test: true, names: vec!["hero".into(), "ui".into()], extra: os(&["--nocapture", "x"]), ..Opts::default() });
        assert!(parse(&os(&["--bogus"])).is_err());
    }

    #[test]
    fn areas_resolve_by_name_and_alias() {
        assert_eq!(area("mobys").unwrap().name, "classes");
        assert_eq!(area("water").unwrap().name, "world");
        assert_eq!(area("vendor").unwrap().name, "ui");
        assert!(area("nope").is_none());
        // Names and aliases are unique.
        let mut all: Vec<&str> = AREAS.iter().flat_map(|a| std::iter::once(a.name).chain(a.aliases.iter().copied())).collect();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
    }

    /// The help lists every area and alias.
    #[test]
    fn help_lists_every_area() {
        for a in AREAS { for n in std::iter::once(a.name).chain(a.aliases.iter().copied()) { assert!(crate::HELP.contains(n), "{n}"); } }
    }

    /// Every binary the table names is an integration binary on disk (`tests/<bin>/main.rs` or `tests/<bin>.rs`).
    #[test]
    fn every_area_binary_exists() {
        let dir = |pkg: &str| match pkg { "rc-trace" => "tools/trace".to_string(), "repo-checks" => "tools/repo-checks".to_string(), p => format!("crates/{p}") };
        for a in AREAS {
            for p in a.parts {
                let (Bin(pkg, bin) | Tests(pkg, bin, _)) = *p;
                let t = repo_root().join(dir(pkg)).join("tests");
                assert!(t.join(bin).join("main.rs").exists() || t.join(format!("{bin}.rs")).exists(), "{}: {pkg} {bin}", a.name);
            }
        }
    }

    fn sel(filters: &[&str], exact: bool, ignored: bool, nocapture: bool) -> Sel {
        Sel { filters: filters.iter().map(|s| s.to_string()).collect(), exact, ignored, nocapture }
    }
    fn args(s: &[Step]) -> Vec<String> { s.iter().map(|s| s.args.join(" ")).collect() }
    const N: &str = "nextest run --workspace --features rc-engine/dev";
    const C: &str = "test --workspace --features rc-engine/dev";

    #[test]
    fn parses_test_filter_and_switches() {
        let o = parse(&os(&["--test", "weapons", "--filter", "hero_doom::", "--exact", "--ignored", "--no-capture"])).unwrap();
        assert_eq!(o.tests, ["weapons"]);
        assert_eq!(o.sel, sel(&["hero_doom::"], true, true, true));
        assert!(parse(&os(&["--test"])).is_err());
        assert!(parse(&os(&["--filter", "--exact"])).is_err());
        assert!(parse(&os(&["--exact"])).is_err(), "--exact without a filter");
    }

    /// `--test` accepts exactly the binaries on disk, and every area binary is one of them.
    #[test]
    fn integration_bins_are_found_on_disk() {
        let b = integration_bins();
        for x in ["hero", "weapons", "classes", "world", "ui", "formats", "data", "extract", "movies", "trace", "guards"] { assert!(b.contains(&x.to_string()), "{x}: {b:?}"); }
        for x in ["common", "fixtures", "snapshot"] { assert!(!b.contains(&x.to_string()), "{x}"); }
    }

    #[test]
    fn job_steps_nextest_one_run_with_a_filterset() {
        let s = job_steps(Runner::Nextest, &[area("hero").unwrap(), area("audio").unwrap()], &[], &Sel::default());
        assert_eq!(s.len(), 1);
        let a = s[0].args.join(" ");
        assert!(a.starts_with(&format!("{N} --lib --bins --test hero --test ui --test weapons -E ")), "{a}");
        assert!(a.ends_with("kind(lib) | kind(bin) | binary_id(rc-game::hero) | (binary_id(rc-game::ui) & (test(/^reverb_conformance::/) | test(/^sound_conformance::/))) | (binary_id(rc-game::weapons) & (test(/^hero_guns_novalis::novalis_ryno_salvo_sounds_mix_without_overflow/)))"), "{a}");
    }

    #[test]
    fn job_steps_cargo_test_whole_binaries_then_filtered_runs() {
        let s = job_steps(Runner::CargoTest, &[area("ui").unwrap(), area("audio").unwrap(), area("shared").unwrap()], &[], &Sel::default());
        // `ui` covers audio's ui part; audio's weapons test and the smoke stay filtered runs.
        assert_eq!(args(&s), [
            format!("{C} --lib --bins"),
            format!("{C} --test ui"),
            format!("{C} --test weapons -- hero_guns_novalis::novalis_ryno_salvo_sounds_mix_without_overflow"),
            format!("{C} --test world -- all_levels_smoke::"),
        ]);
    }

    /// `test-job --test <binary>`: that binary alone (no unit tests), with the targeting options per runner.
    #[test]
    fn job_steps_single_binary() {
        let only = ["weapons".to_string()];
        assert_eq!(args(&job_steps(Runner::Nextest, &[], &only, &Sel::default())), [format!("{N} --test weapons")]);
        assert_eq!(args(&job_steps(Runner::CargoTest, &[], &only, &Sel::default())), [format!("{C} --test weapons")]);
        let s = sel(&["hero_doom::a", "hero_doom::b"], true, true, true);
        assert_eq!(args(&job_steps(Runner::Nextest, &[], &only, &s)), [format!("{N} --test weapons -E test(=hero_doom::a) | test(=hero_doom::b) --run-ignored only --no-capture")]);
        assert_eq!(args(&job_steps(Runner::CargoTest, &[], &only, &s)), [format!("{C} --test weapons -- hero_doom::a hero_doom::b --exact --ignored --nocapture")]);
    }

    /// `--test` next to areas: its binary joins the filterset, and it swallows an area's filtered part of it.
    #[test]
    fn job_steps_areas_plus_binary() {
        let only = ["ui".to_string(), "hero".to_string()];
        let a = job_steps(Runner::Nextest, &[area("hero").unwrap(), area("audio").unwrap()], &only, &sel(&["novalis"], false, false, false))[0].args.join(" ");
        assert!(a.starts_with(&format!("{N} --lib --bins --test hero --test weapons --test ui -E ")), "{a}");
        assert!(a.contains("binary(=ui)") && !a.contains("binary(=hero)") && !a.contains("reverb_conformance"), "{a}");
        assert!(a.ends_with(") & (test(novalis))"), "{a}");
        let c = job_steps(Runner::CargoTest, &[area("audio").unwrap()], &["ui".to_string()], &sel(&["novalis"], false, false, false));
        assert_eq!(args(&c), [format!("{C} --lib --bins -- novalis"), format!("{C} --test ui -- novalis"), format!("{C} --test weapons -- novalis")]);
    }

    #[test]
    fn line_diff_marks_changed_added_and_removed_lines() {
        assert!(line_diff("a\nb\n", "a\nb\n").is_empty());
        assert_eq!(line_diff("a\nb\nc", "a\nB"), ["       2 - b", "       2 + B", "       3 - c"]);
    }

    #[test]
    fn quick_and_full_steps() {
        let none = Sel::default();
        assert_eq!(args(&quick_steps(Runner::Nextest, Some("rc_game"), &none)), [format!("{N} --lib --bins -E package(rc-game)")]);
        assert_eq!(args(&quick_steps(Runner::Nextest, None, &none)), [format!("{N} --lib --bins")]);
        assert_eq!(args(&quick_steps(Runner::CargoTest, Some("rc-game"), &none)), [format!("{C} --lib --bins")]);
        let f = sel(&["snapshot"], false, false, true);
        assert_eq!(args(&quick_steps(Runner::Nextest, Some("rc-formats"), &f)), [format!("{N} --lib --bins -E (package(rc-formats)) & (test(snapshot)) --no-capture")]);
        assert_eq!(args(&quick_steps(Runner::Nextest, None, &f)), [format!("{N} --lib --bins -E test(snapshot) --no-capture")]);
        assert_eq!(args(&quick_steps(Runner::CargoTest, None, &f)), [format!("{C} --lib --bins -- snapshot --nocapture")]);
        assert_eq!(args(&full_steps(Runner::Nextest)), [format!("{N} --no-fail-fast")]);
        assert_eq!(args(&full_steps(Runner::CargoTest)), [format!("{C} --no-fail-fast")]);
    }

    /// No step calls a Cargo alias: the fallback spells out its flags (the `test-all` alias is gone).
    #[test]
    fn steps_use_explicit_cargo_flags() {
        let all: Vec<&Area> = AREAS.iter().collect();
        let mut steps = job_steps(Runner::CargoTest, &all, &[], &Sel::default());
        steps.extend(full_steps(Runner::CargoTest));
        steps.push(digest_step(std::path::Path::new("/x")));
        for s in &steps { assert_eq!(&s.args[..4], CARGO_TEST, "{:?}", s.args); }
        let cfg = std::fs::read_to_string(repo_root().join(".cargo/config.toml")).unwrap();
        assert!(!cfg.lines().any(|l| l.trim_start().starts_with("test-all")), ".cargo/config.toml defines test-all again");
    }
}
