//! The grid widget: sizing, painting what is in view, and input.

use std::cell::Cell as StdCell;
use std::rc::Rc;

use vui_core::{Corners, CursorIcon, Event, Fill, Key, NamedKey, Point, PointerButton, Rect, Size, TextStyle, Tone, pt, rect, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Builder, Dirty, Element, Env, EventCx, Focus, IntoChildren, IntoElement, Overflow, PaintCx, Parent, Role, Styled, TickCx, Widget, WidgetEvent, column};

use super::{Align, Cell, Column, GridData, Order};
use crate::canvas::{Canvas, paint};
use crate::spark::draw_spark;
use crate::style::{Ink, VizStyle};
use vui_core::vector::{Cap, Join, Stroke};

/// Height of a row and of the header, and the room either side of a cell's content.
pub(crate) const ROW: f32 = 32.0;
pub(crate) const HEAD: f32 = 36.0;
const PAD: f32 = 12.0;
/// How near a column's right edge the pointer grips it, in points.
const GRIP: f32 = 5.0;
/// How long a changed cell flashes, in seconds.
const FLASH: f32 = 0.9;

pub struct GridWidget<M> {
    data: Rc<dyn GridData>,
    columns: Rc<Vec<Column>>,
    sorted: Option<(usize, Order)>,
    selected: Option<u64>,
    pinned: usize,
    surface: Fill,
    label: String,
    on_sort: Option<Rc<dyn Fn(usize) -> M>>,
    on_select: Option<Rc<dyn Fn(u64) -> M>>,
    on_open: Option<Rc<dyn Fn(u64) -> M>>,
    on_resize: Option<Rc<dyn Fn(usize, f32) -> M>>,
}

/// Where the pointer is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Hit {
    None,
    /// A column's title.
    Head(usize),
    /// The right edge of a column's title, where a drag resizes it.
    Grip(usize),
    Cell(usize, usize),
}

#[derive(Default)]
pub struct GridState {
    /// What was visible when last painted, in window coordinates; input is matched to it.
    view: StdCell<Rect>,
    /// The rows last painted, first to one past the last.
    shown: StdCell<(usize, usize)>,
    hover: Option<Hit>,
    /// Column being resized, with the pointer's x and the column's width when it began.
    resizing: Option<(usize, f32, f32)>,
    flashing: bool,
}

/// Where each column starts, and the width all of them take together.
fn starts(widths: &[f32]) -> (Vec<f32>, f32) {
    let mut xs = Vec::with_capacity(widths.len() + 1);
    let mut x = 0.0;
    for w in widths {
        xs.push(x);
        x += w;
    }
    xs.push(x);
    (xs, x)
}

