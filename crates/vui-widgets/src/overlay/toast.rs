//! Toasts: short notes in the corner of the window that go away by themselves.

use vui_core::{Shadow, Spring, Tone};
use vui_icons::Icon;
use vui_runtime::{Align, Breakpoint, Builder, Div, IntoChildren, Parent, Radius, Role, Styled, column, label, row};

use crate::display::icon::icon;

/// The corner stack the toasts sit in, newest last, above everything. Give each toast a key so the
/// others keep their place as one leaves. Hide them with a timer: `cx.after(Duration::from_secs(4), Msg::Hide(id))`.
pub fn toasts<M: 'static>(items: impl IntoChildren<M>) -> Div<M> {
    column()
        .fixed()
        .right(16.0)
        .bottom(16.0)
        .gap(8.0)
        .align_items(Align::End)
        // A narrow window drops them in from the top, clear of sheets and bottom bars.
        .when(Breakpoint::Compact, |s| s.left(12.0).right(12.0).top(12.0).bottom(vui_runtime::AUTO).align_items(Align::Stretch))
        .role(Role::Status)
        .pointer_events(false)
        .children(items)
}

/// One note, with an optional icon before the text. It is a floating surface like a menu or a
/// dialog: the page's fill with the page's text, or the theme's glass when floating surfaces are
/// glass. (An inverted note would lose its fill to the glass and leave dark text on dark glass.)
pub fn toast<M: 'static>(text: impl Into<String>, lead: Option<Icon>) -> Div<M> {
    row()
        .gap(8.0)
        .align_items(Align::Center)
        .padding_x(14.0)
        .padding_y(10.0)
        .max_width(360.0)
        .bg(Tone::Bg)
        .border(1.0, Tone::Line)
        .rounded(Radius::Card)
        .shadow(Shadow::RAISED)
        .floating_glass()
        .pointer_events(true)
        .transition(Spring::SNAPPY)
        .appear()
        .children((lead.map(|i| icon(i).color(Tone::Text2)), label(text).color(Tone::Text)))
}
