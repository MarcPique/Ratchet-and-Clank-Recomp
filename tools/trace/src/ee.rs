//! EE main-memory image and the inputs that produce one: a PCSX2 savestate (`eeMemory.bin`
//! inside the `.p2s` zip), a raw dump file, or a live PINE read (see [`crate::pine`]).

use crate::zip::Archive;
use anyhow::{bail, ensure, Context, Result};
use std::path::{Path, PathBuf};

/// PCSX2 savestate entry names (pcsx2/SaveState.cpp, v2.8.2).
pub const P2S_EE_MEMORY: &str = "eeMemory.bin";
pub const P2S_SCRATCHPAD: &str = "Scratchpad.bin";
pub const P2S_VERSION: &str = "PCSX2 Savestate Version.id";

/// Main RAM is 32 MiB (PCSX2's optional "extended RAM" makes the savestate entry 128 MiB).
pub const EE_RAM_SIZE: usize = 32 << 20;

/// A copy of EE main RAM, addressed by EE virtual address.
pub struct EeImage {
    pub ram: Vec<u8>,
    /// Where the image came from (for reports).
    pub source: String,
}

impl EeImage {
    pub fn new(ram: Vec<u8>, source: impl Into<String>) -> EeImage { EeImage { ram, source: source.into() } }

    /// Physical main-RAM offset of an EE address: kseg0/kseg1 (0x8.., 0xa..), the uncached and
    /// uncached-accelerated mirrors (0x2.., 0x3..) and plain 0x0.. all map onto main RAM.
    /// Scratchpad (0x7000_0000) and I/O are not main RAM.
    pub fn phys(&self, addr: u32) -> Option<usize> {
        let a = match addr >> 28 {
            0x0 | 0x2 | 0x3 => addr & 0x0fff_ffff,
            0x8 | 0xa => addr & 0x1fff_ffff,
            _ => return None,
        } as usize;
        (a < self.ram.len()).then_some(a)
    }

    pub fn bytes(&self, addr: u32, len: usize) -> Result<&[u8]> {
        let p = self.phys(addr).with_context(|| format!("EE address {addr:#x} is not in main RAM"))?;
        self.ram.get(p..p + len).with_context(|| format!("EE range {addr:#x}+{len:#x} runs past the RAM image"))
    }
    pub fn u32(&self, addr: u32) -> Result<u32> { Ok(u32::from_le_bytes(self.bytes(addr, 4)?.try_into().unwrap())) }
    pub fn u16(&self, addr: u32) -> Result<u16> { Ok(u16::from_le_bytes(self.bytes(addr, 2)?.try_into().unwrap())) }

    /// Every EE address (physical, 0-based) where `pat` occurs, at multiples of `align`.
    pub fn find(&self, pat: &[u8], align: usize) -> Vec<u32> {
        find_all(&self.ram, pat, align).into_iter().map(|p| p as u32).collect()
    }
}

pub fn find_all(hay: &[u8], pat: &[u8], align: usize) -> Vec<usize> {
    let align = align.max(1);
    if pat.is_empty() || pat.len() > hay.len() { return Vec::new(); }
    let mut out = Vec::new();
    let mut i = 0;
    while i + pat.len() <= hay.len() {
        // Cheap first-byte scan before the full compare.
        match hay[i..hay.len() - pat.len() + 1].iter().position(|&b| b == pat[0]) {
            None => break,
            Some(k) => {
                let j = i + k;
                if j % align == 0 && &hay[j..j + pat.len()] == pat { out.push(j); }
                i = j + 1;
            }
        }
    }
    out
}

/// Savestate metadata from `PCSX2 Savestate Version.id`: `u32 save_version; char version[32]`.
pub struct P2sVersion { pub save_version: u32, pub build: String }

pub struct Savestate { pub archive: Archive, pub path: PathBuf }

impl Savestate {
    pub fn open(path: &Path) -> Result<Savestate> { Ok(Savestate { archive: Archive::open(path)?, path: path.to_owned() }) }

    pub fn version(&self) -> Result<P2sVersion> {
        let v = self.archive.read(P2S_VERSION)?;
        ensure!(v.len() >= 4, "short version entry");
        let s = &v[4..];
        let s = &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())];
        Ok(P2sVersion { save_version: u32::from_le_bytes(v[..4].try_into().unwrap()), build: String::from_utf8_lossy(s).into_owned() })
    }

    pub fn ee(&self) -> Result<EeImage> {
        let ram = self.archive.read(P2S_EE_MEMORY)?;
        ensure!(ram.len() >= EE_RAM_SIZE, "{P2S_EE_MEMORY} is {} bytes, expected at least 32 MiB", ram.len());
        Ok(EeImage::new(ram, format!("savestate {}", self.path.display())))
    }
}

/// PCSX2's data folder on macOS (`EmuFolders::DataRoot`), its `sstates` subfolder holds `.p2s` files.
pub fn pcsx2_sstates_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/PCSX2/sstates"))
}

