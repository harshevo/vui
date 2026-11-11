//! Alerts: a tinted box that says something about the page, with an icon for what kind of thing.

use vui_core::{Fill, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, IntoChildren, Parent, Radius, Role, Styled, column, label, row};

use crate::icon::icon;

/// What an alert is about.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AlertKind {
    Info,
    Success,
    Warning,
    Danger,
}

impl AlertKind {
    fn tone(self) -> Tone {
        match self {
            AlertKind::Info => Tone::Blue,
            AlertKind::Success => Tone::Success,
            AlertKind::Warning => Tone::Warning,
            AlertKind::Danger => Tone::Danger,
        }
    }
}

/// A box tinted by `kind`, with its icon, a `title` and an optional line of `detail`. Add buttons
/// or links with [`alert_with`].
pub fn alert<M: 'static>(kind: AlertKind, title: impl Into<String>, detail: impl Into<String>) -> Div<M> {
    alert_with(kind, title, detail, ())
}

/// [`alert`] with `actions` under the text.
pub fn alert_with<M: 'static>(kind: AlertKind, title: impl Into<String>, detail: impl Into<String>, actions: impl IntoChildren<M>) -> Div<M> {
    let tone = kind.tone();
    let glyph = match kind {
        AlertKind::Info => set::INFO,
        AlertKind::Success => set::CHECK_CIRCLE,
        AlertKind::Warning => set::ALERT,
        AlertKind::Danger => set::ALERT_CIRCLE,
    };
    let detail = detail.into();
    let mut buttons = Vec::new();
    actions.append_to(&mut buttons);
    let text = column().gap(2.0).grow(1.0).min_width(0.0).children((
        label(title).weight(500),
        (!detail.is_empty()).then(|| label(detail).color(Tone::Text2)),
        (!buttons.is_empty()).then(|| row().wrap().gap(8.0).margin_top(8.0).children(buttons)),
    ));
    row()
        .gap(12.0)
        .padding(14.0)
        .align_items(Align::Start)
        .bg(Fill::Tone(tone, 0.08))
        .border(1.0, Fill::Tone(tone, 0.28))
        .rounded(Radius::Card)
        .role(if kind == AlertKind::Danger { Role::Status } else { Role::Group })
        .children((icon(glyph).color(tone), text))
}
