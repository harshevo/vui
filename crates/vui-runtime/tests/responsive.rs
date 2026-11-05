mod common;

use common::*;
use vui_core::{rect, size};
use vui_runtime::{Breakpoint, Builder, Parent, Styled, Width, column, div, pct};

fn page() -> vui_runtime::Div<Msg> {
    column().key("root").child(
        column()
            .key("main")
            .padding(10.0)
            .when(Breakpoint::Wide, |s| s.margin_x(pct(15.0)).padding(20.0))
            .child(div().key("line").height(20.0)),
    )
}

#[test]
fn a_rule_applies_only_inside_its_widths() {
    let mut t = ui(1000.0, 300.0);
    t.view(page());
    assert_eq!(t.rect("main"), rect(150.0, 0.0, 700.0, 60.0), "15% of 1000 on each side");
    assert_eq!(t.rect("line"), rect(170.0, 20.0, 660.0, 20.0), "the wide padding too");

    let mut t = ui(800.0, 300.0);
    t.view(page());
    assert_eq!(t.rect("main"), rect(0.0, 0.0, 800.0, 40.0), "a medium window keeps the element's own style");
    assert_eq!(t.rect("line"), rect(10.0, 10.0, 780.0, 20.0));
}

#[test]
fn resizing_the_window_restyles_without_a_new_view() {
    let mut t = ui(800.0, 300.0);
    t.view(page());
    assert_eq!(t.rect("main").x, 0.0);

    t.ui.set_size(size(1200.0, 300.0));
    assert_eq!(t.rect("main"), rect(180.0, 0.0, 840.0, 60.0));

    t.ui.set_size(size(1000.0, 300.0));
    assert_eq!(t.rect("main"), rect(150.0, 0.0, 700.0, 60.0), "percentages follow the window inside a breakpoint");

    t.ui.set_size(size(899.0, 300.0));
    assert_eq!(t.rect("main"), rect(0.0, 0.0, 899.0, 40.0), "and the rule lets go below it");
    assert_eq!(t.rect("line"), rect(10.0, 10.0, 879.0, 20.0));
}

#[test]
fn a_rebuilt_view_keeps_its_rules() {
    let mut t = ui(1000.0, 300.0);
    t.view(page());
    t.view(page());
    assert_eq!(t.rect("main").x, 150.0);
}

#[test]
fn rules_can_rearrange_and_hide() {
    let view = || {
        column().child(
            column()
                .key("pair")
                .when(Breakpoint::Medium.up(), |s| s.row())
                .children((div().key("a").size(100.0, 40.0), div().key("b").size(100.0, 40.0).when(Width::below(400.0), |s| s.hidden(true)))),
        )
    };
    let mut t = ui(700.0, 60.0);
    t.view(view());
    assert_eq!(t.rect("b"), rect(100.0, 0.0, 100.0, 40.0), "side by side on a medium window");

    t.ui.set_size(size(500.0, 300.0));
    assert_eq!(t.rect("b"), rect(0.0, 40.0, 100.0, 40.0), "stacked on a compact one");

    t.ui.set_size(size(300.0, 300.0));
    assert_eq!(t.rect("b").size(), size(0.0, 0.0), "hidden on a very narrow one");
}
