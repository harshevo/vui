//! Editing through the keyboard and input methods: keys, typed characters, Enter, the clipboard
//! and undo, and the caret blink that restarts with every change.

use super::*;

impl<M: 'static> TextFieldWidget<M> {
    pub(super) fn restart_blink(&self, state: &mut FieldState, cx: &mut EventCx<M>) {
        state.blink_epoch = cx.time;
        state.caret_on = true;
        cx.request_wake(BLINK);
    }

    /// What every change ends with: tell the app about new text, keep the caret in view, and show the
    /// caret again rather than in the middle of a blink.
    pub(super) fn after(&self, state: &mut FieldState, cx: &mut EventCx<M>, revision: u64) {
        let edited = state.editor.revision() != revision;
        if edited && let Some(on_change) = &self.on_change {
            cx.emit(on_change(state.editor.text().to_string()));
        }
        self.reveal(state, cx);
        self.restart_blink(state, cx);
        if edited && self.spec.multiline && self.spec.max_rows.is_some() {
            cx.request_layout();
        } else {
            cx.request_paint();
        }
    }

    pub(super) fn focus_changed(&self, state: &mut FieldState, cx: &mut EventCx<M>, focused: bool, by: FocusBy) {
        state.focus.set_target(if focused { 1.0 } else { 0.0 });
        cx.request_frame();
        if focused {
            // Arriving by keyboard selects a single line to type over; a click places the caret itself.
            let select = by != FocusBy::Pointer && (self.spec.select_on_focus || (by == FocusBy::Tab && !self.spec.multiline));
            if select {
                state.editor.select_all();
            }
            self.reveal(state, cx);
            self.restart_blink(state, cx);
        } else {
            // A composition ends with focus: what was typed so far is kept.
            if let Some(text) = state.editor.preedit().map(|p| p.text.clone()) {
                state.editor.commit(&text);
                if let Some(on_change) = &self.on_change {
                    cx.emit(on_change(state.editor.text().to_string()));
                }
            }
            state.drag = None;
            state.clear_hover = false;
        }
        cx.request_paint();
    }

    pub(super) fn key(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        if !key.pressed {
            return false;
        }
        let revision = state.editor.revision();
        let used = self.apply_key(state, cx, key);
        if used {
            self.after(state, cx, revision);
        }
        used
    }

    pub(super) fn apply_key(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
        let (m, multi, editable) = (key.mods, self.spec.multiline, !self.spec.read_only);
        let extend = m.shift;
        // A password's words are not for anyone looking over a shoulder to count.
        let word = if self.spec.password { Unit::Document } else { Unit::Word };
        let Key::Named(named) = key.key else {
            let Key::Character(c) = key.key else { return false };
            return self.character(state, cx, key, c);
        };
        match named {
            NamedKey::ArrowLeft | NamedKey::ArrowRight => {
                let dir = if named == NamedKey::ArrowLeft { Dir::Back } else { Dir::Forward };
                if m.cmd {
                    self.line_edge(state, cx, dir == Dir::Forward, extend);
                } else {
                    state.editor.move_caret(dir, if m.alt { word } else { Unit::Grapheme }, extend);
                }
                true
            }
            NamedKey::ArrowUp | NamedKey::ArrowDown => {
                let down = named == NamedKey::ArrowDown;
                let dir = if down { Dir::Forward } else { Dir::Back };
                if !multi || m.cmd {
                    state.editor.move_caret(dir, Unit::Document, extend);
                } else if m.alt {
                    state.editor.move_caret(dir, Unit::Paragraph, extend);
                } else {
                    self.lines(state, cx, if down { 1 } else { -1 }, extend);
                }
                true
            }
            NamedKey::Home | NamedKey::End => {
                let end = named == NamedKey::End;
                if multi && !m.cmd {
                    self.line_edge(state, cx, end, extend);
                } else {
                    state.editor.move_caret(if end { Dir::Forward } else { Dir::Back }, Unit::Document, extend);
                }
                true
            }
            NamedKey::PageUp | NamedKey::PageDown => {
                let down = named == NamedKey::PageDown;
                if multi {
                    let laid = self.laid(state, cx.env, cx.content);
                    let rows = (laid.frame.inner.h / laid.style.line_height).floor().max(1.0) as i32;
                    self.lines(state, cx, if down { rows } else { -rows }, extend);
                } else {
                    state.editor.move_caret(if down { Dir::Forward } else { Dir::Back }, Unit::Document, extend);
                }
                true
            }
            NamedKey::Backspace | NamedKey::Delete if editable => {
                let dir = if named == NamedKey::Backspace { Dir::Back } else { Dir::Forward };
                let unit = if m.cmd {
                    Unit::Paragraph
                } else if m.alt {
                    word
                } else {
                    Unit::Grapheme
                };
                state.editor.delete(dir, unit);
                true
            }
            NamedKey::Enter => self.enter(state, cx, m),
            NamedKey::Space if editable && !m.cmd && !m.ctrl => {
                let text = key.text.as_deref().filter(|t| !t.is_empty()).unwrap_or(" ");
                state.editor.insert(text);
                true
            }
            // Tab and Escape belong to whoever is around the field.
            _ => false,
        }
    }

    pub(super) fn enter(&self, state: &mut FieldState, cx: &mut EventCx<M>, m: Modifiers) -> bool {
        if self.spec.multiline && !m.cmd {
            if self.spec.read_only {
                return false;
            }
            state.editor.insert("\n");
            return true;
        }
        match &self.on_submit {
            Some(on_submit) => {
                cx.emit(on_submit(state.editor.text().to_string()));
                true
            }
            None => false,
        }
    }

    /// Keys that make a letter: command shortcuts, the control-key editing of macOS text fields, and
    /// typing.
    pub(super) fn character(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent, c: char) -> bool {
        let (m, editable) = (key.mods, !self.spec.read_only);
        let editor = &mut state.editor;
        if m.cmd {
            match c {
                'a' => editor.select_all(),
                // What is typed into a password field never goes on the clipboard.
                'c' => {
                    if editor.has_selection() && !self.spec.password {
                        cx.set_clipboard_text(editor.selected_text());
                    }
                }
                'x' if editable => {
                    if !self.spec.password
                        && let Some(cut) = editor.cut()
                    {
                        cx.set_clipboard_text(&cut);
                    }
                }
                'v' if editable => {
                    if let Some(text) = cx.clipboard_text() {
                        editor.insert(&text);
                    } else if let Some(on_paste) = &self.on_paste_image
                        && let Some(png) = cx.clipboard_image()
                    {
                        cx.emit(on_paste(png));
                    }
                }
                'z' if editable => {
                    if m.shift {
                        editor.redo();
                    } else {
                        editor.undo();
                    }
                }
                _ => return false,
            }
            return true;
        }
        if m.ctrl {
            let extend = m.shift;
            match c {
                'a' => self.line_edge(state, cx, false, extend),
                'e' => self.line_edge(state, cx, true, extend),
                'f' => state.editor.move_caret(Dir::Forward, Unit::Grapheme, extend),
                'b' => state.editor.move_caret(Dir::Back, Unit::Grapheme, extend),
                'n' if self.spec.multiline => self.lines(state, cx, 1, extend),
                'p' if self.spec.multiline => self.lines(state, cx, -1, extend),
                'k' if editable => {
                    state.editor.delete(Dir::Forward, Unit::Paragraph);
                }
                'd' if editable => {
                    state.editor.delete(Dir::Forward, Unit::Grapheme);
                }
                'h' if editable => {
                    state.editor.delete(Dir::Back, Unit::Grapheme);
                }
                _ => return false,
            }
            return true;
        }
        match key.text.as_deref() {
            Some(text) if editable && !text.is_empty() => {
                editor.insert(text);
                true
            }
            _ => false,
        }
    }

    /// Up or down by visual lines.
    pub(super) fn lines(&self, state: &mut FieldState, cx: &mut EventCx<M>, lines: i32, extend: bool) {
        let laid = self.laid(state, cx.env, cx.content);
        state.editor.move_lines(lines, extend, &laid.surface.layout);
    }

    /// To the start or end of the line the caret is on. A single line has no other ends than the text's.
    pub(super) fn line_edge(&self, state: &mut FieldState, cx: &mut EventCx<M>, forward: bool, extend: bool) {
        if self.spec.multiline {
            let laid = self.laid(state, cx.env, cx.content);
            state.editor.move_line_edge(forward, extend, &laid.surface.layout);
        } else {
            state.editor.move_caret(if forward { Dir::Forward } else { Dir::Back }, Unit::Document, extend);
        }
    }

    pub(super) fn ime(&self, state: &mut FieldState, cx: &mut EventCx<M>, ime: &ImeEvent) -> bool {
        if self.spec.read_only {
            return false;
        }
        let revision = state.editor.revision();
        match ime {
            ImeEvent::Preedit { text, cursor } => state.editor.set_preedit(text, *cursor),
            ImeEvent::Commit(text) => {
                state.editor.commit(text);
            }
            ImeEvent::Cancel => state.editor.clear_preedit(),
        }
        self.after(state, cx, revision);
        true
    }

    pub(super) fn action(&self, state: &mut FieldState, cx: &mut EventCx<M>, action: &AccessAction) -> bool {
        match action {
            AccessAction::Click => {
                cx.focus();
                true
            }
            AccessAction::SetValue(text) if !self.spec.read_only => {
                let revision = state.editor.revision();
                state.editor.select_all();
                state.editor.insert(text);
                self.after(state, cx, revision);
                true
            }
            _ => false,
        }
    }
}
