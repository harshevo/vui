//! Sliding segments: two to five choices on one line, and one pill that glides to the chosen one.

use std::rc::Rc;

use vui_core::{Corners, CursorIcon, Event, Fill, Key, NamedKey, PointerButton, Quad, Rect, Size, Spring, SpringValue, TextLayout, TextRole, Tone, pt, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, Base, Builder, Dirty, Env, EventCx, Focus, PaintCx, Radius, Role, TickCx, Widget, WidgetEvent,
    builder,
};

use super::line;

/// Room either side of a label.
const PAD: f32 = 10.0;
/// Space between segments.
const GAP: f32 = 2.0;

type Pick<M> = Rc<dyn Fn(usize) -> M>;

/// The widget behind [`Segments`].
pub struct SegmentsWidget<M> {
    labels: Vec<String>,
    /// A colour per segment for its pill and label when chosen: Buy in green, Sell in red. `None`
    /// is the quiet grey pill.
    tones: Vec<Option<Tone>>,
    chosen: usize,
    height: f32,
    role: TextRole,
    on_pick: Option<Pick<M>>,
}

pub struct SegmentsState {
    layouts: Vec<Rc<TextLayout>>,
    /// Where the pill is, as a segment index: 1.5 is halfway between the second and the third. A
    /// spring moves it, so the pill glides and a new choice mid-glide carries on from where it is.
    pos: SpringValue,
    hover: Option<usize>,
}

impl<M> SegmentsWidget<M> {
    /// Each segment's left edge (from the content box's left) and width. Room beyond what the labels
    /// need is shared out equally, so a segmented control told to fill a width stays even.
    fn spans(&self, state: &SegmentsState, width: f32) -> Vec<(f32, f32)> {
        let n = state.layouts.len();
        if n == 0 {
            return Vec::new();
        }
        let natural: f32 = state.layouts.iter().map(|l| l.size().w + PAD * 2.0).sum::<f32>() + GAP * (n - 1) as f32;
        let extra = ((width - natural) / n as f32).max(0.0);
        let mut x = 0.0;
        state
            .layouts
            .iter()
            .map(|l| {
                let w = l.size().w + PAD * 2.0 + extra;
                let span = (x, w);
                x += w + GAP;
                span
            })
            .collect()
    }

    fn hit(&self, state: &SegmentsState, content: Rect, x: f32) -> Option<usize> {
        let x = x - content.x;
        self.spans(state, content.w).iter().position(|(left, w)| x >= *left - GAP / 2.0 && x < left + w + GAP / 2.0)
    }

    fn pick(&self, cx: &mut EventCx<M>, index: usize) {
        if index != self.chosen
            && index < self.labels.len()
            && let Some(on_pick) = &self.on_pick
        {
            cx.emit(on_pick(index));
        }
    }

    fn tone(&self, index: usize) -> Option<Tone> {
        self.tones.get(index).copied().flatten()
    }
}

