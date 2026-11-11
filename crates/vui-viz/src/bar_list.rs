//! Horizontal bars: one row per item with its label on the left, a bar for its size and the value on
//! the right. The usual way to rank things on a dashboard: top pages, browsers, regions.

use std::rc::Rc;

use vui_core::{Event, Quad, Rect, Size, pt, rect, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, EventCx, PaintCx, Role, TickCx, Widget, WidgetEvent, builder};

use crate::canvas::{Canvas, HAnchor, VAnchor, paint};
use crate::format::Format;
use crate::style::{Ink, VizStyle};

const ROW: f32 = 30.0;
const BAR: f32 = 8.0;
const REVEAL_SECONDS: f32 = 0.7;

#[derive(Clone, Debug, PartialEq)]
pub struct BarList {
    pub rows: Vec<(String, f64)>,
    pub format: Format,
    pub ink: Ink,
    /// The value of a full-width bar; the largest row's by default.
    pub max: Option<f64>,
}

impl Default for BarList {
    fn default() -> BarList {
        BarList::new()
    }
}

impl BarList {
    pub fn new() -> BarList {
        BarList { rows: Vec::new(), format: Format::Auto, ink: Ink::Accent, max: None }
    }

    /// Adds a row. Values that are negative or not numbers count as nothing.
    pub fn row(mut self, label: &str, value: f64) -> BarList {
        self.rows.push((label.to_string(), if value.is_finite() { value.max(0.0) } else { 0.0 }));
        self
    }

    /// Puts the largest first.
    pub fn sorted(mut self) -> BarList {
        self.rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        self
    }

    pub fn format(mut self, format: Format) -> BarList {
        self.format = format;
        self
    }

    pub fn ink(mut self, ink: Ink) -> BarList {
        self.ink = ink;
        self
    }

    /// The value of a full bar, for lists that are shares of something larger, like 100%.
    pub fn max(mut self, value: f64) -> BarList {
        self.max = Some(value);
        self
    }

    fn top(&self) -> f64 {
        self.max.unwrap_or_else(|| self.rows.iter().map(|r| r.1).fold(0.0, f64::max)).max(f64::MIN_POSITIVE)
    }
}

/// Draws the rows in `area`, the bars grown to `reveal` of their length, with row `hover` picked out.
/// Returns each row's rect.
pub fn draw_bar_list(canvas: &mut Canvas, list: &BarList, area: Rect, style: &VizStyle, reveal: f32, hover: Option<usize>) -> Vec<Rect> {
    let values: Vec<String> = list.rows.iter().map(|r| list.format.apply(r.1)).collect();
    let label_w = list.rows.iter().map(|r| canvas.measure(&r.0, &style.label).w).fold(0.0, f32::max).min(area.w * 0.4) + 16.0;
    let value_w = values.iter().map(|v| canvas.measure(v, &style.label_mono).w).fold(0.0, f32::max) + 16.0;
    let track = (area.w - label_w - value_w).max(8.0);
    let eased = 1.0 - (1.0 - reveal.clamp(0.0, 1.0)).powi(3);
    let color = list.ink.resolve(style, 1);
    let mut rows = Vec::with_capacity(list.rows.len());
    for (i, ((label, v), value)) in list.rows.iter().zip(&values).enumerate() {
        let row = rect(area.x, area.y + i as f32 * ROW, area.w, ROW);
        let mid = row.y + ROW / 2.0;
        let lit = hover.is_none_or(|h| h == i);
        if hover == Some(i) {
            canvas.quad(Quad::new(row.inflate(4.0, 0.0)).fill(style.wash).radii(6.0));
        }
        canvas.text(label, &style.label, pt(row.x, mid), HAnchor::Left, VAnchor::Middle, if lit { style.ink } else { style.ink_2 });
        let bar = rect(row.x + label_w, mid - BAR / 2.0, track, BAR);
        canvas.quad(Quad::new(bar).fill(style.wash).radii(BAR / 2.0));
        let w = (track * (*v / list.top()).clamp(0.0, 1.0) as f32 * eased).max(if *v > 0.0 { BAR } else { 0.0 });
        if w > 0.0 {
            canvas.quad(Quad::new(rect(bar.x, bar.y, w, BAR)).fill(if lit { color } else { color.mul_alpha(0.55) }).radii(BAR / 2.0));
        }
        canvas.text(value, &style.label_mono, pt(row.right(), mid), HAnchor::Right, VAnchor::Middle, style.ink_2);
        rows.push(row);
    }
    rows
}

pub struct BarListWidget<M> {
    list: Rc<BarList>,
    label: String,
    animate: bool,
    on_hover: Option<Rc<dyn Fn(Option<usize>) -> M>>,
}

