//! The content most clickable things have in common: an icon, a label, some detail on the right and
//! a chevron. Buttons, sidebar items, badges, keycaps and avatars are this widget with different
//! colours, sizes and boxes around it.

use std::rc::Rc;

use vui_core::{Event, Fill, FontFamily, Size, TextLayout, TextRole, TextStyle, TextWrap, Theme, Tone, pt, size};
use vui_icons::{Icon, Weight};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessAction, AccessInfo, Base, Dirty, Env, EventCx, Focus, FocusBy, PaintCx, Role, TickCx, Widget, WidgetEvent, builder,
};

use crate::icon::{ICON_SIZE, paint_icon, paint_spinner};

/// Space between the parts.
const GAP: f32 = 8.0;

/// Where the content sits in a box wider than itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Start,
    Center,
}

/// How tall the box is at least.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Height {
    /// As tall as the content.
    Content,
    Small,
    /// The theme's control height.
    Control,
    Large,
}

/// Everything that decides how the content looks and how big it is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Spec {
    pub icon: Option<Icon>,
    pub label: String,
    pub detail: String,
    pub trailing: Option<Icon>,
    pub weight: u16,
    pub text: TextRole,
    pub mono: bool,
    pub label_color: Fill,
    pub icon_color: Fill,
    pub detail_color: Fill,
    pub place: Place,
    pub height: Height,
    /// As wide as it is tall: a button with only an icon.
    pub square: bool,
    pub busy: bool,
    pub role: Role,
    pub selected: Option<bool>,
    pub toggled: Option<bool>,
    pub focus: Focus,
}

impl Spec {
    pub fn new(label: impl Into<String>, role: Role) -> Spec {
        Spec {
            icon: None,
            label: label.into(),
            detail: String::new(),
            trailing: None,
            weight: 400,
            text: TextRole::Body,
            mono: false,
            label_color: Tone::Text.into(),
            icon_color: Tone::Text2.into(),
            detail_color: Tone::Text3.into(),
            place: Place::Center,
            height: Height::Control,
            square: false,
            busy: false,
            role,
            selected: None,
            toggled: None,
            focus: Focus::Keyboard,
        }
    }

    fn style(&self, theme: &Theme) -> TextStyle {
        let mut style = TextStyle::from_token(theme.types.get(self.text));
        style.weight = self.weight;
        if self.mono {
            style.family = FontFamily::Mono;
        }
        style
    }

    fn detail_style(&self, theme: &Theme) -> TextStyle {
        let mut style = TextStyle::from_token(theme.types.small);
        style.weight = 400;
        if self.mono {
            style.family = FontFamily::Mono;
        }
        style.wrap = TextWrap::None;
        style
    }

    fn has_lead(&self) -> bool {
        self.icon.is_some() || self.busy
    }
}

/// The widget behind buttons, items, badges and the like.
pub struct ItemWidget<M> {
    pub(crate) spec: Spec,
    /// Sent when the arrow keys move focus onto the item: how segments choose as focus passes.
    pub(crate) on_arrow: Option<Rc<dyn Fn() -> M>>,
}

impl<M> ItemWidget<M> {
    pub(crate) fn new(spec: Spec) -> ItemWidget<M> {
        ItemWidget { spec, on_arrow: None }
    }
}

pub struct ItemState {
    baseline: f32,
}

/// The measured parts of the content.
struct Parts {
    label: Option<Rc<TextLayout>>,
    detail: Option<Rc<TextLayout>>,
    lead: f32,
    trail: f32,
    /// Width of everything with its gaps, and the height of the tallest part.
    natural: Size,
}

