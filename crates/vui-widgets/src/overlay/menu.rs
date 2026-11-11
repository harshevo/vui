//! Menus: a column of commands in a floating card, moved through with the arrow keys.

use vui_core::{Fill, Spring, TextRole, Tone};
use vui_icons::set;
use vui_runtime::{Builder, Div, Focus, FocusGroup, IntoChildren, Parent, Radius, Role, Styled, div};

use super::popover::panel;
use crate::data::item::{Height, Item, Place, Spec};

/// A menu's narrowest width.
const MIN_WIDTH: f32 = 200.0;

/// The items of a menu in its card. Up and Down move between them, Return chooses.
pub fn menu<M: 'static>(items: impl IntoChildren<M>) -> Div<M> {
    panel().min_width(MIN_WIDTH).role(Role::Menu).focus_group(FocusGroup::vertical().wrapping()).children(items)
}

/// A command in a menu. Give it `on_click`; add `.icon(..)`, a shortcut with `.detail("⌘C")`, or make
/// it red with `.text_color(Tone::Danger)` for something that cannot be undone.
pub fn menu_item<M: 'static>(label: impl Into<String>) -> Item<M> {
    let mut spec = Spec::new(label, Role::MenuItem);
    spec.place = Place::Start;
    spec.height = Height::Small;
    spec.text = TextRole::Body;
    spec.focus = Focus::Keyboard;
    Item::from_spec(spec)
        .padding_x(8.0)
        .min_height(30.0)
        .rounded(Radius::Control)
        .hover_bg(Tone::Hover)
        .press_bg(Tone::Wash)
        .transition(Spring::SNAPPY)
        .cursor(vui_core::CursorIcon::Pointer)
}

/// A menu item with a tick when `checked`. Items without the tick keep its room, so labels line up.
pub fn menu_check<M: 'static>(label: impl Into<String>, checked: bool) -> Item<M> {
    let mut item = menu_item(label).icon(set::CHECK).icon_color(if checked { Fill::from(Tone::Accent) } else { Fill::NONE });
    item.toggle_state(checked);
    item
}

/// A hairline between groups of items.
pub fn menu_separator<M: 'static>() -> Div<M> {
    div().height(1.0).margin_y(4.0).margin_x(4.0).bg(Tone::Line).role(Role::Separator)
}

/// A small grey title over a group of items.
pub fn menu_heading<M: 'static>(text: impl Into<String>) -> Item<M> {
    let mut spec = Spec::new(text, Role::Heading);
    spec.place = Place::Start;
    spec.height = Height::Content;
    spec.text = TextRole::Small;
    spec.weight = 500;
    spec.focus = Focus::Never;
    spec.label_color = Tone::Text3.into();
    Item::from_spec(spec).padding_x(8.0).padding_y(6.0)
}
