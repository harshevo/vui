//! Drawers and sheets: a panel that slides in from an edge of the window over a dimmed page, for
//! navigation on a narrow window, filters, details and settings that do not need a whole page.

use vui_core::{Shadow, Spring, Tone};
use vui_runtime::{Align, Breakpoint, Builder, Div, IntoChildren, Justify, Parent, Radius, Role, Styled, column, div, pct};

/// The edge of the window a [`drawer`] comes from.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Edge {
    Left,
    /// The default: details and settings beside the page.
    #[default]
    Right,
    Top,
    Bottom,
}

/// How wide a side drawer is, and how tall a top or bottom one may grow.
const SIDE_WIDTH: f32 = 380.0;

/// A panel against `edge` holding `content`, shown while the app wants it. Pressing the dimmed page
/// or Escape sends `on_close`, and Tab stays inside it while it shows. On a phone-sized window a side
/// drawer takes the whole width.
///
/// ```
/// use vui_runtime::{Parent, title};
/// use vui_widgets::{Edge, drawer};
///
/// #[derive(Clone)]
/// enum Msg { Close }
/// let filters = drawer(Edge::Right, Msg::Close, title("Filters"));
/// ```
pub fn drawer<M: Clone + 'static>(edge: Edge, on_close: M, content: impl IntoChildren<M>) -> Div<M> {
    let panel = column::<M>()
        .padding(20.0)
        .gap(16.0)
        .bg(Tone::Bg)
        .shadow(Shadow::FLOATING)
        .role(Role::Dialog)
        .trap_focus()
        .on_dismiss(on_close)
        .overflow(vui_runtime::Overflow::Scroll)
        .transition(Spring::SNAPPY)
        .appear()
        .children(content);
    let panel = match edge {
        Edge::Left | Edge::Right => panel
            .width(SIDE_WIDTH)
            .max_width(pct(92.0))
            .fill_height()
            .when(Breakpoint::Compact, |s| s.width(pct(100.0)).max_width(pct(100.0))),
        Edge::Top | Edge::Bottom => panel.fill_width().max_height(pct(85.0)).rounded(Radius::Panel),
    };
    let layer = div()
        .key("vui-drawer")
        .fixed()
        .top(0.0)
        .left(0.0)
        .right(0.0)
        .bottom(0.0)
        .bg(Tone::Scrim)
        .transition(Spring::SMOOTH)
        .appear();
    let layer = match edge {
        Edge::Left => layer.row().justify(Justify::Start),
        Edge::Right => layer.row().justify(Justify::End),
        Edge::Top => layer.justify(Justify::Start).align_items(Align::Stretch),
        Edge::Bottom => layer.justify(Justify::End).align_items(Align::Stretch),
    };
    layer.child(panel)
}

/// A drawer from the bottom of the window, with a grabber at its top: actions and short forms on a
/// narrow window.
pub fn sheet<M: Clone + 'static>(on_close: M, content: impl IntoChildren<M>) -> Div<M> {
    let grabber = div().width(36.0).height(4.0).rounded(Radius::Pill).bg(Tone::Line).align_self(Align::Center).a11y_hidden(true);
    drawer(Edge::Bottom, on_close, (grabber, column().gap(16.0).children(content)))
}