impl<M> ItemWidget<M> {
    fn parts(&self, env: &Env) -> Parts {
        let spec = &self.spec;
        let theme = &env.theme;
        let fonts = env.fonts();
        let mut one_line = spec.style(theme);
        one_line.wrap = TextWrap::None;
        let label = (!spec.label.is_empty()).then(|| fonts.layout_cached(&spec.label, &one_line, None));
        let detail = (!spec.detail.is_empty()).then(|| fonts.layout_cached(&spec.detail, &spec.detail_style(theme), None));
        let lead = if spec.has_lead() { ICON_SIZE } else { 0.0 };
        let trail = if spec.trailing.is_some() { ICON_SIZE } else { 0.0 };
        let widths = [lead, label.as_ref().map_or(0.0, |l| l.size().w), detail.as_ref().map_or(0.0, |l| l.size().w), trail];
        let count = widths.iter().filter(|w| **w > 0.0).count();
        let width = widths.iter().sum::<f32>() + GAP * count.saturating_sub(1) as f32;
        let height = [lead, label.as_ref().map_or(0.0, |l| l.size().h), detail.as_ref().map_or(0.0, |l| l.size().h), trail]
            .into_iter()
            .fold(0.0, f32::max);
        Parts { label, detail, lead, trail, natural: size(width, height) }
    }

    fn min_height(&self, theme: &Theme) -> f32 {
        match self.spec.height {
            Height::Content => 0.0,
            Height::Small => 24.0,
            Height::Control => theme.control,
            Height::Large => theme.control_large,
        }
    }
}

impl<M: 'static> Widget<M> for ItemWidget<M> {
    type State = ItemState;

    fn create_state(&self) -> ItemState {
        ItemState { baseline: 0.0 }
    }

    fn update(&self, old: &ItemWidget<M>, _state: &mut ItemState) -> Dirty {
        if self.spec != old.spec { Dirty::Layout } else { Dirty::None }
    }

    fn measure(&self, state: &mut ItemState, env: &Env, known: Pair<Option<f32>>, _avail: Pair<Avail>) -> Size {
        let parts = self.parts(env);
        let height = known.y.unwrap_or_else(|| parts.natural.h.max(self.min_height(&env.theme)));
        let width = known.x.unwrap_or(if self.spec.square { height } else { parts.natural.w });
        let line = parts.label.as_ref().map_or(0.0, |l| l.size().h);
        state.baseline = (height - line) / 2.0 + parts.label.as_ref().map_or(0.0, |l| l.baseline());
        size(width, height)
    }

    fn baseline(&self, state: &ItemState) -> Option<f32> {
        Some(state.baseline)
    }

    fn paint(&self, _state: &ItemState, cx: &mut PaintCx) {
        let spec = &self.spec;
        let parts = self.parts(cx.env);
        let c = cx.content;
        // The box the layout gave the content, before it was moved onto the pixel grid: the label is
        // cut to what is left of this, so one sized to its text is never a fraction too narrow.
        let width = cx.exact.w;
        let group = match spec.place {
            Place::Start => width,
            Place::Center => parts.natural.w.min(width),
        };
        let (mut left, mut right) = match spec.place {
            Place::Start => (c.x, c.right()),
            Place::Center => {
                let x = c.x + (c.w - group.min(c.w)) / 2.0;
                (x, x + group)
            }
        };
        let scale = cx.env.scale;
        let snap = |v: f32| (v * scale).round() / scale;
        let mid = c.y + c.h / 2.0;
        let icon_at = |x: f32| vui_core::rect(snap(x), snap(mid - ICON_SIZE / 2.0), ICON_SIZE, ICON_SIZE);
        let mut room = group;

        if parts.lead > 0.0 {
            let at = icon_at(left);
            if spec.busy {
                paint_spinner(cx, at, spec.icon_color);
            } else if let Some(icon) = spec.icon {
                paint_icon(cx, icon, at, spec.icon_color, Weight::Regular);
            }
            left += parts.lead + GAP;
            room -= parts.lead + GAP;
        }
        if let Some(icon) = spec.trailing {
            right -= parts.trail;
            paint_icon(cx, icon, icon_at(right), Tone::Text3, Weight::Regular);
            right -= GAP;
            room -= parts.trail + GAP;
        }
        if let Some(detail) = &parts.detail {
            let w = detail.size().w;
            right -= w;
            let y = snap(mid - detail.size().h / 2.0);
            let at = pt(right.max(left), y);
            cx.text(detail.clone(), at, spec.detail_color);
            room -= w + GAP;
        }
        if parts.label.is_some() {
            // A label that does not fit ends in an ellipsis rather than running over its neighbours.
            let mut style = spec.style(cx.theme());
            style.max_lines = Some(1);
            let layout = cx.fonts().layout_cached(&spec.label, &style, Some(room.max(0.0) + 0.01));
            let y = snap(mid - layout.size().h / 2.0);
            cx.text(layout, pt(left, y), spec.label_color);
        }
    }

    fn event(&self, _state: &mut ItemState, cx: &mut EventCx<M>, event: &WidgetEvent) -> bool {
        if let WidgetEvent::FocusChanged { focused: true, by: FocusBy::Arrow, .. } = event
            && let Some(select) = &self.on_arrow
        {
            cx.emit(select());
        }
        if !self.spec.busy {
            return false;
        }
        // Something is already under way: the press goes nowhere.
        matches!(event, WidgetEvent::Input(Event::PointerDown { .. }) | WidgetEvent::Action(AccessAction::Click))
    }

    fn tick(&self, _state: &mut ItemState, _cx: &mut TickCx, _dt: f32) -> bool {
        self.spec.busy
    }

    fn animating(&self, _state: &ItemState) -> bool {
        self.spec.busy
    }

    fn access(&self, _state: &ItemState, info: &mut AccessInfo) {
        info.role = Some(self.spec.role);
        info.label = (!self.spec.label.is_empty()).then(|| self.spec.label.clone());
        info.selected = self.spec.selected;
        info.toggled = self.spec.toggled;
    }

    fn focusable(&self) -> Focus {
        self.spec.focus
    }
}

