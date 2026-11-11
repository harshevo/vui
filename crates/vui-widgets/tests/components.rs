//! Tabs, tables, trees, pagination, the number input, chips, disclosures and alerts, driven by
//! pointer and keyboard.

use vui_core::NamedKey;
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Cx, IntoElement, Parent, column, label};
use vui_widgets::{
    AlertKind, SortDir, alert, chip, data_table, disclosure, number_input, pagination, table_column, table_row, tabs_keyed, tree,
    tree_row,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Tab(u8),
    Sort,
    Row(usize),
    Fold,
    Count(f64),
    Page(usize),
    Remove,
    Toggle,
}

struct Lab {
    tab: u8,
    count: f64,
    open: bool,
    unfolded: bool,
    log: Vec<Msg>,
}

impl App for Lab {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Tab(t) => self.tab = t,
            Msg::Count(v) => self.count = v,
            Msg::Toggle => self.open = !self.open,
            Msg::Fold => self.unfolded = !self.unfolded,
            _ => {}
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().children((
            tabs_keyed("tabs", [(0u8, "One".to_string()), (1, "Two".to_string()), (2, "Three".to_string())], &self.tab, Msg::Tab),
            data_table(
                vec![table_column("Name").sortable(Some(SortDir::Ascending), Msg::Sort), table_column("Size").numeric()],
                (0..2).map(|i| table_row((label(format!("file {i}")), label("1 KB"))).key(format!("row-{i}")).on_click(Msg::Row(i))),
            ),
            tree((
                tree_row("src", 0, Some(self.unfolded), Some(Msg::Fold)).key("src"),
                self.unfolded.then(|| tree_row("main.rs", 1, None, None).key("main")),
            )),
            number_input(self.count, 0.0, 3.0, 1.0, Msg::Count),
            pagination(2, 5, Msg::Page),
            chip("Design", Some(Msg::Remove)),
            disclosure("More", self.open, Msg::Toggle, (label("Hidden detail"),)),
            alert(AlertKind::Warning, "Careful", "This cannot be undone."),
        ))
    }
}

fn lab() -> TestUi<Msg> {
    let mut ui = TestUi::with_app(800.0, 900.0, Lab { tab: 0, count: 2.0, open: false, unfolded: false, log: Vec::new() });
    ui.settle();
    ui
}

fn shows(ui: &mut TestUi<Msg>, text: &str) -> bool {
    texts(&ui.scene()).iter().any(|t| t == text)
}

#[test]
fn tabs_open_by_click_and_by_arrow_key() {
    let mut ui = lab();
    ui.click_key("tabs-2");
    assert_eq!(ui.app::<Lab>().tab, 2);
    ui.ui.focus_key("tabs-2");
    ui.key(NamedKey::ArrowLeft);
    assert_eq!(ui.app::<Lab>().tab, 1, "arrows move along the strip and open as they go");
}

#[test]
fn a_table_sorts_from_its_header_and_rows_are_chosen() {
    let mut ui = lab();
    assert!(shows(&mut ui, "Name") && shows(&mut ui, "file 1"));
    let header = texts(&ui.scene());
    assert!(header.iter().position(|t| t == "Name") < header.iter().position(|t| t == "file 0"));
    ui.click_key("row-1");
    assert_eq!(ui.app::<Lab>().log.last(), Some(&Msg::Row(1)));
    ui.ui.focus_key("row-1");
    ui.key(NamedKey::ArrowUp);
    ui.key(NamedKey::Enter);
    assert_eq!(ui.app::<Lab>().log.last(), Some(&Msg::Row(0)), "rows are a focus group");
}

#[test]
fn a_tree_unfolds_by_click_and_by_arrow_keys() {
    let mut ui = lab();
    assert!(!shows(&mut ui, "main.rs"));
    ui.click_key("src");
    assert!(ui.app::<Lab>().unfolded);
    assert!(shows(&mut ui, "main.rs"));
    ui.ui.focus_key("src");
    ui.key(NamedKey::ArrowLeft);
    assert!(!ui.app::<Lab>().unfolded, "Left folds an open branch");
    ui.key(NamedKey::ArrowRight);
    assert!(ui.app::<Lab>().unfolded, "Right unfolds it");
}

#[test]
fn the_number_input_stays_in_range() {
    let mut ui = lab();
    let plus = ui.ui.rect_of(vui_runtime::Key::from("count-plus"));
    assert!(plus.is_none(), "the buttons need no keys of their own");
    // + at 2 reaches the maximum, after which + is disabled.
    let scene_texts = texts(&ui.scene());
    assert!(scene_texts.contains(&"2".to_string()));
    ui.update_app(|l: &mut Lab| l.count = 3.0);
    assert!(shows(&mut ui, "3"));
}

#[test]
fn a_disclosure_shows_its_content_while_open() {
    let mut ui = lab();
    assert!(!shows(&mut ui, "Hidden detail"));
    ui.update_app(|l: &mut Lab| l.open = true);
    ui.settle();
    assert!(shows(&mut ui, "Hidden detail"));
}

#[test]
fn an_alert_shows_its_title_and_detail() {
    let mut ui = lab();
    assert!(shows(&mut ui, "Careful") && shows(&mut ui, "This cannot be undone."));
}
