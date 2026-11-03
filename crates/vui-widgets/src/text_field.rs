//! Text fields: one line or several, with a caret, selection, clipboard, undo and input methods.
//!
//! A field is controlled like everything else in vui: `text_field(value)` shows what the app passes in,
//! typing sends the new text to `on_change`, and the app passes it back on its next `view`. The caret,
//! selection, scroll position and undo history live in the widget's state, which survives those
//! rebuilds. When the app changes the text from outside, the editor takes the new text and keeps the
//! caret where it can.

use std::borrow::Cow;
use std::ops::Range;
use std::rc::Rc;

use unicode_segmentation::UnicodeSegmentation;
use vui_core::{
    Corners, CursorIcon, Event, Fill, FontFamily, Fonts, ImeEvent, Key, KeyEvent, Modifiers, NamedKey, Point, PointerButton, Quad,
    Rect, Size, Spring, SpringValue, TextLayout, TextStyle, TextWrap, Theme, Tone, Vec2, pt, rect, size, vec2,
};
use vui_icons::{Icon, Weight, set};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, FocusBy, PaintCx, Radius, Role, TickCx,
    Widget, WidgetEvent, builder,
};

use crate::editor::{Dir, Editor, Unit, char_at};
use crate::icon::paint_icon;

/// Space between the edge of the field and its text.
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 8.0;
/// The leading icon and the clear button.
const ICON: f32 = 16.0;
const GAP: f32 = 8.0;
const CARET_W: f32 = 1.0;
const DEFAULT_WIDTH: f32 = 240.0;
const MIN_WIDTH: f32 = 96.0;
/// Seconds the caret stays on, and then off.
const BLINK: f32 = 0.53;
const BULLET: char = '•';

type Handler<M> = Rc<dyn Fn(String) -> M>;

/// What sets one kind of field apart from another. Changing any of it can change the field's size.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Spec {
    multiline: bool,
    /// Lines a multi-line field is tall for, and how many it grows to as text is added.
    rows: usize,
    max_rows: Option<usize>,
    password: bool,
    read_only: bool,
    search: bool,
    clearable: bool,
    mono: bool,
    large: bool,
    select_on_focus: bool,
    max_chars: Option<usize>,
    leading: Option<Icon>,
}

/// The widget behind [`TextField`].
pub struct TextFieldWidget<M> {
    value: String,
    placeholder: String,
    on_change: Option<Handler<M>>,
    on_submit: Option<Handler<M>>,
    on_paste_image: Option<Rc<dyn Fn(Vec<u8>) -> M>>,
    spec: Spec,
}

/// A drag that started on a word or a paragraph selects by whole words or paragraphs.
#[derive(Clone)]
struct Drag {
    unit: Unit,
    anchor: Range<usize>,
}

/// What a text field keeps between updates.
pub struct FieldState {
    editor: Editor,
    /// How far the text is scrolled up and to the left inside the field.
    scroll: Vec2,
    /// 0 to 1: how much the border has gone over to the accent.
    focus: SpringValue,
    blink_epoch: f32,
    caret_on: bool,
    drag: Option<Drag>,
    clear_hover: bool,
    baseline: f32,
}

/// Where the parts of a field are.
struct Frame {
    /// Where text is laid out and clipped: inside the padding, the leading icon and the clear button.
    inner: Rect,
    leading: Option<Rect>,
    clear: Option<Rect>,
}

/// The text as laid out: what the editor shows, with bullets for a password, and the conversions
/// between the editor's positions and the layout's.
struct Surface {
    layout: Rc<TextLayout>,
    /// The editor's display text, whose bytes `Editor::to_display` counts.
    shown: String,
    masked: bool,
}

impl Surface {
    fn new(editor: &Editor, fonts: &Fonts, style: &TextStyle, wrap: Option<f32>, masked: bool) -> Surface {
        let shown = editor.display_text().into_owned();
        let layout = {
            let laid: Cow<str> =
                if masked { Cow::Owned(BULLET.to_string().repeat(shown.graphemes(true).count())) } else { Cow::Borrowed(&shown) };
            fonts.layout_cached(&laid, style, wrap)
        };
        Surface { layout, shown, masked }
    }

