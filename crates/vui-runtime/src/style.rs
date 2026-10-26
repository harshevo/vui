//! Layout styling.
//!
//! Every element carries a [`Style`], and [`Styled`] gives elements and widget builders the same
//! chainable vocabulary: `column().gap(8.0).padding(16.0).width(px(240.0))` reads the same on a
//! container, a label or a button. The layout model is flexbox, computed by `layout`.

use vui_core::{Insets, Vec2};

/// A row of items runs along x, a column along y.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub fn cross(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

/// A length: logical pixels, a percentage of the parent, or whatever the content needs.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Len {
    Px(f32),
    /// 0 to 100.
    Pct(f32),
    Auto,
}

pub const fn px(v: f32) -> Len {
    Len::Px(v)
}

/// A percentage of the parent, 0 to 100.
pub const fn pct(v: f32) -> Len {
    Len::Pct(v)
}

pub const AUTO: Len = Len::Auto;

impl From<f32> for Len {
    fn from(v: f32) -> Len {
        Len::Px(v)
    }
}

impl From<i32> for Len {
    fn from(v: i32) -> Len {
        Len::Px(v as f32)
    }
}

impl From<u32> for Len {
    fn from(v: u32) -> Len {
        Len::Px(v as f32)
    }
}

impl Len {
    /// The length in pixels, given what a percentage is of. `None` for `Auto`, and for a percentage of
    /// something that is not decided yet.
    pub fn resolve(self, basis: Option<f32>) -> Option<f32> {
        match self {
            Len::Px(v) => Some(v),
            Len::Pct(p) => basis.map(|b| b * p / 100.0),
            Len::Auto => None,
        }
    }

    pub fn is_auto(self) -> bool {
        self == Len::Auto
    }
}

/// A value for each side of a box.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Sides<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T: Copy> Sides<T> {
    pub const fn all(v: T) -> Sides<T> {
        Sides { top: v, right: v, bottom: v, left: v }
    }

    pub const fn new(top: T, right: T, bottom: T, left: T) -> Sides<T> {
        Sides { top, right, bottom, left }
    }

    /// The side where `axis` begins: left for x, top for y.
    pub fn start(&self, axis: Axis) -> T {
        match axis {
            Axis::X => self.left,
            Axis::Y => self.top,
        }
    }

    /// The side where `axis` ends: right for x, bottom for y.
    pub fn end(&self, axis: Axis) -> T {
        match axis {
            Axis::X => self.right,
            Axis::Y => self.bottom,
        }
    }
}

impl Sides<f32> {
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }

    /// Both sides on `axis`, added.
    pub fn sum(&self, axis: Axis) -> f32 {
        self.start(axis) + self.end(axis)
    }
}

/// Pixel sides for padding and margin. A single number applies to all four, a pair is
/// (vertical, horizontal), and an array is [top, right, bottom, left].
pub type Edges = Sides<f32>;

impl From<f32> for Edges {
    fn from(v: f32) -> Edges {
        Sides::all(v)
    }
}

impl From<i32> for Edges {
    fn from(v: i32) -> Edges {
        Sides::all(v as f32)
    }
}

impl From<(f32, f32)> for Edges {
    fn from((v, h): (f32, f32)) -> Edges {
        Sides::new(v, h, v, h)
    }
}

impl From<[f32; 4]> for Edges {
    fn from([top, right, bottom, left]: [f32; 4]) -> Edges {
        Sides::new(top, right, bottom, left)
    }
}

impl From<Insets> for Edges {
    fn from(i: Insets) -> Edges {
        Sides::new(i.top, i.right, i.bottom, i.left)
    }
}

/// Which way children run.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    Row,
    /// The default: stacking is what most containers do.
    #[default]
    Column,
}

impl Direction {
    pub fn main_axis(self) -> Axis {
        match self {
            Direction::Row => Axis::X,
            Direction::Column => Axis::Y,
        }
    }
}

/// Where an element sits in its parent.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Position {
    /// Takes part in the parent's row or column.
    #[default]
    Flow,
    /// Leaves the flow and is placed against the parent's box with `top`, `left`, `right`, `bottom`.
    Absolute,
}

