//! The text editing model: a string, a selection, and what the editing keys do to them. Nothing here
//! draws or measures; the few moves that depend on how text wraps (up, down, line ends) are handed
//! the layout.

use std::borrow::Cow;
use std::ops::Range;

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use vui_core::{Point, TextLayout, pt};

/// Which way a move or a deletion goes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dir {
    Back,
    Forward,
}

/// How far a move or a deletion reaches.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Unit {
    /// One character as the reader sees it: an emoji or a letter with its accents is one.
    Grapheme,
    /// To the next edge of a word.
    Word,
    /// To the edge of the paragraph, the text between two line breaks.
    Paragraph,
    /// To the edge of the text.
    Document,
}

/// Text being composed by an input method and not yet part of the text: shown with the caret inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    /// The part of the preedit the input method has highlighted, as byte offsets into it.
    pub cursor: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    text: String,
    anchor: usize,
    head: usize,
}

/// Edits that undo together when they follow one another: a run of typing, a run of backspaces.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Group {
    Typing,
    DeleteBack,
    DeleteForward,
}

/// How many steps of history are kept.
const HISTORY: usize = 200;

/// A text, a selection in it, and its history. Positions are byte offsets on grapheme boundaries.
#[derive(Clone, Debug)]
pub struct Editor {
    text: String,
    /// Where the selection started and where its moving end, the caret, is.
    anchor: usize,
    head: usize,
    /// The column up and down keep returning to across lines of different lengths.
    goal_x: Option<f32>,
    multiline: bool,
    max_chars: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    group: Option<Group>,
    preedit: Option<Preedit>,
    /// Counts the changes to the text, so a caller can tell whether an operation edited it.
    revision: u64,
}

/// The grapheme boundary before `pos`, or 0.
fn prev_boundary(text: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let pos = text.floor_char_boundary(pos.min(text.len()));
    GraphemeCursor::new(pos, text.len(), true).prev_boundary(text, 0).ok().flatten().unwrap_or(0)
}

/// The grapheme boundary after `pos`, or the end.
fn next_boundary(text: &str, pos: usize) -> usize {
    if pos >= text.len() {
        return text.len();
    }
    let pos = text.floor_char_boundary(pos);
    GraphemeCursor::new(pos, text.len(), true).next_boundary(text, 0).ok().flatten().unwrap_or(text.len())
}

/// `pos` if it is a grapheme boundary, else the one before it.
fn snap(text: &str, pos: usize) -> usize {
    let pos = text.floor_char_boundary(pos.min(text.len()));
    if pos == 0 || pos == text.len() || GraphemeCursor::new(pos, text.len(), true).is_boundary(text, 0).unwrap_or(true) {
        pos
    } else {
        prev_boundary(text, pos)
    }
}

impl Editor {
    pub fn new() -> Editor {
        Editor {
            text: String::new(),
            anchor: 0,
            head: 0,
            goal_x: None,
            multiline: false,
            max_chars: None,
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
            preedit: None,
            revision: 0,
        }
    }

    /// An editor holding `text` with the caret at its end.
    pub fn with_text(text: &str) -> Editor {
        let mut e = Editor::new();
        e.set_text(text);
        e.move_caret(Dir::Forward, Unit::Document, false);
        e
    }

