//! Input method composition: the uncommitted text shown at the caret, and mapping between offsets
//! in the text and in what is shown.

use super::*;

impl Editor {
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
