//! `ratchet_clank-extract`: the command-line front end. Interface: docs/plan/launcher_contract.md.

use rc_extract::build_db::{self, BUILDS};
use rc_extract::export::{self as tier2, Kinds};
use rc_extract::extract::{self, Options};
use rc_extract::{done_json, error_json, identify, mib, prepare, verify, Code, Error, Event, DEFAULT_THREADS, EXTRACTOR_VERSION};
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::Instant;

const USAGE: &str = "\
usage:
  ratchet_clank-extract identify --iso <image> [--json]
  ratchet_clank-extract extract  --iso <image> --out <data dir> [--ntsc-only] [--json]   (ends with prepare)
  ratchet_clank-extract verify   --out <data dir> [--json]
  ratchet_clank-extract prepare  --out <data dir> [--json]   (build the engine cache <data dir>/cache/v1 from the archive)
  ratchet_clank-extract export   --out <data dir> [--to <dir>] [--what <kinds>] [--level NN] [--json]
                           (usable formats: PNG, WAV, glTF, JSON; default --to <data dir>/exports, --what all)
  ratchet_clank-extract table    --iso <image> --output <file.tsv> [--json]   (developer: regenerate the size/SHA-1 table)
options:
  --json          JSON lines on stdout (launcher mode; docs/plan/launcher_contract.md)
  --ntsc-only     skip the PAL copies (PAL FMVs, PAL scenes, gameplay_pal, PAL credits)
  --threads <n>   copy/hash/export workers (default 4)
  --what <kinds>  export: comma list of textures, audio, models, levels, collision, text, or all
  --level NN      export: only this level (no global data)
  --iso may be omitted when RC_ISO is set. --flag=value also works.
  ratchet_clank-extract --help | --version";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cmd { Identify, Extract, Verify, Prepare, Export, Table, Help, Version }

#[derive(Debug, PartialEq, Eq)]
struct Args {
    cmd: Cmd,
    iso: Option<PathBuf>,
    out: Option<PathBuf>,
    output: Option<PathBuf>,
    to: Option<PathBuf>,
    what: Kinds,
    level: Option<u32>,
    json: bool,
    ntsc_only: bool,
    threads: usize,
}

/// Parses `argv[1..]`. `env_iso` is `RC_ISO`. Errors are the message after `usage: `.
fn parse_args(argv: &[OsString], env_iso: Option<OsString>) -> Result<Args, String> {
    let mut a = Args { cmd: Cmd::Help, iso: None, out: None, output: None, to: None, what: Kinds::ALL, level: None, json: false, ntsc_only: false, threads: DEFAULT_THREADS };
    let mut it = argv.iter();
    let Some(first) = it.next() else { return Err("no command".into()) };
    a.cmd = match first.to_str() {
        Some("identify") => Cmd::Identify,
        Some("extract") => Cmd::Extract,
        Some("verify") => Cmd::Verify,
        Some("prepare") => Cmd::Prepare,
        Some("export") => Cmd::Export,
        Some("table") => Cmd::Table,
        Some("--help" | "-h" | "help") => return Ok(a),
        Some("--version" | "-V") => { a.cmd = Cmd::Version; return Ok(a) }
        _ => return Err(format!("unknown command {first:?}")),
    };
    while let Some(arg) = it.next() {
        let s = arg.to_str().ok_or_else(|| format!("unexpected argument {arg:?}"))?;
        let (flag, inline) = match s.split_once('=') { Some((f, v)) if f.starts_with("--") => (f, Some(OsString::from(v))), _ => (s, None) };
        let mut value = |name: &str| -> Result<OsString, String> {
            match inline.clone().or_else(|| it.next().cloned()) {
                Some(v) if !v.is_empty() => Ok(v),
                _ => Err(format!("{name} needs a value")),
            }
        };
        match flag {
            "--iso" => a.iso = Some(value("--iso")?.into()),
            "--out" => a.out = Some(value("--out")?.into()),
            "--output" => a.output = Some(value("--output")?.into()),
            "--to" => a.to = Some(value("--to")?.into()),
            "--what" => {
                let v = value("--what")?;
                a.what = Kinds::parse(v.to_str().ok_or("--what needs text")?)?;
            }
            "--level" => {
                let v = value("--level")?;
                a.level = Some(v.to_str().and_then(|v| v.parse().ok()).filter(|n| *n < 100).ok_or("--level needs a level number (00..18)")?);
            }
            "--threads" => {
                let v = value("--threads")?;
                a.threads = v.to_str().and_then(|v| v.parse().ok()).filter(|n| (1..=64).contains(n)).ok_or("--threads needs a number from 1 to 64")?;
            }
            "--json" if inline.is_none() => a.json = true,
            "--ntsc-only" if inline.is_none() => a.ntsc_only = true,
            "--help" | "-h" => { a.cmd = Cmd::Help; return Ok(a) }
            _ => return Err(format!("unknown option {s}")),
        }
    }
    if a.iso.is_none() { a.iso = env_iso.filter(|v| !v.is_empty()).map(PathBuf::from); }
    let need = |ok: bool, what: &str| if ok { Ok(()) } else { Err(format!("{what} is required")) };
    match a.cmd {
        Cmd::Identify => need(a.iso.is_some(), "--iso (or RC_ISO)")?,
        Cmd::Extract => { need(a.iso.is_some(), "--iso (or RC_ISO)")?; need(a.out.is_some(), "--out")?; }
        Cmd::Verify | Cmd::Prepare | Cmd::Export => need(a.out.is_some(), "--out")?,
        Cmd::Table => { need(a.iso.is_some(), "--iso (or RC_ISO)")?; need(a.output.is_some(), "--output")?; }
        Cmd::Help | Cmd::Version => {}
    }
    if a.ntsc_only && a.cmd != Cmd::Extract { return Err("--ntsc-only applies to extract only".into()); }
    if a.cmd != Cmd::Export && (a.to.is_some() || a.level.is_some() || a.what != Kinds::ALL) { return Err("--to, --what and --level apply to export only".into()); }
    Ok(a)
}

