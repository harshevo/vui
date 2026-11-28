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
    blocks(text, false)
}

/// Splits text that is still streaming in into blocks, showing what its end has started the way it
/// will look once finished, never as markup: bold, emphasis or code still open runs to the end, a
/// link shows its words before its address has come, and a last line that so far only starts a
/// list item, heading, quote, table, rule or code block waits until it has words of its own.
pub fn parse_streaming(text: &str) -> Vec<Block> {
    blocks(text, true)
}

/// Whether the last line of streaming text only starts a block so far. `before` is the lines ahead
/// of it.
fn unfinished(line: &str, before: &[&str]) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let lone = |c: char| t.chars().all(|x| x == c);
    let digits = t.trim_end_matches(['.', ')']);
    let fenced = |l: &str| ["```", "~~~"].iter().any(|f| l.trim_start().starts_with(f));
    // A fence opens a block when the fences before it are paired; one after an odd count closes it.
    let opens = fenced(t) && before.iter().filter(|l| fenced(l)).count() % 2 == 0;
    let row = |l: &str| l.trim().starts_with('|');
    // A table's head waits for the line under it, and any of its lines for words or dashes.
    let head = row(t) && !before.last().is_some_and(|l| row(l));
    let bare = row(t) && t.chars().all(|c| matches!(c, '|' | ':' | ' '));
    matches!(t, "-" | "*" | "+" | "•" | ">")
        || (t.len() <= 6 && lone('#'))
        || (!digits.is_empty() && digits.len() <= 9 && digits.bytes().all(|b| b.is_ascii_digit()) && digits.len() + 1 >= t.len())
        || (t.len() <= 2 && lone('-'))
        || (t.len() <= 3 && (lone('*') || lone('_')))
        || opens
        || head
        || bare
}

/// Whether runs have any text to show.
fn shows(runs: &[Inline]) -> bool {
    !join(runs).trim().is_empty()
}

