//! The build DB: which disc is which, and which builds Ratchet & Clank Recomp supports.
//!
//! Two tables, like OpenGOAL's serial → ELF-hash DB (`docs/plan/launcher_extractor.md` §2.1):
//! - [`TITLES`]: every known Ratchet & Clank boot-ELF name (= product serial) of RC1 and the sequels
//!   (`docs/formats/disc_layout.md` §2.6), so any R&C disc is named even when it is refused.
//! - [`BUILDS`]: exact builds pinned by serial + boot-ELF SHA-1. Only these can be supported; each supported build
//!   carries its Tier 0 size/SHA-1 table (`data/<serial>.tsv`, hashes and sizes only, no disc bytes).
//!
//! Adding a build: run `ratchet_clank-extract identify --iso <image>` on it. For an unknown build it prints an info line
//! `new build DB row: Build { … }`; paste that row into [`BUILDS`]. Supporting it additionally needs its table
//! (`ratchet_clank-extract table --iso <image> --output data/<serial>.tsv`), `supported: true` and, for a new region, the
//! runtime's address maps.

/// Which game a disc is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Rac1,
    Rac2,
    Rac3,
    Deadlocked,
    Unknown,
}

impl Game {
    /// Id used in the contract (`"game":"rac1"`).
    pub fn id(self) -> &'static str {
        match self { Game::Rac1 => "rac1", Game::Rac2 => "rac2", Game::Rac3 => "rac3", Game::Deadlocked => "racdl", Game::Unknown => "unknown" }
    }
    pub fn title(self) -> &'static str {
        match self {
            Game::Rac1 => "Ratchet & Clank",
            Game::Rac2 => "Ratchet & Clank: Going Commando",
            Game::Rac3 => "Ratchet & Clank: Up Your Arsenal",
            Game::Deadlocked => "Ratchet: Deadlocked",
            Game::Unknown => "",
        }
    }
}

/// A known product serial (the boot ELF's file name).
#[derive(Clone, Copy, Debug)]
pub struct Title {
    pub serial: &'static str,
    pub game: Game,
    /// Free text: which edition (from disc_layout.md §2.6).
    pub edition: &'static str,
}

const fn t(serial: &'static str, game: Game, edition: &'static str) -> Title { Title { serial, game, edition } }

pub const TITLES: &[Title] = &[
    t("SCUS_971.99", Game::Rac1, "original / Greatest Hits"),
    t("SCUS_972.09", Game::Rac1, "demo 1"),
    t("SCUS_972.40", Game::Rac1, "demo 2"),
    t("SCES_509.16", Game::Rac1, "Black Label / Platinum"),
    t("SCED_510.75", Game::Rac1, "demo"),
    t("SCPS_150.37", Game::Rac1, "original"),
    t("SCPS_150.56", Game::Rac2, ""), t("SCES_516.07", Game::Rac2, ""), t("SCUS_972.68", Game::Rac2, ""),
    t("SCUS_973.22", Game::Rac2, ""), t("SCUS_973.23", Game::Rac2, ""), t("SCUS_973.74", Game::Rac2, ""),
    t("SCKA_200.11", Game::Rac2, ""),
    t("PAPX_905.20", Game::Rac3, ""), t("SCED_528.47", Game::Rac3, ""), t("SCED_528.48", Game::Rac3, ""),
    t("SCES_524.56", Game::Rac3, ""), t("SCPS_150.84", Game::Rac3, ""), t("SCUS_973.53", Game::Rac3, ""),
    t("SCUS_974.11", Game::Rac3, ""), t("SCUS_974.13", Game::Rac3, ""), t("TCES_524.56", Game::Rac3, ""),
    t("SCKA_200.37", Game::Rac3, ""),
    t("PCPX_980.17", Game::Deadlocked, ""), t("SCED_536.60", Game::Deadlocked, ""), t("SCES_532.85", Game::Deadlocked, ""),
    t("SCPS_150.99", Game::Deadlocked, ""), t("SCPS_151.00", Game::Deadlocked, ""), t("SCUS_974.65", Game::Deadlocked, ""),
    t("SCUS_974.85", Game::Deadlocked, ""), t("SCUS_974.87", Game::Deadlocked, ""), t("SCKA_200.60", Game::Deadlocked, ""),
];

