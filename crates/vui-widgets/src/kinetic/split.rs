//! The split button and the tone chip: one control where there would be two or three.

use vui_core::{CursorIcon, Fill, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Parent, Radius, Role, Styled, div, row, small};

use crate::ButtonVariant;
use crate::display::icon::icon;

/// One pill split by a hairline: the label does the usual thing (`on_click`), the chevron opens the
/// variants (`on_menu`). Put it in a `dropdown` with the variants as its items.
pub fn split_button<M: Clone + 'static>(label: impl Into<String>, variant: ButtonVariant, on_click: M, on_menu: M) -> Div<M> {
    let label = label.into();
    let (bg, text) = match variant {
        ButtonVariant::Primary => (Fill::from(Tone::Accent), Tone::OnAccent),
        ButtonVariant::Danger => (Fill::from(Tone::Danger), Tone::OnDanger),
        ButtonVariant::Secondary => (Fill::from(Tone::Wash), Tone::Text),
        ButtonVariant::Ghost => (Fill::NONE, Tone::Text),
        ButtonVariant::Soft(tone) => (Fill::Tone(tone, 0.16), tone),
        ButtonVariant::Solid(tone) => (Fill::from(tone), Tone::Bg),
    };
    let half = |key: &str| {
        row()
            .key(key.to_string())
            .role(Role::Button)
            .height(28.0)
            .align_items(Align::Center)
            .hover_bg(Fill::Tone(Tone::Text, 0.10))
            .press_bg(Fill::Tone(Tone::Text, 0.18))
            .cursor(CursorIcon::Pointer)
    };
    row()
        .height(28.0)
        .rounded(Radius::Pill)
        .bg(bg)
        .align_items(Align::Center)
        .overflow(vui_runtime::Overflow::Hidden)
        .shrink(0.0)
        .children((
            half("split-main").name(label.clone()).grow(1.0).justify(vui_runtime::Justify::Center).padding_left(12.0).padding_right(10.0).on_click(on_click).child(small(label).weight(500).color(text).max_lines(1)),
            div().width(1.0).height(16.0).bg(Fill::Tone(text, 0.25)),
            half("split-more").name("More").padding_left(6.0).padding_right(8.0).on_click(on_menu).child(icon(set::CHEVRON_DOWN).square(12.0).color(text)),
        ))
}

/// A status that is also a control: a dot and a word on a wash of `tone`. Clicking sends `on_click`,
/// for example to cycle the state it shows: `tone_chip("Armed", Tone::Success, Some(Msg::Disarm))`.
pub fn tone_chip<M: Clone + 'static>(text: impl Into<String>, tone: Tone, on_click: Option<M>) -> Div<M> {
    let text = text.into();
    let chip = row()
        .height(22.0)
        .padding_x(8.0)
        .gap(6.0)
        .align_items(Align::Center)
        .rounded(Radius::Pill)
        .bg(Fill::Tone(tone, 0.12))
        .shrink(0.0)
        .name(text.clone())
        .children((div().square(6.0).rounded(Radius::Pill).bg(tone), small(text).weight(500).color(tone).max_lines(1)));
    match on_click {
        Some(m) => chip.role(Role::Button).hover_bg(Fill::Tone(tone, 0.22)).cursor(CursorIcon::Pointer).on_click(m),
        None => chip,
    }
}
