use vui_core::{Mode, NamedKey, Theme, pt};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{AccessAction, App, Builder, Cx, IntoElement, Parent, Styled, column, div, label, row};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Add,
    Show,
    Dark,
    FocusField,
    Quit,
    Hover(bool),
}

#[derive(Default)]
struct Counter {
    n: u32,
    dark: bool,
    shown: bool,
    hovers: Vec<bool>,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx) {
        match msg {
            Msg::Add => self.n += 1,
            Msg::Show => self.shown = true,
            Msg::Dark => self.dark = !self.dark,
            Msg::FocusField => cx.focus("field"),
            Msg::Quit => cx.quit(),
            Msg::Hover(on) => self.hovers.push(on),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().children((
            row().gap(8.0).children((
                div().key("add").width(60.0).height(30.0).focusable(true).on_click(Msg::Add).child(label("Add")),
                div().key("show").width(60.0).height(30.0).focusable(true).on_click(Msg::Show),
                div().key("dark").width(60.0).height(30.0).focusable(true).on_click(Msg::Dark),
                div().key("focus").width(60.0).height(30.0).focusable(true).on_click(Msg::FocusField),
                div().key("quit").width(60.0).height(30.0).focusable(true).on_click(Msg::Quit),
            )),
            label(self.n.to_string()),
            div().key("field").height(30.0).focusable(true),
            // Under the pointer once the app has shown it.
            div().key("tip").height(30.0).hidden(!self.shown).on_hover(Msg::Hover),
        ))
    }

    fn theme(&self, _system: Mode) -> Theme {
        if self.dark { Theme::dark() } else { Theme::light() }
    }
}

fn counter() -> TestUi<Msg> {
    TestUi::with_app(400.0, 200.0, Counter::default())
}

#[test]
fn the_first_view_is_there_before_any_event() {
    let mut t = counter();
    assert!(texts(&t.scene()).contains(&"Add".to_string()));
    assert_eq!(t.app::<Counter>().n, 0);
}

#[test]
fn a_click_reaches_update_and_the_view_follows() {
    let mut t = counter();
    t.click_key("add").click_key("add");
    assert_eq!(t.app::<Counter>().n, 2);
    assert!(texts(&t.scene()).contains(&"2".to_string()), "the label shows the new count");
}

#[test]
fn messages_do_not_pile_up_for_the_host() {
    let mut t = counter();
    t.click_key("add");
    assert!(t.messages().is_empty(), "the app already took them");
}

#[test]
fn the_app_chooses_the_theme() {
    let mut t = counter();
    assert_eq!(t.ui.theme().mode, Mode::Light);
    t.click_key("dark");
    assert_eq!(t.ui.theme().mode, Mode::Dark);
    t.click_key("dark");
    assert_eq!(t.ui.theme().mode, Mode::Light);
}

#[test]
fn update_can_ask_for_focus_and_the_ring_follows_the_input() {
    let mut t = counter();
    t.click_key("focus");
    assert_eq!(t.ui.focused_key(), Some("field".into()));
    assert!(!t.inspect("field").focus_visible, "the mouse asked, so no ring");

    let mut t = counter();
    t.tab().tab().tab().tab();
    assert_eq!(t.ui.focused_key(), Some("focus".into()));
    t.key(NamedKey::Enter);
    assert_eq!(t.ui.focused_key(), Some("field".into()));
    assert!(t.inspect("field").focus_visible, "the keyboard asked, so the ring shows");
}

#[test]
fn focusing_a_key_that_does_not_exist_or_cannot_take_focus_does_nothing() {
    let mut t = counter();
    assert!(!t.ui.focus_key("nope"));
    t.view(div().key("plain").height(10.0));
    assert!(!t.ui.focus_key("plain"));
    assert_eq!(t.ui.focused_key(), None);
}

#[test]
fn update_can_ask_to_quit() {
    let mut t = counter();
    assert!(!t.ui.quit_requested());
    t.click_key("quit");
    assert!(t.ui.quit_requested());
}

#[test]
fn hover_changes_caused_by_a_new_view_reach_the_app_in_the_same_call() {
    let mut t = counter();
    // The pointer waits where the tip will appear.
    let tip = pt(100.0, 30.0 + 30.0 + 30.0 + 15.0);
    t.move_to(tip);
    assert!(t.app::<Counter>().hovers.is_empty(), "nothing there yet");
    t.tab().tab().key(NamedKey::Enter);
    assert!(t.app::<Counter>().shown);
    assert_eq!(t.app::<Counter>().hovers, [true], "the new element came up under the pointer");
}