/// One exact build: serial + boot-ELF SHA-1.
#[derive(Clone, Copy, Debug)]
pub struct Build<'a> {
    pub serial: &'a str,
    /// `VER` in `SYSTEM.CNF`.
    pub version: &'a str,
    pub region: &'a str,
    pub elf_size: u64,
    /// Lowercase hex.
    pub elf_sha1: &'a str,
    /// Sectors of the reference dump (informational; files are checked by hash).
    pub image_sectors: u32,
    pub supported: bool,
    /// The Tier 0 table (`path\tsize\tsha1` lines) for supported builds.
    pub table: Option<&'a str>,
}

/// NTSC-U v1.00 reference: boot ELF and ISO hashes as recorded in docs/plan/decisions.md (2026-09-26).
pub const BUILDS: &[Build<'static>] = &[Build {
    serial: "SCUS_971.99",
    version: "1.00",
    region: "NTSC-U",
    elf_size: 1_383_028,
    elf_sha1: "72fd1de379dafd8f243dceeb3be50980f3fd3cd5",
    image_sectors: 2_057_664,
    supported: true,
    table: Some(include_str!("../data/scus_971_99.tsv")),
}];

/// Lowercase with everything but `[a-z0-9]` removed: `SCUS-97199`, `SCUS_971.99` and `scus97199` compare equal.
pub fn normalize_serial(s: &str) -> String { s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect() }

pub fn title(serial: &str) -> Option<&'static Title> {
    let n = normalize_serial(serial);
    TITLES.iter().find(|t| normalize_serial(t.serial) == n)
}

/// Region from the product-code prefix (`SCUS` → NTSC-U, `SCES`/`SCED` → PAL, `SCPS` → NTSC-J, `SCKA` → NTSC-K).
pub fn region_of(serial: &str) -> Option<&'static str> {
    let p: String = serial.chars().take(4).collect::<String>().to_ascii_uppercase();
    Some(match p.as_str() {
        "SCUS" | "SLUS" => "NTSC-U",
        "SCES" | "SCED" | "SLES" | "TCES" => "PAL",
        "SCPS" | "SLPS" | "SLPM" | "PAPX" | "PCPX" => "NTSC-J",
        "SCKA" | "SLKA" => "NTSC-K",
        _ => return None,
    })
}

/// When the serial is unknown: the game from the boot ELF's strings, in the order of disc_layout.md §2.6
/// (the sequels also contain "Ratchet & Clank").
pub fn game_from_elf(elf: &[u8]) -> Game {
    let has = |s: &str| elf.windows(s.len()).any(|w| w == s.as_bytes());
    if has("Deadlocked") { Game::Deadlocked }
    else if has("Up Your Arsenal") { Game::Rac3 }
    else if has("Going Commando") { Game::Rac2 }
    else if has("Ratchet & Clank") { Game::Rac1 }
    else { Game::Unknown }
}

/// The ready-to-paste [`BUILDS`] row for a build not in the DB (OpenGOAL's `log_potential_new_db_entry`).
pub fn db_row(serial: &str, version: &str, region: &str, elf_size: u64, elf_sha1: &str, image_sectors: u32) -> String {
    format!(
        "Build {{ serial: \"{serial}\", version: \"{version}\", region: \"{region}\", elf_size: {elf_size}, elf_sha1: \"{elf_sha1}\", \
         image_sectors: {image_sectors}, supported: false, table: None }},"
    )
}

/// One row of a Tier 0 table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableEntry {
    pub path: String,
    pub size: u64,
    pub sha1: [u8; 20],
}

