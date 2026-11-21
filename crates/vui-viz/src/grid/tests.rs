use std::cell::Cell as Counter;
use std::rc::Rc;

use vui_core::{Command, NamedKey, Tone, pt};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Styled};

use super::*;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Sort(usize),
    Select(u64),
    Open(u64),
    Resize(usize, f32),
}

/// 2,300 companies in 12 columns, counting how many cells are read.
struct Market {
    reads: Counter<usize>,
    names: Vec<String>,
    prices: Vec<String>,
}

impl Market {
    fn new(rows: usize) -> Market {
        Market { reads: Counter::new(0), names: (0..rows).map(|i| format!("STOCK{i}")).collect(), prices: (0..rows).map(|i| format!("{}.50", 100 + i)).collect() }
    }
}

impl GridData for Market {
    fn rows(&self) -> usize {
        self.names.len()
    }

    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        self.reads.set(self.reads.get() + 1);
        match col {
            0 => Cell::Strong(&self.names[row]),
            1 => Cell::Text(&self.prices[row]),
            2 => Cell::Toned(if row.is_multiple_of(2) { "+1.2%" } else { "-0.8%" }, if row.is_multiple_of(2) { Tone::Success } else { Tone::Danger }),
            3 => Cell::Range { low: 0.0, high: 10.0, value: (row % 11) as f32 },
            4 => Cell::Bar { fraction: (row % 10) as f32 / 10.0, label: "42", tone: Tone::Accent },
            5 => Cell::Spark(&[1.0, 3.0, 2.0, 5.0, 4.0], Tone::Success),
            _ => Cell::Text("x"),
        }
    }

    fn key(&self, row: usize) -> u64 {
        1000 + row as u64
    }
}

fn columns() -> Vec<Column> {
    let mut v = vec![
        Column::new("Symbol", 120.0).sortable(),
        Column::new("Price", 90.0).numeric().sortable(),
        Column::new("1d", 80.0).numeric(),
        Column::new("52w", 120.0),
        Column::new("Volume", 110.0).numeric(),
        Column::new("1y", 100.0),
    ];
    v.extend((6..12).map(|i| Column::new(format!("Col {i}"), 100.0)));
    v
}

fn grid(rows: usize) -> (Rc<Market>, DataGrid<Msg>) {
    let data = Rc::new(Market::new(rows));
    let g = data_grid::<Msg>(data.clone(), columns()).pinned(1).sorted(1, Order::Descending).on_sort(Msg::Sort).on_select(Msg::Select).on_open(Msg::Open).on_resize(Msg::Resize);
    (data, g)
}

fn open(rows: usize) -> (Rc<Market>, TestUi<Msg>) {
    let (data, g) = grid(rows);
    let mut t = TestUi::new(640.0, 400.0);
    t.view(g.key("grid").fill());
    // Input is matched to what was painted, so a frame goes first, as it does in a running app.
    let _ = t.scene();
    (data, t)
}

fn text_at(t: &mut TestUi<Msg>, text: &str) -> Option<vui_core::Point> {
    t.scene().walk().find_map(|c| match c {
        Command::Text(x) if x.layout.text() == text => Some(x.origin),
        _ => None,
    })
}

#[test]
fn only_the_rows_in_view_are_read() {
    let (data, mut t) = open(2300);
    let _ = t.scene();
    let reads = data.reads.get();
    // 400 points shows (400 − 36) / 32 ≈ 12 rows; every column of those and no more.
    assert!(reads > 0 && reads <= 14 * 12 * 2, "{reads} cells read for a screenful of a 2,300-row table");
    let names = texts(&t.scene());
    assert!(names.contains(&"STOCK0".to_string()) && names.contains(&"STOCK10".to_string()));
    assert!(!names.contains(&"STOCK100".to_string()), "a row far below is never read");
}

#[test]
fn the_header_names_the_columns_and_marks_the_sort() {
    let (_, mut t) = open(50);
    let names = texts(&t.scene());
    for title in ["Symbol", "Price", "1d", "52w", "Volume"] {
        assert!(names.contains(&title.to_string()), "{title}");
    }
    let h = t.rect("grid");
    // Pressing the sortable "Price" title sends its column; "1d" is not sortable and sends nothing.
    t.click(pt(h.x + 120.0 + 60.0, h.y + 18.0));
    assert_eq!(t.messages(), vec![Msg::Sort(1)]);
    t.click(pt(h.x + 120.0 + 90.0 + 40.0, h.y + 18.0));
    assert!(t.messages().is_empty());
}

#[test]
fn clicking_a_row_selects_it_and_two_clicks_open_it() {
    let (_, mut t) = open(50);
    let h = t.rect("grid");
    let row3 = pt(h.x + 200.0, h.y + 36.0 + 3.0 * 32.0 + 16.0);
    t.click(row3);
    assert_eq!(t.messages(), vec![Msg::Select(1003)]);
    t.double_click(row3);
    let sent = t.messages();
    assert!(sent.contains(&Msg::Open(1003)), "{sent:?}");
}

