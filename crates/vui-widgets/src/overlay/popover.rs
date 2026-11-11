//! A card that floats next to the element that opened it.

use vui_core::{Shadow, Spring, Tone};
use vui_runtime::{Align, Builder, Div, Element, IntoChildren, IntoElement, Key, Parent, Radius, Styled, div, pct};

use super::menu::menu;

/// Space between the anchor and the card.
const GAP: f32 = 6.0;

/// Which side of its anchor a popover opens on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Side {
    /// Under it, left edges lined up. The default.
    #[default]
    Below,
    /// Under it, right edges lined up: for a button at the right of a toolbar.
    BelowEnd,
    Above,
    AboveEnd,
}

/// The floating card's look: the page colour, a hairline border, round corners and a deep shadow.
/// Popovers, menus and selects are drawn in one; use it for your own floating content too.
pub fn panel<M: 'static>() -> Div<M> {
    div()
        .bg(Tone::Bg)
        .border(1.0, Tone::Line)
        .rounded(Radius::Card)
        .shadow(Shadow::FLOATING)
        .padding(4.0)
        .transition(Spring::SNAPPY)
        .appear()
}

/// A card next to `anchor`, shown while `open`. A press outside it or Escape sends `on_close`; so does
/// pressing the anchor again, which is how a button that opens it also closes it.
///
/// ```
/// # use vui_runtime::{Builder, Parent, Styled, column, label};
/// # use vui_widgets::{button, panel, popover};
/// # #[derive(Clone)] enum Msg { Toggle, Close }
/// # let open = true;
/// let filter = popover(
///     button("Filter").on_click(Msg::Toggle),
///     open,
///     Msg::Close,
///     panel().padding(12.0).child(label("Only unread")),
/// );
/// ```
pub fn popover<M: Clone + 'static>(anchor: impl IntoElement<M>, open: bool, on_close: M, content: impl IntoElement<M>) -> Popover<M> {
    Popover { anchor: anchor.into_element(), content: content.into_element(), open, on_close, side: Side::Below, match_width: false, key: None }
}

/// A button-like `trigger` that opens a [`menu`] of `items` under it.
pub fn dropdown<M: Clone + 'static>(trigger: impl IntoElement<M>, open: bool, on_close: M, items: impl IntoChildren<M>) -> Popover<M> {
    popover(trigger, open, on_close, menu(items))
}

/// See [`popover`].
pub struct Popover<M: 'static> {
    anchor: Element<M>,
    content: Element<M>,
    open: bool,
    on_close: M,
    side: Side,
    match_width: bool,
    key: Option<Key>,
}

impl<M: Clone + 'static> Popover<M> {
    /// Keys the box that holds the anchor and the card, so tests and the app can find it.
    pub fn key(mut self, key: impl Into<Key>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Which side of the anchor it opens on.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Makes the card as wide as the anchor: a select under its field.
    pub fn match_width(mut self) -> Self {
        self.match_width = true;
        self
    }
}

impl<M: Clone + 'static> IntoElement<M> for Popover<M> {
    fn into_element(self) -> Element<M> {
        let Popover { anchor, content, open, on_close, side, match_width, key } = self;
        let card = open.then(|| {
            let layer = div::<M>().key("vui-popover").float().on_dismiss(on_close).autofocus();
            let layer = match side {
                Side::Below => layer.top(pct(100.0)).left(0.0).margin_y(GAP),
                Side::BelowEnd => layer.top(pct(100.0)).right(0.0).margin_y(GAP),
                Side::Above => layer.bottom(pct(100.0)).left(0.0).margin_y(GAP),
                Side::AboveEnd => layer.bottom(pct(100.0)).right(0.0).margin_y(GAP),
            };
            let layer = if match_width { layer.width(pct(100.0)) } else { layer };
            layer.child(content)
        });
        let holder = div().align_items(Align::Stretch).children((anchor, card));
        match key {
            Some(key) => holder.key(key).into_element(),
            None => holder.into_element(),
        }
    }
}
