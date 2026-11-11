//! Layout for forms and settings: a label over a control, and a title beside a control.

use vui_core::Tone;
use vui_runtime::{Align, Builder, Div, IntoElement, Parent, Styled, column, label, row, small};

/// A label over a control, 8 points apart: `field("Name", text_field(name))`.
///
/// The label is also the control's name for assistive technology (unless the control was named
/// already), so a screen reader says "Name, text field" once instead of reading the label and then an
/// unnamed field.
pub fn field<M: 'static, C: Builder<M> + IntoElement<M>>(name: impl Into<String>, control: C) -> Div<M> {
    let name = name.into();
    let heading = small(name.clone()).color(Tone::Text2).weight(500).a11y_hidden(true);
    column().gap(8.0).children((heading, control.default_name(name)))
}

/// A line of a settings list: a title with an optional line under it on the left, the control on the
/// right. An empty `detail` leaves the line out. Lines are at least 48 high, so a list of them keeps
/// one rhythm whether its controls are 20 high (a switch) or 32 (a field).
///
/// As in [`field`], the title names the control unless the control was named already, and the detail
/// line describes it.
pub fn setting<M: 'static, C: Builder<M> + IntoElement<M>>(
    title: impl Into<String>,
    detail: impl Into<String>,
    control: C,
) -> Div<M> {
    let title = title.into();
    let detail = detail.into();
    let mut control = control.default_name(title.clone());
    let mut text = column().grow(1.0).min_width(0.0).child(label(title).color(Tone::Text).a11y_hidden(true));
    if !detail.is_empty() {
        text = text.child(small(detail.clone()).color(Tone::Text3).a11y_hidden(true));
        control = control.describe(detail);
    }
    row().align_items(Align::Center).gap(16.0).padding_y(8.0).min_height(48.0).child(text).child(control_slot(control))
}

/// The control keeps its size however long the text beside it is.
fn control_slot<M: 'static>(control: impl IntoElement<M>) -> Div<M> {
    row().shrink(0.0).align_items(Align::Center).child(control)
}