impl<M: 'static> Widget<M> for SegmentsWidget<M> {
    type State = SegmentsState;

    fn create_state(&self) -> SegmentsState {
        SegmentsState { layouts: Vec::new(), pos: SpringValue::new(self.chosen as f32, Spring::SNAPPY), hover: None }
    }

    fn update(&self, old: &SegmentsWidget<M>, state: &mut SegmentsState) -> Dirty {
        if self.chosen != old.chosen {
            state.pos.set_target(self.chosen as f32);
        }
        if self.labels != old.labels || self.height != old.height || self.role != old.role {
            Dirty::Layout
        } else if self.chosen != old.chosen || self.tones != old.tones {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, state: &mut SegmentsState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        state.layouts = self.labels.iter().map(|l| line(env, l, self.role, 500, false)).collect();
        let n = state.layouts.len();
        let natural: f32 = state.layouts.iter().map(|l| l.size().w + PAD * 2.0).sum::<f32>() + GAP * n.saturating_sub(1) as f32;
        size(known.x.unwrap_or(natural), known.y.unwrap_or(self.height))
    }

    fn paint(&self, state: &SegmentsState, cx: &mut PaintCx) {
        let content = cx.content;
        let spans = self.spans(state, content.w);
        let n = spans.len();
        if n == 0 {
            return;
        }
        let h = content.h;
        let pill = |cx: &mut PaintCx, x: f32, w: f32, fill: Fill| {
            let rect = Rect::new(content.x + x, content.y, w, h);
            cx.fill_rounded(rect, Corners::all(h / 2.0), fill);
        };

        // The hovered segment, when it is not the chosen one, under a faint wash.
        if let Some(i) = state.hover.filter(|i| *i != self.chosen && *i < n) {
            let (x, w) = spans[i];
            pill(cx, x, w, Fill::Tone(Tone::Hover, cx.hover));
        }

        // The pill, between the two segments it is passing.
        let p = state.pos.value().clamp(0.0, (n - 1) as f32);
        let (i, j) = (p.floor() as usize, (p.ceil() as usize).min(n - 1));
        let f = p - i as f32;
        let (x, w) = (spans[i].0 + (spans[j].0 - spans[i].0) * f, spans[i].1 + (spans[j].1 - spans[i].1) * f);
        let fill = match self.tone(self.chosen) {
            Some(tone) => Fill::Tone(tone, 0.16),
            None => Fill::from(Tone::Wash),
        };
        pill(cx, x, w, fill);
        if cx.focus > 0.01 {
            let ring = Rect::new(content.x + x - 2.0, content.y - 2.0, w + 4.0, h + 4.0);
            let accent = cx.color(Fill::Tone(Tone::Accent, cx.focus));
            cx.quad(Quad::new(ring).radii(Corners::all(ring.h / 2.0)).border(1.5, accent));
        }

        for (k, layout) in state.layouts.iter().enumerate() {
            let (x, w) = spans[k];
            let text = layout.size();
            let at = pt(content.x + x + (w - text.w) / 2.0, content.y + (h - text.h) / 2.0);
            let color = if k == self.chosen { self.tone(k).unwrap_or(Tone::Text) } else { Tone::Text2 };
            cx.text(layout.clone(), at, color);
        }
    }

    fn event(&self, state: &mut SegmentsState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Input(Event::PointerMoved { pos, .. }) => {
                let hover = self.hit(state, cx.content, pos.x);
                if hover != state.hover {
                    state.hover = hover;
                    cx.request_paint();
                }
                false
            }
            WidgetEvent::HoverChanged(false) => {
                state.hover = None;
                cx.request_paint();
                false
            }
            WidgetEvent::Input(Event::PointerDown { pos, button: PointerButton::Primary, .. }) => {
                // Focus follows the click, so the arrow keys carry on from here.
                cx.focus();
                if let Some(i) = self.hit(state, cx.content, pos.x) {
                    self.pick(cx, i);
                }
                true
            }
            WidgetEvent::Input(Event::Key(key)) if key.pressed => match key.key {
                Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowUp) => {
                    if self.chosen > 0 {
                        self.pick(cx, self.chosen - 1);
                    }
                    true
                }
                Key::Named(NamedKey::ArrowRight) | Key::Named(NamedKey::ArrowDown) => {
                    self.pick(cx, self.chosen + 1);
                    true
                }
                _ => false,
            },
            WidgetEvent::Action(AccessAction::Increment) => {
                self.pick(cx, self.chosen + 1);
                true
            }
            WidgetEvent::Action(AccessAction::Decrement) => {
                if self.chosen > 0 {
                    self.pick(cx, self.chosen - 1);
                }
                true
            }
            WidgetEvent::Action(AccessAction::SetValue(label)) => {
                if let Some(i) = self.labels.iter().position(|l| l == label) {
                    self.pick(cx, i);
                }
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut SegmentsState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.pos, dt)
    }

    fn animating(&self, state: &SegmentsState) -> bool {
        !state.pos.is_settled()
    }

    fn access(&self, _state: &SegmentsState, info: &mut AccessInfo) {
        info.role = Some(Role::Group);
        info.value = self.labels.get(self.chosen).cloned();
        info.actions = vec![AccessActionKind::Increment, AccessActionKind::Decrement, AccessActionKind::SetValue];
    }

    fn focusable(&self) -> Focus {
        Focus::Keyboard
    }

    fn cursor(&self) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }
}

/// Choices side by side with a pill under the chosen one that glides when the choice changes.
pub struct Segments<M: 'static> {
    base: Base<M>,
    widget: SegmentsWidget<M>,
}

builder!(Segments<M> => SegmentsWidget<M>);

impl<M: 'static> Segments<M> {
    /// A colour for each segment's pill when it is chosen, in order; `None` keeps the grey one.
    pub fn tones(mut self, tones: impl IntoIterator<Item = Option<Tone>>) -> Self {
        self.widget.tones = tones.into_iter().collect();
        self
    }

    /// 24 high instead of 28, for a dense toolbar.
    pub fn small(mut self) -> Self {
        self.widget.height = 24.0;
        self
    }

    /// Body-size labels instead of small ones, 32 high.
    pub fn large(mut self) -> Self {
        self.widget.height = 32.0;
        self.widget.role = TextRole::Body;
        self
    }
}

/// `(value, label)` for each choice, the chosen value, and what to send when another is picked by
/// click, arrow keys or assistive technology: `segments([(Side::Buy, "Buy"), (Side::Sell, "Sell")],
/// &side, Msg::Side)`. Key it to find it in tests.
pub fn segments<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, impl Into<String>)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Segments<M> {
    let (values, labels): (Vec<T>, Vec<String>) = options.into_iter().map(|(v, l)| (v, l.into())).unzip();
    let chosen = values.iter().position(|v| v == selected).unwrap_or(0);
    let on_pick: Pick<M> = Rc::new(move |i| on_select(values[i].clone()));
    let widget = SegmentsWidget { labels, tones: Vec::new(), chosen, height: 28.0, role: TextRole::Small, on_pick: Some(on_pick) };
    Segments { base: Base::new(), widget }.rounded(Radius::Pill).focus_ring(false)
}