    /// Whether line breaks can be typed. Without it they become spaces.
    pub fn set_multiline(&mut self, on: bool) {
        self.multiline = on;
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// The most characters the text can hold.
    pub fn set_max_chars(&mut self, max: Option<usize>) {
        self.max_chars = max;
    }

    // ---- reading --------------------------------------------------------------------------------

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// A number that changes whenever the text does.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Where the caret is: the end of the selection that moves.
    pub fn caret(&self) -> usize {
        self.head
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.head
    }

    /// The selected bytes in order, empty when there is only a caret.
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    /// The anchor and the caret as character counts, which is how assistive technology counts.
    pub fn selection_chars(&self) -> (usize, usize) {
        let chars = |byte: usize| self.text[..byte].chars().count();
        (chars(self.anchor), chars(self.head))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ---- setting --------------------------------------------------------------------------------

    /// Replaces the whole text from outside, as an app does when it changes the value it passes in. The
    /// selection stays where it can, and a caret at the end stays at the end. The history starts over
    /// because it describes another text.
    pub fn set_text(&mut self, text: &str) {
        if self.text == text {
            return;
        }
        let at_end = self.anchor == self.text.len() && self.head == self.text.len();
        self.text = text.to_string();
        self.revision += 1;
        if at_end {
            self.anchor = self.text.len();
            self.head = self.text.len();
        } else {
            self.anchor = snap(&self.text, self.anchor);
            self.head = snap(&self.text, self.head);
        }
        self.undo.clear();
        self.redo.clear();
        self.group = None;
        self.goal_x = None;
    }

    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.anchor = snap(&self.text, anchor);
        self.head = snap(&self.text, head);
        self.group = None;
        self.goal_x = None;
    }

    pub fn select_all(&mut self) {
        self.set_selection(0, self.text.len());
    }

    /// The word containing the character that starts at `byte`, or the run of spaces or punctuation
    /// there. At the end of the text it is the last word, and a line break is no word at all.
    pub fn word_range_at(&self, byte: usize) -> Range<usize> {
        let byte = if byte >= self.text.len() { prev_boundary(&self.text, self.text.len()) } else { byte };
        self.text
            .split_word_bound_indices()
            .map(|(i, w)| i..i + w.len())
            .find(|r| r.start <= byte && byte < r.end)
            .filter(|r| !self.text[r.clone()].contains(['\n', '\r']))
            .unwrap_or(byte..byte)
    }

    /// The paragraph containing `byte`, without its line break.
    pub fn paragraph_range_at(&self, byte: usize) -> Range<usize> {
        let byte = byte.min(self.text.len());
        self.paragraph_start(byte)..self.paragraph_end(byte)
    }

    /// Selects the word at `byte`: what a double click does.
    pub fn select_word_at(&mut self, byte: usize) {
        let range = self.word_range_at(byte);
        self.set_selection(range.start, range.end);
    }

    /// Selects the paragraph at `byte`: what a triple click does.
    pub fn select_paragraph_at(&mut self, byte: usize) {
        let range = self.paragraph_range_at(byte);
        self.set_selection(range.start, range.end);
    }

    // ---- moving ---------------------------------------------------------------------------------

    fn paragraph_start(&self, from: usize) -> usize {
        self.text[..from].rfind('\n').map_or(0, |i| i + 1)
    }

    fn paragraph_end(&self, from: usize) -> usize {
        self.text[from..].find('\n').map_or(self.text.len(), |i| from + i)
    }

    /// The start of the word before `from`: the word it is inside, or the one before.
    fn word_start_before(&self, from: usize) -> usize {
        self.text.unicode_word_indices().map(|(i, _)| i).take_while(|&i| i < from).last().unwrap_or(0)
    }

    /// The end of the word after `from`: the word it is inside, or the one after.
    fn word_end_after(&self, from: usize) -> usize {
        self.text.unicode_word_indices().map(|(i, w)| i + w.len()).find(|&end| end > from).unwrap_or(self.text.len())
    }

    fn reach(&self, from: usize, dir: Dir, unit: Unit) -> usize {
        match (unit, dir) {
            (Unit::Grapheme, Dir::Back) => prev_boundary(&self.text, from),
            (Unit::Grapheme, Dir::Forward) => next_boundary(&self.text, from),
            (Unit::Word, Dir::Back) => self.word_start_before(from),
            (Unit::Word, Dir::Forward) => self.word_end_after(from),
            (Unit::Paragraph, Dir::Back) => self.paragraph_start(from),
            (Unit::Paragraph, Dir::Forward) => self.paragraph_end(from),
            (Unit::Document, Dir::Back) => 0,
            (Unit::Document, Dir::Forward) => self.text.len(),
        }
    }

    fn move_head(&mut self, to: usize, extend: bool) {
        self.head = to;
        if !extend {
            self.anchor = to;
        }
        self.group = None;
    }

    /// Moves the caret, or with `extend` the selection's moving end. A plain left or right with a
    /// selection collapses it to that side instead of moving.
    pub fn move_caret(&mut self, dir: Dir, unit: Unit, extend: bool) {
        self.goal_x = None;
        if !extend && unit == Unit::Grapheme && self.has_selection() {
            let edge = match dir {
                Dir::Back => self.selection().start,
                Dir::Forward => self.selection().end,
            };
            self.move_head(edge, false);
            return;
        }
        let to = self.reach(self.head, dir, unit);
        self.move_head(to, extend);
    }

    /// Moves up or down by visual lines, staying in the column the first move started from. Past the
    /// first or last line it goes to the start or end of the text. `layout` is the layout of `text()`.
    pub fn move_lines(&mut self, lines: i32, extend: bool, layout: &TextLayout) {
        let caret = layout.caret_rect(self.head);
        let x = *self.goal_x.get_or_insert(caret.x);
        let y = caret.y + caret.h * 0.5 + lines as f32 * caret.h;
        let to = if y < 0.0 {
            0
        } else if y >= layout.size().h {
            self.text.len()
        } else {
            snap(&self.text, layout.hit(pt(x, y)))
        };
        self.move_head(to, extend);
    }

    /// Moves to the start or end of the visual line the caret is on.
    pub fn move_line_edge(&mut self, forward: bool, extend: bool, layout: &TextLayout) {
        self.goal_x = None;
        let caret = layout.caret_rect(self.head);
        let y = caret.y + caret.h * 0.5;
        let mut to = snap(&self.text, layout.hit(pt(if forward { layout.size().w + 1.0e4 } else { -1.0 }, y)));
        // The end of a wrapped line is also the start of the next one, and the caret shows there.
        if forward && layout.caret_rect(to).y > caret.y + caret.h * 0.5 {
            to = prev_boundary(&self.text, to);
        }
        self.move_head(to, extend);
    }

    // ---- editing --------------------------------------------------------------------------------

    /// Makes `text` fit: line breaks become spaces in a single line, control characters go, and
    /// only as much as the character limit leaves room for stays. `replacing` is how many characters
    /// the insertion replaces.
    fn clean(&self, text: &str, replacing: usize) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' | '\n' => {
                    if c == '\r' && chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    out.push(if self.multiline { '\n' } else { ' ' });
                }
                '\t' => out.push(if self.multiline { '\t' } else { ' ' }),
                c if c.is_control() => {}
                c => out.push(c),
            }
        }
        if let Some(max) = self.max_chars {
            let room = max.saturating_sub(self.text.chars().count() - replacing);
            if out.chars().count() > room {
                let mut kept = String::new();
                let mut count = 0;
                for g in out.graphemes(true) {
                    count += g.chars().count();
                    if count > room {
                        break;
                    }
                    kept.push_str(g);
                }
                out = kept;
            }
        }
        out
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot { text: self.text.clone(), anchor: self.anchor, head: self.head }
    }

    /// Replaces `range` with `with`, remembering how to undo it unless it continues the last edit.
    fn edit(&mut self, range: Range<usize>, with: &str, group: Option<Group>) {
        if group.is_none() || self.group != group {
            self.undo.push(self.snapshot());
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
        }
        self.group = group;
        self.redo.clear();
        self.text.replace_range(range.clone(), with);
        self.revision += 1;
        self.head = range.start + with.len();
        self.anchor = self.head;
        self.goal_x = None;
    }

    /// Types or pastes `text` over the selection. Returns false if nothing changed.
    pub fn insert(&mut self, text: &str) -> bool {
        self.insert_as(text, false)
    }

    /// `typed` is for text that arrives like keystrokes whatever its length, as an input method's does.
    fn insert_as(&mut self, text: &str, typed: bool) -> bool {
        let range = self.selection();
        let replacing = self.text[range.clone()].chars().count();
        let clean = self.clean(text, replacing);
        if clean.is_empty() && range.is_empty() {
            return false;
        }
        let typing = typed || clean.graphemes(true).count() == 1;
        self.edit(range, &clean, typing.then_some(Group::Typing));
        true
    }

    /// Deletes the selection, or if there is none, what lies `unit` away in `dir`. Returns false if
    /// there was nothing to delete.
    pub fn delete(&mut self, dir: Dir, unit: Unit) -> bool {
        let selected = self.has_selection();
        let range = if selected {
            self.selection()
        } else {
            let mut to = self.reach(self.head, dir, unit);
            if to == self.head && unit == Unit::Paragraph {
                // At the edge of a paragraph, deleting to it takes the line break.
                to = self.reach(self.head, dir, Unit::Grapheme);
            }
            match dir {
                Dir::Back => to..self.head,
                Dir::Forward => self.head..to,
            }
        };
        if range.is_empty() {
            return false;
        }
        let group = match (selected, unit, dir) {
            (false, Unit::Grapheme, Dir::Back) => Some(Group::DeleteBack),
            (false, Unit::Grapheme, Dir::Forward) => Some(Group::DeleteForward),
            _ => None,
        };
        self.edit(range, "", group);
        true
    }

    /// Removes the selection and returns it, for Cut.
    pub fn cut(&mut self) -> Option<String> {
        let text = self.has_selection().then(|| self.selected_text().to_string())?;
        self.delete(Dir::Back, Unit::Grapheme);
        Some(text)
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else { return false };
        self.redo.push(self.snapshot());
        self.restore(previous);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(self.snapshot());
        self.restore(next);
        true
    }

    fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.revision += 1;
        self.anchor = s.anchor;
        self.head = s.head;
        self.group = None;
        self.goal_x = None;
        self.preedit = None;
    }

    // ---- input methods --------------------------------------------------------------------------

    /// Shows `text` being composed at the caret. Starting one replaces the selection; empty text ends
    /// the composition.
    pub fn set_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        if text.is_empty() {
            self.preedit = None;
            return;
        }
        if self.preedit.is_none() && self.has_selection() {
            // The selection goes as the composition starts and what is committed joins that step.
            let range = self.selection();
            self.edit(range, "", Some(Group::Typing));
        }
        let cursor = cursor.map(|(a, b)| (text.floor_char_boundary(a.min(text.len())), text.floor_char_boundary(b.min(text.len()))));
        self.preedit = Some(Preedit { text: text.to_string(), cursor });
    }

    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }

    pub fn clear_preedit(&mut self) {
        self.preedit = None;
    }

    /// Ends the composition and types what the input method settled on.
    pub fn commit(&mut self, text: &str) -> bool {
        self.preedit = None;
        self.insert_as(text, true)
    }

    /// What to draw: the text with the composition at the caret.
    pub fn display_text(&self) -> Cow<'_, str> {
        match &self.preedit {
            None => Cow::Borrowed(&self.text),
            Some(p) => Cow::Owned(format!("{}{}{}", &self.text[..self.head], p.text, &self.text[self.head..])),
        }
    }

    /// Where the caret is drawn in `display_text`: inside the composition if the input method put it
    /// there.
    pub fn display_caret(&self) -> usize {
        match &self.preedit {
            None => self.head,
            Some(p) => self.head + p.cursor.map_or(p.text.len(), |(_, end)| end),
        }
    }

    /// The composition's place in `display_text`.
    pub fn preedit_range(&self) -> Option<Range<usize>> {
        self.preedit.as_ref().map(|p| self.head..self.head + p.text.len())
    }

    /// A position in `text` as a position in `display_text`.
    pub fn to_display(&self, byte: usize) -> usize {
        match &self.preedit {
            Some(p) if byte > self.head => byte + p.text.len(),
            _ => byte,
        }
    }

    /// A position in `display_text` as a position in `text`. Inside the composition it is the caret.
    pub fn from_display(&self, byte: usize) -> usize {
        match &self.preedit {
            Some(p) if byte > self.head => {
                if byte < self.head + p.text.len() {
                    self.head
                } else {
                    byte - p.text.len()
                }
            }
            _ => byte,
        }
    }
}