fn blocks(text: &str, streaming: bool) -> Vec<Block> {
    let mut lines: Vec<&str> = text.lines().collect();
    while streaming && lines.split_last().is_some_and(|(last, before)| unfinished(last, before)) {
        lines.pop();
    }
    let end = lines.len();
    // The block that holds the last line of streaming text keeps what that line opened open, and
    // shows only once it has words.
    let runs = |text: &str, tail: bool| inline_runs(text, streaming && tail);
    let keep = |runs: &[Inline], tail: bool| !(streaming && tail) || shows(runs);
    let mut blocks = Vec::new();
    let mut i = 0;
    let mut para: Vec<&str> = Vec::new();
    let flush = |para: &mut Vec<&str>, blocks: &mut Vec<Block>, tail: bool| {
        if !para.is_empty() {
            let text = runs(&para.join(" "), tail);
            if keep(&text, tail) {
                blocks.push(Block::Paragraph(text));
            }
            para.clear();
        }
    };
    while i < end {
        let line = lines[i];
        let t = line.trim();
        if t.is_empty() {
            flush(&mut para, &mut blocks, false);
            i += 1;
            continue;
        }
        if let Some(fence) = ["```", "~~~"].into_iter().find(|f| t.starts_with(f)) {
            flush(&mut para, &mut blocks, false);
            let lang = t[3..].trim().to_string();
            let mut body = Vec::new();
            i += 1;
            while i < end && !lines[i].trim_start().starts_with(fence) {
                body.push(lines[i]);
                i += 1;
            }
            // Still open, its closing fence half typed is not code.
            if streaming && i >= end && body.last().is_some_and(|l| !l.trim().is_empty() && fence.starts_with(l.trim())) {
                body.pop();
            }
            blocks.push(Block::Code { lang, text: body.join("\n") });
            i += 1;
            continue;
        }
        if let Some((level, text)) = heading(t) {
            flush(&mut para, &mut blocks, false);
            let tail = i + 1 == end;
            let text = runs(text, tail);
            if keep(&text, tail) {
                blocks.push(Block::Heading(level, text));
            }
            i += 1;
            continue;
        }
        if rule(t) && para.is_empty() {
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        if let Some((alt, src)) = image(t) {
            flush(&mut para, &mut blocks, false);
            blocks.push(Block::Image { alt, src });
            i += 1;
            continue;
        }
        if t.starts_with('|') && i + 1 < end && table_divider(lines[i + 1]) {
            flush(&mut para, &mut blocks, false);
            let head = cells(t).iter().map(|c| inlines(c)).collect();
            let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
            i += 2;
            while i < end && lines[i].trim().starts_with('|') {
                let tail = i + 1 == end;
                let row = cells(lines[i]);
                let last = row.len().saturating_sub(1);
                rows.push(row.iter().enumerate().map(|(n, c)| runs(c, tail && n == last)).collect());
                i += 1;
            }
            if streaming && i == end && rows.last().is_some_and(|r| !r.iter().any(|c| shows(c))) {
                rows.pop();
            }
            blocks.push(Block::Table { head, rows });
            continue;
        }
        if t.starts_with('>') {
            flush(&mut para, &mut blocks, false);
            let mut quote = Vec::new();
            while i < end && lines[i].trim().starts_with('>') {
                quote.push(lines[i].trim().trim_start_matches('>').trim());
                i += 1;
            }
            let text = runs(&quote.join(" "), i == end);
            if keep(&text, i == end) {
                blocks.push(Block::Quote(text));
            }
            continue;
        }
        if bullet(line).is_some() || numbered(line).is_some() {
            flush(&mut para, &mut blocks, false);
            let start = numbered(line).map(|(n, _)| n);
            let mut texts: Vec<String> = Vec::new();
            while i < end {
                let l = lines[i];
                let item = if start.is_some() { numbered(l).map(|(_, r)| r) } else { bullet(l) };
                if let Some(item) = item {
                    texts.push(item.trim().to_string());
                } else if !l.trim().is_empty() && l.starts_with([' ', '\t']) && !texts.is_empty() {
                    // A wrapped or nested line joins the item above it.
                    let last = texts.last_mut().expect("checked");
                    let more = bullet(l).or(numbered(l).map(|(_, r)| r)).unwrap_or(l.trim());
                    last.push_str(if bullet(l).is_some() || numbered(l).is_some() { " · " } else { " " });
                    last.push_str(more.trim());
                } else {
                    break;
                }
                i += 1;
            }
            let tail = i == end;
            let last = texts.len().saturating_sub(1);
            let mut items: Vec<Vec<Inline>> = texts.iter().enumerate().map(|(n, s)| runs(s, tail && n == last)).collect();
            if items.last().is_some_and(|item| !keep(item, tail)) {
                items.pop();
            }
            if !items.is_empty() {
                blocks.push(Block::List { start, items });
            }
            continue;
        }
        para.push(t);
        i += 1;
    }
    flush(&mut para, &mut blocks, true);
    blocks
}

/// Splits a line into styled runs.
pub fn inlines(text: &str) -> Vec<Inline> {
    inline_runs(text, false)
}

/// Splits a line into styled runs. With `tail` the line is the end of text still streaming in: what
/// it has opened and not closed yet shows the way it will once closed, and markup with nothing
/// after it yet shows nothing.
fn inline_runs(text: &str, tail: bool) -> Vec<Inline> {
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
            // Still open: code to the end, without a closing fence half typed.
            if tail {
                push_plain(&mut plain, &mut out);
                let code = rest[ticks..].trim_end_matches('`').trim();
                if !code.is_empty() {
                    out.push(Inline::Code(code.to_string()));
                }
                break;
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
            if tail {
                match rest[open..].find(']') {
                    // The words of a link still coming show as they come, its bracket not.
                    None => {
                        k += open;
                        continue;
                    }
                    // Its address still coming, or perhaps about to: the words alone.
                    Some(close) if { rest[open + close + 1..].is_empty() || (rest[open + close + 1..].starts_with('(') && !rest[open + close + 1..].contains(')')) } => {
                        plain.push_str(&strip(&rest[open..open + close]));
                        break;
                    }
                    _ => {}
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
            let found = rest[run..].find(marker);
            if opens && let Some(end) = found {
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
            if tail {
                // Opened at the start of a word and not closed yet: styled to the end, without a
                // closing marker half typed.
                if opens && found.is_none() && before.is_none_or(|b| !word(b)) {
                    push_plain(&mut plain, &mut out);
                    let inner = rest[run..].trim_end_matches(c).to_string();
                    out.push(match run {
                        3 => Inline::StrongEmphasis(inner),
                        2 => Inline::Strong(inner),
                        _ => Inline::Emphasis(inner),
                    });
                    break;
                }
                // A marker with nothing after it yet.
                if after.is_none() && !(c == '_' && before.is_some_and(word)) {
                    break;
                }
            }
        }
        // ~~struck~~ shows as plain text.
        if c == '~' && rest.starts_with("~~") {
            if let Some(end) = rest[2..].find("~~") {
                plain.push_str(&rest[2..2 + end]);
                let next = at + 2 + end + 2;
                k = chars.iter().position(|(i, _)| *i >= next).unwrap_or(chars.len());
                continue;
            }
            if tail {
                k += 2;
                continue;
            }
        }
        // A backslash keeps the next character literal.
        if c == '\\' && k + 1 < chars.len() && chars[k + 1].1.is_ascii_punctuation() {
            plain.push(chars[k + 1].1);
            k += 2;
            continue;
        }
        // The first of a pair of tildes, or a backslash, with nothing after it yet.
        if tail && k + 1 == chars.len() && matches!(c, '~' | '\\') {
            break;
        }
        plain.push(c);
        k += 1;
    }
    push_plain(&mut plain, &mut out);
    out.retain(|i| !matches!(i, Inline::Strong(t) | Inline::Emphasis(t) | Inline::StrongEmphasis(t) if t.is_empty()));
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
    flatten(parse(text))
}

/// [`plain`] of text still streaming in: what its end has opened shows the way it will once closed.
pub fn plain_streaming(text: &str) -> String {
    flatten(parse_streaming(text))
}

fn flatten(blocks: Vec<Block>) -> String {
    blocks
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
