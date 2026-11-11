//! Hover cards: a card of detail that shows while the pointer rests on something, such as a person's
//! profile over their name. Unlike a tooltip it can hold anything, and the pointer can move into it.

use vui_runtime::{Align, Builder, Div, Element, IntoElement, Parent, Styled, div, pct};

use super::popover::Side;

/// `anchor`, with `content` floating beside it while `open`. `on_hover` hears when the pointer comes
/// onto the anchor or the card (`true`) and leaves both (`false`); set `open` from it.
///
/// ```
/// use vui_runtime::{Parent, Styled, label};
/// use vui_widgets::{Side, hover_card, panel};
///
/// #[derive(Clone)]
/// enum Msg { Peek(bool) }
/// let name = hover_card(label("Ada Lovelace"), true, Msg::Peek, panel().padding(12.0).child(label("Mathematician")), Side::Below);
/// ```
pub fn hover_card<M: 'static>(anchor: impl IntoElement<M>, open: bool, on_hover: impl Fn(bool) -> M + 'static, content: impl IntoElement<M>, side: Side) -> Div<M> {
    let card: Option<Element<M>> = open.then(|| {
        // The card touches the anchor (its spacing is padding), so the pointer can cross into it.
        let layer = div::<M>().key("vui-hover-card").float();
        let layer = match side {
            Side::Below => layer.top(pct(100.0)).left(0.0).padding_top(6.0),
            Side::BelowEnd => layer.top(pct(100.0)).right(0.0).padding_top(6.0),
            Side::Above => layer.bottom(pct(100.0)).left(0.0).padding_bottom(6.0),
            Side::AboveEnd => layer.bottom(pct(100.0)).right(0.0).padding_bottom(6.0),
        };
        layer.child(content).into_element()
    });
    div().align_items(Align::Start).on_hover(on_hover).children((anchor.into_element(), card))
}