impl<M: 'static> GridWidget<M> {
    /// Column widths for a grid `total` wide: the columns' own, and what is left over shared among
    /// the ones that grow.
    fn widths(&self, total: f32) -> Vec<f32> {
        let natural: f32 = self.columns.iter().map(|c| c.width).sum();
        let weight: f32 = self.columns.iter().map(|c| c.grow).sum();
        let extra = if weight > 0.0 { (total - natural).max(0.0) } else { 0.0 };
        self.columns.iter().map(|c| c.width + if weight > 0.0 { extra * c.grow / weight } else { 0.0 }).collect()
    }

    fn natural(&self) -> f32 {
        self.columns.iter().map(|c| c.width).sum()
    }

    fn pinned(&self) -> usize {
        self.pinned.min(self.columns.len())
    }

    fn height(&self) -> f32 {
        HEAD + self.data.rows() as f32 * ROW
    }

    fn row_of_key(&self, key: u64) -> Option<usize> {
        (0..self.data.rows()).find(|r| self.data.key(*r) == key)
    }

    /// What is under `pos` (window coordinates), given the visible area last painted.
    fn hit(&self, state: &GridState, content: Rect, pos: Point) -> Hit {
        let view = state.view.get();
        if !view.contains(pos) {
            return Hit::None;
        }
        let widths = self.widths(content.w);
        let (xs, _) = starts(&widths);
        let pinned = self.pinned();
        let pinned_w = xs[pinned];
        // A column's left edge on screen: the pinned ones ride the view's edge.
        let left = |c: usize| if c < pinned { view.x + xs[c] } else { content.x + xs[c] };
        let col = (0..widths.len()).rev().find(|c| {
            let x = left(*c);
            pos.x >= x && pos.x < x + widths[*c] && (*c < pinned || pos.x >= view.x + pinned_w)
        });
        let Some(col) = col else { return Hit::None };
        if pos.y < view.y + HEAD {
            let edge = left(col) + widths[col];
            return if (pos.x - edge).abs() <= GRIP && (col < pinned || edge > view.x + pinned_w) {
                Hit::Grip(col)
            } else if pos.x - left(col) <= GRIP && col > 0 && (col > pinned || pos.x >= view.x + pinned_w) {
                Hit::Grip(col - 1)
            } else {
                Hit::Head(col)
            };
        }
        let row = ((pos.y - content.y - HEAD) / ROW).floor();
        if row < 0.0 || row as usize >= self.data.rows() { Hit::None } else { Hit::Cell(row as usize, col) }
    }

    fn select(&self, cx: &mut EventCx<M>, content: Rect, row: usize) {
        let key = self.data.key(row);
        if let Some(f) = &self.on_select {
            cx.emit(f(key));
        }
        // The header covers the top of the view, so ask for the room it takes above the row too.
        cx.scroll_into_view(rect(content.x, content.y + row as f32 * ROW, 1.0, ROW + HEAD));
    }

    fn key(&self, state: &mut GridState, cx: &mut EventCx<M>, key: &vui_core::KeyEvent) -> bool {
        if !key.pressed {
            return false;
        }
        let rows = self.data.rows();
        let current = self.selected.and_then(|k| self.row_of_key(k));
        let page = (((state.view.get().h - HEAD) / ROW).floor() as usize).saturating_sub(1).max(1);
        let target = match key.key {
            Key::Named(NamedKey::ArrowDown) => Some(current.map(|r| r + 1).unwrap_or(state.shown.get().0)),
            Key::Named(NamedKey::ArrowUp) => Some(current.map(|r| r.saturating_sub(1)).unwrap_or(state.shown.get().0)),
            Key::Named(NamedKey::PageDown) => Some(current.map(|r| r + page).unwrap_or(state.shown.get().0)),
            Key::Named(NamedKey::PageUp) => Some(current.map(|r| r.saturating_sub(page)).unwrap_or(0)),
            Key::Named(NamedKey::Home) => Some(0),
            Key::Named(NamedKey::End) => Some(rows.saturating_sub(1)),
            Key::Named(NamedKey::Enter) => {
                if let (Some(r), Some(f)) = (current, &self.on_open) {
                    cx.emit(f(self.data.key(r)));
                }
                return current.is_some();
            }
            Key::Character('c') if key.mods.shortcut() => {
                if let Some(r) = current {
                    cx.set_clipboard_text(&self.data.copy_row(r, self.columns.len()));
                }
                return current.is_some();
            }
            _ => None,
        };
        match target {
            Some(r) if rows > 0 => {
                self.select(cx, cx.content, r.min(rows - 1));
                true
            }
            _ => false,
        }
    }

    /// True while a cell in view is still flashing.
    fn any_flash(&self, state: &GridState) -> bool {
        let (a, b) = state.shown.get();
        (a..b.min(self.data.rows())).any(|r| (0..self.columns.len()).any(|c| self.data.flash(r, c).is_some_and(|(age, _)| age < FLASH)))
    }
}

fn body() -> TextStyle {
    TextStyle::new(13.0, 18.0).no_wrap()
}

fn head() -> TextStyle {
    TextStyle::new(12.0, 16.0).weight(500).no_wrap()
}

/// Draws `text` in `cell`, lined up as `align` says, cut to the cell with an ellipsis if it is too long.
fn text_in(cx: &mut PaintCx, cell: Rect, align: Align, text: &str, style: &TextStyle, tone: Tone) {
    if text.is_empty() {
        return;
    }
    let room = (cell.w - PAD * 2.0).max(0.0);
    let one = cx.fonts().layout_cached(text, style, None);
    let layout = if one.size().w > room { cx.fonts().layout_cached(text, &style.clone().max_lines(1), Some(room)) } else { one };
    let s = layout.size();
    let x = match align {
        Align::Start => cell.x + PAD,
        Align::End => cell.right() - PAD - s.w,
        Align::Center => cell.x + (cell.w - s.w) / 2.0,
    };
    cx.text(layout, pt(x, cell.y + (cell.h - s.h) / 2.0), tone);
}

