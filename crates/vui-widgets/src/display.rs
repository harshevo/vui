//! Things that are only looked at: dividers, badges, keycaps, avatars, cards and small headings.

use vui_core::{Fill, ImageId, TextRole, Tone};
use vui_runtime::{Base, Builder, Div, Img, Label, Radius, Role, Styled, builder, div, img, small};

use crate::item::{Height, Item, ItemWidget, Place, Spec};

/// A 1-point line across its parent.
pub fn separator<M: 'static>() -> Div<M> {
    div().height(1.0).shrink(0.0).bg(Tone::Line).role(Role::Separator)
}

/// A 1-point line down its parent.
pub fn separator_vertical<M: 'static>() -> Div<M> {
    div().width(1.0).shrink(0.0).bg(Tone::Line).role(Role::Separator)
}

/// A box for one repeatable unit: a result, a plugin, a saved memory. Never put one inside another.
pub fn card<M: 'static>() -> Div<M> {
    div().padding(16.0).bg(Tone::Bg).border(1.0, Tone::Line).rounded(Radius::Card)
}

/// Small grey heading over a group: "Recent", "Today".
pub fn section_title<M: 'static>(text: impl Into<String>) -> Label<M> {
    small(text).color(Tone::Text3).weight(500).role(Role::Heading)
}

/// A solid dot, 8 points across: live, idle, unread.
pub fn dot<M: 'static>(fill: impl Into<Fill>) -> Div<M> {
    div().square(8.0).shrink(0.0).rounded(Radius::Pill).bg(fill)
}

// ---- badge -----------------------------------------------------------------------------------

/// What a badge says about itself by colour.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BadgeTint {
    Neutral,
    Accent,
    Danger,
    Orange,
    Blue,
    Purple,
}

/// The grey of a chip that must stay visible on any row, even a hovered or open one: a little of the
/// text colour over whatever is behind, where a fixed grey would melt into a row of the same grey.
const CHIP: Fill = Fill::Tone(Tone::Text, 0.07);

/// A short label in a pill: a count, a state, "Beta".
pub struct Badge<M: 'static> {
    base: Base<M>,
    widget: ItemWidget<M>,
}

builder!(Badge<M> => ItemWidget<M>);

impl<M: 'static> Badge<M> {
    pub fn tint(mut self, tint: BadgeTint) -> Self {
        let (text, wash): (Tone, Fill) = match tint {
            BadgeTint::Neutral => (Tone::Text2, CHIP),
            BadgeTint::Accent => (Tone::Accent, Fill::Tone(Tone::Accent, 0.12)),
            BadgeTint::Danger => (Tone::Danger, Fill::Tone(Tone::Danger, 0.12)),
            BadgeTint::Orange => (Tone::Orange, Fill::Tone(Tone::Orange, 0.14)),
            BadgeTint::Blue => (Tone::Blue, Fill::Tone(Tone::Blue, 0.12)),
            BadgeTint::Purple => (Tone::Purple, Fill::Tone(Tone::Purple, 0.12)),
        };
        self.widget.spec.label_color = text.into();
        self.widget.spec.icon_color = text.into();
        self.bg(wash)
    }

    pub fn accent(self) -> Self {
        self.tint(BadgeTint::Accent)
    }

    pub fn danger(self) -> Self {
        self.tint(BadgeTint::Danger)
    }

    pub fn orange(self) -> Self {
        self.tint(BadgeTint::Orange)
    }

    pub fn blue(self) -> Self {
        self.tint(BadgeTint::Blue)
    }

    pub fn purple(self) -> Self {
        self.tint(BadgeTint::Purple)
    }
}

pub fn badge<M: 'static>(text: impl Into<String>) -> Badge<M> {
    let mut spec = Spec::new(text, Role::Label);
    spec.text = TextRole::Small;
    spec.weight = 500;
    spec.height = Height::Content;
    spec.place = Place::Center;
    spec.focus = vui_runtime::Focus::Never;
    let badge = Badge { base: Base::new(), widget: ItemWidget::new(spec) };
    badge.padding_x(8.0).padding_y(1.0).rounded(Radius::Pill).tint(BadgeTint::Neutral)
}

// ---- keycap ----------------------------------------------------------------------------------

/// A key as it is printed on a keyboard: "⌘K", "Esc".
pub fn kbd<M: 'static>(keys: impl Into<String>) -> Item<M> {
    let mut spec = Spec::new(keys, Role::Label);
    spec.text = TextRole::Small;
    spec.mono = true;
    spec.label_color = Tone::Text2.into();
    spec.height = Height::Content;
    spec.place = Place::Center;
    spec.focus = vui_runtime::Focus::Never;
    Item::from_spec(spec).padding_x(6.0).padding_y(1.0).border(1.0, Tone::Line).rounded(Radius::Px(5.0))
}

// ---- avatar ----------------------------------------------------------------------------------

/// The first letters of the first two words of a name: "Ada Lovelace" is "AL".
fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|word| word.chars().next()).take(2).flat_map(char::to_uppercase).collect()
}

/// A round badge with someone's initials, 32 points across.
pub fn avatar<M: 'static>(name: &str) -> Item<M> {
    avatar_sized(name, 32.0)
}

pub fn avatar_sized<M: 'static>(name: &str, side: f32) -> Item<M> {
    let mut spec = Spec::new(initials(name), Role::Image);
    spec.text = if side >= 40.0 { TextRole::Body } else { TextRole::Small };
    spec.weight = 500;
    spec.label_color = Tone::Text2.into();
    spec.height = Height::Content;
    spec.place = Place::Center;
    spec.focus = vui_runtime::Focus::Never;
    Item::from_spec(spec).square(side).shrink(0.0).bg(CHIP).rounded(Radius::Pill).name(name.to_string())
}

/// A round picture.
pub fn avatar_image<M: 'static>(id: ImageId, side: f32) -> Img<M> {
    img(id).fit(vui_core::Fit::Cover).square(side).shrink(0.0).rounded(Radius::Pill).ring()
}
