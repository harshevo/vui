//! Grid tracks: the columns a grid container lays its children into.

/// The most tracks a grid can list one by one. `Columns::fill` has no limit.
pub const MAX_TRACKS: usize = 12;

/// One column of a grid.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Track {
    /// Exactly this many logical pixels.
    Px(f32),
    /// A share of the width left after the pixel and auto columns, by weight: two `Fr(1.0)` columns
    /// split it evenly, `Fr(2.0)` next to `Fr(1.0)` takes two thirds.
    Fr(f32),
    /// As wide as the widest thing in it.
    Auto,
}

/// A share of the leftover width; see [`Track::Fr`].
pub const fn fr(weight: f32) -> Track {
    Track::Fr(weight)
}

/// How a grid makes its columns.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Columns {
    /// These columns, at most [`MAX_TRACKS`] of them.
    Tracks([Track; MAX_TRACKS], u8),
    /// As many equal columns at least this wide as the width holds, and at least one: the grid of
    /// cards that reflows from four columns to one as the window narrows. CSS calls it
    /// `repeat(auto-fill, minmax(min, 1fr))`.
    Fill(f32),
}

impl Columns {
    /// `n` equal columns.
    pub fn equal(n: usize) -> Columns {
        Columns::tracks(std::iter::repeat_n(fr(1.0), n.clamp(1, MAX_TRACKS)))
    }

    /// Columns listed one by one; any past [`MAX_TRACKS`] are dropped.
    pub fn tracks(tracks: impl IntoIterator<Item = Track>) -> Columns {
        let mut list = [Track::Auto; MAX_TRACKS];
        let mut len = 0;
        for (slot, track) in list.iter_mut().zip(tracks) {
            *slot = track;
            len += 1;
        }
        if len == 0 {
            list[0] = fr(1.0);
            len = 1;
        }
        Columns::Tracks(list, len as u8)
    }

    /// Columns at least `min` wide, as many as fit; see [`Columns::Fill`].
    pub fn fill(min: f32) -> Columns {
        Columns::Fill(min.max(1.0))
    }
}

impl From<usize> for Columns {
    fn from(n: usize) -> Columns {
        Columns::equal(n)
    }
}

impl From<i32> for Columns {
    fn from(n: i32) -> Columns {
        Columns::equal(n.max(1) as usize)
    }
}

impl<const N: usize> From<[Track; N]> for Columns {
    fn from(tracks: [Track; N]) -> Columns {
        Columns::tracks(tracks)
    }
}

/// A grid container's layout: its columns, and how tall its rows are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Grid {
    pub columns: Columns,
    /// Every row this tall. `None` makes each row as tall as its tallest cell.
    pub row: Option<f32>,
}
