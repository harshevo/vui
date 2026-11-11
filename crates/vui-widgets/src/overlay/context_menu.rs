//! Context menus: a menu where the secondary click landed.

use vui_core::Point;
use vui_runtime::{Builder, Div, IntoChildren, IntoElement, Parent, Styled, div};

use super::menu::menu;

/// `target` opens a menu of `items` at the pointer on a secondary click (`on_open` gets where, in
/// window coordinates; keep it and pass it back as `at`). A press outside or Escape sends `on_close`.
/// The menu is kept inside the window.
pub fn context_menu<M: Clone + 'static>(
    target: impl IntoElement<M>,
    at: Option<Point>,
    on_open: impl Fn(Point) -> M + 'static,
    on_close: M,
    items: impl IntoChildren<M>,
) -> Div<M> {
    let layer = at.map(|p| {
        div().key("vui-context-menu").fixed().left(p.x).top(p.y).on_dismiss(on_close).autofocus().child(menu(items))
    });
    div().on_secondary(move |click| on_open(click.pos)).children((target, layer))
}