fn arrow(canvas: &mut Canvas, at: Point, up: bool, color: vui_core::Color) {
    let d = if up { -1.0 } else { 1.0 };
    let points = [pt(at.x - 3.5, at.y - 1.5 * d), pt(at.x, at.y + 2.0 * d), pt(at.x + 3.5, at.y - 1.5 * d)];
    canvas.vector().polyline(&points, &Stroke::new(1.5).join(Join::Round).cap(Cap::Round), color);
}

impl<M: 'static> GridWidget<M> {
    fn paint_cell(&self, cx: &mut PaintCx, style: &VizStyle, cell: Rect, align: Align, data: Cell) {
        match data {
            Cell::Empty => text_in(cx, cell, align, "–", &body(), Tone::Text3),
            Cell::Text(t) => text_in(cx, cell, align, t, &body(), Tone::Text),
            Cell::Toned(t, tone) => text_in(cx, cell, align, t, &body(), tone),
            Cell::Strong(t) => text_in(cx, cell, align, t, &body().weight(500), Tone::Text),
            Cell::Spark(values, tone) => {
                let area = rect(cell.x + PAD, cell.y + 5.0, (cell.w - PAD * 2.0).max(0.0), cell.h - 10.0);
                let values: Vec<f64> = values.iter().map(|v| *v as f64).collect();
                let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
                draw_spark(&mut canvas, &values, area, style, Ink::Color(cx.color(tone)));
                paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
            }
            Cell::Range { low, high, value } => {
                let track = rect(cell.x + PAD, cell.y + cell.h / 2.0 - 1.5, (cell.w - PAD * 2.0).max(0.0), 3.0);
                cx.fill_rounded(track, Corners::all(1.5), Tone::Line);
                if high > low && value.is_finite() && track.w > 10.0 {
                    // The dot's ring stays inside the track, so its edge lines up with the column's title.
                    let t = ((value - low) / (high - low)).clamp(0.0, 1.0);
                    let x = track.x + 5.0 + (track.w - 10.0) * t;
                    cx.fill_rounded(rect(track.x, track.y, x - track.x, track.h), Corners::all(1.5), Fill::Tone(Tone::Text3, 0.55));
                    let c = pt(x, track.y + track.h / 2.0);
                    cx.fill_rounded(rect(c.x - 5.0, c.y - 5.0, 10.0, 10.0), Corners::all(5.0), Tone::Bg);
                    cx.fill_rounded(rect(c.x - 3.5, c.y - 3.5, 7.0, 7.0), Corners::all(3.5), Tone::Text);
                }
            }
            Cell::Bar { fraction, label, tone } => {
                let text_w = cx.fonts().layout_cached(label, &body(), None).size().w;
                let room = (cell.w - PAD * 2.0 - text_w - 8.0).max(0.0);
                let track = rect(cell.x + PAD, cell.y + cell.h / 2.0 - 2.0, room, 4.0);
                if room >= 12.0 {
                    cx.fill_rounded(track, Corners::all(2.0), Tone::Line);
                    let f = fraction.clamp(0.0, 1.0);
                    if f > 0.0 {
                        cx.fill_rounded(rect(track.x, track.y, (track.w * f).max(4.0), track.h), Corners::all(2.0), tone);
                    }
                }
                text_in(cx, cell, Align::End, label, &body(), Tone::Text);
            }
        }
    }
}