impl Default for Editor {
    fn default() -> Editor {
        Editor::new()
    }
}

/// The character a click at `point` (relative to the layout) landed on, as the byte where it starts.
/// `TextLayout::hit` finds the nearest caret position instead, which for the right half of a letter is
/// the edge after it; selecting a word has to start from the letter. Past the end of a line it is the
/// last letter on that line. Assumes text that runs left to right.
pub fn char_at(text: &str, layout: &TextLayout, point: Point) -> usize {
    let point = pt(point.x, point.y.clamp(0.0, (layout.size().h - 1.0).max(0.0)));
    let at = snap(text, layout.hit(point));
    let edge = layout.caret_rect(at);
    // The caret can sit at the start of the next line when a wrapped line ends there: that is still
    // past the end of this one.
    let mut target = if point.x < edge.x || point.y < edge.y { prev_boundary(text, at) } else { at };
    let line_ended = target >= text.len() || text[target..].starts_with(['\n', '\r']);
    if line_ended && target > 0 && !text[..target].ends_with('\n') {
        target = prev_boundary(text, target);
    }
    target
}

#[cfg(test)]
mod tests {
    use vui_core::{Fonts, TextStyle, Theme};

    use super::*;

    fn ed(text: &str, anchor: usize, head: usize) -> Editor {
        let mut e = Editor::new();
        e.set_multiline(true);
        e.set_text(text);
        e.set_selection(anchor, head);
        e
    }

