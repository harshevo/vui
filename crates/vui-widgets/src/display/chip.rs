//! Chips: a removable tag, such as a filter that is on or a recipient in a field.

use vui_core::{Fill, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Parent, Radius, Styled, row};

use crate::display::icon::icon;

/// A pill with `text` and, given `on_remove`, an × that sends it.
pub fn chip<M: Clone + 'static>(text: impl Into<String>, on_remove: Option<M>) -> Div<M> {
    let text = text.into();
    let remove = on_remove.map(|m| {
        row()
            .square(16.0)
            .rounded(Radius::Pill)
            .align_items(Align::Center)
            .justify(vui_runtime::Justify::Center)
            .hover_bg(Tone::Hover)
            .cursor(vui_core::CursorIcon::Pointer)
            .focusable(true)
            .role(vui_runtime::Role::Button)
            .name(format!("Remove {text}"))
            .on_click(m)
            .child(icon(set::X).square(12.0).color(Tone::Text2))
    });
    row()
        .height(26.0)
        .padding_left(10.0)
        .padding_right(if remove.is_some() { 4.0 } else { 10.0 })
        .gap(4.0)
        .align_items(Align::Center)
        .bg(Fill::Tone(Tone::Text, 0.06))
        .rounded(Radius::Pill)
        .children((vui_runtime::small(text).weight(500), remove))
}