impl<M: 'static> Widget<M> for GridWidget<M> {
    type State = GridState;

    fn create_state(&self) -> GridState {
        GridState::default()
    }

    fn update(&self, old: &Self, state: &mut GridState) -> Dirty {
        state.hover = None;
        state.flashing = self.any_flash(state);
        if *self.columns != *old.columns || self.pinned != old.pinned || self.data.rows() != old.data.rows() { Dirty::Layout } else { Dirty::Paint }
    }

    fn measure(&self, _state: &mut GridState, _env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        size(known.x.unwrap_or(0.0).max(self.natural()), self.height())
    }

    fn paint(&self, state: &GridState, cx: &mut PaintCx) {
        let content = cx.content;
        let view = cx.clip.and_then(|c| c.intersection(content)).unwrap_or(content);
        state.view.set(view);
        let widths = self.widths(content.w);
        let (xs, _) = starts(&widths);
        let pinned = self.pinned();
        let pinned_w = xs[pinned];
        let style = VizStyle::from_palette(cx.palette());
        let rows = self.data.rows();
        let top = content.y + HEAD;
        let first = (((view.y + HEAD - top) / ROW).floor().max(0.0) as usize).min(rows);
        let last = ((((view.bottom() - top) / ROW).ceil().max(0.0) as usize) + 1).min(rows);
        state.shown.set((first, last));
        let scrolled = view.x > content.x + 0.5;
        let scroll_area = rect(view.x + pinned_w, view.y, (view.w - pinned_w).max(0.0), view.h);
        let left = |c: usize| if c < pinned { view.x + xs[c] } else { content.x + xs[c] };
        let visible_cols: Vec<usize> = (0..widths.len()).filter(|c| *c < pinned || (left(*c) + widths[*c] > scroll_area.x && left(*c) < scroll_area.right())).collect();
        let hovered_row = match state.hover {
            Some(Hit::Cell(r, _)) => Some(r),
            _ => None,
        };
        let band = |r: usize| rect(view.x, top + r as f32 * ROW, view.w, ROW);
        let tint = |r: usize| -> Option<Fill> {
            if self.selected == Some(self.data.key(r)) {
                Some(Fill::Tone(Tone::Accent, 0.10))
            } else if hovered_row == Some(r) {
                Some(Fill::Tone(Tone::Hover, 1.0))
            } else {
                None
            }
        };

        // Rows: the highlight, then the scrolling columns cut to their area, then the pinned ones.
        for r in first..last {
            let b = band(r);
            if let Some(f) = tint(r) {
                cx.fill_rounded(b.inflate(-4.0, 0.0), Corners::all(6.0), f);
            }
            cx.fill_rect(rect(view.x + PAD, b.bottom() - 1.0, view.w - PAD * 2.0, 1.0), Fill::Tone(Tone::Line, 0.5));
        }
        let clip = scroll_area.intersection(rect(view.x, view.y + HEAD, view.w, view.h - HEAD));
        if let Some(area) = clip {
            cx.clipped(area, |cx| {
                for r in first..last {
                    for &c in visible_cols.iter().filter(|c| **c >= pinned) {
                        let cell = rect(left(c), top + r as f32 * ROW, widths[c], ROW);
                        self.cell(cx, &style, cell, r, c);
                    }
                }
            });
        }
        if pinned > 0 {
            let strip = rect(view.x, view.y + HEAD, pinned_w, view.h - HEAD);
            cx.fill_rect(strip, self.surface);
            for r in first..last {
                if let Some(f) = tint(r) {
                    cx.clipped(strip, |cx| cx.fill_rounded(band(r).inflate(-4.0, 0.0), Corners::all(6.0), f));
                }
                cx.clipped(strip, |cx| cx.fill_rect(rect(view.x + PAD, band(r).bottom() - 1.0, pinned_w - PAD, 1.0), Fill::Tone(Tone::Line, 0.5)));
                for (c, w) in widths.iter().enumerate().take(pinned) {
                    let cell = rect(left(c), top + r as f32 * ROW, *w, ROW);
                    cx.clipped(strip, |cx| self.cell(cx, &style, cell, r, c));
                }
            }
            if scrolled {
                cx.fill_rect(rect(view.x + pinned_w - 1.0, view.y + HEAD, 1.0, view.h - HEAD), Tone::Line);
            }
        }

        // The header rides the top of the view.
        let header = rect(view.x, view.y, view.w, HEAD);
        cx.fill_rect(header, self.surface);
        let draw_head = |cx: &mut PaintCx, c: usize| {
            let cell = rect(left(c), view.y, widths[c], HEAD);
            let col = &self.columns[c];
            let sorted = self.sorted.filter(|s| s.0 == c);
            let room = if sorted.is_some() { 16.0 } else { 0.0 };
            // A right-aligned title keeps its end over the numbers and the arrow goes before it.
            let titled = match col.align {
                Align::End => rect(cell.x + room, cell.y, cell.w - room, cell.h),
                _ => rect(cell.x, cell.y, cell.w - room, cell.h),
            };
            text_in(cx, titled, col.align, &col.title, &head(), Tone::Text2);
            if let Some((_, order)) = sorted {
                let w = cx.fonts().layout_cached(&col.title, &head(), None).size().w.min((titled.w - PAD * 2.0).max(0.0));
                let at = match col.align {
                    Align::Start => cell.x + PAD + w + 9.5,
                    Align::End => cell.right() - PAD - w - 9.5,
                    Align::Center => titled.x + (titled.w + w) / 2.0 + 9.5,
                };
                let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
                arrow(&mut canvas, pt(at, cell.y + HEAD / 2.0), order == Order::Ascending, cx.color(Tone::Text2));
                paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
            }
            let hot = matches!(state.hover, Some(Hit::Grip(g)) if g == c) || state.resizing.is_some_and(|r| r.0 == c);
            if hot {
                cx.fill_rect(rect(cell.right() - 1.0, cell.y + 8.0, 2.0, HEAD - 16.0), Fill::Tone(Tone::Accent, 0.6));
            }
        };
        if let Some(area) = scroll_area.intersection(header) {
            cx.clipped(area, |cx| {
                for &c in visible_cols.iter().filter(|c| **c >= pinned) {
                    draw_head(cx, c);
                }
            });
        }
        if pinned > 0 {
            let strip = rect(view.x, view.y, pinned_w, HEAD);
            cx.fill_rect(strip, self.surface);
            cx.clipped(strip, |cx| {
                for c in 0..pinned {
                    draw_head(cx, c);
                }
            });
        }
        cx.fill_rect(rect(view.x + PAD, view.y + HEAD - 1.0, view.w - PAD * 2.0, 1.0), Tone::Line);
    }

    fn event(&self, state: &mut GridState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => {
                if state.hover.take().is_some() {
                    cx.request_paint();
                }
                false
            }
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                if let Some((col, x0, w0)) = state.resizing {
                    let w = (w0 + pos.x - x0).max(self.columns[col].min_width);
                    if let Some(f) = &self.on_resize {
                        cx.emit(f(col, w));
                    }
                    cx.request_layout();
                    return true;
                }
                let hit = self.hit(state, cx.content, *pos);
                let now = Some(hit).filter(|h| *h != Hit::None);
                if now != state.hover {
                    state.hover = now;
                    cx.request_paint();
                }
                false
            }
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, click_count, .. }) => {
                let hit = self.hit(state, cx.content, *pos);
                match hit {
                    Hit::Grip(c) => {
                        state.resizing = Some((c, pos.x, self.columns[c].width));
                        cx.capture_pointer();
                        cx.request_paint();
                        true
                    }
                    Hit::Head(c) => {
                        if self.columns[c].sortable
                            && let Some(f) = &self.on_sort
                        {
                            cx.emit(f(c));
                        }
                        true
                    }
                    Hit::Cell(r, _) => {
                        cx.focus();
                        let key = self.data.key(r);
                        if let Some(f) = &self.on_select {
                            cx.emit(f(key));
                        }
                        if *click_count >= 2
                            && let Some(f) = &self.on_open
                        {
                            cx.emit(f(key));
                        }
                        true
                    }
                    Hit::None => false,
                }
            }
            WidgetEvent::Input(Event::PointerUp { .. }) => {
                if state.resizing.take().is_some() {
                    cx.release_pointer();
                    cx.request_paint();
                    return true;
                }
                false
            }
            WidgetEvent::Input(Event::Key(k)) => self.key(state, cx, k),
            _ => false,
        }
    }

    fn tick(&self, state: &mut GridState, cx: &mut TickCx, _dt: f32) -> bool {
        state.flashing = self.any_flash(state);
        if state.flashing {
            cx.request_paint();
        }
        state.flashing
    }

    fn animating(&self, state: &GridState) -> bool {
        state.flashing
    }

    fn access(&self, _state: &GridState, info: &mut AccessInfo) {
        info.role = Some(Role::List);
        info.label = Some(if self.label.is_empty() { "Table".to_string() } else { self.label.clone() });
        let sort = self.sorted.map(|(c, o)| format!(", sorted by {} {}", self.columns[c].title, if o == Order::Ascending { "ascending" } else { "descending" })).unwrap_or_default();
        info.value = Some(format!("{} rows{sort}", self.data.rows()));
    }

    fn focusable(&self) -> Focus {
        Focus::Click
    }

    fn cursor(&self) -> Option<CursorIcon> {
        None
    }
}