    fn caret(text: &str, at: usize) -> Editor {
        ed(text, at, at)
    }

    fn layout(text: &str, width: Option<f32>) -> TextLayout {
        let fonts = Fonts::bundled_only();
        fonts.layout(text, &TextStyle::from_token(Theme::light().types.body), width)
    }

    // ---- inserting and deleting

    #[test]
    fn typing_inserts_at_the_caret_and_moves_it() {
        let mut e = Editor::new();
        for c in "héllo".chars() {
            assert!(e.insert(&c.to_string()));
        }
        assert_eq!((e.text(), e.caret(), e.anchor()), ("héllo", 6, 6));
        e.set_selection(1, 1);
        e.insert("X");
        assert_eq!((e.text(), e.caret()), ("hXéllo", 2));
    }

    #[test]
    fn inserting_over_a_selection_replaces_it() {
        let mut e = ed("hello world", 6, 11);
        e.insert("there");
        assert_eq!((e.text(), e.caret(), e.has_selection()), ("hello there", 11, false));
        let mut e = ed("hello world", 11, 6);
        e.insert("");
        assert_eq!(e.text(), "hello ", "a backwards selection works too");
        assert!(!Editor::new().insert(""), "nothing to do");
    }

    #[test]
    fn a_single_line_turns_line_breaks_into_spaces_and_drops_control_characters() {
        let mut e = Editor::new();
        e.insert("a\r\nb\nc\rd\te\u{7}f\u{0}");
        assert_eq!(e.text(), "a b c d ef");
        let mut m = Editor::new();
        m.set_multiline(true);
        m.insert("a\r\nb\rc\td\u{7}");
        assert_eq!(m.text(), "a\nb\nc\td");
    }

