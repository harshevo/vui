//! The treemap as a widget: tiles coloured red to green by their value, labels that are drawn only
//! where they fit, an outline on the tile under the pointer, and arrow keys that walk from tile to
//! tile.

use std::rc::Rc;

use vui_core::{Color, Corners, Event, Fill, Key, NamedKey, PointerButton, Quad, Size, TextStyle, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, EventCx, Focus, PaintCx, Role, Widget, WidgetEvent, builder};

use super::{Laid, Placed, Treemap, heat, layout, neighbour, tile_at};

const RADIUS: f32 = 4.0;
/// The most a tile's colour ever covers its background: text stays readable at full strength.
const STRONGEST: f32 = 0.62;
const FAINTEST: f32 = 0.14;

type Pick<M> = Rc<dyn Fn(usize) -> M>;
type Hover<M> = Rc<dyn Fn(Option<usize>) -> M>;

pub struct TreemapWidget<M> {
    map: Rc<Treemap>,
    label: String,
    selected: Option<usize>,
    on_hover: Option<Hover<M>>,
    on_pick: Option<Pick<M>>,
}

#[derive(Default)]
pub struct TreemapState {
    hover: Option<usize>,
    /// The tile the arrow keys are on.
    cursor: Option<usize>,
}

fn label_style(big: bool) -> TextStyle {
    if big { TextStyle::new(15.0, 20.0).weight(600).no_wrap() } else { TextStyle::new(12.0, 16.0).weight(600).no_wrap() }
}

fn note_style(big: bool) -> TextStyle {
    if big { TextStyle::new(13.0, 16.0).no_wrap() } else { TextStyle::new(11.0, 14.0).no_wrap() }
}

fn heading_style() -> TextStyle {
    TextStyle::new(11.0, 14.0).weight(600).no_wrap()
}

impl<M: 'static> TreemapWidget<M> {
    fn laid(&self, content: vui_core::Rect) -> Laid {
        layout(&self.map, content)
    }

    fn weight(&self, i: usize) -> f64 {
        self.map.get(i).map_or(0.0, |t| t.weight)
    }

    fn tone(&self, value: f64) -> (Tone, f32) {
        if !value.is_finite() {
            return (Tone::Wash, 0.0);
        }
        let k = heat(value, self.map.cap);
        (if value >= 0.0 { Tone::Success } else { Tone::Danger }, FAINTEST + (STRONGEST - FAINTEST) * k)
    }

    fn outline(cx: &mut PaintCx, p: &Placed, width: f32, color: Color) {
        cx.quad(Quad::new(p.rect.inflate(-width / 2.0, -width / 2.0)).radii(Corners::all(RADIUS)).border(width, color));
    }
}