impl<M: 'static> GridWidget<M> {
    /// One body cell, with its flash behind it.
    fn cell(&self, cx: &mut PaintCx, style: &VizStyle, at: Rect, row: usize, col: usize) {
        if let Some((age, tone)) = self.data.flash(row, col).filter(|f| f.0 < FLASH) {
            let k = 1.0 - age / FLASH;
            cx.fill_rounded(at.inflate(-2.0, -3.0), Corners::all(4.0), Fill::Tone(tone, 0.22 * k * k));
        }
        self.paint_cell(cx, style, at, self.columns[col].align, self.data.cell(row, col));
    }
}

/// A grid and the scrolling box around it. Style it as any element: `.fill()`, `.key(…)`.
pub struct DataGrid<M: 'static> {
    outer: vui_runtime::Div<M>,
    widget: GridWidget<M>,
}

impl<M: 'static> Styled for DataGrid<M> {
    fn style_mut(&mut self) -> &mut vui_runtime::Style {
        self.outer.style_mut()
    }
}

impl<M: 'static> Builder<M> for DataGrid<M> {
    fn base_mut(&mut self) -> &mut Base<M> {
        self.outer.base_mut()
    }
}

impl<M: 'static> DataGrid<M> {
    /// The column the grid is sorted by and which way, shown as an arrow in its title.
    pub fn sorted(mut self, column: usize, order: Order) -> Self {
        self.widget.sorted = Some((column, order));
        self
    }

    /// The row to show as chosen, by its [`GridData::key`].
    pub fn selected(mut self, key: Option<u64>) -> Self {
        self.widget.selected = key;
        self
    }

    /// Keeps the first `n` columns in view while the rest scroll under them.
    pub fn pinned(mut self, n: usize) -> Self {
        self.widget.pinned = n;
        self
    }

    /// The colour the header and the pinned columns are filled with, to hide what scrolls under
    /// them: the colour of whatever the grid sits on. The sheet's by default.
    pub fn surface(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.surface = fill.into();
        self
    }

    /// What assistive technology calls the table.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// Pressing a sortable column's title sends its index.
    pub fn on_sort(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.widget.on_sort = Some(Rc::new(f));
        self
    }

    /// Clicking a row, or moving to it with the arrow keys, sends its key.
    pub fn on_select(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.widget.on_select = Some(Rc::new(f));
        self
    }

    /// Double-clicking a row, or Return on the chosen one, sends its key.
    pub fn on_open(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.widget.on_open = Some(Rc::new(f));
        self
    }

    /// Dragging the edge of a column's title sends the column and its new width; the app keeps the
    /// widths and passes them back in the columns.
    pub fn on_resize(mut self, f: impl Fn(usize, f32) -> M + 'static) -> Self {
        self.widget.on_resize = Some(Rc::new(f));
        self
    }
}

impl<M: 'static> IntoElement<M> for DataGrid<M> {
    fn into_element(self) -> Element<M> {
        let (natural, height) = (self.widget.natural(), self.widget.height());
        let inner = Element::new(self.widget, Base::new(), Vec::new()).min_width(natural).height(height).shrink(0.0);
        self.outer.child(inner).into_element()
    }
}

impl<M: 'static> IntoChildren<M> for DataGrid<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.into_element());
    }
}

/// A grid of `data` under a header for `columns`.
///
/// ```ignore
/// let grid = data_grid(snapshot, columns).pinned(1).sorted(2, Order::Descending).fill().on_sort(Msg::Sort);
/// ```
pub fn data_grid<M: 'static>(data: Rc<dyn GridData>, columns: Vec<Column>) -> DataGrid<M> {
    let outer = column().overflow(Overflow::Scroll).min_height(0.0).min_width(0.0).role(Role::None);
    DataGrid {
        outer,
        widget: GridWidget {
            data,
            columns: Rc::new(columns),
            sorted: None,
            selected: None,
            pinned: 0,
            surface: Fill::Tone(Tone::Bg, 1.0),
            label: String::new(),
            on_sort: None,
            on_select: None,
            on_open: None,
            on_resize: None,
        },
    }
}