    #[test]
    fn the_character_limit_counts_characters_and_keeps_whole_graphemes() {
        let mut e = Editor::new();
        e.set_max_chars(Some(5));
        e.insert("abcdefgh");
        assert_eq!(e.text(), "abcde");
        assert!(!e.insert("x"), "full");
        e.set_selection(1, 3);
        e.insert("XYZ");
        assert_eq!(e.text(), "aXYde", "replacing frees room: two out, two in");
        let mut f = Editor::new();
        f.set_max_chars(Some(3));
        f.insert("ab👨‍👩‍👧");
        assert_eq!(f.text(), "ab", "a family emoji is 5 characters, and half of one is never kept");
    }

    #[test]
    fn backspace_deletes_a_whole_grapheme() {
        for (text, left) in [("abc👍🏽", "abc"), ("abe\u{301}", "ab"), ("a🇫🇷", "a"), ("a👨‍👩‍👧", "a"), ("a\r\n", "a")] {
            let mut e = caret(text, text.len());
            assert!(e.delete(Dir::Back, Unit::Grapheme), "{text:?}");
            assert_eq!(e.text(), left, "{text:?}");
        }
        assert!(!caret("abc", 0).delete(Dir::Back, Unit::Grapheme));
    }

    #[test]
    fn forward_delete_takes_the_grapheme_after_the_caret() {
        let mut e = caret("e\u{301}x", 0);
        e.delete(Dir::Forward, Unit::Grapheme);
        assert_eq!(e.text(), "x");
        assert!(!caret("abc", 3).delete(Dir::Forward, Unit::Grapheme));
    }

    #[test]
    fn deleting_with_a_selection_removes_just_the_selection() {
        let mut e = ed("hello world", 2, 8);
        assert!(e.delete(Dir::Back, Unit::Word));
        assert_eq!((e.text(), e.caret()), ("herld", 2));
        let mut e = ed("hello world", 2, 8);
        assert!(e.delete(Dir::Forward, Unit::Grapheme));
        assert_eq!(e.text(), "herld");
    }

    #[test]
    fn deleting_by_word_goes_back_to_the_start_of_the_word() {
        let mut e = caret("one two  three", 14);
        e.delete(Dir::Back, Unit::Word);
        assert_eq!(e.text(), "one two  ");
        e.delete(Dir::Back, Unit::Word);
        assert_eq!(e.text(), "one ");
        let mut e = caret("one two", 6);
        e.delete(Dir::Back, Unit::Word);
        assert_eq!(e.text(), "one o", "from the middle of a word, back to its start");
        let mut e = caret("one two", 1);
        e.delete(Dir::Forward, Unit::Word);
        assert_eq!(e.text(), "o two", "forward takes the rest of the word");
    }

    #[test]
    fn deleting_by_paragraph_takes_a_line_break_at_the_edge() {
        let mut e = caret("ab\ncd", 4);
        e.delete(Dir::Back, Unit::Paragraph);
        assert_eq!(e.text(), "ab\nd");
        e.delete(Dir::Back, Unit::Paragraph);
        assert_eq!(e.text(), "abd", "at the start of a paragraph, the break before it goes");
        let mut e = caret("ab\ncd", 2);
        e.delete(Dir::Forward, Unit::Paragraph);
        assert_eq!(e.text(), "abcd", "at the end of a paragraph, the break after it goes");
        assert!(!caret("ab", 2).delete(Dir::Forward, Unit::Paragraph), "and at the end of the text there is nothing");
    }

    #[test]
    fn cut_returns_what_it_removed() {
        let mut e = ed("hello world", 0, 5);
        assert_eq!(e.cut().as_deref(), Some("hello"));
        assert_eq!(e.text(), " world");
        assert_eq!(e.cut(), None, "no selection, nothing cut");
    }

    // ---- moving

    #[test]
    fn arrows_step_over_graphemes() {
        let text = "a👨‍👩‍👧b";
        let mut e = caret(text, 0);
        let mut stops = vec![0];
        for _ in 0..3 {
            e.move_caret(Dir::Forward, Unit::Grapheme, false);
            stops.push(e.caret());
        }
        assert_eq!(stops, [0, 1, 1 + "👨‍👩‍👧".len(), text.len()]);
        e.move_caret(Dir::Forward, Unit::Grapheme, false);
        assert_eq!(e.caret(), text.len(), "the end stays the end");
        e.move_caret(Dir::Back, Unit::Grapheme, false);
        assert_eq!(e.caret(), 1 + "👨‍👩‍👧".len());
    }

    #[test]
    fn a_plain_arrow_collapses_a_selection_to_its_edge_without_moving_past_it() {
        let mut e = ed("hello world", 2, 8);
        e.move_caret(Dir::Back, Unit::Grapheme, false);
        assert_eq!((e.caret(), e.anchor()), (2, 2));
        let mut e = ed("hello world", 8, 2);
        e.move_caret(Dir::Forward, Unit::Grapheme, false);
        assert_eq!((e.caret(), e.anchor()), (8, 8));
    }