pub struct BarListState {
    reveal: f32,
    hover: Option<usize>,
}

impl<M: 'static> Widget<M> for BarListWidget<M> {
    type State = BarListState;

    fn create_state(&self) -> BarListState {
        BarListState { reveal: if self.animate { 0.0 } else { 1.0 }, hover: None }
    }

    fn update(&self, old: &BarListWidget<M>, state: &mut BarListState) -> Dirty {
        state.hover = state.hover.filter(|i| *i < self.list.rows.len());
        if self.list.rows.len() != old.list.rows.len() {
            Dirty::Layout
        } else if Rc::ptr_eq(&self.list, &old.list) || *self.list == *old.list {
            Dirty::None
        } else {
            Dirty::Paint
        }
    }

    fn measure(&self, _state: &mut BarListState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            // It can be drawn narrow, so it never holds a row open at its usual width.
            Avail::MinContent => 160.0,
            Avail::MaxContent => 320.0,
        });
        size(width, known.y.unwrap_or(self.list.rows.len() as f32 * ROW))
    }

    fn paint(&self, state: &BarListState, cx: &mut PaintCx) {
        let style = VizStyle::from_palette(cx.palette());
        let mut canvas = Canvas::new(cx.fonts(), cx.env.scale);
        draw_bar_list(&mut canvas, &self.list, cx.content, &style, state.reveal, state.hover);
        paint(cx.scene, canvas.finish(), cx.clip, cx.alpha);
    }

    fn event(&self, state: &mut BarListState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        let over = match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) if cx.content.contains(*pos) => {
                Some(((pos.y - cx.content.y) / ROW) as usize).filter(|i| *i < self.list.rows.len())
            }
            WidgetEvent::Input(Event::PointerMoved { .. }) | WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => None,
            _ => return false,
        };
        if over != state.hover {
            state.hover = over;
            if let Some(f) = &self.on_hover {
                cx.emit(f(over));
            }
            cx.request_paint();
        }
        false
    }

    fn tick(&self, state: &mut BarListState, _cx: &mut TickCx, dt: f32) -> bool {
        state.reveal = (state.reveal + dt / REVEAL_SECONDS).min(1.0);
        state.reveal < 1.0
    }

    fn animating(&self, state: &BarListState) -> bool {
        state.reveal < 1.0
    }

    fn access(&self, _state: &BarListState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        let parts: Vec<String> = self.list.rows.iter().map(|(l, v)| format!("{l} {}", self.list.format.apply(*v))).collect();
        info.label = Some(if self.label.is_empty() { format!("Bar chart: {}", parts.join(", ")) } else { self.label.clone() });
    }
}

pub struct BarListView<M: 'static> {
    base: Base<M>,
    widget: BarListWidget<M>,
}

builder!(BarListView<M> => BarListWidget<M>);

impl<M: 'static> BarListView<M> {
    /// What assistive technology reads out; by default every row and its value.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// Skip the bars growing in when the list first appears.
    pub fn still(mut self) -> Self {
        self.widget.animate = false;
        self
    }

    /// Sends the index of the row under the pointer as it changes, and `None` when it leaves.
    pub fn on_hover(mut self, f: impl Fn(Option<usize>) -> M + 'static) -> Self {
        self.widget.on_hover = Some(Rc::new(f));
        self
    }
}

/// Horizontal bars, one row per item: `bar_list(Rc::new(BarList::new().row("Chrome", 64.0).sorted()))`.
pub fn bar_list<M: 'static>(data: Rc<BarList>) -> BarListView<M> {
    BarListView { base: Base::new(), widget: BarListWidget { list: data, label: String::new(), animate: true, on_hover: None } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::{Command, Fonts, Theme};

    #[test]
    fn bars_are_sized_by_their_share_of_the_largest_and_sorted_on_request() {
        let list = BarList::new().row("b", 25.0).row("a", 100.0).row("bad", f64::NAN).sorted();
        assert_eq!(list.rows[0].0, "a");
        assert_eq!(list.rows[2].1, 0.0);
        let fonts = Fonts::bundled_only();
        let style = VizStyle::from_palette(&Theme::light().palette);
        let mut canvas = Canvas::new(&fonts, 1.0);
        let rows = draw_bar_list(&mut canvas, &list, rect(0.0, 0.0, 300.0, 90.0), &style, 1.0, None);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].y, ROW);
        let bars: Vec<f32> = canvas
            .finish()
            .iter()
            .filter_map(|c| match c {
                Command::Quad(q) if q.fill == style.accent => Some(q.rect.w),
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2, "a bar for each row with a value");
        assert!((bars[0] / bars[1] - 4.0).abs() < 0.01, "100 is four times 25: {bars:?}");
    }
}