impl<M: 'static> Widget<M> for TreemapWidget<M> {
    type State = TreemapState;

    fn create_state(&self) -> TreemapState {
        TreemapState::default()
    }

    fn update(&self, old: &Self, state: &mut TreemapState) -> Dirty {
        state.hover = state.hover.filter(|i| *i < self.map.len());
        state.cursor = state.cursor.filter(|i| *i < self.map.len());
        if Rc::ptr_eq(&self.map, &old.map) || *self.map == *old.map { Dirty::Paint } else { Dirty::Layout }
    }

    fn measure(&self, _state: &mut TreemapState, _env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let width = known.x.unwrap_or(match avail.x {
            Avail::Definite(w) => w,
            Avail::MinContent => 160.0,
            Avail::MaxContent => 560.0,
        });
        // Never asks for more height than it is offered, so a box that is short of room does not spill.
        let height = known.y.unwrap_or_else(|| {
            let want = (width * 0.6).clamp(160.0, 560.0);
            match avail.y {
                Avail::Definite(h) => want.min(h),
                Avail::MinContent => 160.0,
                Avail::MaxContent => want,
            }
        });
        size(width, height)
    }

    fn paint(&self, state: &TreemapState, cx: &mut PaintCx) {
        let laid = self.laid(cx.content);
        for (gi, head) in &laid.headings {
            let layout = cx.fonts().layout_cached(&self.map.groups[*gi].label, &heading_style(), None);
            let s = layout.size();
            cx.clipped(*head, |cx| cx.text(layout, pt(head.x, head.y + (head.h - s.h) / 2.0), Tone::Text2));
        }
        for p in &laid.tiles {
            let Some(tile) = self.map.get(p.index) else { continue };
            let r = p.rect;
            cx.fill_rounded(r, Corners::all(RADIUS), Tone::Wash);
            let (tone, alpha) = self.tone(tile.value);
            if alpha > 0.0 {
                cx.fill_rounded(r, Corners::all(RADIUS), Fill::Tone(tone, alpha));
            }
            let big = r.w.min(r.h) >= 96.0;
            let label = cx.fonts().layout_cached(&tile.label, &label_style(big), None);
            let (lw, lh) = (label.size().w, label.size().h);
            if r.w >= lw + 12.0 && r.h >= lh + 12.0 {
                let top = r.y + 6.0;
                cx.text(label, pt(r.x + 6.0, top), Tone::Text);
                if !tile.note.is_empty() {
                    let note = cx.fonts().layout_cached(&tile.note, &note_style(big), None);
                    if r.w >= note.size().w + 12.0 && r.h >= lh + note.size().h + 14.0 {
                        cx.text(note, pt(r.x + 6.0, top + lh + 1.0), Tone::Text);
                    }
                }
            }
        }
        let on = |cx: &PaintCx, tone: Tone, a: f32| cx.color(Fill::Tone(tone, a));
        if let Some(p) = self.selected.and_then(|i| laid.tiles.iter().find(|t| t.index == i)) {
            let c = on(cx, Tone::Accent, 1.0);
            Self::outline(cx, p, 2.0, c);
        }
        if let Some(p) = state.hover.and_then(|i| laid.tiles.iter().find(|t| t.index == i)) {
            let c = on(cx, Tone::Text, 0.7);
            Self::outline(cx, p, 2.0, c);
        }
        if cx.focused
            && let Some(p) = state.cursor.and_then(|i| laid.tiles.iter().find(|t| t.index == i))
        {
            let c = on(cx, Tone::Accent, 1.0);
            Self::outline(cx, p, 2.0, c);
        }
    }

    fn event(&self, state: &mut TreemapState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                let now = tile_at(&self.laid(cx.content), *pos);
                self.hover(state, cx, now);
                false
            }
            WidgetEvent::HoverChanged(false) | WidgetEvent::Input(Event::PointerLeft) => {
                self.hover(state, cx, None);
                false
            }
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                if let Some(i) = tile_at(&self.laid(cx.content), *pos) {
                    cx.focus();
                    state.cursor = Some(i);
                    self.pick(cx, i);
                    cx.request_paint();
                    return true;
                }
                false
            }
            WidgetEvent::Input(Event::Key(k)) if k.pressed => {
                let (dx, dy) = match k.key {
                    Key::Named(NamedKey::ArrowLeft) => (-1, 0),
                    Key::Named(NamedKey::ArrowRight) => (1, 0),
                    Key::Named(NamedKey::ArrowUp) => (0, -1),
                    Key::Named(NamedKey::ArrowDown) => (0, 1),
                    Key::Named(NamedKey::Enter) | Key::Character(' ') => {
                        if let Some(i) = state.cursor {
                            self.pick(cx, i);
                        }
                        return state.cursor.is_some();
                    }
                    _ => return false,
                };
                let laid = self.laid(cx.content);
                let to = match state.cursor.or(self.selected) {
                    Some(from) if state.cursor.is_some() => neighbour(&laid, from, dx, dy),
                    // The first press only lands on a tile: the selected one, else the biggest.
                    from => from.or_else(|| laid.tiles.iter().max_by(|a, b| self.weight(a.index).total_cmp(&self.weight(b.index))).map(|t| t.index)),
                };
                let Some(to) = to else { return false };
                state.cursor = Some(to);
                if let Some(p) = laid.tiles.iter().find(|t| t.index == to) {
                    cx.scroll_into_view(p.rect);
                }
                cx.request_paint();
                true
            }
            _ => false,
        }
    }

    fn access(&self, _state: &TreemapState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
        info.label = Some(if self.label.is_empty() { "Treemap".to_string() } else { self.label.clone() });
        let biggest = self.map.groups.iter().flat_map(|g| g.tiles.iter()).filter(|t| t.weight.is_finite()).max_by(|a, b| a.weight.total_cmp(&b.weight));
        info.value = Some(match biggest {
            Some(t) => format!("{} tiles, the largest {}", self.map.len(), t.label),
            None => "No tiles".to_string(),
        });
    }

    fn focusable(&self) -> Focus {
        if self.on_pick.is_some() { Focus::Click } else { Focus::Never }
    }
}

impl<M: 'static> TreemapWidget<M> {
    fn hover(&self, state: &mut TreemapState, cx: &mut EventCx<M>, now: Option<usize>) {
        if now != state.hover {
            state.hover = now;
            if let Some(f) = &self.on_hover {
                cx.emit(f(now));
            }
            cx.request_paint();
        }
    }

    fn pick(&self, cx: &mut EventCx<M>, i: usize) {
        if let Some(f) = &self.on_pick {
            cx.emit(f(i));
        }
    }
}

pub struct TreemapView<M: 'static> {
    base: Base<M>,
    widget: TreemapWidget<M>,
}

builder!(TreemapView<M> => TreemapWidget<M>);

impl<M: 'static> TreemapView<M> {
    /// What assistive technology calls it.
    pub fn name(mut self, label: impl Into<String>) -> Self {
        self.widget.label = label.into();
        self
    }

    /// Outlines tile `i`, the one the rest of the screen is showing.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.widget.selected = i;
        self
    }

    /// Sends the number of the tile under the pointer as it changes, `None` when it leaves.
    pub fn on_hover(mut self, f: impl Fn(Option<usize>) -> M + 'static) -> Self {
        self.widget.on_hover = Some(Rc::new(f));
        self
    }

    /// Sends the number of a tile when it is clicked, or entered with the arrow keys.
    pub fn on_pick(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.widget.on_pick = Some(Rc::new(f));
        self
    }
}

/// A treemap that fills the room it is given: `treemap(Rc::new(Treemap::new().group("Banks").tile("HDFCBANK", "+1.2%", 12.4, 0.012)))`.
pub fn treemap<M: 'static>(data: Rc<Treemap>) -> TreemapView<M> {
    TreemapView { base: Base::new(), widget: TreemapWidget { map: data, label: String::new(), selected: None, on_hover: None, on_pick: None } }
}
