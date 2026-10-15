//! Nested JSON for the Tier 2 exports (glTF documents, sidecars, level tables): a value tree, a writer and a
//! reader. The flat contract lines keep using `crate::json`; this one handles arrays and objects of any depth.
//!
//! Numbers: `F32` prints the shortest text that reads back as the same f32 (Rust's `Display`), so exported floats
//! round-trip bit-exactly through any f64 JSON reader; non-finite floats become `null`. `Int` covers every integer
//! the exports write (u32 register halves, file sizes, i64 register words are written as hex strings instead).

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    F32(f32),
    /// Only produced by the reader (every number reads as f64).
    F64(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl From<bool> for J { fn from(v: bool) -> Self { J::Bool(v) } }
impl From<&str> for J { fn from(v: &str) -> Self { J::Str(v.to_string()) } }
impl From<String> for J { fn from(v: String) -> Self { J::Str(v) } }
impl From<&String> for J { fn from(v: &String) -> Self { J::Str(v.clone()) } }
impl From<f32> for J { fn from(v: f32) -> Self { J::F32(v) } }
impl From<Vec<J>> for J { fn from(v: Vec<J>) -> Self { J::Arr(v) } }
impl<T: Into<J>> From<Option<T>> for J { fn from(v: Option<T>) -> Self { v.map_or(J::Null, Into::into) } }
macro_rules! int_from { ($($t:ty),*) => { $(impl From<$t> for J { fn from(v: $t) -> Self { J::Int(v as i64) } })* } }
int_from!(i8, u8, i16, u16, i32, u32, i64, usize);
impl From<u64> for J { fn from(v: u64) -> Self { J::Int(i64::try_from(v).unwrap_or(i64::MAX)) } }
impl<T: Copy + Into<J>, const N: usize> From<[T; N]> for J { fn from(v: [T; N]) -> Self { J::Arr(v.iter().map(|&x| x.into()).collect()) } }
impl<T: Copy + Into<J>> From<&[T]> for J { fn from(v: &[T]) -> Self { J::Arr(v.iter().map(|&x| x.into()).collect()) } }

/// `0x…` text of a register or flag word (JSON numbers cannot hold every u64 exactly).
pub fn hex(v: u64) -> J { J::Str(format!("{v:#x}")) }

/// Lowercase hex of raw bytes (palettes, raw records in sidecars).
pub fn hex_bytes(b: &[u8]) -> J {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b { let _ = write!(s, "{x:02x}"); }
    J::Str(s)
}

/// An object under construction, keys in insertion order.
#[derive(Clone, Debug, Default)]
pub struct Obj(pub Vec<(String, J)>);

impl Obj {
    pub fn new() -> Self { Obj(Vec::new()) }
    pub fn set(mut self, k: &str, v: impl Into<J>) -> Self { self.0.push((k.to_string(), v.into())); self }
    pub fn put(&mut self, k: &str, v: impl Into<J>) { self.0.push((k.to_string(), v.into())); }
    pub fn build(self) -> J { J::Obj(self.0) }
}

impl From<Obj> for J { fn from(o: Obj) -> Self { J::Obj(o.0) } }

impl J {
    pub fn get(&self, k: &str) -> Option<&J> {
        match self { J::Obj(o) => o.iter().find(|(key, _)| key == k).map(|(_, v)| v), _ => None }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match *self { J::Int(i) => Some(i as f64), J::F32(f) => Some(f as f64), J::F64(f) => Some(f), _ => None }
    }
    /// A non-negative integer (as a reader sees it: an f64 with no fraction).
    pub fn as_usize(&self) -> Option<usize> {
        let f = self.as_f64()?;
        (f >= 0.0 && f.fract() == 0.0 && f < 9.0e15).then_some(f as usize)
    }
    pub fn as_str(&self) -> Option<&str> { if let J::Str(s) = self { Some(s) } else { None } }
    pub fn as_arr(&self) -> Option<&[J]> { if let J::Arr(a) = self { Some(a) } else { None } }
    pub fn as_obj(&self) -> Option<&[(String, J)]> { if let J::Obj(o) = self { Some(o) } else { None } }

    /// One line, no spaces (glTF documents).
    pub fn compact(&self) -> String { let mut s = String::new(); self.write(&mut s, None, 0); s }

    /// Two-space indentation; arrays of scalars stay on one line (sidecars and tables stay readable).
    pub fn pretty(&self) -> String { let mut s = String::new(); self.write(&mut s, Some(2), 0); s.push('\n'); s }

    fn is_scalar(&self) -> bool { !matches!(self, J::Arr(_) | J::Obj(_)) }

    fn write(&self, out: &mut String, indent: Option<usize>, depth: usize) {
        let nl = |out: &mut String, d: usize| if let Some(w) = indent { out.push('\n'); for _ in 0..w * d { out.push(' '); } };
        match self {
            J::Null => out.push_str("null"),
            J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            J::Int(i) => { let _ = write!(out, "{i}"); }
            J::F32(f) => if f.is_finite() { let _ = write!(out, "{f}"); } else { out.push_str("null") },
            J::F64(f) => if f.is_finite() { let _ = write!(out, "{f}"); } else { out.push_str("null") },
            J::Str(s) => crate::json::escape(s, out),
            J::Arr(a) => {
                out.push('[');
                let flat = indent.is_none() || a.iter().all(J::is_scalar);
                for (i, v) in a.iter().enumerate() {
                    if i > 0 { out.push(','); if flat && indent.is_some() { out.push(' '); } }
                    if !flat { nl(out, depth + 1); }
                    v.write(out, indent, depth + 1);
                }
                if !flat && !a.is_empty() { nl(out, depth); }
                out.push(']');
            }
            J::Obj(o) => {
                out.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 { out.push(','); }
                    nl(out, depth + 1);
                    crate::json::escape(k, out);
                    out.push(':');
                    if indent.is_some() { out.push(' '); }
                    v.write(out, indent, depth + 1);
                }
                if !o.is_empty() { nl(out, depth); }
                out.push('}');
            }
        }
    }
}

