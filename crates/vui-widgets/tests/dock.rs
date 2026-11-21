//! Docking driven by the pointer: a card's title dragged onto another stack's edge splits it, a
//! click on the title of a card holding several opens the menu of them, a double click fills the
//! dock, and a title let go outside every zone is torn off.

use vui_core::Point;
use vui_runtime::testing::TestUi;
use vui_runtime::{App, Cx, IntoElement, label};
use vui_widgets::{DockEvent, DockMsg, DockNode, DockPanel, DockTree, Docking, dock};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Dock(DockMsg<&'static str>),
}

struct Desk {
    docking: Docking<&'static str>,
    torn: Vec<&'static str>,
}

impl App for Desk {
    type Msg = Msg;

    fn update(&mut self, Msg::Dock(m): Msg, _cx: &mut Cx<Msg>) {
        if let Some(DockEvent::TearOff(p)) = self.docking.update(m) {
            self.torn.push(p);
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        dock(&self.docking, |p, _| DockPanel::new(p.to_string(), label(format!("{p} here"))), Msg::Dock)
    }
}

fn desk() -> TestUi<Msg> {
    let tree = DockTree::new(DockNode::row(vec![DockNode::stack(vec!["watch", "news"]), DockNode::stack(vec!["chart"])], vec![300.0]));
    TestUi::with_app(900.0, 600.0, Desk { docking: Docking::new(tree), torn: Vec::new() })
}

fn docking(t: &TestUi<Msg>) -> &Docking<&'static str> {
    &t.app::<Desk>().docking
}

/// Holds the tab, moves past the drag threshold, then to `to`, and lets go there.
fn drag(t: &mut TestUi<Msg>, tab: &str, to: Point) {
    let from = t.center(tab);
    t.press(from);
    t.move_to(Point { x: from.x + 20.0, y: from.y + 10.0 });
    t.move_to(to);
    t.release(to);
}

#[test]
fn the_title_of_a_card_of_several_lists_them_and_a_pick_shows_it() {
    let mut t = desk();
    assert!(t.ui.rect_of("tab-news").is_none(), "no tabs: only the shown panel's title");
    t.click_key("tab-watch");
    assert_eq!(docking(&t).menu, Some("watch"));
    t.click_key("card-tab-news");
    assert_eq!(docking(&t).tree.shown(), vec![&"news", &"chart"]);
    assert_eq!(docking(&t).menu, None, "a pick closes the menu");
}

#[test]
fn a_double_click_on_a_title_fills_the_dock_and_another_brings_the_layout_back() {
    let mut t = desk();
    t.click_key("tab-chart");
    t.click_key("tab-chart");
    assert_eq!(docking(&t).zoomed(), Some(&"chart"));
    assert!(t.ui.rect_of("tab-watch").is_none(), "only the filling card is drawn");
    t.click_key("menu-chart");
    t.click_key("card-zoom-chart");
    assert_eq!(docking(&t).zoomed(), None);
    assert!(t.ui.rect_of("tab-watch").is_some());
}

#[test]
fn a_tab_dropped_on_a_stack_edge_splits_it() {
    let mut t = desk();
    let from = t.center("tab-watch");
    t.press(from);
    t.move_to(Point { x: from.x + 20.0, y: from.y + 10.0 });
    assert!(docking(&t).drag.is_some(), "moving past the threshold starts a drag");
    let bottom = t.center("zone-chart-bottom");
    t.move_to(bottom);
    assert_eq!(docking(&t).drag.as_ref().and_then(|d| d.over), Some(("chart", vui_widgets::Zone::Bottom)), "the zone under the pointer lit");
    t.release(bottom);
    assert!(docking(&t).drag.is_none());
    assert_eq!(docking(&t).tree.shown(), vec![&"news", &"chart", &"watch"]);
}

