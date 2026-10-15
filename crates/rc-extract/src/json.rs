//! Just enough JSON for the contract: writing flat objects (the JSON-lines output, `extract-info.json`) and
//! reading them back (`verify` reads `extract-info.json`; tests read the output lines).

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Num(i64),
    Bool(bool),
    Null,
    StrList(Vec<String>),
}

impl From<&str> for Value { fn from(s: &str) -> Self { Value::Str(s.to_string()) } }
impl From<String> for Value { fn from(s: String) -> Self { Value::Str(s) } }
impl From<bool> for Value { fn from(b: bool) -> Self { Value::Bool(b) } }
impl From<u64> for Value { fn from(n: u64) -> Self { Value::Num(n as i64) } }
impl From<i64> for Value { fn from(n: i64) -> Self { Value::Num(n) } }
impl From<i32> for Value { fn from(n: i32) -> Self { Value::Num(n as i64) } }
impl From<u32> for Value { fn from(n: u32) -> Self { Value::Num(n as i64) } }
impl From<usize> for Value { fn from(n: usize) -> Self { Value::Num(n as i64) } }

pub fn escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => { let _ = write!(out, "\\u{:04x}", c as u32); }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `{"k":v,...}` on one line, keys in the given order.
pub fn object(fields: &[(&str, Value)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 { out.push(','); }
        escape(k, &mut out);
        out.push(':');
        match v {
            Value::Str(s) => escape(s, &mut out),
            Value::Num(n) => { let _ = write!(out, "{n}"); }
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Null => out.push_str("null"),
            Value::StrList(l) => {
                out.push('[');
                for (j, s) in l.iter().enumerate() {
                    if j > 0 { out.push(','); }
                    escape(s, &mut out);
                }
                out.push(']');
            }
        }
    }
    out.push('}');
    out
}

/// Parses one flat JSON object (string, integer, bool, null and string-array values). Whitespace is allowed
/// anywhere JSON allows it. Returns the fields in order.
pub fn parse_object(s: &str) -> Option<Vec<(String, Value)>> {
    let mut p = Parser { s: s.as_bytes(), i: 0 };
    p.ws();
    p.eat(b'{')?;
    let mut out = Vec::new();
    p.ws();
    if p.peek() == Some(b'}') { p.i += 1; } else {
        loop {
            p.ws();
            let k = p.string()?;
            p.ws();
            p.eat(b':')?;
            p.ws();
            let v = p.value()?;
            out.push((k, v));
            p.ws();
            match p.next()? { b',' => continue, b'}' => break, _ => return None }
        }
    }
    p.ws();
    (p.i == p.s.len()).then_some(out)
}

pub fn get<'a>(obj: &'a [(String, Value)], key: &str) -> Option<&'a Value> { obj.iter().find(|(k, _)| k == key).map(|(_, v)| v) }

struct Parser<'a> { s: &'a [u8], i: usize }

impl Parser<'_> {
    fn peek(&self) -> Option<u8> { self.s.get(self.i).copied() }
    fn next(&mut self) -> Option<u8> { let c = self.peek()?; self.i += 1; Some(c) }
    fn eat(&mut self, c: u8) -> Option<()> { (self.next()? == c).then_some(()) }
    fn ws(&mut self) { while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) { self.i += 1; } }

    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.i;
            while !matches!(self.peek()?, b'"' | b'\\') { self.i += 1; }
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).ok()?);
            match self.next()? {
                b'"' => return Some(out),
                _ => match self.next()? {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'u' => {
                        let h = std::str::from_utf8(self.s.get(self.i..self.i + 4)?).ok()?;
                        self.i += 4;
                        out.push(char::from_u32(u32::from_str_radix(h, 16).ok()?)?);
                    }
                    _ => return None,
                },
            }
        }
    }

    fn value(&mut self) -> Option<Value> {
        match self.peek()? {
            b'"' => Some(Value::Str(self.string()?)),
            b't' => self.word("true", Value::Bool(true)),
            b'f' => self.word("false", Value::Bool(false)),
            b'n' => self.word("null", Value::Null),
            b'[' => {
                self.i += 1;
                let mut l = Vec::new();
                self.ws();
                if self.peek() == Some(b']') { self.i += 1; return Some(Value::StrList(l)); }
                loop {
                    self.ws();
                    l.push(self.string()?);
                    self.ws();
                    match self.next()? { b',' => continue, b']' => return Some(Value::StrList(l)), _ => return None }
                }
            }
            _ => {
                let start = self.i;
                if self.peek() == Some(b'-') { self.i += 1; }
                while matches!(self.peek(), Some(b'0'..=b'9')) { self.i += 1; }
                std::str::from_utf8(&self.s[start..self.i]).ok()?.parse().ok().map(Value::Num)
            }
        }
    }

    fn word(&mut self, w: &str, v: Value) -> Option<Value> {
        if self.s.get(self.i..self.i + w.len())? != w.as_bytes() { return None; }
        self.i += w.len();
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_round_trip_with_escapes() {
        let line = object(&[
            ("type", "info".into()),
            ("message", "a \"quoted\" path C:\\x\nnext\u{1}".into()),
            ("n", 4_214_095_872u64.into()),
            ("neg", (-3i64).into()),
            ("ok", true.into()),
            ("none", Value::Null),
            ("list", Value::StrList(vec!["SCUS_971.99".into(), "é".into()])),
        ]);
        assert_eq!(line, r#"{"type":"info","message":"a \"quoted\" path C:\\x\nnext\u0001","n":4214095872,"neg":-3,"ok":true,"none":null,"list":["SCUS_971.99","é"]}"#);
        let back = parse_object(&line).unwrap();
        assert_eq!(get(&back, "message"), Some(&Value::Str("a \"quoted\" path C:\\x\nnext\u{1}".into())));
        assert_eq!(get(&back, "n"), Some(&Value::Num(4_214_095_872)));
        assert_eq!(get(&back, "neg"), Some(&Value::Num(-3)));
        assert_eq!(get(&back, "ok"), Some(&Value::Bool(true)));
        assert_eq!(get(&back, "list"), Some(&Value::StrList(vec!["SCUS_971.99".into(), "é".into()])));
        assert_eq!(parse_object(" { \"a\" : 1 , \"b\":\"\\u0041\" }\n").unwrap(), vec![("a".into(), Value::Num(1)), ("b".into(), Value::Str("A".into()))]);
        assert_eq!(parse_object("{}").unwrap(), vec![]);
        for bad in ["", "{", "{\"a\"}", "{\"a\":1,}", "{\"a\":1} x", "[1]", "{\"a\":tru}"] { assert!(parse_object(bad).is_none(), "{bad}"); }
    }
}
