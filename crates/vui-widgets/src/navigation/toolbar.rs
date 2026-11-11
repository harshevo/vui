//! Toolbars: a row of tools that Tab treats as one stop and the arrow keys move along.

use vui_core::Tone;
use vui_runtime::{Align, Builder, Div, FocusGroup, IntoChildren, Parent, Role, Styled, div, row};

/// `tools` in a row: buttons, toggles, selects, with [`toolbar_separator`]s between groups. Tab
/// enters and leaves it as one stop; Left and Right move between the tools.
///
/// ```
/// use vui_icons::set;
/// use vui_runtime::Builder;
/// use vui_widgets::{icon_button, toolbar, toolbar_separator};
///
/// #[derive(Clone)]
/// enum Msg { Bold, Italic, Link }
/// let bar = toolbar((
///     icon_button(set::BOLD, "Bold").on_click(Msg::Bold),
///     icon_button(set::ITALIC, "Italic").on_click(Msg::Italic),
///     toolbar_separator(),
///     icon_button(set::LINK, "Link").on_click(Msg::Link),
/// ));
/// ```
pub fn toolbar<M: 'static>(tools: impl IntoChildren<M>) -> Div<M> {
    row().gap(2.0).padding(4.0).align_items(Align::Center).role(Role::Toolbar).focus_group(FocusGroup::horizontal()).children(tools)
}

/// A short upright line between groups of tools.
pub fn toolbar_separator<M: 'static>() -> Div<M> {
    div().width(1.0).height(20.0).margin_x(4.0).bg(Tone::Line).role(Role::Separator)
}
