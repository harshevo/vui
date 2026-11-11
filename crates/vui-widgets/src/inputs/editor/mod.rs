//! The text editing model: a string, a selection, and what the editing keys do to them. Nothing here
//! draws or measures; the few moves that depend on how text wraps (up, down, line ends) are handed
//! the layout.

use std::borrow::Cow;
use std::ops::Range;

use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use vui_core::{Point, TextLayout, pt};

mod changes;
mod composition;
mod movement;

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

    // ---- editing --------------------------------------------------------------------------------

    // ---- input methods --------------------------------------------------------------------------
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
mod tests;
