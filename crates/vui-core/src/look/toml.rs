//! The part of TOML a look file uses: `[tables]` and `[dotted.tables]`, `key = value` with bare or
//! quoted keys, strings, integers, floats, booleans, arrays and `{ inline = tables }`, and `#`
//! comments. Dates, multi-line strings and arrays of tables are not read; they are reported with
//! their line.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

pub type Table = BTreeMap<String, Value>;

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Integers read as numbers too.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }
}

/// The document in `text`, or the first problem with its line number.
pub fn parse(text: &str) -> Result<Table, String> {
    let mut root = Table::new();
    let mut path: Vec<String> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = n + 1;
        let mut r = Reader { s: raw.as_bytes(), at: 0, line };
        r.space();
        if r.done() {
            continue;
        }
        if r.peek() == Some(b'[') {
            if r.s.get(r.at + 1) == Some(&b'[') {
                return Err(format!("line {line}: arrays of tables aren't supported"));
            }
            r.at += 1;
            path = r.key_path()?;
            r.expect(b']')?;
            r.end()?;
            table_at(&mut root, &path, line)?;
            continue;
        }
        let key = r.key_path()?;
        r.expect(b'=')?;
        let value = r.value()?;
        r.end()?;
        let mut full = path.clone();
        full.extend(key);
        let (last, parents) = full.split_last().expect("a key has a name");
        let table = table_at(&mut root, parents, line)?;
        if table.insert(last.clone(), value).is_some() {
            return Err(format!("line {line}: {} is set twice", full.join(".")));
        }
    }
    Ok(root)
}

/// The table at `path`, made if missing.
fn table_at<'t>(root: &'t mut Table, path: &[String], line: usize) -> Result<&'t mut Table, String> {
    let mut t = root;
    for key in path {
        let v = t.entry(key.clone()).or_insert_with(|| Value::Table(Table::new()));
        t = match v {
            Value::Table(inner) => inner,
            _ => return Err(format!("line {line}: {key} is a value, not a table")),
        };
    }
    Ok(t)
}

struct Reader<'a> {
    s: &'a [u8],
    at: usize,
    line: usize,
}

impl Reader<'_> {
    fn err<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("line {}: {what}", self.line))
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.at += 1;
        }
    }

    /// Nothing but a comment left.
    fn done(&self) -> bool {
        matches!(self.peek(), None | Some(b'#'))
    }

    fn end(&mut self) -> Result<(), String> {
        self.space();
        if self.done() { Ok(()) } else { self.err("unexpected text after the value") }
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.space();
        if self.peek() == Some(c) {
            self.at += 1;
            Ok(())
        } else {
            self.err(&format!("expected `{}`", c as char))
        }
    }

    /// `a.b."c d"`
    fn key_path(&mut self) -> Result<Vec<String>, String> {
        let mut keys = Vec::new();
        loop {
            self.space();
            let key = match self.peek() {
                Some(b'"') => self.string()?,
                _ => {
                    let start = self.at;
                    while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'-') {
                        self.at += 1;
                    }
                    if start == self.at {
                        return self.err("expected a key");
                    }
                    String::from_utf8_lossy(&self.s[start..self.at]).into_owned()
                }
            };
            keys.push(key);
            self.space();
            if self.peek() == Some(b'.') {
                self.at += 1;
            } else {
                return Ok(keys);
            }
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.space();
        match self.peek() {
            Some(b'"') => {
                if self.s[self.at..].starts_with(b"\"\"\"") {
                    return self.err("multi-line strings aren't supported");
                }
                self.string().map(Value::Str)
            }
            Some(b'\'') => {
                self.at += 1;
                let start = self.at;
                while self.peek().is_some_and(|c| c != b'\'') {
                    self.at += 1;
                }
                let s = String::from_utf8_lossy(&self.s[start..self.at]).into_owned();
                self.expect(b'\'')?;
                Ok(Value::Str(s))
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.space();
                    if self.peek() == Some(b']') {
                        self.at += 1;
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value()?);
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b']') => {}
                        _ => return self.err("expected `,` or `]` (arrays stay on one line)"),
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut table = Table::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Value::Table(table));
                }
                loop {
                    let key = self.key_path()?;
                    self.expect(b'=')?;
                    let v = self.value()?;
                    let (last, parents) = key.split_last().expect("a key has a name");
                    let t = table_at(&mut table, parents, self.line)?;
                    if t.insert(last.clone(), v).is_some() {
                        return self.err(&format!("{} is set twice", key.join(".")));
                    }
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Value::Table(table));
                        }
                        _ => return self.err("expected `,` or `}`"),
                    }
                }
            }
            Some(b't') if self.s[self.at..].starts_with(b"true") => {
                self.at += 4;
                Ok(Value::Bool(true))
            }
            Some(b'f') if self.s[self.at..].starts_with(b"false") => {
                self.at += 5;
                Ok(Value::Bool(false))
            }
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' => self.number(),
            _ => self.err("expected a value"),
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'+')) {
            self.at += 1;
        }
        let text: String = String::from_utf8_lossy(&self.s[start..self.at]).chars().filter(|c| *c != '_').collect();
        if let Ok(n) = text.parse::<i64>() {
            return Ok(Value::Int(n));
        }
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(Value::Float(f)),
            _ => self.err(&format!("{text} isn't a number")),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1; // the opening quote
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None => return self.err("unterminated string"),
                Some(b'"') => {
                    self.at += 1;
                    return String::from_utf8(out).or_else(|_| self.err("invalid UTF-8"));
                }
                Some(b'\\') => {
                    self.at += 1;
                    let c = match self.peek() {
                        Some(b'n') => b'\n',
                        Some(b't') => b'\t',
                        Some(b'"') => b'"',
                        Some(b'\\') => b'\\',
                        _ => return self.err("unknown escape"),
                    };
                    out.push(c);
                    self.at += 1;
                }
                Some(c) => {
                    out.push(c);
                    self.at += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tables_values_and_comments() {
        let doc = parse(
            r##"
# a comment
schema_version = 1
theme = "porcelain"   # trailing
[base]
radii = { control = 10, card = 16.5 }
peek = true
list = [1, 2.5, "x"]
[glass.dark]
tint = "#14171D"
"quoted key" = -3
"##,
        )
        .unwrap();
        assert_eq!(doc["schema_version"], Value::Int(1));
        assert_eq!(doc["theme"].as_str(), Some("porcelain"));
        let base = doc["base"].as_table().unwrap();
        assert_eq!(base["radii"].as_table().unwrap()["card"].as_f64(), Some(16.5));
        assert_eq!(base["peek"].as_bool(), Some(true));
        assert_eq!(base["list"], Value::Array(vec![Value::Int(1), Value::Float(2.5), Value::Str("x".into())]));
        let dark = doc["glass"].as_table().unwrap()["dark"].as_table().unwrap();
        assert_eq!(dark["tint"].as_str(), Some("#14171D"));
        assert_eq!(dark["quoted key"], Value::Int(-3));
    }

    #[test]
    fn problems_name_their_line() {
        assert_eq!(parse("a = 1\na = 2").unwrap_err(), "line 2: a is set twice");
        assert!(parse("a = \"open").unwrap_err().starts_with("line 1"));
        assert!(parse("[x]\ny = 1 2").unwrap_err().starts_with("line 2"));
        assert!(parse("[[arr]]").unwrap_err().contains("arrays of tables"));
        assert!(parse("a = 1\n[a]").unwrap_err().contains("is a value"));
    }
}