/// How children sit across the container's main axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
    /// Grows to fill the line. The default.
    Stretch,
    /// Lines the first baselines up. Only meaningful in a row.
    Baseline,
}

/// How children are spread along the container's main axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// What happens to content that does not fit.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Drawn anyway.
    #[default]
    Visible,
    /// Cut off at the box.
    Hidden,
    /// Cut off at the box and scrollable along the container's direction. Children are laid out at
    /// their natural size on that axis.
    Scroll,
}

/// Everything layout reads. All of it is plain data, so a style is cheap to copy and compare.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Style {
    pub direction: Direction,
    /// Takes no space, paints nothing and is skipped by input and accessibility.
    pub hidden: bool,
    pub position: Position,
    pub width: Len,
    pub height: Len,
    /// `Auto` lets a flex item shrink no further than its content; anywhere else it means 0.
    pub min_width: Len,
    pub min_height: Len,
    /// `Auto` means no limit.
    pub max_width: Len,
    pub max_height: Len,
    /// Width divided by height. Fills in whichever size is missing.
    pub aspect_ratio: Option<f32>,
    pub padding: Edges,
    pub margin: Sides<Len>,
    /// Distances from the parent's edges, for absolute children.
    pub inset: Sides<Len>,
    /// `x` separates items that sit side by side, `y` items that sit one above the other. A row's
    /// items are `x` apart and its wrapped lines `y` apart; a column's items are `y` apart.
    pub gap: Vec2,
    pub wrap: bool,
    pub align_items: Align,
    pub align_self: Option<Align>,
    pub justify: Justify,
    pub grow: f32,
    pub shrink: f32,
    pub basis: Len,
    pub overflow: Overflow,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            direction: Direction::Column,
            hidden: false,
            position: Position::Flow,
            width: Len::Auto,
            height: Len::Auto,
            min_width: Len::Auto,
            min_height: Len::Auto,
            max_width: Len::Auto,
            max_height: Len::Auto,
            aspect_ratio: None,
            padding: Sides::all(0.0),
            margin: Sides::all(Len::Px(0.0)),
            inset: Sides::all(Len::Auto),
            gap: Vec2::ZERO,
            wrap: false,
            align_items: Align::Stretch,
            align_self: None,
            justify: Justify::Start,
            grow: 0.0,
            shrink: 1.0,
            basis: Len::Auto,
            overflow: Overflow::Visible,
        }
    }
}

