//! Dialogs: a card in the middle of the window over a dimmed page, which keeps the keyboard until it
//! closes.

use vui_core::{Shadow, Spring, Tone};
use vui_runtime::{Align, Breakpoint, Builder, Div, IntoChildren, Justify, Parent, Radius, Role, Styled, column, div, pct, row};

/// How wide a dialog is, at most.
const WIDTH: f32 = 440.0;

/// A dialog with `content` stacked in a card: a title, some text, [`dialog_actions`]. Pressing the
/// dimmed page around it or Escape sends `on_close`; Tab stays inside it while it shows. Show it while
/// the app wants it open: `self.confirming.then(|| dialog(Msg::Cancel, (...)))`.
pub fn dialog<M: Clone + 'static>(on_close: M, content: impl IntoChildren<M>) -> Div<M> {
    let card = column::<M>()
        .width(WIDTH)
        .max_width(pct(92.0))
        // On a phone-sized window it becomes a sheet across the bottom.
        .when(Breakpoint::Compact, |s| s.width(pct(100.0)).max_width(pct(100.0)).padding(20.0))
        .padding(24.0)
        .gap(16.0)
        .bg(Tone::Bg)
        .rounded(Radius::Panel)
        .shadow(Shadow::FLOATING)
        .role(Role::Dialog)
        .trap_focus()
        .on_dismiss(on_close)
        .transition(Spring::SNAPPY)
        .appear()
        .children(content);
    div()
        .key("vui-dialog")
        .fixed()
        .top(0.0)
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .bg(Tone::Scrim)
        .transition(Spring::SMOOTH)
        .appear()
        .align_items(Align::Center)
        .justify(Justify::Center)
        .when(Breakpoint::Compact, |s| s.justify(Justify::End).padding(8.0))
        .child(card)
}

/// The buttons along the bottom of a dialog, at the right: the safe choice first, the main one last.
pub fn dialog_actions<M: 'static>(buttons: impl IntoChildren<M>) -> Div<M> {
    row().gap(8.0).wrap().justify(Justify::End).margin_top(8.0).children(buttons)
}