/// Writes events as JSON lines or as text. Write errors (a closed pipe) are ignored: the exit code still tells.
struct Output {
    json: bool,
    tty: bool,
    /// Last printed tenth, per stage, for non-terminal text output.
    last_tenth: Option<(rc_extract::Stage, u64)>,
}

impl Output {
    fn line(&self, s: &str) {
        let mut o = std::io::stdout().lock();
        let _ = writeln!(o, "{s}");
        let _ = o.flush();
    }

    fn event(&mut self, e: Event) {
        if self.json { return self.line(&e.to_json()); }
        match e {
            Event::Progress { stage, done, total, file } => {
                let pct = if total == 0 { 100.0 } else { done as f64 * 100.0 / total as f64 };
                let text = format!("{:<8} {pct:5.1}%  {:>8.1} / {:.1} MiB  {file}", stage.name(), mib(done), mib(total));
                if self.tty {
                    let mut o = std::io::stdout().lock();
                    let _ = write!(o, "\r\x1b[2K{text}{}", if done >= total { "\n" } else { "" });
                    let _ = o.flush();
                } else {
                    let tenth = (done * 10).checked_div(total).unwrap_or(10);
                    if self.last_tenth != Some((stage, tenth)) {
                        self.last_tenth = Some((stage, tenth));
                        self.line(&text);
                    }
                }
            }
            Event::Info(m) => self.line(&m),
            Event::Disc(d) => {
                let game = if d.title.is_empty() { "unknown game".to_string() } else { d.title.clone() };
                self.line(&format!(
                    "disc: {game}, {} ({}, v{}), boot ELF SHA-1 {}: {}",
                    d.serial, d.region, d.version, d.elf_sha1, if d.supported { "supported" } else { "not supported" }
                ));
            }
        }
    }

    fn finish(&mut self, r: Result<String, Error>, t0: Instant) -> i32 {
        let ms = t0.elapsed().as_millis() as u64;
        match r {
            Ok(summary) => {
                if !summary.is_empty() { self.event(Event::Info(summary)); }
                if self.json { self.line(&done_json(ms)) } else { self.line(&format!("done in {:.2} s", ms as f64 / 1e3)) }
                0
            }
            Err(e) => {
                if self.json { self.line(&error_json(&e)) } else { eprintln!("{e}") }
                e.exit_code()
            }
        }
    }
}

fn main() {
    let t0 = Instant::now();
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let json = argv.iter().any(|a| a == "--json");
    let mut out = Output { json, tty: std::io::stdout().is_terminal(), last_tenth: None };
    let args = match parse_args(&argv, std::env::var_os("RC_ISO")) {
        Ok(a) => a,
        Err(m) => {
            let code = out.finish(Err(Error::new(Code::Internal, format!("usage: {m} (see ratchet_clank-extract --help)"))), t0);
            std::process::exit(code);
        }
    };
    match args.cmd {
        Cmd::Help => { println!("{USAGE}"); return; }
        Cmd::Version => { println!("ratchet_clank-extract {EXTRACTOR_VERSION}"); return; }
        _ => {}
    }
    let r = {
        let mut emit = |e: Event| out.event(e);
        run(&args, &mut emit)
    };
    let code = out.finish(r, t0);
    std::process::exit(code);
}

