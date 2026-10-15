//! `identify`: which disc is this? Serial from `SYSTEM.CNF`, SHA-1 of the boot ELF, looked up in the build DB.
//! Reads only the PVD, the directory, `SYSTEM.CNF` and the boot ELF (about 1.4 MB), so it takes milliseconds.

use crate::build_db::{self, Build, Game};
use crate::sha1::{hex, sha1};
use crate::{Code, DiscInfo, Emit, Error, Event, Stage};
use rc_formats::iso9660::{DiscError, IsoImage, SECTOR_SIZE};
use std::fs::File;
use std::path::Path;

/// Everything `identify` learned about a disc image.
#[derive(Clone, Debug)]
pub struct Identified {
    /// The boot ELF's name from `SYSTEM.CNF` `BOOT2` (`SCUS_971.99`).
    pub serial: String,
    pub region: String,
    /// `VER` from `SYSTEM.CNF`.
    pub version: String,
    /// `VMODE` from `SYSTEM.CNF` (`NTSC`/`PAL`), empty if absent.
    pub vmode: String,
    pub game: Game,
    /// Edition text from the title table, if the serial is known.
    pub edition: &'static str,
    pub elf_size: u64,
    pub elf_sha1: [u8; 20],
    pub sectors: u32,
    /// Index into the build list passed in, when serial + ELF hash match a build.
    pub build: Option<usize>,
    pub supported: bool,
}

impl Identified {
    pub fn disc_info(&self) -> DiscInfo {
        DiscInfo {
            serial: self.serial.clone(),
            region: self.region.clone(),
            version: self.version.clone(),
            supported: self.supported,
            game: self.game,
            title: self.game.title().to_string(),
            elf_sha1: hex(&self.elf_sha1),
        }
    }
}

fn read_error(what: &str, e: DiscError) -> Error {
    match e {
        DiscError::Io(e) => Error::new(Code::CannotRead, format!("cannot read {what}: {e}")),
        // Our reader bounds-checks every read against the image size: a read past the end = truncated image.
        DiscError::Format(e) => Error::new(Code::CannotRead, format!("cannot read {what}: {e} (truncated or damaged image?)")),
    }
}

/// Opens the image and reads the ISO 9660 structure. Codes 10 and 11.
pub fn open_image(path: &Path) -> Result<IsoImage<File>, Error> {
    let file = File::open(path).map_err(|e| Error::new(Code::CannotRead, format!("cannot open {}: {e}", path.display())))?;
    let meta = file.metadata().map_err(|e| Error::new(Code::CannotRead, format!("cannot read {}: {e}", path.display())))?;
    if meta.is_dir() { return Err(Error::new(Code::CannotRead, format!("{} is a folder, not a disc image", path.display()))); }
    let iso = IsoImage::new(file).map_err(|e| match e {
        DiscError::Io(e) => Error::new(Code::CannotRead, format!("cannot read {}: {e}", path.display())),
        DiscError::Format(e) => Error::new(Code::NotIso, format!("{} is not an ISO 9660 disc image ({e})", path.display())),
    })?;
    if iso.raw_sector_size() != SECTOR_SIZE {
        return Err(Error::new(Code::NotIso, format!(
            "{} is a raw {}-byte-sector image (.bin); only plain 2048-byte-sector .iso images are supported for now",
            path.display(), iso.raw_sector_size())));
    }
    if iso.sector_count() < iso.volume_sectors() {
        return Err(Error::new(Code::CannotRead, format!(
            "{} is truncated: {} of the {} sectors its volume descriptor declares ({:.1}%)",
            path.display(), iso.sector_count(), iso.volume_sectors(), 100.0 * iso.sector_count() as f64 / iso.volume_sectors() as f64)));
    }
    Ok(iso)
}

/// `KEY = value` from SYSTEM.CNF (value up to `;`, CR, LF).
fn cnf_value<'a>(cnf: &'a str, key: &str) -> Option<&'a str> {
    cnf.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().split(';').next().unwrap_or("").trim())
    })
}