    /// A position in the editor's display text as a position in the layout.
    fn to_layout(&self, shown: usize) -> usize {
        if self.masked {
            let at = self.shown.floor_char_boundary(shown.min(self.shown.len()));
            self.shown[..at].graphemes(true).count() * BULLET.len_utf8()
        } else {
            shown
        }
    }

    /// A position in the layout as a position in the editor's display text.
    fn to_shown(&self, laid: usize) -> usize {
        if self.masked {
            let n = laid / BULLET.len_utf8();
            self.shown.grapheme_indices(true).nth(n).map_or(self.shown.len(), |(i, _)| i)
        } else {
            laid
        }
    }

    fn caret(&self, editor: &Editor) -> Rect {
        self.layout.caret_rect(self.to_layout(editor.display_caret()))
    }

    fn selection_rects(&self, editor: &Editor) -> Vec<Rect> {
        let range = editor.selection();
        self.layout.selection_rects(self.to_layout(editor.to_display(range.start)), self.to_layout(editor.to_display(range.end)))
    }

    fn preedit_rects(&self, editor: &Editor) -> Vec<Rect> {
        editor
            .preedit_range()
            .map(|r| self.layout.selection_rects(self.to_layout(r.start), self.to_layout(r.end)))
            .unwrap_or_default()
    }

    /// The text position nearest `p`, which is relative to the layout.
    fn hit(&self, editor: &Editor, p: Point) -> usize {
        editor.from_display(self.to_shown(self.layout.hit(p)))
    }

    /// The text position of the character under `p`.
    fn char_at(&self, editor: &Editor, p: Point) -> usize {
        editor.from_display(self.to_shown(char_at(self.layout.text(), &self.layout, p)))
    }
}

/// The parts of a field worked out together for one call.
struct Laid {
    style: TextStyle,
    frame: Frame,
    surface: Surface,
}

