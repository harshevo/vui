//! The chainable layout vocabulary: [`Styled`](super::Styled), shared by every element builder.

use super::*;

/// The chainable layout vocabulary. Implement `style_mut` and everything else follows.
pub trait Styled: Sized {
    fn style_mut(&mut self) -> &mut Style;

    /// Edits the style directly, for anything the chainable methods do not cover.
    fn style(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(self.style_mut());
        self
    }

    fn width(mut self, w: impl Into<Len>) -> Self {
        self.style_mut().width = w.into();
        self
    }

    fn height(mut self, h: impl Into<Len>) -> Self {
        self.style_mut().height = h.into();
        self
    }

    fn size(self, w: impl Into<Len>, h: impl Into<Len>) -> Self {
        self.width(w).height(h)
    }

    /// Width and height both `side` pixels.
    fn square(self, side: f32) -> Self {
        self.width(side).height(side)
    }

    fn min_width(mut self, w: impl Into<Len>) -> Self {
        self.style_mut().min_width = w.into();
        self
    }

    fn min_height(mut self, h: impl Into<Len>) -> Self {
        self.style_mut().min_height = h.into();
        self
    }

    fn max_width(mut self, w: impl Into<Len>) -> Self {
        self.style_mut().max_width = w.into();
        self
    }

    fn max_height(mut self, h: impl Into<Len>) -> Self {
        self.style_mut().max_height = h.into();
        self
    }

    fn fill_width(self) -> Self {
        self.width(pct(100.0))
    }

    fn fill_height(self) -> Self {
        self.height(pct(100.0))
    }

    /// Fills the parent in both directions.
    fn fill(self) -> Self {
        self.fill_width().fill_height()
    }

    fn padding(mut self, p: impl Into<Edges>) -> Self {
        self.style_mut().padding = p.into();
        self
    }

    fn padding_x(mut self, v: f32) -> Self {
        let p = &mut self.style_mut().padding;
        p.left = v;
        p.right = v;
        self
    }

    fn padding_y(mut self, v: f32) -> Self {
        let p = &mut self.style_mut().padding;
        p.top = v;
        p.bottom = v;
        self
    }

    fn margin(mut self, m: impl Into<Edges>) -> Self {
        let e = m.into();
        self.style_mut().margin = Sides::new(Len::Px(e.top), Len::Px(e.right), Len::Px(e.bottom), Len::Px(e.left));
        self
    }

    /// Left and right margins: pixels, or a percentage of the parent's width.
    fn margin_x(mut self, v: impl Into<Len>) -> Self {
        let v = v.into();
        let m = &mut self.style_mut().margin;
        m.left = v;
        m.right = v;
        self
    }

    /// Top and bottom margins: pixels, or a percentage of the parent's width (as in CSS).
    fn margin_y(mut self, v: impl Into<Len>) -> Self {
        let v = v.into();
        let m = &mut self.style_mut().margin;
        m.top = v;
        m.bottom = v;
        self
    }

    fn padding_top(mut self, v: f32) -> Self {
        self.style_mut().padding.top = v;
        self
    }

    fn margin_top(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().margin.top = v.into();
        self
    }

    fn padding_right(mut self, v: f32) -> Self {
        self.style_mut().padding.right = v;
        self
    }

    fn margin_right(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().margin.right = v.into();
        self
    }

    fn padding_bottom(mut self, v: f32) -> Self {
        self.style_mut().padding.bottom = v;
        self
    }

    fn margin_bottom(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().margin.bottom = v.into();
        self
    }

    fn padding_left(mut self, v: f32) -> Self {
        self.style_mut().padding.left = v;
        self
    }

    fn margin_left(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().margin.left = v.into();
        self
    }

    /// Soaks up the free space before the element, pushing it to the far end of a row.
    fn margin_left_auto(mut self) -> Self {
        self.style_mut().margin.left = Len::Auto;
        self
    }

    /// Soaks up the free space after the element, pushing what follows it to the far end of a row.
    fn margin_right_auto(mut self) -> Self {
        self.style_mut().margin.right = Len::Auto;
        self
    }

    /// Pushes the element to the bottom of a column.
    fn margin_top_auto(mut self) -> Self {
        self.style_mut().margin.top = Len::Auto;
        self
    }

    /// Space between children, on both axes.
    fn gap(mut self, g: f32) -> Self {
        self.style_mut().gap = Vec2 { x: g, y: g };
        self
    }

