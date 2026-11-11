//! Number inputs: a value with buttons to step it down and up.

use vui_core::{NamedKey, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Focus, Justify, Parent, Radius, Role, Styled, label, row};

use crate::button::icon_button;

/// `value` between `min` and `max`, with − and + buttons that move it by `step`. Up and Down do the
/// same while it has focus. `on_change` hears the new value, already kept in range.
pub fn number_input<M: Clone + 'static>(value: f64, min: f64, max: f64, step: f64, on_change: impl Fn(f64) -> M + Clone + 'static) -> Div<M> {
    let clamp = move |v: f64| v.clamp(min, max);
    let text = if step.fract() == 0.0 { format!("{value:.0}") } else { format!("{value}") };
    let keys = {
        let on_change = on_change.clone();
        move |k: &vui_core::KeyEvent| match k.key {
            vui_core::Key::Named(NamedKey::ArrowUp) if k.pressed && value < max => Some(on_change(clamp(value + step))),
            vui_core::Key::Named(NamedKey::ArrowDown) if k.pressed && value > min => Some(on_change(clamp(value - step))),
            _ => None,
        }
    };
    row()
        .height(32.0)
        .padding_x(2.0)
        .align_items(Align::Center)
        .bg(Tone::Field)
        .border(1.0, Tone::Line)
        .rounded(Radius::Control)
        .focus_mode(Focus::Keyboard)
        .role(Role::Slider)
        .on_key(keys)
        .children((
            icon_button(set::MINUS, "Decrease").small().disabled(value <= min).on_click(on_change(clamp(value - step))),
            row().min_width(40.0).grow(1.0).justify(Justify::Center).child(label(text)),
            icon_button(set::PLUS, "Increase").small().disabled(value >= max).on_click(on_change(clamp(value + step))),
        ))
}