impl<M: 'static> TextFieldWidget<M> {
    fn style(&self, theme: &Theme) -> TextStyle {
        let mut style = TextStyle::from_token(theme.types.body);
        if self.spec.mono {
            style.family = FontFamily::Mono;
        }
        if !self.spec.multiline {
            style.wrap = TextWrap::None;
        }
        style
    }

    fn show_clear(&self, editor: &Editor) -> bool {
        self.spec.clearable && !self.spec.read_only && !editor.is_empty()
    }

    fn frame(&self, content: Rect, style: &TextStyle, scale: f32, show_clear: bool) -> Frame {
        let snap = |v: f32| (v * scale).round() / scale;
        let line = style.line_height;
        let top = snap(if self.spec.multiline { content.y + PAD_Y } else { content.y + (content.h - line) / 2.0 });
        let icon_y = snap(top + (line - ICON) / 2.0);
        let mut left = content.x + PAD_X;
        let mut right = content.right() - PAD_X;
        let leading = self.spec.leading.map(|_| {
            let at = rect(left, icon_y, ICON, ICON);
            left += ICON + GAP;
            at
        });
        let clear = show_clear.then(|| {
            right -= ICON;
            let at = rect(right, icon_y, ICON, ICON);
            right -= GAP;
            at
        });
        let height = if self.spec.multiline { (content.bottom() - PAD_Y - top).max(line) } else { line };
        Frame { inner: rect(left, top, (right - left).max(0.0), height), leading, clear }
    }

    fn laid(&self, state: &FieldState, env: &Env, content: Rect) -> Laid {
        let style = self.style(&env.theme);
        let frame = self.frame(content, &style, env.scale, self.show_clear(&state.editor));
        let wrap = self.spec.multiline.then_some(frame.inner.w);
        let surface = Surface::new(&state.editor, env.fonts(), &style, wrap, self.spec.password);
        Laid { style, frame, surface }
    }

    /// Keeps the scroll offset inside the text.
    fn clamp_scroll(&self, scroll: Vec2, surface: &Surface, inner: Rect) -> Vec2 {
        let text = surface.layout.size();
        if self.spec.multiline {
            vec2(0.0, scroll.y.clamp(0.0, (text.h - inner.h).max(0.0)))
        } else {
            vec2(scroll.x.clamp(0.0, (text.w + CARET_W - inner.w).max(0.0)), 0.0)
        }
    }

    /// Scrolls until the caret is in view and tells the input method where it is.
    fn reveal(&self, state: &mut FieldState, cx: &mut EventCx<M>) {
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

    fn restart_blink(&self, state: &mut FieldState, cx: &mut EventCx<M>) {
        state.blink_epoch = cx.time;
        state.caret_on = true;
        cx.request_wake(BLINK);
    }

    /// What every change ends with: tell the app about new text, keep the caret in view, and show the
    /// caret again rather than in the middle of a blink.
    fn after(&self, state: &mut FieldState, cx: &mut EventCx<M>, revision: u64) {
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

    // ---- focus ----------------------------------------------------------------------------------

    fn focus_changed(&self, state: &mut FieldState, cx: &mut EventCx<M>, focused: bool, by: FocusBy) {
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

    // ---- keyboard -------------------------------------------------------------------------------

    fn key(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
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

    fn apply_key(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent) -> bool {
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

    fn enter(&self, state: &mut FieldState, cx: &mut EventCx<M>, m: Modifiers) -> bool {
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
    fn character(&self, state: &mut FieldState, cx: &mut EventCx<M>, key: &KeyEvent, c: char) -> bool {
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
    fn lines(&self, state: &mut FieldState, cx: &mut EventCx<M>, lines: i32, extend: bool) {
        let laid = self.laid(state, cx.env, cx.content);
        state.editor.move_lines(lines, extend, &laid.surface.layout);
    }

    /// To the start or end of the line the caret is on. A single line has no other ends than the text's.
    fn line_edge(&self, state: &mut FieldState, cx: &mut EventCx<M>, forward: bool, extend: bool) {
        if self.spec.multiline {
            let laid = self.laid(state, cx.env, cx.content);
            state.editor.move_line_edge(forward, extend, &laid.surface.layout);
        } else {
            state.editor.move_caret(if forward { Dir::Forward } else { Dir::Back }, Unit::Document, extend);
        }
    }

    // ---- pointer --------------------------------------------------------------------------------

    fn pointer_down(&self, state: &mut FieldState, cx: &mut EventCx<M>, pos: Point, mods: Modifiers, count: u8) -> bool {
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

    fn pointer_moved(&self, state: &mut FieldState, cx: &mut EventCx<M>, pos: Point) -> bool {
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

    fn scroll(&self, state: &mut FieldState, cx: &mut EventCx<M>, delta: Vec2) -> bool {
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

    // ---- input methods and assistive technology -------------------------------------------------

    fn ime(&self, state: &mut FieldState, cx: &mut EventCx<M>, ime: &ImeEvent) -> bool {
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

    fn action(&self, state: &mut FieldState, cx: &mut EventCx<M>, action: &AccessAction) -> bool {
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

impl<M: 'static> Widget<M> for TextFieldWidget<M> {
    type State = FieldState;

    fn create_state(&self) -> FieldState {
        let mut editor = Editor::new();
        editor.set_multiline(self.spec.multiline);
        editor.set_max_chars(self.spec.max_chars);
        editor.set_text(&self.value);
        FieldState {
            editor,
            scroll: Vec2::ZERO,
            focus: SpringValue::new(0.0, Spring::critical(600.0)),
            blink_epoch: 0.0,
            caret_on: true,
            drag: None,
            clear_hover: false,
            baseline: 0.0,
        }
    }

    fn update(&self, old: &TextFieldWidget<M>, state: &mut FieldState) -> Dirty {
        state.editor.set_multiline(self.spec.multiline);
        state.editor.set_max_chars(self.spec.max_chars);
        let mut dirty = Dirty::None;
        // The app changed the value, or a controlled field is being held to the value it passed.
        let outside = self.value != old.value || self.on_change.is_some();
        if outside && state.editor.text() != self.value {
            state.editor.set_text(&self.value);
            dirty = if self.spec.multiline && self.spec.max_rows.is_some() { Dirty::Layout } else { Dirty::Paint };
        }
        if self.spec != old.spec {
            dirty = Dirty::Layout;
        } else if self.placeholder != old.placeholder {
            dirty = dirty.max(Dirty::Paint);
        }
        dirty
    }

    fn measure(&self, state: &mut FieldState, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let theme = &env.theme;
        let style = self.style(theme);
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w.min(DEFAULT_WIDTH),
            Avail::MinContent => MIN_WIDTH,
            Avail::MaxContent => DEFAULT_WIDTH,
        });
        let height = if self.spec.multiline {
            let frame = self.frame(rect(0.0, 0.0, width, 1000.0), &style, env.scale, self.show_clear(&state.editor));
            let surface = Surface::new(&state.editor, env.fonts(), &style, Some(frame.inner.w), false);
            let lines = (surface.layout.size().h / style.line_height).round().max(1.0) as usize;
            let most = self.spec.max_rows.unwrap_or(self.spec.rows).max(self.spec.rows);
            known.y.unwrap_or(lines.clamp(self.spec.rows, most) as f32 * style.line_height + 2.0 * PAD_Y)
        } else {
            known.y.unwrap_or(if self.spec.large { theme.control_large } else { theme.control })
        };
        let probe = env.fonts().layout_cached("Ag", &style, None);
        let top = if self.spec.multiline { PAD_Y } else { (height - style.line_height) / 2.0 };
        state.baseline = top + probe.baseline();
        size(width, height)
    }

    fn baseline(&self, state: &FieldState) -> Option<f32> {
        Some(state.baseline)
    }

    fn paint(&self, state: &FieldState, cx: &mut PaintCx) {
        let palette = cx.palette().clone();
        let alpha = cx.alpha;
        let focus = state.focus.value().clamp(0.0, 1.0);

        // The border goes over to the accent as the field gains focus, and a faint halo grows around it.
        let edge = palette.line.lerp(palette.text_3, cx.hover * 0.7).lerp(palette.accent, focus);
        let (outline, radii, smooth) = (cx.rect, cx.radii, cx.smooth);
        cx.quad(Quad::new(outline).radii(radii).smooth(smooth).border(1.0, edge.mul_alpha(alpha)));
        if focus > 0.01 {
            let halo = Quad::new(outline.inflate(3.0, 3.0))
                .radii(Corners::all(radii.max() + 3.0))
                .smooth(smooth)
                .border(3.0, palette.accent.mul_alpha(0.16 * focus * alpha));
            cx.quad(halo);
        }

        let style = self.style(cx.theme());
        let scale = cx.env.scale;
        let frame = self.frame(cx.content, &style, scale, self.show_clear(&state.editor));
        // Wrap to the width the events and the measuring used, not the one moved onto the pixel grid.
        let wrap = self.spec.multiline.then(|| self.frame(cx.exact, &style, scale, self.show_clear(&state.editor)).inner.w);
        let surface = Surface::new(&state.editor, cx.env.fonts(), &style, wrap, self.spec.password);

        if let (Some(icon), Some(at)) = (self.spec.leading, frame.leading) {
            paint_icon(cx, icon, at, Tone::Text3, Weight::Regular);
        }
        if let Some(at) = frame.clear {
            paint_icon(cx, set::X_CIRCLE, at, if state.clear_hover { Tone::Text2 } else { Tone::Text3 }, Weight::Regular);
        }

        let inner = frame.inner;
        let scroll = self.clamp_scroll(state.scroll, &surface, inner);
        let origin = pt(inner.x - scroll.x, inner.y - scroll.y);
        let active = cx.focused;
        let key_window = cx.window_focused;
        let editor = &state.editor;
        cx.clipped(inner, |cx| {
            if active {
                let fill = if key_window { Fill::Tone(Tone::Accent, 0.28) } else { Fill::Tone(Tone::Text, 0.12) };
                for r in surface.selection_rects(editor) {
                    cx.fill_rounded(r.translate(vec2(origin.x, origin.y)), Corners::all(2.0), fill);
                }
            }
            if editor.is_empty() && editor.preedit().is_none() {
                if !self.placeholder.is_empty() {
                    let layout = cx.fonts().layout_cached(&self.placeholder, &style, wrap);
                    cx.text(layout, origin, Tone::Text3);
                }
            } else {
                cx.text(surface.layout.clone(), origin, Tone::Text);
            }
            // The text still being composed is underlined.
            for r in surface.preedit_rects(editor) {
                cx.fill_rounded(rect(origin.x + r.x, origin.y + r.bottom() - 3.0, r.w, 1.0), Corners::ZERO, Tone::Text);
            }
            if active && key_window && state.caret_on && !editor.has_selection() && !self.spec.read_only {
                let caret = surface.caret(editor);
                let height = (style.size * 1.3).round().min(caret.h);
                let x = ((origin.x + caret.x) * scale).round() / scale;
                let y = origin.y + caret.y + (caret.h - height) / 2.0;
                cx.fill_rounded(rect(x, y, CARET_W, height), Corners::all(CARET_W / 2.0), Tone::Accent);
            }
        });
    }

    fn event(&self, state: &mut FieldState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::FocusChanged { focused, by, .. } => {
                self.focus_changed(state, cx, *focused, *by);
                false
            }
            WidgetEvent::WindowFocus(on) => {
                if *on {
                    self.restart_blink(state, cx);
                }
                cx.request_paint();
                false
            }
            WidgetEvent::HoverChanged(false) => {
                if state.clear_hover {
                    state.clear_hover = false;
                    cx.request_paint();
                }
                false
            }
            WidgetEvent::Action(action) => self.action(state, cx, action),
            WidgetEvent::Input(input) => match input {
                Event::PointerDown { pos, button: PointerButton::Primary, mods, click_count } => {
                    self.pointer_down(state, cx, *pos, *mods, *click_count)
                }
                Event::PointerMoved { pos, .. } => self.pointer_moved(state, cx, *pos),
                Event::PointerUp { .. } => state.drag.take().is_some(),
                Event::Key(key) => self.key(state, cx, key),
                Event::Ime(ime) => self.ime(state, cx, ime),
                Event::Scroll { delta, .. } => self.scroll(state, cx, *delta),
                _ => false,
            },
            _ => false,
        }
    }

    fn tick(&self, state: &mut FieldState, cx: &mut TickCx, dt: f32) -> bool {
        let moving = state.focus.step(dt);
        if cx.focused && cx.window_focused && !self.spec.read_only {
            let since = (cx.time - state.blink_epoch).max(0.0);
            let phase = (since / BLINK).floor();
            let on = phase as i64 % 2 == 0;
            if on != state.caret_on {
                state.caret_on = on;
                cx.request_paint();
            }
            cx.request_wake(((phase + 1.0) * BLINK - since).max(0.01));
        }
        moving
    }

    fn access(&self, state: &FieldState, info: &mut AccessInfo) {
        let editor = &state.editor;
        info.role = Some(if self.spec.search { Role::SearchInput } else { Role::TextInput });
        // The placeholder names the field when the app has not.
        if !self.placeholder.is_empty() {
            info.label = Some(self.placeholder.clone());
        }
        if self.spec.password {
            let count = |upto: usize| editor.text()[..upto].graphemes(true).count();
            info.value = Some(BULLET.to_string().repeat(count(editor.text().len())));
            info.selection = Some((count(editor.anchor()), count(editor.caret())));
        } else {
            info.value = Some(editor.text().to_string());
            info.selection = Some(editor.selection_chars());
        }
        if !self.spec.read_only {
            info.actions.push(AccessActionKind::SetValue);
        }
    }

    fn focusable(&self) -> Focus {
        Focus::Click
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Text)
    }
}

/// A text field, single line or several.
pub struct TextField<M: 'static> {
    base: Base<M>,
    widget: TextFieldWidget<M>,
}

builder!(TextField<M> => TextFieldWidget<M>);

impl<M: 'static> TextField<M> {
    fn new(value: impl Into<String>, spec: Spec) -> TextField<M> {
        let field = TextField {
            base: Base::new(),
            widget: TextFieldWidget { value: value.into(), placeholder: String::new(), on_change: None, on_submit: None, on_paste_image: None, spec },
        };
        // The field shows its own focus, so the runtime's ring stays off.
        field.bg(Tone::Field).rounded(Radius::Control).focus_ring(false)
    }

    /// Hint shown while the field is empty. It also names the field for assistive technology unless
    /// the element has a name of its own.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.widget.placeholder = text.into();
        self
    }

    /// Sent with the whole new text after every edit. Without it the field edits freely and takes the
    /// value only at the start.
    pub fn on_change(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.widget.on_change = Some(Rc::new(f));
        self
    }

    /// Sent with the text when Return is pressed in a single line, or Command-Return in several.
    pub fn on_submit(mut self, f: impl Fn(String) -> M + 'static) -> Self {
        self.widget.on_submit = Some(Rc::new(f));
        self
    }

    /// Sent with a picture pasted while the clipboard holds no text, as PNG bytes. Without it, a
    /// picture on the clipboard pastes nothing.
    pub fn on_paste_image(mut self, f: impl Fn(Vec<u8>) -> M + 'static) -> Self {
        self.widget.on_paste_image = Some(Rc::new(f));
        self
    }

    /// Shows bullets for the text and keeps it off the clipboard. One line only.
    pub fn password(mut self) -> Self {
        self.widget.spec.password = true;
        self.widget.spec.multiline = false;
        self
    }

    /// The text can be selected and copied but not changed.
    pub fn read_only(mut self, on: bool) -> Self {
        self.widget.spec.read_only = on;
        self
    }

    pub fn max_chars(mut self, max: usize) -> Self {
        self.widget.spec.max_chars = Some(max);
        self
    }

    /// An icon before the text.
    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.widget.spec.leading = Some(icon);
        self
    }

    /// A button after the text that empties the field, shown while there is text.
    pub fn clearable(mut self) -> Self {
        self.widget.spec.clearable = true;
        self
    }

    /// How many lines tall a multi-line field is at least.
    pub fn rows(mut self, rows: usize) -> Self {
        self.widget.spec.rows = rows.max(1);
        self
    }

    /// Lets a multi-line field grow with its text up to this many lines, and scroll beyond.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.widget.spec.max_rows = Some(rows.max(1));
        self
    }

    pub fn mono(mut self) -> Self {
        self.widget.spec.mono = true;
        self
    }

    /// The taller field for primary inputs.
    pub fn large(mut self) -> Self {
        self.widget.spec.large = true;
        self
    }

    /// Selects everything when focus arrives by anything but a click.
    pub fn select_on_focus(mut self) -> Self {
        self.widget.spec.select_on_focus = true;
        self
    }
}

const FIELD: Spec = Spec {
    multiline: false,
    rows: 1,
    max_rows: None,
    password: false,
    read_only: false,
    search: false,
    clearable: false,
    mono: false,
    large: false,
    select_on_focus: false,
    max_chars: None,
    leading: None,
};

/// A single-line field showing `value`.
pub fn text_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, FIELD)
}

/// A field for several lines. Three tall until `rows` or `max_rows` say otherwise.
pub fn text_area<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { multiline: true, rows: 3, ..FIELD })
}

/// A field for finding things: a magnifier, a button that clears it, and the search role.
pub fn search_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { search: true, clearable: true, leading: Some(set::SEARCH), ..FIELD }).placeholder("Search")
}

pub fn password_field<M: 'static>(value: impl Into<String>) -> TextField<M> {
    TextField::new(value, Spec { password: true, ..FIELD })
}