#[test]
fn the_host_can_change_the_app_from_outside() {
    let mut t = counter();
    t.update_app(|c: &mut Counter| c.dark = true);
    assert_eq!(t.ui.theme().mode, Mode::Dark);
    t.update_app(|c: &mut Counter| c.n = 7);
    assert!(texts(&t.scene()).contains(&"7".to_string()));
}

#[test]
fn assistive_technology_reaches_the_app_too() {
    let mut t = counter();
    let tree = t.ui.access_tree();
    let add = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Add")).expect("the add button is named after its label").id;
    t.ui.do_action(add, AccessAction::Click);
    assert_eq!(t.app::<Counter>().n, 1);
}

#[test]
fn asking_for_the_wrong_app_type_is_none() {
    struct Other;
    impl App for Other {
        type Msg = Msg;
        fn update(&mut self, _: Msg, _: &mut Cx) {}
        fn view(&self) -> impl IntoElement<Msg> {
            div()
        }
    }
    let t = counter();
    assert!(t.ui.app::<Other>().is_none());
    assert!(t.ui.app::<Counter>().is_some());
}

struct Follower;

impl App for Follower {
    type Msg = Msg;

    fn update(&mut self, _msg: Msg, _cx: &mut Cx) {}

    fn view(&self) -> impl IntoElement<Msg> {
        column().child(label("hi"))
    }
}

#[test]
fn by_default_an_app_follows_the_system_mode() {
    let mut t = TestUi::with_app(100.0, 100.0, Follower);
    assert_eq!(t.ui.theme().mode, Mode::Light);
    t.ui.set_system_mode(Mode::Dark);
    assert_eq!(t.ui.theme().mode, Mode::Dark);
    assert_eq!(t.ui.theme().palette.bg, Theme::dark().palette.bg);
    assert!(t.ui.needs_paint());
}

#[test]
fn an_app_with_its_own_theme_ignores_the_system() {
    let mut t = counter();
    t.ui.set_system_mode(Mode::Dark);
    assert_eq!(t.ui.theme().mode, Mode::Light);
}

#[test]
fn the_window_drags_from_the_canvas_but_not_from_controls() {
    let mut t = TestUi::<Msg>::new(300.0, 100.0);
    t.view(
        row().window_drag(true).height(100.0).children((
            div().key("button").width(60.0).height(30.0).on_click(Msg::Add),
            div().key("field").width(60.0).height(30.0).focusable(true),
            div().key("plain").width(60.0).height(30.0),
        )),
    );
    let (button, field, plain) = (t.center("button"), t.center("field"), t.center("plain"));
    assert!(!t.ui.drags_window(button));
    assert!(!t.ui.drags_window(field));
    assert!(t.ui.drags_window(plain));
    assert!(t.ui.drags_window(pt(250.0, 80.0)), "empty canvas");

    let mut t = TestUi::<Msg>::new(300.0, 100.0);
    t.view(row().height(100.0).child(div().width(60.0).height(30.0)));
    assert!(!t.ui.drags_window(pt(10.0, 10.0)), "nothing asked for it");
}

/// Takes clicks while it shows something to click.
struct Gate;

impl App for Gate {
    type Msg = bool;

    fn update(&mut self, through: bool, cx: &mut Cx) {
        cx.pass_clicks(through);
        cx.take_keys(!through);
    }

    fn view(&self) -> impl IntoElement<bool> {
        div()
    }
}

#[test]
fn update_can_ask_for_clicks_to_pass_through_or_not() {
    let mut t = TestUi { ui: vui_runtime::Ui::with_app(vui_core::Resources::bundled_only(), vui_core::size(100.0, 100.0), 1.0, Gate) };
    assert_eq!(t.ui.passes_clicks(), None, "the window's own setting until the app says");
    t.ui.send(false);
    assert_eq!(t.ui.passes_clicks(), Some(false));
    t.ui.send(true);
    t.settle();
    assert_eq!(t.ui.passes_clicks(), Some(true));
}

#[test]
fn update_can_ask_for_the_keyboard_and_give_it_back() {
    let mut t = TestUi { ui: vui_runtime::Ui::with_app(vui_core::Resources::bundled_only(), vui_core::size(100.0, 100.0), 1.0, Gate) };
    assert_eq!(t.ui.takes_keys(), None);
    t.ui.send(false);
    assert_eq!(t.ui.takes_keys(), Some(true));
    t.ui.send(true);
    t.settle();
    assert_eq!(t.ui.takes_keys(), Some(false));
}
