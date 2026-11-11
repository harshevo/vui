//! The pointer and scrolling: placing the caret, selecting by dragging, and keeping the caret in
//! view.

use super::*;

impl<M: 'static> TextFieldWidget<M> {
    /// Keeps the scroll offset inside the text.
    pub(super) fn clamp_scroll(&self, scroll: Vec2, surface: &Surface, inner: Rect) -> Vec2 {
        let text = surface.layout.size();
        if self.spec.multiline {
            vec2(0.0, scroll.y.clamp(0.0, (text.h - inner.h).max(0.0)))
        } else {
            vec2(scroll.x.clamp(0.0, (text.w + CARET_W - inner.w).max(0.0)), 0.0)
        }
    }

    /// Scrolls until the caret is in view and tells the input method where it is.
    pub(super) fn reveal(&self, state: &mut FieldState, cx: &mut EventCx<M>) {
        let laid = self.laid(state, cx.env, cx.content);
        let caret = laid.surface.caret(&state.editor);
        let view = laid.frame.inner;
        let mut scroll = state.scroll;
        if self.spec.multiline {
            if caret.y < scroll.y {
                scroll.y = caret.y;
            } else if caret.bottom() > scroll.y + view.h {
                scroll.y = caret.bottom() - view.h;
            }
        } else if caret.x < scroll.x {
            scroll.x = caret.x;
        } else if caret.x + CARET_W > scroll.x + view.w {
            scroll.x = caret.x + CARET_W - view.w;
        }
        state.scroll = self.clamp_scroll(scroll, &laid.surface, view);
        if cx.focused {
            let at = rect(view.x + caret.x - state.scroll.x, view.y + caret.y - state.scroll.y, CARET_W, caret.h);
            cx.set_ime_area(Some(at));
        }
    }

    pub(super) fn pointer_down(&self, state: &mut FieldState, cx: &mut EventCx<M>, pos: Point, mods: Modifiers, count: u8) -> bool {
        let revision = state.editor.revision();
        let laid = self.laid(state, cx.env, cx.content);
        if let Some(clear) = laid.frame.clear
            && clear.inflate(4.0, 4.0).contains(pos)
        {
            state.editor.select_all();
            state.editor.delete(Dir::Back, Unit::Grapheme);
            self.after(state, cx, revision);
            return true;
        }
        let local = pt(pos.x - laid.frame.inner.x + state.scroll.x, pos.y - laid.frame.inner.y + state.scroll.y);
        let at = laid.surface.hit(&state.editor, local);
        let whole = 0..state.editor.text().len();
        let editor = &mut state.editor;
        match count {
            0 | 1 => {
                let anchor = if mods.shift { editor.anchor() } else { at };
                editor.set_selection(anchor, at);
                state.drag = Some(Drag { unit: Unit::Grapheme, anchor: anchor..anchor });
            }
            2 => {
                let range = if self.spec.password { whole } else { editor.word_range_at(laid.surface.char_at(editor, local)) };
                editor.set_selection(range.start, range.end);
                state.drag = Some(Drag { unit: Unit::Word, anchor: range });
            }
            _ => {
                let range = if self.spec.multiline && !self.spec.password { editor.paragraph_range_at(at) } else { whole };
                editor.set_selection(range.start, range.end);
                state.drag = Some(Drag { unit: Unit::Paragraph, anchor: range });
            }
        }
        self.after(state, cx, revision);
        true
    }

    pub(super) fn pointer_moved(&self, state: &mut FieldState, cx: &mut EventCx<M>, pos: Point) -> bool {
        let Some(drag) = state.drag.clone() else {
            // Not pressed: only the clear button reacts to where the pointer is.
            let style = self.style(&cx.env.theme);
            let frame = self.frame(cx.content, &style, cx.env.scale, self.show_clear(&state.editor));
            let over = frame.clear.is_some_and(|c| c.inflate(4.0, 4.0).contains(pos));
            if over != state.clear_hover {
                state.clear_hover = over;
                cx.request_paint();
            }
            return false;
        };
        let revision = state.editor.revision();
        let laid = self.laid(state, cx.env, cx.content);
        let local = pt(pos.x - laid.frame.inner.x + state.scroll.x, pos.y - laid.frame.inner.y + state.scroll.y);
        let whole = 0..state.editor.text().len();
        let editor = &mut state.editor;
        match drag.unit {
            Unit::Word | Unit::Paragraph => {
                let under = if self.spec.password {
                    whole
                } else if drag.unit == Unit::Word {
                    editor.word_range_at(laid.surface.char_at(editor, local))
                } else if self.spec.multiline {
                    editor.paragraph_range_at(laid.surface.hit(editor, local))
                } else {
                    whole
                };
                // The word or paragraph first selected stays selected; the selection reaches whichever
                // whole unit the pointer is over.
                let (anchor, head) = if under.start < drag.anchor.start {
                    (drag.anchor.end, under.start)
                } else if under.end > drag.anchor.end {
                    (drag.anchor.start, under.end)
                } else {
                    (drag.anchor.start, drag.anchor.end)
                };
                editor.set_selection(anchor, head);
            }
            _ => {
                let at = laid.surface.hit(editor, local);
                editor.set_selection(drag.anchor.start, at);
            }
        }
        self.after(state, cx, revision);
        true
    }

    pub(super) fn scroll(&self, state: &mut FieldState, cx: &mut EventCx<M>, delta: Vec2) -> bool {
        let laid = self.laid(state, cx.env, cx.content);
        let current = state.scroll;
        // Content moves with the fingers, so the text slides the other way over the window.
        let wanted = if self.spec.multiline { vec2(0.0, current.y - delta.y) } else { vec2(current.x - delta.x, 0.0) };
        let next = self.clamp_scroll(wanted, &laid.surface, laid.frame.inner);
        if next == current {
            return false;
        }
        state.scroll = next;
        cx.request_paint();
        true
    }
}
