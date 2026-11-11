//! The pie and the bar list in a running UI: drawing in, hover, and what they read out.

use std::rc::Rc;

use vui_core::pt;
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Role, Styled, column};
use vui_viz::{BarList, Pie, bar_list, pie};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Slice(Option<usize>),
    Row(Option<usize>),
}

struct Page {
    seen: Vec<Msg>,
}

impl App for Page {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        self.seen.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let share = Pie::new().slice("Direct", 50.0).slice("Search", 30.0).slice("Social", 20.0).donut().caption("Visits");
        let list = BarList::new().row("Chrome", 64.0).row("Safari", 25.0);
        column().padding(16.0).gap(16.0).children((
            pie(Rc::new(share)).key("p").width(400.0).on_hover(Msg::Slice),
            bar_list(Rc::new(list)).key("b").width(300.0).on_hover(Msg::Row),
        ))
    }
}

fn page() -> TestUi<Msg> {
    TestUi::with_app(480.0, 600.0, Page { seen: Vec::new() })
}

#[test]
fn it_draws_in_then_holds_still_and_shows_the_total() {
    let mut t = page();
    assert!(t.ui.needs_frame(), "the slices sweep round when it first shows");
    t.settle();
    assert!(!t.ui.needs_frame());
    let words = texts(&t.scene());
    assert!(words.iter().any(|w| w == "100") && words.iter().any(|w| w == "Visits"), "{words:?}");
    assert!(words.iter().any(|w| w == "50%"), "the legend gives shares");
}

#[test]
fn hovering_a_slice_names_it_in_the_hole_and_tells_the_app() {
    let mut t = page();
    t.settle();
    let r = t.rect("p");
    // The circle is on the left with the legend beside it; just right of the top is the first slice.
    let side = r.h;
    let at = pt(r.x + side / 2.0 + 20.0, r.y + 20.0);
    t.move_to(at).settle();
    assert_eq!(t.app::<Page>().seen, vec![Msg::Slice(Some(0))]);
    assert!(texts(&t.scene()).iter().any(|w| w == "Direct · 50%"));
    t.leave().settle();
    assert_eq!(t.app::<Page>().seen.last(), Some(&Msg::Slice(None)));
}

#[test]
fn a_bar_list_is_a_row_per_item_and_hover_tells_the_app() {
    let mut t = page();
    t.settle();
    let r = t.rect("b");
    assert_eq!(r.h, 60.0, "two rows of 30");
    t.move_to(pt(r.x + 10.0, r.y + 45.0)).settle();
    assert_eq!(t.app::<Page>().seen.last(), Some(&Msg::Row(Some(1))));
}

#[test]
fn both_read_as_images_with_their_values() {
    let mut t = page();
    let tree = t.ui.access_tree();
    let pie = tree.nodes.iter().find(|n| n.name.as_deref().is_some_and(|l| l.starts_with("Pie chart"))).expect("the pie");
    assert_eq!(pie.role, Role::Image);
    assert!(pie.name.as_deref().unwrap().contains("Direct 50%"));
    assert!(tree.nodes.iter().any(|n| n.name.as_deref() == Some("Bar chart: Chrome 64, Safari 25")));
}
