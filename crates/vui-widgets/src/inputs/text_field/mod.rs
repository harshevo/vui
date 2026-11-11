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

use crate::inputs::editor::{Dir, Editor, Unit, char_at};
use crate::display::icon::paint_icon;

mod builder;
mod editing;
mod pointer;
mod widget;

pub use builder::{TextField, password_field, search_field, text_area, text_field};

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

    // ---- focus ----------------------------------------------------------------------------------

    // ---- keyboard -------------------------------------------------------------------------------

    // ---- pointer --------------------------------------------------------------------------------

    // ---- input methods and assistive technology -------------------------------------------------
}

builder!(TextField<M> => TextFieldWidget<M>);
