//! Parsing: markdown text into blocks and inline runs, and back to plain text for speech and
//! summaries.

/// A run of inline text, before it meets a theme.
#[derive(Clone, Debug, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(String),
    Emphasis(String),
    StrongEmphasis(String),
    Code(String),
    Link { text: String, url: String },
}

/// A block of a document.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading(u8, Vec<Inline>),
    Paragraph(Vec<Inline>),
    /// Items, each its inline text; `start` is the first number of a numbered list.
    List { start: Option<u64>, items: Vec<Vec<Inline>> },
    Quote(Vec<Inline>),
    Code { lang: String, text: String },
    Rule,
    Image { alt: String, src: String },
    Table { head: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
}

pub(crate) fn heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        Some((hashes as u8, line[hashes..].trim().trim_end_matches('#').trim_end()))
    } else {
        None
    }
}

pub(crate) fn bullet(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["- ", "* ", "+ ", "• "].iter().find_map(|m| t.strip_prefix(m))
}

pub(crate) fn numbered(line: &str) -> Option<(u64, &str)> {
    let t = line.trim_start();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = &t[digits..];
    let rest = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    Some((t[..digits].parse().ok()?, rest))
}

pub(crate) fn rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

pub(crate) fn image(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let src = rest.strip_suffix(')')?;
    (!src.contains(' ') || src.starts_with('<')).then(|| (alt.to_string(), src.trim_matches(['<', '>']).to_string()))
}

pub(crate) fn cells(line: &str) -> Vec<String> {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

pub(crate) fn table_divider(line: &str) -> bool {
    let t = line.trim();
    t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// Splits `text` into blocks.
pub fn parse(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(inlines(&para.join(" "))));
            para.clear();
        }
    };
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut blocks);
            i += 1;
            continue;
        }
        if let Some(fence) = ["```", "~~~"].into_iter().find(|f| t.starts_with(f)) {
            flush(&mut para, &mut blocks);
            let lang = t[3..].trim().to_string();
            let mut body = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(fence) {
                body.push(lines[i]);
                i += 1;
            }
            blocks.push(Block::Code { lang, text: body.join("\n") });
            i += 1;
            continue;
        }
        if let Some((level, text)) = heading(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Heading(level, inlines(text)));
            i += 1;
            continue;
        }
        if rule(t) && para.is_empty() {
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some((alt, src)) = image(t) {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Image { alt, src });
            i += 1;
            continue;
        }
        if t.starts_with('|') && i + 1 < lines.len() && table_divider(lines[i + 1]) {
            flush(&mut para, &mut blocks);
            let head = cells(t).iter().map(|c| inlines(c)).collect();
            let mut rows = Vec::new();
            i += 2;
            while i < lines.len() && lines[i].trim().starts_with('|') {
                rows.push(cells(lines[i]).iter().map(|c| inlines(c)).collect());
                i += 1;
            }
            blocks.push(Block::Table { head, rows });
            continue;
        }
        if t.starts_with('>') {
            flush(&mut para, &mut blocks);
            let mut quote = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('>') {
                quote.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            blocks.push(Block::Quote(inlines(&quote.join(" "))));
            continue;
        }
        if bullet(line).is_some() || numbered(line).is_some() {
            flush(&mut para, &mut blocks);
            let start = numbered(line).map(|(n, _)| n);
            let mut items: Vec<String> = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                let item = if start.is_some() { numbered(l).map(|(_, r)| r) } else { bullet(l) };
                if let Some(item) = item {
                    items.push(item.trim().to_string());
                } else if !l.trim().is_empty() && l.starts_with([' ', '\t']) && !items.is_empty() {
                    // A wrapped or nested line joins the item above it.
                    let last = items.last_mut().expect("checked");
                    let more = bullet(l).or(numbered(l).map(|(_, r)| r)).unwrap_or(l.trim());
                    last.push_str(if bullet(l).is_some() || numbered(l).is_some() { " · " } else { " " });
                    last.push_str(more.trim());
                } else {
                    break;
                }
                i += 1;
            }
            blocks.push(Block::List { start, items: items.iter().map(|s| inlines(s)).collect() });
            continue;
        }
        para.push(t);
        i += 1;
    }
    flush(&mut para, &mut blocks);
    blocks
}

