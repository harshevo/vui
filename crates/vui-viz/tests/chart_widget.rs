//! The chart widget in a running UI: the draw-in, hover, linked guides, dragging and the keys.

use std::rc::Rc;

use vui_core::pt;
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Builder, Cx, IntoElement, Parent, Role, Styled, column};
use vui_viz::chart::chart;
use vui_viz::{Chart, Format};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Hover(Option<f64>),
}

struct Page {
    data: Rc<Chart>,
    seen: Vec<Option<f64>>,
    guide: Option<f64>,
}

impl App for Page {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        let Msg::Hover(x) = msg;
        self.seen.push(x);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().padding(16.0).child(
            chart(self.data.clone())
                .key("c")
                .width(500.0)
                .height(280.0)
                .name("Price")
                .on_hover(Msg::Hover)
                .guide(self.guide),
        )
    }
}

fn page() -> TestUi<Msg> {
    let pts: Vec<(f64, f64)> = (0..100)
        .map(|i| (f64::from(i), 100.0 + f64::from(i)))
        .collect();
    let data = Rc::new(Chart::new().line("Close", &pts).y_format(Format::Fixed(0)));
    TestUi::with_app(
        540.0,
        320.0,
        Page {
            data,
            seen: Vec::new(),
            guide: None,
        },
    )
}

fn probe_texts(t: &mut TestUi<Msg>) -> Vec<String> {
    let scene = t.scene();
    texts(&scene)
}

#[test]
fn it_draws_in_and_then_holds_still() {
    let mut t = page();
    let first = t.scene().commands.len();
    t.settle();
    let settled = t.scene().commands.len();
    assert!(settled >= first, "the marks are all there at the end");
    assert!(!probe_texts(&mut t).is_empty(), "axis labels are drawn");
}

#[test]
fn hovering_shows_the_values_and_tells_the_app_where() {
    let mut t = page();
    t.settle();
    let r = t.rect("c");
    assert!(
        !probe_texts(&mut t).iter().any(|s| s == "Close"),
        "no tooltip until the pointer is on the plot"
    );
    t.move_to(pt(r.x + r.w * 0.5, r.y + r.h * 0.4));
    let shown = probe_texts(&mut t);
    assert!(shown.iter().any(|s| s == "Close"), "{shown:?}");
    let seen = t.app::<Page>().seen.clone();
    let x = seen.last().copied().flatten().expect("the app heard an x");
    assert!(x > 35.0 && x < 65.0, "about the middle of 0..99: {x}");
    t.leave();
    assert_eq!(t.app::<Page>().seen.last(), Some(&None));
    assert!(!probe_texts(&mut t).iter().any(|s| s == "Close"));
}

#[test]
fn a_guide_from_elsewhere_shows_the_probe_without_the_pointer() {
    let mut t = page();
    t.settle();
    t.update_app::<Page, _>(|p| p.guide = Some(70.0));
    let shown = probe_texts(&mut t);
    assert!(
        shown.iter().any(|s| s == "Close") && shown.iter().any(|s| s == "170"),
        "{shown:?}"
    );
}

#[test]
fn dragging_pans_a_zoomed_chart_and_a_double_click_shows_everything() {
    let mut t = page();
    t.settle();
    let r = t.rect("c");
    let mid = pt(r.x + r.w * 0.5, r.y + r.h * 0.5);
    let head = |t: &mut TestUi<Msg>| t.app::<Page>().seen.last().copied().flatten().unwrap();
    t.tab().type_text("++++");
    t.settle();
    t.move_to(mid);
    let before = head(&mut t);
    t.press(mid)
        .move_to(pt(mid.x + 120.0, mid.y))
        .release(pt(mid.x + 120.0, mid.y));
    t.move_to(mid);
    let after = head(&mut t);
    assert!(
        after < before - 3.0,
        "dragging right pulls earlier data under the pointer: {before} -> {after}"
    );
    t.double_click(mid);
    t.move_to(pt(mid.x + 1.0, mid.y));
    let full = head(&mut t);
    assert!((full - 49.5).abs() < 6.0, "back to the whole range: {full}");
}

#[test]
fn it_reads_as_an_image_named_for_what_it_shows() {
    let mut t = page();
    let tree = t.ui.access_tree();
    let n = tree.find("Price").expect("named").clone();
    assert_eq!(n.role, Role::Image);
    assert!(n.focusable);
}