    #[test]
    fn shift_extends_from_the_anchor_either_way() {
        let mut e = caret("hello", 2);
        e.move_caret(Dir::Forward, Unit::Grapheme, true);
        e.move_caret(Dir::Forward, Unit::Grapheme, true);
        assert_eq!((e.anchor(), e.caret(), e.selection()), (2, 4, 2..4));
        for _ in 0..4 {
            e.move_caret(Dir::Back, Unit::Grapheme, true);
        }
        assert_eq!((e.anchor(), e.caret(), e.selection()), (2, 0, 0..2), "the caret crossed the anchor");
        assert_eq!(e.selected_text(), "he");
    }

    #[test]
    fn word_moves_land_on_word_edges() {
        let mut e = caret("one, two  three", 0);
        let mut stops = Vec::new();
        for _ in 0..3 {
            e.move_caret(Dir::Forward, Unit::Word, false);
            stops.push(e.caret());
        }
        assert_eq!(stops, [3, 8, 15], "to the end of each word");
        let mut stops = Vec::new();
        for _ in 0..3 {
            e.move_caret(Dir::Back, Unit::Word, false);
            stops.push(e.caret());
        }
        assert_eq!(stops, [10, 5, 0], "to the start of each word");
    }

    #[test]
    fn a_word_move_with_no_words_goes_to_the_end() {
        let mut e = caret("... !!", 1);
        e.move_caret(Dir::Forward, Unit::Word, false);
        assert_eq!(e.caret(), 6);
        e.move_caret(Dir::Back, Unit::Word, false);
        assert_eq!(e.caret(), 0);
    }

    #[test]
    fn paragraph_and_document_moves() {
        let mut e = caret("ab\ncd\nef", 4);
        e.move_caret(Dir::Back, Unit::Paragraph, false);
        assert_eq!(e.caret(), 3);
        e.move_caret(Dir::Forward, Unit::Paragraph, false);
        assert_eq!(e.caret(), 5);
        e.move_caret(Dir::Back, Unit::Document, true);
        assert_eq!((e.anchor(), e.caret()), (5, 0));
        e.move_caret(Dir::Forward, Unit::Document, false);
        assert_eq!((e.anchor(), e.caret()), (8, 8));
    }

    #[test]
    fn selecting_by_double_and_triple_click() {
        let mut e = caret("hello big world", 0);
        e.select_word_at(7);
        assert_eq!(e.selected_text(), "big");
        e.select_word_at(5);
        assert_eq!(e.selected_text(), " ", "between words it takes the space");
        e.select_word_at(15);
        assert_eq!(e.selected_text(), "world", "at the very end, the last word");
        let mut e = caret("one\ntwo three\nfour", 0);
        e.select_paragraph_at(6);
        assert_eq!(e.selected_text(), "two three");
        e.select_paragraph_at(0);
        assert_eq!(e.selected_text(), "one");
        e.select_paragraph_at(18);
        assert_eq!(e.selected_text(), "four");
        e.select_all();
        assert_eq!(e.selection(), 0..18);
    }

    #[test]
    fn a_double_click_on_a_line_break_selects_nothing() {
        let mut e = caret("a\n\nb", 0);
        e.select_word_at(1);
        assert_eq!(e.selection(), 1..1);
        e.select_word_at(2);
        assert_eq!(e.selection(), 2..2, "an empty line has no word");
        assert_eq!(caret("", 0).word_range_at(0), 0..0);
    }

    #[test]
    fn a_click_lands_on_the_letter_under_it_not_the_nearest_edge() {
        let text = "hello world";
        let l = layout(text, None);
        let x = |b: usize| l.caret_rect(b).x;
        let y = 5.0;
        assert_eq!(char_at(text, &l, pt(x(0) + 1.0, y)), 0);
        assert_eq!(char_at(text, &l, pt(x(5) - 1.0, y)), 4, "the right half of the o is still the o");
        assert_eq!(char_at(text, &l, pt(x(5) + 1.0, y)), 5, "and just past it is the space");
        assert_eq!(char_at(text, &l, pt(x(11) + 50.0, y)), 10, "past the end it is the last letter");
        assert_eq!(char_at(text, &l, pt(-20.0, y)), 0, "and before the start the first");
        assert_eq!(char_at(text, &l, pt(x(5) - 1.0, -30.0)), 4, "above the line counts as on it");
    }

    #[test]
    fn past_the_end_of_a_line_it_is_the_last_letter_on_that_line() {
        let text = "ab\n\ncd";
        let l = layout(text, None);
        let line = |n: f32| l.caret_rect(0).h * (n + 0.5);
        assert_eq!(char_at(text, &l, pt(300.0, line(0.0))), 1, "past ab");
        assert_eq!(char_at(text, &l, pt(5.0, line(1.0))), 3, "the empty line has only its break");
        assert_eq!(char_at(text, &l, pt(300.0, line(2.0))), 5, "past cd");
    }

