//! Empty states: what a list or page shows while it has nothing in it.

use vui_core::{Fill, Tone};
use vui_icons::Icon;
use vui_runtime::{Align, Builder, Div, IntoChildren, Justify, Parent, Radius, Styled, column, div, label};

use crate::icon::icon;

/// An icon in a soft circle, a `title`, a line of `detail`, and what to do about it (`actions`),
/// centred in the space it is given.
pub fn empty_state<M: 'static>(glyph: Icon, title: impl Into<String>, detail: impl Into<String>, actions: impl IntoChildren<M>) -> Div<M> {
    column().padding(32.0).gap(8.0).align_items(Align::Center).justify(Justify::Center).children((
        div().square(48.0).rounded(Radius::Pill).bg(Fill::Tone(Tone::Accent, 0.10)).align_items(Align::Center).justify(Justify::Center).margin_bottom(8.0).child(icon(glyph).color(Tone::Accent)),
        label(title).weight(600),
        label(detail).color(Tone::Text2).max_width(360.0),
        vui_runtime::row().gap(8.0).margin_top(12.0).children(actions),
    ))
}
