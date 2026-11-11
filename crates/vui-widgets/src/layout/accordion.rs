//! Disclosure: a heading that shows or hides what is under it. Several in a column make an accordion.

use vui_core::{CursorIcon, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Focus, IntoChildren, Parent, Role, Styled, column, label, row};

use crate::display::icon::icon;

/// A section titled `title` whose `content` shows while `open`; pressing the heading, or Return or
/// Space on it, sends `on_toggle`. The chevron points down while open.
pub fn disclosure<M: Clone + 'static>(title: impl Into<String>, open: bool, on_toggle: M, content: impl IntoChildren<M>) -> Div<M> {
    let title = title.into();
    let heading = row()
        .height(44.0)
        .gap(8.0)
        .align_items(Align::Center)
        .focus_mode(Focus::Keyboard)
        .role(Role::Button)
        .name(title.clone())
        .cursor(CursorIcon::Pointer)
        .on_click(on_toggle)
        .children((label(title).weight(500).grow(1.0), icon(if open { set::CHEVRON_UP } else { set::CHEVRON_DOWN }).color(Tone::Text3)));
    let body = open.then(|| column().gap(8.0).padding_bottom(16.0).transition(Spring::SNAPPY).appear().children(content));
    column().role(Role::Group).children((heading, body))
}

/// Disclosures stacked with hairlines between them.
pub fn accordion<M: 'static>(sections: impl IntoChildren<M>) -> Div<M> {
    let mut items = Vec::new();
    sections.append_to(&mut items);
    let mut out = column();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out = out.child(vui_runtime::div().height(1.0).bg(Tone::Line));
        }
        out = out.child(item);
    }
    out
}
