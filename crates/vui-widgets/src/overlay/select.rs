//! A select: a field showing the current choice, which opens a menu of the others.

use std::rc::Rc;

use vui_core::{NamedKey, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Focus, Parent, Radius, Role, Styled, label, row};

use super::menu::menu_check;
use super::popover::{Popover, popover};
use crate::icon::icon;
use super::menu::menu;

/// One choice out of a list, in a field that opens a menu under itself. `on_open` hears whether the
/// menu should be open (the field was pressed, Down was pressed on it, or it should close); `on_pick`
/// hears the choice. Close the menu when handling `on_pick`.
///
/// ```
/// # use vui_widgets::select;
/// # #[derive(Clone)] enum Msg { Open(bool), Size(u32) }
/// let size = select([(12, "Small".to_string()), (14, "Medium".to_string())], &14, false, Msg::Open, Msg::Size);
/// ```
pub fn select<M: Clone + 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    open: bool,
    on_open: impl Fn(bool) -> M + 'static,
    on_pick: impl Fn(T) -> M + 'static,
) -> Popover<M> {
    let on_open = Rc::new(on_open);
    let options: Vec<(T, String)> = options.into_iter().collect();
    let current = options.iter().find(|(v, _)| v == selected).map(|(_, l)| l.clone()).unwrap_or_default();
    let toggle = on_open(!open);
    let down = {
        let on_open = on_open.clone();
        move |k: &vui_core::KeyEvent| {
            (k.pressed && !open && k.key == vui_core::Key::Named(NamedKey::ArrowDown)).then(|| on_open(true))
        }
    };
    let field = row::<M>()
        .height(32.0)
        .padding_x(10.0)
        .gap(8.0)
        .align_items(Align::Center)
        .bg(Tone::Field)
        .border(1.0, Tone::Line)
        .rounded(Radius::Control)
        .hover_bg(Tone::Hover)
        .transition(Spring::SNAPPY)
        .focus_mode(Focus::Keyboard)
        .role(Role::Button)
        .name(current.clone())
        .cursor(vui_core::CursorIcon::Pointer)
        .on_click(toggle)
        .on_key(down)
        .children((label(current).grow(1.0), icon(set::CHEVRONS_UP_DOWN).color(Tone::Text3)));
    let on_pick = Rc::new(on_pick);
    let items: Vec<_> = options
        .into_iter()
        .map(|(value, text)| {
            let chosen = value == *selected;
            let on_pick = on_pick.clone();
            menu_check(text, chosen).on_click_with(move || on_pick(value.clone()))
        })
        .collect();
    popover(field, open, on_open(false), menu(items)).match_width()
}