/// Resolves `--state`: a `.p2s` path, a directory (newest `SCUS-97199*.p2s` in it), `latest`
/// (newest `SCUS-97199*.p2s` in PCSX2's sstates folder), or the name of a kept savestate in
/// `~/PS2/ratchet1/savestates/` (`novalis_spawn` = `novalis_spawn.p2s` there; see `save-state`).
pub fn resolve_state(arg: &str) -> Result<PathBuf> {
    let p = PathBuf::from(arg);
    if arg != "latest" && !p.exists() && !arg.contains('/') {
        let kept = crate::savestates_dir().join(if arg.ends_with(".p2s") { arg.to_string() } else { format!("{arg}.p2s") });
        if kept.exists() { return Ok(kept); }
    }
    let dir = if arg == "latest" { pcsx2_sstates_dir().context("HOME not set")? } else if p.is_dir() { p } else { return Ok(p) };
    newest_state(&dir)
}

/// The newest `SCUS-97199*.p2s` in `dir`.
pub fn newest_state(dir: &Path) -> Result<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))? {
        let e = e?;
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.starts_with("SCUS-97199") && name.ends_with(".p2s")) { continue; }
        let t = e.metadata()?.modified()?;
        if best.as_ref().is_none_or(|(bt, _)| t > *bt) { best = Some((t, e.path())); }
    }
    match best {
        Some((_, p)) => Ok(p),
        None => bail!("no SCUS-97199*.p2s savestate in {}", dir.display()),
    }
}

/// Where an EE image comes from on the command line.
pub enum EeSource { State(PathBuf), Raw(PathBuf), Pine(u16) }

impl EeSource {
    pub fn load(&self) -> Result<EeImage> {
        match self {
            EeSource::State(p) => {
                let s = Savestate::open(p)?;
                if let Ok(v) = s.version() {
                    eprintln!("savestate {}: save version {:#010x}, PCSX2 {}", p.display(), v.save_version, v.build);
                }
                s.ee()
            }
            EeSource::Raw(p) => {
                let ram = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
                ensure!(ram.len() >= 0x20_0000, "{} is only {} bytes; expected an EE RAM dump", p.display(), ram.len());
                Ok(EeImage::new(ram, format!("raw dump {}", p.display())))
            }
            EeSource::Pine(slot) => {
                let mut c = crate::pine::Pine::connect(*slot)?;
                eprintln!("PINE: {} | game {} | status {}", c.version().unwrap_or_default(), c.game_id().unwrap_or_default(), c.status_name());
                let t = std::time::Instant::now();
                let ram = c.read(0, EE_RAM_SIZE)?;
                eprintln!("PINE: read 32 MiB of EE RAM in {:.1} s", t.elapsed().as_secs_f64());
                Ok(EeImage::new(ram, format!("PINE slot {slot}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::{build, METHOD_STORE, METHOD_ZSTD};

    #[test]
    fn address_mirrors() {
        let e = EeImage::new(vec![0; EE_RAM_SIZE], "t");
        assert_eq!(e.phys(0x0012_3450), Some(0x12_3450));
        assert_eq!(e.phys(0x2012_3450), Some(0x12_3450));
        assert_eq!(e.phys(0x3012_3450), Some(0x12_3450));
        assert_eq!(e.phys(0x8012_3450), Some(0x12_3450));
        assert_eq!(e.phys(0x7000_0000), None);
        assert_eq!(e.phys(0x0200_0000), None);
    }

    #[test]
    fn find_respects_alignment() {
        let mut h = vec![0u8; 64];
        h[5..8].copy_from_slice(b"abc");
        h[16..19].copy_from_slice(b"abc");
        assert_eq!(find_all(&h, b"abc", 1), [5, 16]);
        assert_eq!(find_all(&h, b"abc", 16), [16]);
    }

    /// A synthetic savestate laid out like PCSX2 v2.8.2 writes it: stored version indicator,
    /// zstd-compressed components.
    #[test]
    fn savestate_container() {
        let mut ram = vec![0u8; EE_RAM_SIZE];
        ram[0x10_0000..0x10_0004].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        let mut ver = (0x9a59u32 << 16).to_le_bytes().to_vec();
        let mut b = *b"v2.8.2\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        b[31] = 0;
        ver.extend_from_slice(&b);
        let z = build(&[(P2S_VERSION, &ver, METHOD_STORE), (P2S_EE_MEMORY, &ram, METHOD_ZSTD), (P2S_SCRATCHPAD, &[1; 0x4000], METHOD_ZSTD)]);
        let dir = std::env::temp_dir().join(format!("rc-trace-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("SCUS-97199 (00000000).01.p2s");
        std::fs::write(&path, z).unwrap();
        assert_eq!(resolve_state(dir.to_str().unwrap()).unwrap(), path);
        let s = Savestate::open(&path).unwrap();
        let v = s.version().unwrap();
        assert_eq!((v.save_version, v.build.as_str()), (0x9a59_0000, "v2.8.2"));
        let e = s.ee().unwrap();
        assert_eq!(e.u32(0x0010_0000).unwrap(), 0xdead_beef);
        assert_eq!(e.u32(0x2010_0000).unwrap(), 0xdead_beef);
        std::fs::remove_dir_all(&dir).ok();
    }
}
