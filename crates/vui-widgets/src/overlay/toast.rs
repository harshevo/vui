//! Toasts: short notes in the corner of the window that go away by themselves.

use vui_core::{Shadow, Spring, Tone};
use vui_icons::Icon;
use vui_runtime::{Align, Builder, Div, IntoChildren, Parent, Radius, Role, Styled, column, label, row};

use crate::icon::icon;

/// The corner stack the toasts sit in, newest last, above everything. Give each toast a key so the
/// others keep their place as one leaves. Hide them with a timer: `cx.after(Duration::from_secs(4), Msg::Hide(id))`.
pub fn toasts<M: 'static>(items: impl IntoChildren<M>) -> Div<M> {
    column().fixed().right(16.0).bottom(16.0).gap(8.0).align_items(Align::End).role(Role::Status).pointer_events(false).children(items)
}

/// One note: dark on a light page, light on a dark one, with an optional icon before the text.
pub fn toast<M: 'static>(text: impl Into<String>, lead: Option<Icon>) -> Div<M> {
    row()
        .gap(8.0)
        .align_items(Align::Center)
        .padding_x(14.0)
        .padding_y(10.0)
        .max_width(360.0)
        .bg(Tone::Text)
        .rounded(Radius::Card)
        .shadow(Shadow::RAISED)
        .pointer_events(true)
        .transition(Spring::SNAPPY)
        .appear()
        .children((lead.map(|i| icon(i).color(Tone::Bg)), label(text).color(Tone::Bg)))
}
