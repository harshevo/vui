//! Moving the caret and growing the selection: by character, word, line and paragraph.

use super::*;

impl Editor {
    pub(super) fn paragraph_start(&self, from: usize) -> usize {
        self.text[..from].rfind('\n').map_or(0, |i| i + 1)
    }

    pub(super) fn paragraph_end(&self, from: usize) -> usize {
        self.text[from..].find('\n').map_or(self.text.len(), |i| from + i)
    }

    /// The start of the word before `from`: the word it is inside, or the one before.
    pub(super) fn word_start_before(&self, from: usize) -> usize {
        self.text.unicode_word_indices().map(|(i, _)| i).take_while(|&i| i < from).last().unwrap_or(0)
    }

    /// The end of the word after `from`: the word it is inside, or the one after.
    pub(super) fn word_end_after(&self, from: usize) -> usize {
        self.text.unicode_word_indices().map(|(i, w)| i + w.len()).find(|&end| end > from).unwrap_or(self.text.len())
    }

    pub(super) fn reach(&self, from: usize, dir: Dir, unit: Unit) -> usize {
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

    pub(super) fn move_head(&mut self, to: usize, extend: bool) {
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
}
