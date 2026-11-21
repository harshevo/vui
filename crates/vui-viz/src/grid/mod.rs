//! A grid for thousands of rows: it reads its cells from a shared snapshot as it paints, so only the
//! rows in view cost anything, whatever the size of the table.
//!
//! The grid is one widget inside a scrolling box. It is as large as all its rows and columns, and
//! paints only what the box shows, so scrolling (wheel, trackpad, scrollbars) is the runtime's own.
//! The header and the first columns stay in view by being painted at the edge of what is showing.
//!
//! Cells are plain text, a number with a tone, a sparkline, a range bar (a value between a low and a
//! high, like a 52-week range) or an inline bar. The app owns the data, the sort and the selection:
//! the grid reports clicks, key presses and resized columns as messages.

mod widget;

#[cfg(test)]
mod tests;

use std::rc::Rc;

use vui_core::Tone;

pub use widget::{DataGrid, data_grid};

/// How a column's cells and title line up.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Align {
    Start,
    /// For numbers: the digits line up on their right edge.
    End,
    Center,
}

/// Which way a column is sorted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Order {
    Ascending,
    Descending,
}

/// One column: its title, how wide, how it lines up, and whether pressing its title sorts.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub title: String,
    pub width: f32,
    pub min_width: f32,
    pub align: Align,
    pub sortable: bool,
    /// A share of the room the table's columns leave over when it is wider than they are; 0 keeps the
    /// column at its width.
    pub grow: f32,
}

impl Column {
    pub fn new(title: impl Into<String>, width: f32) -> Column {
        Column { title: title.into(), width, min_width: 48.0, align: Align::Start, sortable: false, grow: 0.0 }
    }

    /// Lines the title and cells up at the right: numbers, dates.
    pub fn numeric(mut self) -> Column {
        self.align = Align::End;
        self
    }

    pub fn centered(mut self) -> Column {
        self.align = Align::Center;
        self
    }

    pub fn sortable(mut self) -> Column {
        self.sortable = true;
        self
    }

    /// The narrowest a drag can make the column.
    pub fn min_width(mut self, width: f32) -> Column {
        self.min_width = width;
        self
    }

    pub fn grow(mut self, weight: f32) -> Column {
        self.grow = weight;
        self
    }
}

/// What a cell shows. Borrowed from the snapshot, so reading a cell allocates nothing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Cell<'a> {
    /// A dash: nothing is known.
    Empty,
    Text(&'a str),
    /// Text in a tone: green and red for up and down, grey for the faint.
    Toned(&'a str, Tone),
    /// The row's name: a little heavier.
    Strong(&'a str),
    /// A line over time with a dot on the last value, in a tone.
    Spark(&'a [f32], Tone),
    /// Where `value` sits between `low` and `high`: a range bar with a marker.
    Range {
        low: f32,
        high: f32,
        value: f32,
    },
    /// A bar filled to `fraction` (0 to 1), with its number beside it.
    Bar {
        fraction: f32,
        label: &'a str,
        tone: Tone,
    },
}

impl Cell<'_> {
    /// The cell as text for the clipboard; pictures have none.
    pub fn text(&self) -> &str {
        match self {
            Cell::Text(t) | Cell::Toned(t, _) | Cell::Strong(t) => t,
            Cell::Bar { label, .. } => label,
            _ => "",
        }
    }
}

/// The table's contents, read by the grid as it paints.
pub trait GridData {
    fn rows(&self) -> usize;

    fn cell(&self, row: usize, col: usize) -> Cell<'_>;

    /// A number that names the row and stays with it through a re-sort, so the selection follows the
    /// company rather than the place in the list. The index by default.
    fn key(&self, row: usize) -> u64 {
        row as u64
    }

    /// Seconds since the cell's value changed, and the tone to flash, while it should still flash.
    fn flash(&self, row: usize, col: usize) -> Option<(f32, Tone)> {
        let _ = (row, col);
        None
    }

    /// The row as tab-separated text, for Copy.
    fn copy_row(&self, row: usize, columns: usize) -> String {
        (0..columns).map(|c| self.cell(row, c).text().to_string()).collect::<Vec<_>>().join("\t")
    }
}

impl GridData for Vec<Vec<String>> {
    fn rows(&self) -> usize {
        self.len()
    }

    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        self.get(row).and_then(|r| r.get(col)).map(|t| Cell::Text(t)).unwrap_or(Cell::Empty)
    }
}

/// A snapshot of text rows, for small tables and tests.
pub fn rows_of(rows: Vec<Vec<String>>) -> Rc<dyn GridData> {
    Rc::new(rows)
}
