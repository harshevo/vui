//! Changing the text: inserting, deleting and cutting, cleaned to what the field accepts, and the
//! undo history that groups typing into words.

use super::*;

impl Editor {
    /// Makes `text` fit: line breaks become spaces in a single line, control characters go, and
    /// only as much as the character limit leaves room for stays. `replacing` is how many characters
    /// the insertion replaces.
    pub(super) fn clean(&self, text: &str, replacing: usize) -> String {
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

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot { text: self.text.clone(), anchor: self.anchor, head: self.head }
    }

    /// Replaces `range` with `with`, remembering how to undo it unless it continues the last edit.
    pub(super) fn edit(&mut self, range: Range<usize>, with: &str, group: Option<Group>) {
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
    pub(super) fn insert_as(&mut self, text: &str, typed: bool) -> bool {
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

    pub(super) fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.revision += 1;
        self.anchor = s.anchor;
        self.head = s.head;
        self.group = None;
        self.goal_x = None;
        self.preedit = None;
    }
}
