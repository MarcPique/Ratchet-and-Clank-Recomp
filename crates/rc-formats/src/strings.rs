//! Level message text (help boxes, banners, subtitles) from the `gameplay_ntsc` / `gameplay_pal` file.
//! Spec: docs/plan/hud_text.md §5. Snapshot-tested in `tests/formats/golden.rs` (the English messages, HUD test).
//!
//! The gameplay header word at `+0x10 + 4·lang` points to a text block (the level loader 0x255958 copies the
//! block of the current language to the level heap and relocates it):
//!
//! ```text
//! u32 count, u32 size
//! count × { s32 text_offset (from the block start), s32 id, s32 help_audio (-1 none; voice = audio + 30000), s32 0 }
//! NUL-terminated strings
//! ```
//!
//! `msg_string__Fi` (0x2259e8) looks an id up with `Help_FindIndex` (0x225978): a linear scan, first match,
//! "Paradox! This message does not exist" when absent.

use crate::buf::{invalid, Buf, Result};

/// Languages as the game numbers them (`0x15ed88`); 1 is unused (its block is empty on the disc).
pub mod lang {
    pub const ENGLISH: u32 = 0;
    pub const FRENCH: u32 = 2;
    pub const GERMAN: u32 = 3;
    pub const SPANISH: u32 = 4;
    pub const ITALIAN: u32 = 5;
}

/// Language slots in the gameplay header (+0x10 .. +0x2c).
pub const LANGUAGE_SLOTS: u32 = 8;

/// `msg_string__Fi`'s fallback for an unknown id.
pub const MISSING_MESSAGE: &[u8] = b"Paradox! This message does not exist";

/// One text entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: i32,
    /// Bytes up to (not including) the NUL, control codes kept (§5 "Encoding").
    pub text: Vec<u8>,
    /// Help voice clip index, −1 = none.
    pub help_audio: i32,
}

/// Parses one text block (`count`, `size`, entries, strings) starting at byte 0 of `block`.
pub fn parse_text_block(block: &[u8]) -> Result<Vec<Message>> {
    let b = Buf(block);
    let count = b.u32(0)? as usize;
    let size = b.u32(4)? as usize;
    if count > 0x10000 { return invalid("implausible text block count"); }
    let body = b.sub(0, size.max(8), "text block")?;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let e = 8 + 16 * k;
        let (off, id, audio) = (body.i32(e)?, body.i32(e + 4)?, body.i32(e + 8)?);
        let Ok(off) = usize::try_from(off) else { return invalid("negative text offset") };
        let tail = body.tail(off, "message text")?.bytes();
        let Some(end) = tail.iter().position(|&c| c == 0) else { return invalid("message text without NUL") };
        out.push(Message { id, text: tail[..end].to_vec(), help_audio: audio });
    }
    Ok(out)
}

/// The messages of language `lang` from the decompressed gameplay file (empty for a language slot with no
/// entries).
pub fn parse_strings(gameplay: &[u8], lang: u32) -> Result<Vec<Message>> {
    if lang >= LANGUAGE_SLOTS { return invalid("language out of range"); }
    let off = Buf(gameplay).u32(0x10 + 4 * lang as usize)? as usize;
    parse_text_block(Buf(gameplay).tail(off, "text block")?.bytes())
}

/// `Help_FindIndex`: index of the first entry with `id`.
pub fn find_index(messages: &[Message], id: i32) -> Option<usize> { messages.iter().position(|m| m.id == id) }

/// `msg_string__Fi`: the text of `id`, or the "Paradox!" fallback.
pub fn lookup(messages: &[Message], id: i32) -> &[u8] { find_index(messages, id).map_or(MISSING_MESSAGE, |i| &messages[i].text) }

/// Readable form of a message: printable ASCII as is, everything else as `\xNN` (so colour codes 0x08..0x0f,
/// pad icons 0x10..0x19, the newline 0x01 and accented letters 0x80.. stay visible and round-trip).
pub fn display(text: &[u8]) -> String {
    let mut s = String::with_capacity(text.len());
    for &c in text {
        if (0x20..0x7f).contains(&c) && c != b'\\' { s.push(c as char) } else { s.push_str(&format!("\\x{c:02x}")) }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(entries: &[(i32, &[u8], i32)]) -> Vec<u8> {
        let mut strings = Vec::new();
        let mut table = Vec::new();
        let base = 8 + 16 * entries.len();
        for (id, t, a) in entries {
            for v in [(base + strings.len()) as i32, *id, *a, 0] { table.extend_from_slice(&v.to_le_bytes()); }
            strings.extend_from_slice(t);
            strings.push(0);
        }
        let mut b = Vec::new();
        b.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        b.extend_from_slice(&((base + strings.len()) as u32).to_le_bytes());
        b.extend(table);
        b.extend(strings);
        b
    }

    #[test]
    fn parse_and_lookup_first_match() {
        let blk = block(&[(1000, b"Gadgetron \x0cInfobots\x08 give", 4), (7, b"x", -1), (1000, b"second", 0)]);
        let mut g = vec![0u8; 0x40];
        g[0x10..0x14].copy_from_slice(&0x40u32.to_le_bytes());
        g.extend(blk);
        let m = parse_strings(&g, lang::ENGLISH).unwrap();
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].help_audio, 4);
        assert_eq!(lookup(&m, 1000), b"Gadgetron \x0cInfobots\x08 give");
        assert_eq!(lookup(&m, 5), MISSING_MESSAGE);
        assert_eq!(display(&m[0].text), "Gadgetron \\x0cInfobots\\x08 give");
    }
}
