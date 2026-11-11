//! Data tables: a header of column titles, sortable, over rows that line up under them.

use std::rc::Rc;

use vui_core::{CursorIcon, Fill, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Breakpoint, Builder, Div, Element, Focus, FocusGroup, IntoChildren, IntoElement, Justify, Parent, Role, Styled, column, div, row, small};

use crate::icon::icon;

/// Which way a column is sorted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SortDir {
    Ascending,
    Descending,
}

/// How a column takes its width.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ColWidth {
    /// This many points.
    Fixed(f32),
    /// A share of the room the fixed columns leave, by weight.
    Flex(f32),
}

/// One column: its title, width, alignment, and whether and how it sorts.
pub struct TableColumn<M: 'static> {
    title: String,
    width: ColWidth,
    align: Justify,
    sorted: Option<SortDir>,
    on_sort: Option<Rc<dyn Fn() -> M>>,
    hide_below: Option<Breakpoint>,
}

/// A column titled `title`, sharing the free width equally with the other flexible ones.
pub fn table_column<M: 'static>(title: impl Into<String>) -> TableColumn<M> {
    TableColumn { title: title.into(), width: ColWidth::Flex(1.0), align: Justify::Start, sorted: None, on_sort: None, hide_below: None }
}

impl<M: 'static> TableColumn<M> {
    pub fn width(mut self, width: f32) -> Self {
        self.width = ColWidth::Fixed(width);
        self
    }

    pub fn flex(mut self, weight: f32) -> Self {
        self.width = ColWidth::Flex(weight);
        self
    }

    /// Lines the title and cells up at the right: numbers, sizes, dates.
    pub fn numeric(mut self) -> Self {
        self.align = Justify::End;
        self
    }

    /// Leaves the column out on windows narrower than `bp`, so the columns that matter keep their
    /// room on a small window: `.hide_below(Breakpoint::Medium)` for details a phone-sized window can
    /// do without.
    pub fn hide_below(mut self, bp: Breakpoint) -> Self {
        self.hide_below = Some(bp);
        self
    }

    /// Pressing the title sends `msg`; `sorted` shows the arrow when the table is sorted by it.
    pub fn sortable(mut self, sorted: Option<SortDir>, msg: M) -> Self
    where
        M: Clone,
    {
        self.sorted = sorted;
        self.on_sort = Some(Rc::new(move || msg.clone()));
        self
    }
}

/// How a column's cells are sized, lined up and hidden.
type Spec = (ColWidth, Justify, Option<Breakpoint>);

/// Space between columns, and padding at each end of a row.
const GAP: f32 = 16.0;
const ROW_HEIGHT: f32 = 40.0;

fn sized<M: 'static>(cell: Div<M>, (width, align, hide): Spec) -> Div<M> {
    let cell = cell.justify(align).align_items(Align::Center).min_width(0.0);
    let cell = match hide {
        Some(bp) => cell.when(bp.down(), |s| s.hidden(true)),
        None => cell,
    };
    match width {
        ColWidth::Fixed(w) => cell.width(w).shrink(0.0),
        ColWidth::Flex(f) => cell.flex(f),
    }
}

/// A table of `rows` under a header for `columns`. Each row is a [`table_row`] with one cell per
/// column. Rows are a focus group: the arrow keys move between them.
pub fn data_table<M: 'static>(columns: Vec<TableColumn<M>>, rows: impl IntoIterator<Item = TableRow<M>>) -> Div<M> {
    let specs: Vec<Spec> = columns.iter().map(|c| (c.width, c.align, c.hide_below)).collect();
    let header = row().height(36.0).padding_x(GAP / 2.0).gap(GAP).border(0.0, Fill::NONE).children(
        columns
            .into_iter()
            .map(|c| {
                // A sorted column shows its direction; a sortable one a faint hint that it can be.
                let arrow = match (c.sorted, c.on_sort.is_some()) {
                    (Some(SortDir::Ascending), _) => Some(icon(set::ARROW_UP).square(14.0).color(Tone::Text2)),
                    (Some(SortDir::Descending), _) => Some(icon(set::ARROW_DOWN).square(14.0).color(Tone::Text2)),
                    (None, true) => Some(icon(set::ARROW_UP_DOWN).square(13.0).color(Tone::Text3)),
                    (None, false) => None,
                };
                let title = row().gap(4.0).align_items(Align::Center).children((small(c.title).weight(500).color(Tone::Text2), arrow));
                let cell = sized(div().row(), (c.width, c.align, c.hide_below)).child(title);
                match c.on_sort {
                    Some(f) => cell.cursor(CursorIcon::Pointer).focusable(true).role(Role::Button).on_click_with(move || f()),
                    None => cell,
                }
            })
            .collect::<Vec<_>>(),
    );
    let body = column().role(Role::List).focus_group(FocusGroup::vertical()).children(
        rows.into_iter()
            .map(|r| {
                let cells: Vec<Element<M>> = r
                    .cells
                    .into_iter()
                    .zip(specs.iter().copied().chain(std::iter::repeat((ColWidth::Flex(1.0), Justify::Start, None))))
                    .map(|(cell, spec)| sized(div().row(), spec).child(cell).into_element())
                    .collect();
                r.row.children(cells)
            })
            .collect::<Vec<_>>(),
    );
    column().role(Role::Group).children((header, div().height(1.0).bg(Tone::Line), body))
}

/// One row of a [`data_table`]; build it with [`table_row`].
pub struct TableRow<M: 'static> {
    row: Div<M>,
    cells: Vec<Element<M>>,
}

/// A row with `cells`, one per column, in order: text with `label`, or anything else.
pub fn table_row<M: 'static>(cells: impl IntoChildren<M>) -> TableRow<M> {
    let mut out = Vec::new();
    cells.append_to(&mut out);
    let row = row()
        .min_height(ROW_HEIGHT)
        .padding_x(GAP / 2.0)
        .gap(GAP)
        .align_items(Align::Center)
        .border(0.0, Fill::NONE)
        .rounded(vui_runtime::Radius::Px(8.0))
        .hover_bg(Tone::Hover)
        .transition(Spring::SNAPPY)
        .role(Role::ListItem);
    TableRow { row, cells: out }
}

impl<M: 'static> TableRow<M> {
    /// Shows the row as chosen.
    pub fn selected(mut self, selected: bool) -> Self {
        if selected {
            self.row = self.row.bg(Fill::Tone(Tone::Accent, 0.10)).hover_bg(Fill::NONE);
        }
        self
    }

    /// Pressing the row, or Return while it has focus, sends `msg`.
    pub fn on_click(mut self, msg: M) -> Self
    where
        M: Clone,
    {
        self.row = self.row.focus_mode(Focus::Keyboard).cursor(CursorIcon::Pointer).on_click(msg);
        self
    }

    pub fn key(mut self, key: impl Into<vui_runtime::Key>) -> Self {
        self.row = self.row.key(key);
        self
    }

    /// What assistive technology reads for the row: usually its first cell's text.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.row = self.row.name(name);
        self
    }
}
