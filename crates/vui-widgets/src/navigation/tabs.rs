//! Tabs: a row of views to switch between, with an accent line under the open one.

use std::rc::Rc;

use vui_core::{Fill, Spring, TextRole, Tone};
use vui_runtime::{Align, Breakpoint, Builder, Div, Focus, FocusGroup, IntoChildren, Parent, Role, Styled, column, div, row};

use crate::data::item::{Height, Item, Place, Spec};

/// How thick the line under the open tab is.
const LINE: f32 = 2.0;

/// The tab strip: `(value, label)` for each tab, the open one, and what to send when another is
/// chosen. Arrow keys move along the strip and open as they go; Tab moves past it. Put the open view
/// in a [`tab_panel`] under it. Keys are `{key}-{index}` when the strip is keyed with `tabs_keyed`.
pub fn tabs<M: 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    strip(None, options, selected, on_select)
}

/// [`tabs`] with the strip keyed `key` and each tab `{key}-{index}`.
pub fn tabs_keyed<M: 'static, T: PartialEq + Clone + 'static>(
    key: &str,
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    strip(Some(key), options, selected, on_select).key(key.to_string())
}

fn strip<M: 'static, T: PartialEq + Clone + 'static>(
    key: Option<&str>,
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    on_select: impl Fn(T) -> M + 'static,
) -> Div<M> {
    let on_select = Rc::new(on_select);
    let tabs: Vec<_> = options
        .into_iter()
        .enumerate()
        .map(|(i, (value, label))| {
            let open = value == *selected;
            let mut spec = Spec::new(label, Role::Tab);
            spec.weight = 500;
            spec.text = TextRole::Body;
            spec.height = Height::Content;
            spec.place = Place::Center;
            spec.label_color = if open { Tone::Text } else { Tone::Text2 }.into();
            spec.selected = Some(open);
            spec.focus = Focus::Keyboard;
            let pick = {
                let on_select = on_select.clone();
                move || on_select(value.clone())
            };
            let label = Item::from_spec(spec)
                .padding_x(4.0)
                .padding_y(8.0)
                .tab_default(open)
                .select_on_arrow(pick.clone())
                .on_click_with(pick)
                .cursor(vui_core::CursorIcon::Pointer);
            let label = match key {
                Some(key) => label.key(format!("{key}-{i}")),
                None => label,
            };
            column().align_items(Align::Stretch).children((
                label,
                div().height(LINE).rounded(vui_runtime::Radius::Pill).bg(if open { Fill::from(Tone::Accent) } else { Fill::NONE }).transition(Spring::SNAPPY),
            ))
        })
        .collect();
    row()
        .gap(20.0)
        .border(0.0, Fill::NONE)
        .role(Role::TabList)
        .focus_group(FocusGroup::horizontal())
        // More tabs than a narrow window has room for scroll sideways.
        .when(Breakpoint::Compact, |s| s.overflow(vui_runtime::Overflow::Scroll).gap(16.0))
        .children(tabs)
        .child(div().fill_width().height(1.0).absolute().bottom(0.0).left(0.0).bg(Tone::Line))
}

/// The open tab's view, under the strip.
pub fn tab_panel<M: 'static>(content: impl IntoChildren<M>) -> Div<M> {
    column().padding_top(16.0).role(Role::TabPanel).children(content)
}