impl Style {
    /// The gap that separates items along `axis`.
    pub fn gap_along(&self, axis: Axis) -> f32 {
        match axis {
            Axis::X => self.gap.x,
            Axis::Y => self.gap.y,
        }
    }
}

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

    fn margin_x(mut self, v: f32) -> Self {
        let m = &mut self.style_mut().margin;
        m.left = Len::Px(v);
        m.right = Len::Px(v);
        self
    }

    fn margin_y(mut self, v: f32) -> Self {
        let m = &mut self.style_mut().margin;
        m.top = Len::Px(v);
        m.bottom = Len::Px(v);
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

    fn top(mut self, v: f32) -> Self {
        self.style_mut().inset.top = Len::Px(v);
        self
    }

    fn left(mut self, v: f32) -> Self {
        self.style_mut().inset.left = Len::Px(v);
        self
    }

    fn right(mut self, v: f32) -> Self {
        self.style_mut().inset.right = Len::Px(v);
        self
    }

    fn bottom(mut self, v: f32) -> Self {
        self.style_mut().inset.bottom = Len::Px(v);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    struct S(Style);

    impl Styled for S {
        fn style_mut(&mut self) -> &mut Style {
            &mut self.0
        }
    }

    fn s() -> S {
        S(Style::default())
    }

    #[test]
    fn defaults_are_a_stretching_column_that_shrinks() {
        let d = Style::default();
        assert_eq!(d.direction, Direction::Column);
        assert_eq!(d.align_items, Align::Stretch);
        assert_eq!((d.grow, d.shrink), (0.0, 1.0));
        assert!(d.width.is_auto() && d.min_width.is_auto() && d.basis.is_auto());
    }

    #[test]
    fn lengths_resolve_against_their_basis() {
        assert_eq!(px(12.0).resolve(None), Some(12.0));
        assert_eq!(pct(50.0).resolve(Some(200.0)), Some(100.0));
        assert_eq!(pct(50.0).resolve(None), None, "a percentage of an undecided size is auto");
        assert_eq!(AUTO.resolve(Some(200.0)), None);
        assert_eq!(Len::from(8), px(8.0));
    }

    #[test]
    fn size_setters_write_the_style() {
        let st = s().width(px(120.0)).height(pct(50.0)).min_width(0.0).max_height(300).0;
        assert_eq!(st.width, Len::Px(120.0));
        assert_eq!(st.height, Len::Pct(50.0));
        assert_eq!(st.min_width, Len::Px(0.0));
        assert_eq!(st.max_height, Len::Px(300.0));
        assert_eq!(s().square(24.0).0.height, Len::Px(24.0));
        let f = s().fill().0;
        assert_eq!((f.width, f.height), (Len::Pct(100.0), Len::Pct(100.0)));
    }

    #[test]
    fn padding_shorthands_follow_css_order() {
        let both = s().padding((8.0, 16.0)).0.padding;
        assert_eq!((both.top, both.right, both.bottom, both.left), (8.0, 16.0, 8.0, 16.0));
        let four = s().padding([1.0, 2.0, 3.0, 4.0]).0.padding;
        assert_eq!((four.top, four.right, four.bottom, four.left), (1.0, 2.0, 3.0, 4.0));
        let x = s().padding(4.0).padding_x(10.0).0.padding;
        assert_eq!((x.left, x.right, x.top), (10.0, 10.0, 4.0));
        assert_eq!(Edges::from(Insets::new(1.0, 2.0, 3.0, 4.0)).left, 4.0);
    }

    #[test]
    fn sides_know_their_axes() {
        let e = Edges::from([1.0, 2.0, 3.0, 4.0]);
        assert_eq!((e.start(Axis::X), e.end(Axis::X)), (4.0, 2.0));
        assert_eq!((e.start(Axis::Y), e.end(Axis::Y)), (1.0, 3.0));
        assert_eq!((e.sum(Axis::X), e.sum(Axis::Y)), (6.0, 4.0));
        assert_eq!((e.horizontal(), e.vertical()), (6.0, 4.0));
    }

    #[test]
    fn auto_margins_push_the_element() {
        let m = s().margin(4.0).margin_left_auto().0.margin;
        assert_eq!(m.left, Len::Auto);
        assert_eq!(m.right, Len::Px(4.0));
    }

    #[test]
    fn row_and_column_set_the_direction() {
        assert_eq!(s().column().row().0.direction, Direction::Row);
        assert_eq!(s().row().column().0.direction, Direction::Column);
        assert_eq!(Direction::Row.main_axis(), Axis::X);
        assert_eq!(Axis::X.cross(), Axis::Y);
    }

    #[test]
    fn flex_shares_space_from_a_zero_basis() {
        let st = s().flex(2.0).0;
        assert_eq!((st.grow, st.shrink, st.basis), (2.0, 1.0, Len::Px(0.0)));
    }

    #[test]
    fn inset_pins_every_edge_and_absolute_leaves_the_flow() {
        let st = s().absolute().inset(4.0).0;
        assert_eq!(st.position, Position::Absolute);
        assert_eq!(st.inset.left, Len::Px(4.0));
        assert_eq!(st.inset.bottom, Len::Px(4.0));
    }

    #[test]
    fn the_gap_is_looked_up_by_axis() {
        let st = s().gap(6.0).style(|s| s.gap.y = 10.0).0;
        assert_eq!((st.gap_along(Axis::X), st.gap_along(Axis::Y)), (6.0, 10.0));
    }

    #[test]
    fn centre_sets_both_alignments() {
        let st = s().center().0;
        assert_eq!((st.align_items, st.justify), (Align::Center, Justify::Center));
    }
}
