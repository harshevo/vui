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
#[path = "editor/tests.rs"]
mod tests;