/// A line of content: an icon, a label, some detail and a chevron, in a box that is yours to style.
/// Nothing happens when it is clicked until it has a handler; see [`item`].
pub struct Item<M: 'static> {
    base: Base<M>,
    widget: ItemWidget<M>,
}

builder!(Item<M> => ItemWidget<M>);

impl<M: 'static> Item<M> {
    pub(crate) fn from_spec(spec: Spec) -> Item<M> {
        Item { base: Base::new(), widget: ItemWidget::new(spec) }
    }

    /// Reports the item to assistive technology as checked or not.
    pub(crate) fn toggle_state(&mut self, on: bool) {
        self.widget.spec.toggled = Some(on);
    }

    pub(crate) fn select_on_arrow(mut self, f: impl Fn() -> M + 'static) -> Self {
        self.widget.on_arrow = Some(Rc::new(f));
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.widget.spec.icon = Some(icon);
        self
    }

    /// An icon at the far end, such as a chevron on a row that opens something.
    pub fn trailing(mut self, icon: Icon) -> Self {
        self.widget.spec.trailing = Some(icon);
        self
    }

    /// Small grey text before the trailing icon: a count, a shortcut, a time.
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        self.widget.spec.detail = text.into();
        self
    }

    pub fn weight(mut self, weight: u16) -> Self {
        self.widget.spec.weight = weight;
        self
    }

    pub fn text_color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.spec.label_color = fill.into();
        self
    }

    pub fn icon_color(mut self, fill: impl Into<Fill>) -> Self {
        self.widget.spec.icon_color = fill.into();
        self
    }
}

/// A label, with an icon and detail if you add them. It is static text to assistive technology; make it
/// a button by giving it `on_click` and `.focus_mode(Focus::Keyboard)` and `.role(Role::Button)`.
pub fn item<M: 'static>(label: impl Into<String>) -> Item<M> {
    let mut spec = Spec::new(label, Role::Label);
    spec.focus = Focus::Never;
    spec.height = Height::Content;
    spec.place = Place::Start;
    Item::from_spec(spec)
}
