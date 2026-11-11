//! Breadcrumbs: where a page sits, each step back a link.

use vui_core::{CursorIcon, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Parent, Role, Styled, label, row};

use crate::icon::icon;

/// The path to the current page: `(label, message)` for each step, the message sent when the step is
/// pressed, `None` for the last step, which is where you are.
pub fn breadcrumbs<M: Clone + 'static>(steps: impl IntoIterator<Item = (String, Option<M>)>) -> Div<M> {
    let steps: Vec<_> = steps.into_iter().collect();
    let last = steps.len().saturating_sub(1);
    let mut trail = row().wrap().gap(6.0).align_items(Align::Center).role(Role::Group).name("Breadcrumbs");
    for (i, (text, go)) in steps.into_iter().enumerate() {
        if i > 0 {
            trail = trail.child(icon(set::CHEVRON_RIGHT).square(14.0).color(Tone::Text3));
        }
        let here = i == last;
        let step = label(text).color(if here { Tone::Text } else { Tone::Text2 }).weight(if here { 500 } else { 400 });
        trail = match go {
            Some(msg) if !here => trail.child(
                row().padding_x(4.0).padding_y(2.0).rounded(vui_runtime::Radius::Px(6.0)).hover_bg(Tone::Hover).cursor(CursorIcon::Pointer)
                    .focusable(true).role(Role::Link).on_click(msg).child(step),
            ),
            _ => trail.child(step),
        };
    }
    trail
}