/// Parses a complete JSON text (RFC 8259; numbers as f64, duplicate keys kept in order).
pub fn parse(s: &str) -> Result<J, String> {
    let mut p = P { s: s.as_bytes(), i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() { return Err(format!("trailing data at byte {}", p.i)); }
    Ok(v)
}

struct P<'a> { s: &'a [u8], i: usize }

impl P<'_> {
    fn err<T>(&self, what: &str) -> Result<T, String> { Err(format!("{what} at byte {}", self.i)) }
    fn peek(&self) -> Option<u8> { self.s.get(self.i).copied() }
    fn ws(&mut self) { while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) { self.i += 1; } }
    fn eat(&mut self, c: u8) -> Result<(), String> { if self.peek() == Some(c) { self.i += 1; Ok(()) } else { self.err(&format!("expected '{}'", c as char)) } }

    fn value(&mut self, depth: usize) -> Result<J, String> {
        if depth > 256 { return self.err("nesting too deep"); }
        match self.peek() {
            Some(b'{') => {
                self.i += 1;
                let mut o = Vec::new();
                self.ws();
                if self.peek() == Some(b'}') { self.i += 1; return Ok(J::Obj(o)); }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    self.eat(b':')?;
                    self.ws();
                    o.push((k, self.value(depth + 1)?));
                    self.ws();
                    match self.peek() { Some(b',') => self.i += 1, Some(b'}') => { self.i += 1; return Ok(J::Obj(o)); } _ => return self.err("expected ',' or '}'") }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.peek() == Some(b']') { self.i += 1; return Ok(J::Arr(a)); }
                loop {
                    self.ws();
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    match self.peek() { Some(b',') => self.i += 1, Some(b']') => { self.i += 1; return Ok(J::Arr(a)); } _ => return self.err("expected ',' or ']'") }
                }
            }
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b't') => self.word("true", J::Bool(true)),
            Some(b'f') => self.word("false", J::Bool(false)),
            Some(b'n') => self.word("null", J::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.err("unexpected character"),
        }
    }

    fn word(&mut self, w: &str, v: J) -> Result<J, String> {
        if self.s.get(self.i..self.i + w.len()) != Some(w.as_bytes()) { return self.err("bad literal"); }
        self.i += w.len();
        Ok(v)
    }

    fn number(&mut self) -> Result<J, String> {
        let start = self.i;
        if self.peek() == Some(b'-') { self.i += 1; }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; },
            _ => return self.err("bad number"),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) { return self.err("bad fraction"); }
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) { self.i += 1; }
            if !matches!(self.peek(), Some(b'0'..=b'9')) { return self.err("bad exponent"); }
            while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; }
        }
        let t = std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?;
        t.parse::<f64>().map(J::F64).map_err(|e| format!("{e} at byte {start}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.i;
            while !matches!(self.peek(), Some(b'"' | b'\\') | None) {
                if self.s[self.i] < 0x20 { return self.err("control character in string"); }
                self.i += 1;
            }
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).map_err(|e| e.to_string())?);
            match self.peek() {
                None => return self.err("unterminated string"),
                Some(b'"') => { self.i += 1; return Ok(out); }
                _ => {
                    self.i += 1;
                    let c = self.peek().ok_or("unterminated escape")?;
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xd800..0xdc00).contains(&cp) && self.s.get(self.i..self.i + 2) == Some(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000 + ((cp - 0xd800) << 10) + (lo.wrapping_sub(0xdc00) & 0x3ff);
                            }
                            out.push(char::from_u32(cp).ok_or("bad \\u escape")?);
                        }
                        _ => return self.err("bad escape"),
                    }
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.s.get(self.i..self.i + 4).ok_or("short \\u escape")?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_back_nested_values() {
        let v = Obj::new()
            .set("s", "a \"b\"\n\u{1}é")
            .set("i", -7i32)
            .set("f", 0.1f32)
            .set("big", 16_777_217.0f32)
            .set("nan", f32::NAN)
            .set("arr", [1.5f32, -2.0, 3.25])
            .set("nested", J::Arr(vec![Obj::new().set("k", true).build(), J::Null, J::Arr(vec![])]))
            .set("empty", Obj::new())
            .build();
        for text in [v.compact(), v.pretty()] {
            let back = parse(&text).unwrap();
            assert_eq!(back.get("s").and_then(J::as_str), Some("a \"b\"\n\u{1}é"));
            assert_eq!(back.get("i").and_then(J::as_f64), Some(-7.0));
            // The f32 round-trips bit-exactly through the shortest decimal text.
            assert_eq!(back.get("f").and_then(J::as_f64).map(|f| f as f32), Some(0.1f32));
            assert_eq!(back.get("big").and_then(J::as_f64).map(|f| f as f32), Some(16_777_217.0f32));
            assert_eq!(back.get("nan"), Some(&J::Null));
            assert_eq!(back.get("arr").and_then(J::as_arr).map(|a| a.len()), Some(3));
            assert_eq!(back.get("nested").and_then(J::as_arr).unwrap()[0].get("k"), Some(&J::Bool(true)));
            assert_eq!(back.get("empty"), Some(&J::Obj(vec![])));
        }
        assert!(v.pretty().contains("\"arr\": [1.5, -2, 3.25]"), "scalar arrays stay on one line: {}", v.pretty());
        assert_eq!(parse(r#"{"a":"😀"}"#).unwrap().get("a").and_then(J::as_str), Some("😀"));
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "01", "1.", "-", "\"\u{1}\"", "[1] x", "tru"] { assert!(parse(bad).is_err(), "{bad:?}"); }
        assert_eq!(hex(0x5360b), J::Str("0x5360b".into()));
        assert_eq!(hex_bytes(&[0, 0xab]), J::Str("00ab".into()));
    }
}