/// Splits a line into styled runs.
pub fn inlines(text: &str) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    let mut plain = String::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut k = 0;
    let push_plain = |plain: &mut String, out: &mut Vec<Inline>| {
        if !plain.is_empty() {
            out.push(Inline::Text(std::mem::take(plain)));
        }
    };
    let word = |c: char| c.is_alphanumeric();
    while k < chars.len() {
        let (at, c) = chars[k];
        let rest = &text[at..];
        // `code`
        if c == '`' {
            let ticks = rest.bytes().take_while(|b| *b == b'`').count();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence) {
                push_plain(&mut plain, &mut out);
                out.push(Inline::Code(rest[ticks..ticks + end].trim().to_string()));
                let next = at + ticks + end + ticks;
                k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                continue;
            }
        }
        // [text](url), and ![alt](src) inline, which shows as its alt text linked to the picture.
        if c == '[' || (c == '!' && rest[1..].starts_with('[')) {
            let open = if c == '!' { 2 } else { 1 };
            if let Some(close) = rest.find("](")
                && close > open - 1
                && let Some(end) = rest[close + 2..].find(')')
            {
                let label = &rest[open..close];
                let url = rest[close + 2..close + 2 + end].trim();
                if !label.contains('\n') && !url.contains(' ') && !url.is_empty() {
                    push_plain(&mut plain, &mut out);
                    out.push(Inline::Link { text: if label.is_empty() { url.to_string() } else { strip(label) }, url: url.to_string() });
                    let next = at + close + 2 + end + 1;
                    k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                    continue;
                }
            }
        }
        // ***both***, **strong** / __strong__, *emphasis* / _emphasis_
        if c == '*' || c == '_' {
            let run = rest.chars().take_while(|x| *x == c).count().min(3);
            let marker = &rest[..run];
            let before = if k == 0 { None } else { Some(chars[k - 1].1) };
            let after = rest[run..].chars().next();
            // `_` inside a word (snake_case) is not markup.
            let opens = after.is_some_and(|a| !a.is_whitespace()) && !(c == '_' && before.is_some_and(word));
            if opens && let Some(end) = rest[run..].find(marker) {
                let inner = &rest[run..run + end];
                let closes = !inner.is_empty() && !inner.ends_with(char::is_whitespace) && !(c == '_' && rest[run + end + run..].chars().next().is_some_and(word));
                if closes {
                    push_plain(&mut plain, &mut out);
                    out.push(match run {
                        3 => Inline::StrongEmphasis(inner.to_string()),
                        2 => Inline::Strong(inner.to_string()),
                        _ => Inline::Emphasis(inner.to_string()),
                    });
                    let next = at + run + end + run;
                    k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                    continue;
                }
            }
        }
        // ~~struck~~ shows as plain text.
        if c == '~' && rest.starts_with("~~") && let Some(end) = rest[2..].find("~~") {
            plain.push_str(&rest[2..2 + end]);
            let next = at + 2 + end + 2;
            k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
            continue;
        }
        // A backslash keeps the next character literal.
        if c == '\\' && k + 1 < chars.len() && chars[k + 1].1.is_ascii_punctuation() {
            plain.push(chars[k + 1].1);
            k += 2;
            continue;
        }
        plain.push(c);
        k += 1;
    }
    push_plain(&mut plain, &mut out);
    out
}

/// The text of some inline markdown with the markup taken out, for places that show plain text.
pub fn strip(text: &str) -> String {
    inlines(text)
        .into_iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) | Inline::Code(t) => t,
            Inline::Link { text, .. } => text,
        })
        .collect()
}

/// The whole document as plain text: what a notch line or a spoken reply should use.
pub fn plain(text: &str) -> String {
    parse(text)
        .into_iter()
        .map(|b| match b {
            Block::Heading(_, i) | Block::Paragraph(i) | Block::Quote(i) => join(&i),
            Block::List { items, .. } => items.iter().map(|i| join(i)).collect::<Vec<_>>().join("; "),
            Block::Code { text, .. } => text,
            Block::Rule => String::new(),
            Block::Image { alt, .. } => alt,
            Block::Table { head, rows } => std::iter::once(&head).chain(rows.iter()).map(|r| r.iter().map(|c| join(c)).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("; "),
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn join(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(t) | Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) | Inline::Code(t) => t.as_str(),
            Inline::Link { text, .. } => text.as_str(),
        })
        .collect()
}