#[test]
fn the_arrow_keys_walk_the_rows_and_return_opens() {
    let (data, g) = grid(50);
    let mut t = TestUi::new(640.0, 400.0);
    t.view(g.selected(Some(1004)).key("grid").fill());
    let _ = (data, t.scene());
    let h = t.rect("grid");
    t.click(pt(h.x + 200.0, h.y + 36.0 + 4.0 * 32.0 + 16.0));
    t.messages();
    t.key(NamedKey::ArrowDown);
    assert_eq!(t.messages(), vec![Msg::Select(1005)]);
    t.key(NamedKey::ArrowUp);
    assert_eq!(t.messages(), vec![Msg::Select(1003)]);
    t.key(NamedKey::End);
    assert_eq!(t.messages(), vec![Msg::Select(1049)]);
    t.key(NamedKey::Home);
    assert_eq!(t.messages(), vec![Msg::Select(1000)]);
    t.key(NamedKey::Enter);
    assert_eq!(t.messages(), vec![Msg::Open(1004)], "Return opens the chosen row");
}

#[test]
fn dragging_a_title_edge_sends_the_new_width() {
    let (_, mut t) = open(50);
    let h = t.rect("grid");
    // The right edge of "Symbol" is at x = 120.
    let edge = pt(h.x + 120.0, h.y + 18.0);
    t.press(edge).move_to(pt(edge.x + 30.0, edge.y)).release(pt(edge.x + 30.0, edge.y));
    let sent = t.messages();
    assert!(sent.iter().any(|m| matches!(m, Msg::Resize(0, w) if (*w - 150.0).abs() < 0.5)), "{sent:?}");
    // It stops at the column's least width.
    let edge = pt(h.x + 120.0, h.y + 18.0);
    t.press(edge).move_to(pt(edge.x - 500.0, edge.y)).release(pt(edge.x - 500.0, edge.y));
    assert!(t.messages().iter().all(|m| !matches!(m, Msg::Resize(0, w) if *w < 48.0)));
}

#[test]
fn the_header_and_the_first_column_stay_while_the_rest_scrolls() {
    let (_, mut t) = open(2300);
    let h = t.rect("grid");
    let before = text_at(&mut t, "Symbol").unwrap();
    t.wheel(pt(h.x + 300.0, h.y + 200.0), -400.0, -640.0);
    t.settle();
    let after = text_at(&mut t, "Symbol").unwrap();
    assert_eq!(before, after, "the title stays where it was");
    let names = texts(&t.scene());
    assert!(!names.contains(&"STOCK0".to_string()), "the top rows scrolled away");
    let first_name = names.iter().find(|n| n.starts_with("STOCK")).unwrap();
    let at = text_at(&mut t, first_name).unwrap();
    assert!((at.x - (h.x + 12.0)).abs() < 0.5, "the first column holds the left edge: {at:?}");
    // The columns to its right moved: "Col 6" is further left than where it began.
    let col6 = text_at(&mut t, "Col 6");
    assert!(col6.is_none_or(|p| p.x < h.x + 120.0 + 90.0 + 80.0 + 120.0 + 110.0 + 100.0 + 12.0 - 100.0));
}

#[test]
fn copy_puts_the_chosen_row_on_the_clipboard_as_tabbed_text() {
    let (_, g) = grid(10);
    let mut t = TestUi::new(640.0, 400.0);
    let clip = Rc::new(vui_runtime::MemoryClipboard::default());
    t.ui.set_clipboard(clip.clone());
    t.view(g.selected(Some(1002)).key("grid").fill());
    let _ = t.scene();
    let h = t.rect("grid");
    t.click(pt(h.x + 200.0, h.y + 36.0 + 2.0 * 32.0 + 16.0));
    t.shortcut('c');
    use vui_runtime::Clipboard;
    let copied = clip.get().unwrap_or_default();
    assert_eq!(copied.split('\t').take(3).collect::<Vec<_>>(), vec!["STOCK2", "102.50", "+1.2%"]);
}

struct Flashing;

impl GridData for Flashing {
    fn rows(&self) -> usize {
        3
    }

    fn cell(&self, _: usize, _: usize) -> Cell<'_> {
        Cell::Text("1,234.50")
    }

    fn flash(&self, row: usize, col: usize) -> Option<(f32, Tone)> {
        (row == 1 && col == 0).then_some((0.1, Tone::Success))
    }
}

#[test]
fn a_changed_cell_flashes_and_keeps_the_frames_coming() {
    let mut t = TestUi::<Msg>::new(400.0, 200.0);
    t.view(data_grid::<Msg>(Rc::new(Flashing), vec![Column::new("Price", 120.0).numeric()]).key("grid").fill());
    let scene = t.scene();
    let green = scene.walk().any(|c| matches!(c, Command::Quad(q) if q.fill.g > q.fill.r && q.fill.a > 0.02 && q.fill.a < 0.2));
    assert!(green, "a faint green wash sits behind the changed cell");
}
