//! Docking driven by the pointer: a tab dragged onto another stack's edge splits it, a click only
//! switches tabs, and a tab let go outside every zone is torn off.

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
fn a_click_on_a_tab_shows_it() {
    let mut t = desk();
    t.click_key("tab-news");
    assert_eq!(docking(&t).tree.shown(), vec![&"news", &"chart"]);
}

#[test]
fn a_tab_dropped_on_a_stack_edge_splits_it() {
    let mut t = desk();
    let from = t.center("tab-news");
    t.press(from);
    t.move_to(Point { x: from.x + 20.0, y: from.y + 10.0 });
    assert!(docking(&t).drag.is_some(), "moving past the threshold starts a drag");
    let bottom = t.center("zone-chart-bottom");
    t.move_to(bottom);
    assert_eq!(docking(&t).drag.as_ref().and_then(|d| d.over), Some(("chart", vui_widgets::Zone::Bottom)), "the zone under the pointer lit");
    t.release(bottom);
    assert!(docking(&t).drag.is_none());
    assert_eq!(docking(&t).tree.shown(), vec![&"watch", &"chart", &"news"]);
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