/// Runs the command; `Ok` carries a closing summary line.
fn run(a: &Args, emit: &mut dyn FnMut(Event)) -> Result<String, Error> {
    let t0 = Instant::now();
    match a.cmd {
        Cmd::Identify => {
            identify::identify(a.iso.as_ref().unwrap(), BUILDS, emit)?;
            Ok(String::new())
        }
        Cmd::Extract => {
            let opts = Options { ntsc_only: a.ntsc_only, threads: a.threads, cancel: None };
            let (_, s) = extract::extract(a.iso.as_ref().unwrap(), a.out.as_ref().unwrap(), BUILDS, &opts, emit)?;
            let secs = t0.elapsed().as_secs_f64();
            Ok(format!("extracted {} files, {:.1} MiB in {secs:.1} s ({:.0} MiB/s); the disc image is no longer needed", s.files, mib(s.bytes), mib(s.bytes) / secs.max(1e-3)))
        }
        Cmd::Verify => {
            let (_, s) = verify::verify(a.out.as_ref().unwrap(), BUILDS, a.threads, None, emit)?;
            Ok(format!("verified {} files, {:.1} MiB: all match", s.files, mib(s.bytes)))
        }
        Cmd::Prepare => {
            let p = prepare::prepare(a.out.as_ref().unwrap(), a.threads, None, emit)?;
            Ok(format!("prepared {} lumps ({} built, {} kept) in {:.1} s", p.lumps, p.built, p.up_to_date, t0.elapsed().as_secs_f64()))
        }
        Cmd::Export => {
            let out = a.out.as_ref().unwrap();
            let to = a.to.clone().unwrap_or_else(|| out.join(tier2::DEFAULT_DIR));
            let opts = tier2::Options { to: to.clone(), kinds: a.what, level: a.level, threads: a.threads, cancel: None };
            let x = tier2::export(out, &opts, emit)?;
            let skipped = if x.skipped > 0 { format!(" ({} items skipped, named above)", x.skipped) } else { String::new() };
            Ok(format!("exported {} files, {:.1} MiB to {} in {:.1} s{skipped}", x.files, mib(x.bytes), to.display(), t0.elapsed().as_secs_f64()))
        }
        Cmd::Table => {
            let (id, rows) = extract::table(a.iso.as_ref().unwrap(), BUILDS, a.threads, emit)?;
            let path = a.output.as_ref().unwrap();
            std::fs::write(path, build_db::format_table(&id.serial, &id.version, &rows))
                .map_err(|e| Error::new(Code::WriteFailed, format!("cannot write {}: {e}", path.display())))?;
            Ok(format!("wrote {} rows to {}", rows.len(), path.display()))
        }
        Cmd::Help | Cmd::Version => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Result<Args, String> { parse_args(&s.iter().map(OsString::from).collect::<Vec<_>>(), None) }

    #[test]
    fn parses_the_contract_command_lines() {
        let a = args(&["identify", "--iso", "/x/My Disc.iso", "--json"]).unwrap();
        assert_eq!((a.cmd, a.iso.as_deref(), a.json), (Cmd::Identify, Some(std::path::Path::new("/x/My Disc.iso")), true));
        let a = args(&["extract", "--iso", "d.iso", "--out", "/data", "--ntsc-only", "--json"]).unwrap();
        assert_eq!((a.cmd, a.out.as_deref(), a.ntsc_only, a.threads), (Cmd::Extract, Some(std::path::Path::new("/data")), true, DEFAULT_THREADS));
        let a = args(&["prepare", "--out", "/data", "--json"]).unwrap();
        assert_eq!((a.cmd, a.out.as_deref(), a.json), (Cmd::Prepare, Some(std::path::Path::new("/data")), true));
        let a = args(&["export", "--out", "/data", "--to", "/x", "--what", "textures,audio", "--level", "01", "--json"]).unwrap();
        assert_eq!((a.cmd, a.to.as_deref(), a.level, a.what.textures, a.what.models), (Cmd::Export, Some(std::path::Path::new("/x")), Some(1), true, false));
        let a = args(&["export", "--out=/data"]).unwrap();
        assert_eq!((a.to, a.level, a.what), (None, None, Kinds::ALL));
        let a = args(&["verify", "--out=/data", "--threads=8"]).unwrap();
        assert_eq!((a.cmd, a.out.as_deref(), a.json, a.threads), (Cmd::Verify, Some(std::path::Path::new("/data")), false, 8));
        assert_eq!(args(&["--version"]).unwrap().cmd, Cmd::Version);
        assert_eq!(args(&["extract", "--help"]).unwrap().cmd, Cmd::Help);
        let a = parse_args(&[OsString::from("identify")], Some(OsString::from("/env.iso"))).unwrap();
        assert_eq!(a.iso.as_deref(), Some(std::path::Path::new("/env.iso")));
        let a = parse_args(&["identify", "--iso", "/flag.iso"].map(OsString::from), Some(OsString::from("/env.iso"))).unwrap();
        assert_eq!(a.iso.as_deref(), Some(std::path::Path::new("/flag.iso")));
    }

    #[test]
    fn rejects_bad_command_lines() {
        for bad in [
            &[][..], &["frobnicate"], &["identify"], &["extract", "--iso", "a"], &["verify"], &["prepare"], &["prepare", "--out", "o", "--ntsc-only"], &["identify", "--iso"],
            &["identify", "--iso", "a", "--bogus"], &["verify", "--out", "o", "--ntsc-only"], &["verify", "--out", "o", "--threads", "0"],
            &["table", "--iso", "a"], &["identify", "--iso", "a", "--json=1"], &["export"], &["export", "--out", "o", "--what", "meshes"],
            &["export", "--out", "o", "--level", "x"], &["verify", "--out", "o", "--to", "t"], &["prepare", "--out", "o", "--level", "1"],
        ] {
            assert!(args(bad).is_err(), "{bad:?}");
        }
    }
}