#[test]
fn a_tab_let_go_over_no_zone_is_torn_off() {
    let mut t = desk();
    let from = t.center("tab-chart");
    // Off the dock's zones: the tab bar's own strip at the far right is inside a zone too, so go
    // outside the window's content altogether.
    drag(&mut t, "tab-chart", Point { x: from.x, y: -40.0 });
    assert_eq!(t.app::<Desk>().torn, vec!["chart"]);
    assert!(docking(&t).tree.contains(&"chart"), "the app decides what a tear-off does");
}

#[test]
fn without_titles_a_grip_drags_the_card_and_the_last_pressed_card_is_kept() {
    let mut t = desk();
    t.update_app(|d: &mut Desk| d.docking.titles = false);
    t.settle();
    let words = vui_runtime::testing::texts(&t.scene());
    assert!(!words.iter().any(|w| w == "chart" || w == "watch"), "no card names its panel: {words:?}");
    assert!(t.rect("tab-chart").w > 0.0, "the grip is still there to drag");
    assert_eq!(docking(&t).used, None);
    let inside = t.center("card-chart");
    t.click(inside);
    assert_eq!(docking(&t).used, Some("chart"), "a press anywhere in a card makes it the last used");
    t.click_key("menu-watch");
    assert_eq!(docking(&t).used, Some("watch"));
    assert_eq!(docking(&t).menu, Some("watch"), "being used leaves the card's menu open");
    t.click_key("menu-watch");
    let watch = t.rect("card-watch");
    let before = docking(&t).tree.clone();
    drag(&mut t, "tab-chart", Point { x: watch.x + watch.w / 2.0, y: watch.y + watch.h * 0.9 });
    assert_ne!(docking(&t).tree, before, "the grip drags like a title");
    assert!(t.app::<Desk>().torn.is_empty(), "dropped on a zone, not torn off");
}

/// A desk whose watchlist is never drawn wider than 260 points, nor its book taller than 200.
struct Capped {
    docking: Docking<&'static str>,
}

impl App for Capped {
    type Msg = Msg;

    fn update(&mut self, Msg::Dock(m): Msg, _cx: &mut Cx<Msg>) {
        self.docking.update(m);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        dock(
            &self.docking,
            |p, _| {
                let panel = DockPanel::new(p.to_string(), label(format!("{p} here")));
                match *p {
                    "watch" => panel.max_width(260.0),
                    "book" => panel.max_height(200.0),
                    _ => panel,
                }
            },
            Msg::Dock,
        )
    }
}

#[test]
fn a_panel_with_a_most_is_drawn_no_bigger_and_its_edge_stops_there() {
    // The watchlist was saved at 300 points, wider than its most.
    let right = DockNode::column(vec![DockNode::stack(vec!["chart"]), DockNode::stack(vec!["book"])], vec![0.0, 320.0]);
    let tree = DockTree::new(DockNode::row(vec![DockNode::stack(vec!["watch"]), right], vec![300.0]));
    let mut t = TestUi::with_app(900.0, 600.0, Capped { docking: Docking::new(tree) });
    let watch = t.ui.rect_of("card-watch").unwrap();
    assert_eq!(watch.w, 260.0, "drawn at its most, not the 300 saved");
    assert_eq!(t.ui.rect_of("card-book").unwrap().h, 200.0, "the book too, down the column");
    let chart = t.ui.rect_of("card-chart").unwrap();
    assert!(chart.h > 300.0, "the chart takes what the book leaves");

    // Dragging the edge wider stops at the most; the size kept is the most, so dragging back
    // narrows it at once.
    let edge = Point { x: watch.x + watch.w + 3.0, y: watch.y + watch.h / 2.0 };
    t.press(edge);
    t.move_to(Point { x: edge.x + 120.0, y: edge.y });
    t.release(Point { x: edge.x + 120.0, y: edge.y });
    let DockNode::Split { sizes, .. } = &t.app::<Capped>().docking.tree.root else { panic!() };
    assert_eq!(sizes[0], 260.0);
    t.press(edge);
    t.move_to(Point { x: edge.x - 40.0, y: edge.y });
    t.release(Point { x: edge.x - 40.0, y: edge.y });
    assert_eq!(t.ui.rect_of("card-watch").unwrap().w, 220.0);
}