    #[test]
    fn at_a_soft_wrap_a_click_past_the_end_of_the_first_line_stays_on_it() {
        let text = "aaaa bbbb cccc dddd eeee";
        let l = layout(text, Some(80.0));
        assert!(l.line_count() >= 2);
        let h = l.caret_rect(0).h;
        let second_line_start = l.hit(pt(0.0, h * 1.5));
        assert!(char_at(text, &l, pt(79.0, h * 0.5)) < second_line_start);
    }

    #[test]
    fn positions_snap_to_grapheme_boundaries() {
        let text = "ae\u{301}b";
        let mut e = caret(text, 0);
        e.set_selection(2, 100);
        assert_eq!((e.anchor(), e.caret()), (1, text.len()), "inside the é, and past the end");
    }

    // ---- lines

    #[test]
    fn up_and_down_keep_their_column_across_short_lines() {
        let text = "hello world\nhi\nsecond line";
        let l = layout(text, None);
        let mut e = caret(text, 8);
        e.move_lines(1, false, &l);
        assert_eq!(e.caret(), 14, "the short line ends before the column");
        e.move_lines(1, false, &l);
        assert!((e.caret() as i32 - 23).abs() <= 1, "back to the column on the long one: {}", e.caret());
        e.move_lines(-2, false, &l);
        assert!((e.caret() as i32 - 8).abs() <= 1, "two lines up in one go: {}", e.caret());
    }

    #[test]
    fn up_from_the_first_line_goes_to_the_start_and_down_from_the_last_to_the_end() {
        let text = "one\ntwo";
        let l = layout(text, None);
        let mut e = caret(text, 2);
        e.move_lines(-1, false, &l);
        assert_eq!(e.caret(), 0);
        e.move_lines(1, false, &l);
        e.move_lines(1, false, &l);
        assert_eq!(e.caret(), 7);
        e.move_lines(-1, true, &l);
        assert_eq!(e.anchor(), 7, "extending keeps the anchor");
    }

    #[test]
    fn a_trailing_line_break_gives_a_last_empty_line_to_move_to() {
        let text = "ab\n";
        let l = layout(text, None);
        let mut e = caret(text, 1);
        e.move_lines(1, false, &l);
        assert_eq!(e.caret(), 3);
    }

    #[test]
    fn line_edges_are_visual_lines_in_wrapped_text() {
        let text = "the quick brown fox jumps over the lazy dog";
        let l = layout(text, Some(90.0));
        assert!(l.line_count() >= 3);
        let mut e = caret(text, 22);
        e.move_line_edge(false, false, &l);
        let start = e.caret();
        assert!(start > 0 && start < 22, "start of the wrapped line: {start}");
        assert_eq!(l.caret_rect(start).y, l.caret_rect(22).y);
        e.set_selection(22, 22);
        e.move_line_edge(true, false, &l);
        let end = e.caret();
        assert!(end > 22 && end < text.len());
        assert_eq!(l.caret_rect(end).y, l.caret_rect(22).y, "the end of the line stays on the line");
        // And from there, End again does not wander onto the next line.
        e.move_line_edge(true, false, &l);
        assert_eq!(e.caret(), end);
    }

    // ---- history

    #[test]
    fn typing_is_one_undo_step_and_undo_restores_the_selection_too() {
        let mut e = ed("abc", 1, 2);
        e.insert("X");
        e.insert("Y");
        e.insert("Z");
        assert_eq!(e.text(), "aXYZc");
        assert!(e.undo());
        assert_eq!((e.text(), e.anchor(), e.caret()), ("abc", 1, 2), "the replaced selection comes back with it");
    }

    #[test]
    fn typed_runs_undo_together_but_a_caret_move_breaks_the_run() {
        let mut e = Editor::new();
        for c in "abc".chars() {
            e.insert(&c.to_string());
        }
        e.move_caret(Dir::Back, Unit::Grapheme, false);
        for c in "XY".chars() {
            e.insert(&c.to_string());
        }
        assert_eq!(e.text(), "abXYc");
        e.undo();
        assert_eq!(e.text(), "abc");
        e.undo();
        assert_eq!(e.text(), "");
        assert!(!e.undo());
    }

    #[test]
    fn redo_replays_and_a_new_edit_forgets_it() {
        let mut e = Editor::new();
        e.insert("a");
        e.move_caret(Dir::Back, Unit::Document, false);
        e.insert("b");
        e.undo();
        assert!(e.can_redo());
        assert!(e.redo());
        assert_eq!(e.text(), "ba");
        e.undo();
        e.insert("c");
        assert!(!e.can_redo(), "typing after an undo ends the redo history");
        assert!(!e.redo());
    }

