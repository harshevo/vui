//! Layout styling.
//!
//! Every element carries a [`Style`], and [`Styled`] gives elements and widget builders the same
//! chainable vocabulary: `column().gap(8.0).padding(16.0).width(px(240.0))` reads the same on a
//! container, a label or a button. The layout model is flexbox, computed by `layout`.

use vui_core::{Insets, Vec2};

mod grid;
mod styled;

pub use grid::{Columns, Grid, MAX_TRACKS, Track, fr};
pub use styled::Styled;

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
    /// Leaves the flow and is placed against the window, above everything: dialogs, toasts, scrims.
    /// With no insets it is centred.
    Fixed,
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

/// How long a scroll container's scrollbar thumb is.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum Thumb {
    /// As much of the track as the view is of the content: long for a little overflow, short for a
    /// lot. The default.
    #[default]
    Proportional,
    /// This fraction of the track (0 to 1), however long the content is: a small marker that does
    /// not change size as content grows.
    Fixed(f32),
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
    /// A scroll container that stays at its end while it is there, as a log or a chat does.
    pub follow_end: bool,
    /// How long the scrollbar thumb is.
    pub thumb: Thumb,
    /// Drawn and clicked above everything else in the window, and not cut off by scrolling or
    /// clipping ancestors: menus and popovers.
    pub float: bool,
    /// Lays the children out in a grid of rows and columns instead of a row or column; `direction`
    /// and `wrap` are then ignored. See [`Grid`].
    pub grid: Option<Grid>,
    /// How many grid columns the element takes inside a grid. 1 by default.
    pub span: u16,
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
            follow_end: false,
            thumb: Thumb::Proportional,
            float: false,
            grid: None,
            span: 1,
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

#[cfg(test)]
mod tests;