/// Parses `path\tsize\tsha1` lines; `#` lines and blank lines are comments.
pub fn parse_table(text: &str) -> Result<Vec<TableEntry>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') { continue; }
        let mut f = line.split('\t');
        let (Some(path), Some(size), Some(sha), None) = (f.next(), f.next(), f.next(), f.next()) else {
            return Err(format!("table line {}: expected 3 tab-separated fields", i + 1));
        };
        let size = size.parse().map_err(|_| format!("table line {}: bad size", i + 1))?;
        let sha1 = crate::sha1::parse_hex(sha).ok_or_else(|| format!("table line {}: bad SHA-1", i + 1))?;
        out.push(TableEntry { path: path.to_string(), size, sha1 });
    }
    Ok(out)
}

/// Renders a table in the committed format.
pub fn format_table(serial: &str, version: &str, entries: &[TableEntry]) -> String {
    let total: u64 = entries.iter().map(|e| e.size).sum();
    let mut s = format!(
        "# Ratchet & Clank Recomp Tier 0 table: {serial} v{version}. {} files, {total} bytes.\n\
         # Sizes and SHA-1 hashes only (no disc bytes). Columns: path<TAB>size<TAB>sha1.\n\
         # Regenerate: ratchet_clank-extract table --iso <image> --output crates/rc-extract/data/<serial>.tsv\n",
        entries.len()
    );
    for e in entries { s += &format!("{}\t{}\t{}\n", e.path, e.size, crate::sha1::hex(&e.sha1)); }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serials_regions_and_games() {
        assert_eq!(title("scus97199").unwrap().serial, "SCUS_971.99");
        assert_eq!(title("SCUS-97199").unwrap().game, Game::Rac1);
        assert_eq!(title("SCUS_972.68").unwrap().game, Game::Rac2);
        assert!(title("SLUS_200.00").is_none());
        assert_eq!(region_of("SCES_509.16"), Some("PAL"));
        assert_eq!(region_of("SCPS_150.37"), Some("NTSC-J"));
        assert_eq!(region_of("XXXX_000.00"), None);
        assert_eq!(game_from_elf(b"..Ratchet & Clank: Going Commando.."), Game::Rac2);
        assert_eq!(game_from_elf(b"..Ratchet & Clank.."), Game::Rac1);
        assert_eq!(game_from_elf(b"Jak"), Game::Unknown);
        let serials: std::collections::HashSet<String> = TITLES.iter().map(|t| normalize_serial(t.serial)).collect();
        assert_eq!(serials.len(), TITLES.len(), "duplicate serial in TITLES");
    }

    #[test]
    fn builtin_table_parses_and_matches_its_build() {
        let b = &BUILDS[0];
        let table = parse_table(b.table.unwrap()).unwrap();
        assert!(table.len() > 2900, "{} rows", table.len());
        let boot = table.iter().find(|e| e.path == "boot/SCUS_971.99").unwrap();
        assert_eq!((boot.size, crate::sha1::hex(&boot.sha1)), (b.elf_size, b.elf_sha1.to_string()));
        let paths: std::collections::HashSet<&str> = table.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths.len(), table.len(), "duplicate path");
        // Hashes and sizes only: every line is exactly path, decimal size, 40 hex digits.
        for line in b.table.unwrap().lines().filter(|l| !l.starts_with('#')) {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 3, "{line}");
            assert!(f[0].chars().all(|c| c.is_ascii_alphanumeric() || "/_.".contains(c)), "{line}");
            assert!(f[1].chars().all(|c| c.is_ascii_digit()) && f[2].len() == 40, "{line}");
        }
    }

    #[test]
    fn table_round_trip_and_errors() {
        let e = vec![TableEntry { path: "toc.bin".into(), size: 3, sha1: crate::sha1::sha1(b"abc") }];
        assert_eq!(parse_table(&format_table("X", "1", &e)).unwrap(), e);
        assert!(parse_table("a\t1").is_err());
        assert!(parse_table("a\tx\tda39a3ee5e6b4b0d3255bfef95601890afd80709").is_err());
        assert!(parse_table("a\t1\tzz").is_err());
        assert!(db_row("SCUS_971.99", "1.01", "NTSC-U", 5, "ab", 7).starts_with("Build { serial: \"SCUS_971.99\", version: \"1.01\""));
    }
}