    #[test]
    fn backspaces_undo_together_and_forward_deletes_are_their_own_run() {
        let mut e = caret("hello", 5);
        for _ in 0..3 {
            e.delete(Dir::Back, Unit::Grapheme);
        }
        e.delete(Dir::Forward, Unit::Grapheme);
        assert_eq!(e.text(), "he");
        e.undo();
        e.undo();
        assert_eq!(e.text(), "hello", "the three backspaces went back as one step, and the forward delete as another");
    }

    #[test]
    fn replacing_the_text_from_outside_clears_history_and_keeps_the_caret_where_it_can() {
        let mut e = Editor::new();
        e.insert("hello");
        e.set_text("hello");
        assert!(e.can_undo(), "the same text is not a change");
        e.set_text("hi");
        assert!(!e.can_undo());
        assert_eq!((e.caret(), e.text()), (2, "hi"), "the caret was at the end, so it stays there");
        e.set_selection(1, 1);
        e.set_text("hello there");
        assert_eq!(e.caret(), 1, "and one in the middle stays where it was");
        e.set_text("h");
        assert_eq!(e.caret(), 1, "or lands on the end if the text no longer reaches it");
        e.set_selection(0, 1);
        e.set_text("abc");
        assert_eq!((e.anchor(), e.caret()), (0, 1), "a selection is kept as well");
    }

    #[test]
    fn history_is_bounded() {
        let mut e = Editor::new();
        for i in 0..HISTORY + 50 {
            e.insert("x");
            e.set_selection(i, i);
        }
        let mut steps = 0;
        while e.undo() {
            steps += 1;
        }
        assert_eq!(steps, HISTORY);
    }

    // ---- input methods

    #[test]
    fn a_composition_shows_inside_the_text_and_commits_into_it() {
        let mut e = caret("ab", 1);
        e.set_preedit("かん", Some((0, 3)));
        assert_eq!(e.display_text(), "aかんb");
        assert_eq!(e.text(), "ab", "the real text is untouched");
        assert_eq!(e.display_caret(), 1 + 3, "the caret sits where the input method said");
        assert_eq!(e.preedit_range(), Some(1..7));
        assert_eq!((e.to_display(0), e.to_display(1), e.to_display(2)), (0, 1, 8));
        assert_eq!((e.from_display(0), e.from_display(1), e.from_display(4), e.from_display(8)), (0, 1, 1, 2), "inside the composition is the caret");
        assert!(e.commit("漢字"));
        assert_eq!((e.text(), e.caret(), e.preedit()), ("a漢字b", 7, None));
    }

    #[test]
    fn the_caret_defaults_to_the_end_of_a_composition() {
        let mut e = Editor::new();
        e.set_preedit("abc", None);
        assert_eq!(e.display_caret(), 3);
        e.set_preedit("", None);
        assert_eq!(e.preedit(), None, "empty text ends it");
        assert_eq!(e.display_text(), "");
    }

    #[test]
    fn starting_a_composition_replaces_the_selection() {
        let mut e = ed("hello", 1, 4);
        e.set_preedit("x", None);
        assert_eq!((e.text(), e.caret()), ("ho", 1));
        e.set_preedit("xy", None);
        assert_eq!(e.text(), "ho", "going on with it does not eat more");
        e.clear_preedit();
        e.undo();
        assert_eq!(e.text(), "hello", "and undo gets the selection back");
    }

    #[test]
    fn a_composition_over_a_selection_undoes_as_one_step() {
        let mut e = ed("hello", 1, 4);
        e.set_preedit("か", None);
        e.commit("漢");
        assert_eq!(e.text(), "h漢o");
        assert!(e.undo());
        assert_eq!((e.text(), e.anchor(), e.caret()), ("hello", 1, 4));
        assert!(!e.undo());
    }

    #[test]
    fn committed_words_join_the_typing_run() {
        let mut e = Editor::new();
        e.commit("今日");
        e.commit("は");
        assert_eq!(e.text(), "今日は");
        e.undo();
        assert_eq!(e.text(), "", "one step, like typing");
    }

    #[test]
    fn a_preedit_cursor_is_clamped_to_the_preedit() {
        let mut e = Editor::new();
        e.set_preedit("é", Some((1, 99)));
        assert_eq!(e.preedit().and_then(|p| p.cursor), Some((0, 2)), "inside the é is moved to its start, past the end to the end");
    }

    #[test]
    fn selection_counts_characters_not_bytes() {
        let e = ed("héllo→x", 1, 7);
        assert_eq!(e.selection_chars(), (1, 5));
    }

    #[test]
    fn undo_ends_a_composition() {
        let mut e = Editor::new();
        e.insert("a");
        e.move_caret(Dir::Back, Unit::Document, false);
        e.insert("b");
        e.set_preedit("x", None);
        e.undo();
        assert_eq!((e.text(), e.preedit()), ("a", None));
    }
}
