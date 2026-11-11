//! Elements above the page: fixed placement against the window, dismissal by an outside press or
//! Escape, focus that moves into what appears, and Tab kept inside a dialog.

use vui_core::{NamedKey, pt};
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Div, Overflow, Parent, Styled, column, div, label};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Close,
    CloseInner,
    Pressed,
}

fn button(key: &str) -> Div<Msg> {
    div().key(key.to_string()).size(60.0, 24.0).focusable(true).on_click(Msg::Pressed)
}

#[test]
fn a_fixed_element_is_placed_against_the_window_wherever_it_is_declared() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().padding(20.0).child(
        div().key("scroller").height(100.0).overflow(Overflow::Scroll).padding(30.0).children((
            div().height(400.0),
            div().key("corner").fixed().right(10.0).bottom(10.0).size(50.0, 20.0),
            div().key("centre").fixed().size(100.0, 60.0),
        )),
    ));
    assert_eq!(ui.rect("corner"), vui_core::rect(340.0, 270.0, 50.0, 20.0));
    assert_eq!(ui.rect("centre"), vui_core::rect(150.0, 120.0, 100.0, 60.0));
    // Scrolling what it sits in does not move it.
    ui.wheel(pt(100.0, 60.0), 0.0, -50.0);
    assert_eq!(ui.rect("corner"), vui_core::rect(340.0, 270.0, 50.0, 20.0));
}

#[test]
fn a_fixed_element_follows_the_window_size() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().child(div().key("toast").fixed().right(16.0).bottom(16.0).size(100.0, 40.0)));
    ui.ui.set_size(vui_core::size(600.0, 500.0));
    assert_eq!(ui.rect("toast"), vui_core::rect(484.0, 444.0, 100.0, 40.0));
}

#[test]
fn a_fixed_element_takes_no_room_in_the_flow() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().children((div().fixed().size(100.0, 100.0), div().key("next").height(20.0))));
    assert_eq!(ui.rect("next").y, 0.0);
}

#[test]
fn a_press_outside_dismisses_and_goes_no_further() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().children((
        button("under"),
        div().key("menu").float().top(40.0).left(0.0).size(120.0, 80.0).on_dismiss(Msg::Close).child(button("item")),
    )));
    ui.click_key("item");
    assert_eq!(ui.messages(), [Msg::Pressed], "a press inside does not dismiss");
    ui.click_key("under");
    assert_eq!(ui.messages(), [Msg::Close], "the button under the press is not pressed");
}

#[test]
fn escape_dismisses_the_topmost_first() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().child(
        div().key("outer").fixed().size(200.0, 200.0).on_dismiss(Msg::Close).child(
            div().key("inner").float().top(0.0).left(0.0).size(50.0, 50.0).on_dismiss(Msg::CloseInner),
        ),
    ));
    ui.key(NamedKey::Escape);
    assert_eq!(ui.messages(), [Msg::CloseInner]);
}

#[test]
fn escape_goes_to_the_focused_element_first() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().child(
        div().fixed().size(200.0, 200.0).on_dismiss(Msg::Close).child(
            button("field").on_key(|k| (k.key == vui_core::Key::Named(NamedKey::Escape)).then_some(Msg::Pressed)),
        ),
    ));
    ui.click_key("field");
    ui.messages();
    ui.key(NamedKey::Escape);
    assert_eq!(ui.messages(), [Msg::Pressed]);
}

#[test]
fn autofocus_takes_focus_when_the_element_appears() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().children((button("a"), None::<Div<Msg>>)));
    assert_eq!(ui.ui.focused_key(), None);
    ui.view(column().children((button("a"), Some(button("b").autofocus()))));
    assert_eq!(ui.ui.focused_key(), Some("b".into()));
    // Only on arrival: moving focus away keeps it away on the next view.
    ui.click_key("a");
    ui.view(column().children((button("a"), Some(button("b").autofocus()))));
    assert_eq!(ui.ui.focused_key(), Some("a".into()));
}

fn page(dialog: bool) -> Div<Msg> {
    column().children((
        button("page-1"),
        button("page-2"),
        dialog.then(|| div().fixed().size(200.0, 120.0).trap_focus().children((button("ok"), button("cancel")))),
    ))
}

#[test]
fn a_focus_trap_takes_focus_and_keeps_tab_inside() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(page(false));
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("page-1".into()));
    ui.view(page(true));
    assert_eq!(ui.ui.focused_key(), Some("ok".into()), "focus moves into the dialog");
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("cancel".into()));
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("ok".into()), "Tab wraps inside the dialog");
    ui.view(page(false));
    ui.tab();
    assert!(ui.ui.focused_key().is_some_and(|k| k == "page-1".into() || k == "page-2".into()));
}

#[test]
fn insets_take_percentages() {
    let mut ui = TestUi::<Msg>::new(400.0, 300.0);
    ui.view(column().child(
        div().key("anchor").size(100.0, 40.0).child(div().key("below").float().top(vui_runtime::pct(100.0)).left(0.0).child(label("Menu"))),
    ));
    let anchor = ui.rect("anchor");
    assert_eq!(ui.rect("below").y, anchor.y + anchor.h);
}
