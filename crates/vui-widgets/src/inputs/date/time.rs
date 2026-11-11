//! The time field: hours and minutes, each stepped with the arrow keys or its buttons.

use std::rc::Rc;

use vui_core::{Key, KeyEvent, NamedKey, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Focus, Parent, Radius, Role, Styled, column, label, row};

use crate::buttons::button::icon_button;

/// One part of the time: its digits, with small buttons above and below.
fn part<M: Clone + 'static>(value: u8, name: &str, up: M, down: M) -> Div<M> {
    let (key_up, key_down) = (up.clone(), down.clone());
    let keys = move |k: &KeyEvent| match k.key {
        Key::Named(NamedKey::ArrowUp) if k.pressed => Some(key_up.clone()),
        Key::Named(NamedKey::ArrowDown) if k.pressed => Some(key_down.clone()),
        _ => None,
    };
    column()
        .align_items(Align::Center)
        .rounded(Radius::Control)
        .focus_mode(Focus::Keyboard)
        .role(Role::Slider)
        .name(format!("{name}, {value}"))
        .on_key(keys)
        .children((
            icon_button(set::CHEVRON_UP, format!("More {name}")).small().on_click(up),
            label(format!("{value:02}")).mono().weight(600),
            icon_button(set::CHEVRON_DOWN, format!("Fewer {name}")).small().on_click(down),
        ))
}

/// `hour` (0 to 23) and `minute`, each stepped up and down by its buttons or the arrow keys, minutes
/// by `minute_step`; both wrap round. `on_change` hears the new hour and minute.
///
/// ```
/// use vui_widgets::time_field;
///
/// #[derive(Clone)]
/// enum Msg { Alarm(u8, u8) }
/// let alarm = time_field(7, 30, 5, Msg::Alarm);
/// ```
pub fn time_field<M: Clone + 'static>(hour: u8, minute: u8, minute_step: u8, on_change: impl Fn(u8, u8) -> M + 'static) -> Div<M> {
    let on: Rc<dyn Fn(u8, u8) -> M> = Rc::new(on_change);
    let (h, m, step) = (hour % 24, minute % 60, minute_step.clamp(1, 30));
    let hours = part(h, "hours", on((h + 1) % 24, m), on((h + 23) % 24, m));
    let minutes = part(m, "minutes", on(h, (m + step) % 60), on(h, (m + 60 - step) % 60));
    row()
        .padding_x(6.0)
        .gap(2.0)
        .align_items(Align::Center)
        .bg(Tone::Field)
        .border(1.0, Tone::Line)
        .rounded(Radius::Control)
        .role(Role::Group)
        .name(format!("{h:02}:{m:02}"))
        .children((hours, label(":").weight(600).color(Tone::Text2), minutes))
}