/// Identifies the image without deciding support: codes 10, 11 and 20. Emits the `disc` line whenever
/// `SYSTEM.CNF` names a boot file (also before returning code 20).
pub fn probe(path: &Path, builds: &[Build], emit: Emit) -> Result<(Identified, IsoImage<File>), Error> {
    let iso = open_image(path)?;
    let not_game = |m: String| Error::new(Code::NotRatchet, m);
    let Some(cnf) = iso.find("/SYSTEM.CNF") else {
        return Err(not_game("no SYSTEM.CNF: not a PlayStation 2 game disc".into()));
    };
    let cnf = iso.read_file(cnf).map_err(|e| read_error("SYSTEM.CNF", e))?;
    let cnf = String::from_utf8_lossy(&cnf).into_owned();
    let Some(boot) = cnf_value(&cnf, "BOOT2") else { return Err(not_game("SYSTEM.CNF has no BOOT2 line: not a PlayStation 2 game disc".into())) };
    let boot_path = boot.strip_prefix("cdrom0:").unwrap_or(boot).replace('\\', "/");
    let serial = boot_path.rsplit('/').next().unwrap_or("").to_string();
    let version = cnf_value(&cnf, "VER").unwrap_or("").to_string();
    let vmode = cnf_value(&cnf, "VMODE").unwrap_or("").to_string();
    let Some(elf_entry) = iso.find(&boot_path).cloned() else {
        return Err(not_game(format!("SYSTEM.CNF boots {boot_path}, which is not on the disc")));
    };

    let total = elf_entry.size as u64;
    emit(Event::Progress { stage: Stage::Identify, done: 0, total, file: serial.clone() });
    let elf = iso.read_file(&elf_entry).map_err(|e| read_error(&format!("the boot ELF {serial}"), e))?;
    let elf_sha1 = sha1(&elf);
    emit(Event::Progress { stage: Stage::Identify, done: total, total, file: serial.clone() });

    let title = build_db::title(&serial);
    let game = match title { Some(t) => t.game, None => build_db::game_from_elf(&elf) };
    let region = build_db::region_of(&serial).map(str::to_string).unwrap_or_else(|| match vmode.as_str() {
        "PAL" => "PAL".to_string(),
        "NTSC" => "NTSC".to_string(),
        _ => "unknown".to_string(),
    });
    let build = builds.iter().position(|b| {
        build_db::normalize_serial(b.serial) == build_db::normalize_serial(&serial) && b.elf_sha1.eq_ignore_ascii_case(&hex(&elf_sha1))
    });
    let supported = game == Game::Rac1 && build.is_some_and(|i| builds[i].supported);
    let id = Identified {
        version: build.map(|i| builds[i].version.to_string()).unwrap_or(version),
        region: build.map(|i| builds[i].region.to_string()).unwrap_or(region),
        serial,
        vmode,
        game,
        edition: title.map(|t| t.edition).unwrap_or(""),
        elf_size: total,
        elf_sha1,
        sectors: iso.sector_count(),
        build,
        supported,
    };
    emit(Event::Disc(id.disc_info()));
    if game == Game::Unknown {
        return Err(not_game(format!("{} ({}) is not a Ratchet & Clank disc", id.serial, iso.volume_id())));
    }
    Ok((id, iso))
}

/// Code 21 unless the disc is a supported build. For an RC1 build not in the DB, first emits the
/// `new build DB row: …` info line.
pub fn require_supported(id: &Identified, builds: &[Build], emit: Emit) -> Result<(), Error> {
    if id.supported {
        let b = &builds[id.build.expect("supported implies a build")];
        if b.image_sectors != id.sectors {
            emit(Event::Info(format!(
                "note: this image has {} sectors, the reference dump {}; every file is still checked by SHA-1",
                id.sectors, b.image_sectors)));
        }
        return Ok(());
    }
    let only = "Ratchet & Clank Recomp supports Ratchet & Clank NTSC-U SCUS_971.99 v1.00 only for now";
    let msg = match (id.game, id.build) {
        (Game::Rac1, Some(_)) => format!("Ratchet & Clank {} v{} ({}, {}) is recognised but not supported yet; {only}", id.serial, id.version, id.region, id.edition),
        (Game::Rac1, None) => {
            emit(Event::Info(format!(
                "new build DB row: {}",
                build_db::db_row(&id.serial, &id.version, &id.region, id.elf_size, &hex(&id.elf_sha1), id.sectors)
            )));
            format!(
                "unknown Ratchet & Clank build {} v{} ({}; boot ELF SHA-1 {}); {only}",
                id.serial, id.version, id.region, hex(&id.elf_sha1)
            )
        }
        (g, _) => format!("{} ({}, {}) is not supported; {only}", g.title(), id.serial, id.region),
    };
    Err(Error::new(Code::Unsupported, msg))
}

/// `identify`: [`probe`] + [`require_supported`].
pub fn identify(path: &Path, builds: &[Build], emit: Emit) -> Result<(Identified, IsoImage<File>), Error> {
    let (id, iso) = probe(path, builds, emit)?;
    require_supported(&id, builds, emit)?;
    Ok((id, iso))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cnf_values() {
        let cnf = "BOOT2 = cdrom0:\\SCUS_971.99;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n\r\n";
        assert_eq!(cnf_value(cnf, "BOOT2"), Some("cdrom0:\\SCUS_971.99"));
        assert_eq!(cnf_value(cnf, "VER"), Some("1.00"));
        assert_eq!(cnf_value(cnf, "VMODE"), Some("NTSC"));
        assert_eq!(cnf_value(cnf, "NOPE"), None);
    }
}