    /// Lays children out left to right.
    fn row(mut self) -> Self {
        self.style_mut().direction = Direction::Row;
        self
    }

    /// Lays children out top to bottom.
    fn column(mut self) -> Self {
        self.style_mut().direction = Direction::Column;
        self
    }

    /// Starts a new line when the children do not fit.
    fn wrap(mut self) -> Self {
        self.style_mut().wrap = true;
        self
    }

    /// Where children sit across the main axis.
    fn align_items(mut self, a: Align) -> Self {
        self.style_mut().align_items = a;
        self
    }

    /// Where this element sits across its parent's main axis, overriding the parent's `align_items`.
    fn align_self(mut self, a: Align) -> Self {
        self.style_mut().align_self = Some(a);
        self
    }

    fn justify(mut self, j: Justify) -> Self {
        self.style_mut().justify = j;
        self
    }

    /// Centres children both ways.
    fn center(self) -> Self {
        self.align_items(Align::Center).justify(Justify::Center)
    }

    /// Takes the space left over, shared with other growing siblings by weight.
    fn grow(mut self, weight: f32) -> Self {
        self.style_mut().grow = weight;
        self
    }

    /// 0 keeps the element at its natural size when the row or column is too small.
    fn shrink(mut self, weight: f32) -> Self {
        self.style_mut().shrink = weight;
        self
    }

    fn basis(mut self, b: impl Into<Len>) -> Self {
        self.style_mut().basis = b.into();
        self
    }

    /// Shares the room left in the row or column by `weight`, whatever the natural sizes are.
    fn flex(self, weight: f32) -> Self {
        self.grow(weight).shrink(1.0).basis(0.0)
    }

    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.style_mut().aspect_ratio = Some(ratio);
        self
    }

    /// Takes the element out of the flow and places it against its parent with `top`, `left`,
    /// `right` and `bottom`. With none of them set it sits where the parent's alignment puts it.
    fn absolute(mut self) -> Self {
        self.style_mut().position = Position::Absolute;
        self
    }

    fn top(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().inset.top = v.into();
        self
    }

    fn left(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().inset.left = v.into();
        self
    }

    fn right(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().inset.right = v.into();
        self
    }

    fn bottom(mut self, v: impl Into<Len>) -> Self {
        self.style_mut().inset.bottom = v.into();
        self
    }

    /// Like [`absolute`](Self::absolute), but above everything else in the window: drawn last, hit
    /// first, and not cut off by the scroll containers or clipping boxes it sits in. Place it with
    /// `top`, `left`, `right`, `bottom` against its parent, as a menu under the button that opened it.
    fn float(mut self) -> Self {
        self.style_mut().float = true;
        self.absolute()
    }

    /// Places the element against the window instead of its parent, above everything else, wherever
    /// it is declared: a dialog, a toast, a scrim behind a sheet. `top`, `left`, `right` and `bottom`
    /// are measured from the window's edges; with none set it is centred in the window.
    fn fixed(mut self) -> Self {
        self.style_mut().float = true;
        self.style_mut().position = Position::Fixed;
        self
    }

    /// Pins all four edges to the parent's, inset by `v`.
    fn inset(self, v: f32) -> Self {
        self.top(v).left(v).right(v).bottom(v)
    }

    /// Takes no space and draws nothing while `hidden` is true.
    fn hidden(mut self, hidden: bool) -> Self {
        self.style_mut().hidden = hidden;
        self
    }

    /// Cuts off or scrolls content that overflows the box. `Hidden` also lets a flex item shrink
    /// below its content.
    fn overflow(mut self, overflow: Overflow) -> Self {
        self.style_mut().overflow = overflow;
        self
    }

    /// For a scroll container: start at the end, and keep up with content that grows there while
    /// the reader is at the end. Scrolling back up lets go until they scroll down to the end again.
    fn follow_end(mut self) -> Self {
        self.style_mut().follow_end = true;
        self
    }

    /// For a scroll container: a scrollbar thumb `fraction` (0 to 1) of the track long, whatever
    /// the content's length, instead of one that shrinks and grows with it.
    fn fixed_thumb(mut self, fraction: f32) -> Self {
        self.style_mut().thumb = Thumb::Fixed(fraction.clamp(0.0, 1.0));
        self
    }
}
