//! Carousels: one slide at a time out of several, with arrows, dots and the arrow keys to move.

use std::rc::Rc;

use vui_core::{Fill, Key, KeyEvent, NamedKey, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Element, IntoChildren, Justify, Parent, Radius, Role, Styled, column, div, pct, row};

use crate::buttons::button::icon_button;

/// `slides`, showing the one at `index`, with arrows at its sides and a dot for each slide under it.
/// `on_change` hears the slide asked for; Left and Right do the same while it has focus, and the ends
/// wrap round.
///
/// ```
/// use vui_runtime::label;
/// use vui_widgets::carousel;
///
/// #[derive(Clone)]
/// enum Msg { Slide(usize) }
/// let tour = carousel(0, (label("Welcome"), label("Your inbox"), label("You're set")), Msg::Slide);
/// ```
pub fn carousel<M: Clone + 'static>(index: usize, slides: impl IntoChildren<M>, on_change: impl Fn(usize) -> M + 'static) -> Div<M> {
    let mut all = Vec::new();
    slides.append_to(&mut all);
    let n = all.len().max(1);
    let index = index.min(n - 1);
    let on_change: Rc<dyn Fn(usize) -> M> = Rc::new(on_change);
    let (prev, next) = ((index + n - 1) % n, (index + 1) % n);
    let shown: Option<Element<M>> = (index < all.len()).then(|| all.swap_remove(index));

    let keys = {
        let on_change = on_change.clone();
        move |k: &KeyEvent| match k.key {
            Key::Named(NamedKey::ArrowLeft) if k.pressed => Some(on_change(prev)),
            Key::Named(NamedKey::ArrowRight) if k.pressed => Some(on_change(next)),
            _ => None,
        }
    };
    let arrow = |icon, name: &str, to: usize, left: bool| {
        let button = icon_button(icon, name).on_click(on_change(to)).bg(Tone::Bg).shadow(vui_core::Shadow::RAISED).absolute().top(pct(50.0)).margin_top(-16.0);
        if left { button.left(8.0) } else { button.right(8.0) }
    };
    let dots: Vec<Div<M>> = (0..n)
        .map(|i| {
            let on = i == index;
            div()
                .height(8.0)
                .width(if on { 20.0 } else { 8.0 })
                .rounded(Radius::Pill)
                .bg(if on { Fill::from(Tone::Accent) } else { Fill::Tone(Tone::Text, 0.18) })
                .transition(Spring::SNAPPY)
                .cursor(vui_core::CursorIcon::Pointer)
                .role(Role::Tab)
                .name(format!("Slide {} of {n}", i + 1))
                .on_click(on_change(i))
        })
        .collect();
    let stage = div()
        .key("vui-carousel-stage")
        .overflow(vui_runtime::Overflow::Hidden)
        .rounded(Radius::Card)
        .focusable(true)
        .on_key(keys)
        .role(Role::Group)
        .name(format!("Slide {} of {n}", index + 1))
        .child(div().key(index).fill_width().transition(Spring::SMOOTH).appear().children(shown))
        .children((n > 1).then(|| arrow(set::CHEVRON_LEFT, "Previous slide", prev, true)))
        .children((n > 1).then(|| arrow(set::CHEVRON_RIGHT, "Next slide", next, false)));
    column().gap(12.0).align_items(Align::Stretch).children((stage, row().gap(6.0).justify(Justify::Center).role(Role::TabList).children(dots)))
}
