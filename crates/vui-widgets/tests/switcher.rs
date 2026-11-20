//! The switcher: one word for what is showing, a menu of the rest, a flip for two, steps for an order.

use vui_core::NamedKey;
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Cx, IntoElement, Parent, Styled, column};
use vui_widgets::switcher;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open(bool),
    View(u8),
    Side(bool),
    Day(u8),
    NamedOpen(bool),
    Named(u8),
}

#[derive(Default)]
struct Views {
    view: u8,
    open: bool,
    buy: bool,
    day: u8,
    named: u8,
    named_open: bool,
}

impl App for Views {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Open(open) => self.open = open,
            Msg::View(v) => {
                self.view = v;
                self.open = false;
            }
            Msg::Side(buy) => self.buy = buy,
            Msg::Day(d) => self.day = d,
            Msg::NamedOpen(open) => self.named_open = open,
            Msg::Named(n) => {
                self.named = n;
                self.named_open = false;
            }
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let views = [(0, "Positions".to_string()), (1, "Changes".to_string()), (2, "Trend".to_string())];
        let sides = [(true, "Buy".to_string()), (false, "Sell".to_string())];
        let days = [(0, "18 Nov".to_string()), (1, "19 Nov".to_string()), (2, "20 Nov".to_string())];
        column().padding(16.0).gap(8.0).children((
            switcher(views, &self.view, self.open, Msg::Open, Msg::View).key("views").name("View"),
            switcher([(0, "Long".to_string()), (1, "Short".to_string())], &self.named, self.named_open, Msg::NamedOpen, Msg::Named).key("named").ids(["long", "short"]).menu(),
            switcher(sides, &self.buy, false, Msg::Open, Msg::Side).key("side"),
            switcher(days, &self.day, false, Msg::Open, Msg::Day).key("day").steps(),
        ))
    }
}

fn app() -> TestUi<Msg> {
    let mut ui = TestUi::with_app(400.0, 300.0, Views { buy: true, ..Views::default() });
    ui.settle();
    ui
}

fn shows(ui: &mut TestUi<Msg>, text: &str) -> bool {
    texts(&ui.scene()).iter().any(|t| t == text)
}

#[test]
fn only_the_chosen_word_shows_until_the_menu_opens() {
    let mut ui = app();
    assert!(shows(&mut ui, "Positions"));
    assert!(!shows(&mut ui, "Changes"), "the other views stay out of sight");
    ui.click_key("views").settle();
    assert!(ui.app::<Views>().open);
    assert!(shows(&mut ui, "Trend"));
    let (word, item) = (ui.rect("views"), ui.rect("views-2"));
    assert!(item.y >= word.y + word.h, "the menu opens under the word");
    ui.click_key("views-2").settle();
    assert_eq!(ui.app::<Views>().view, 2);
    assert!(!ui.app::<Views>().open);
    assert!(!shows(&mut ui, "Positions"));
}

#[test]
fn two_choices_flip_on_a_click_without_a_menu() {
    let mut ui = app();
    assert!(shows(&mut ui, "Buy"));
    assert!(!shows(&mut ui, "Sell"));
    ui.click_key("side").settle();
    assert!(!ui.app::<Views>().buy);
    assert!(!ui.app::<Views>().open, "no menu for two");
    ui.click_key("side").settle();
    assert!(ui.app::<Views>().buy);
}

#[test]
fn steps_walk_an_order_and_stop_at_its_ends() {
    let mut ui = app();
    ui.click_key("day-back").settle();
    assert_eq!(ui.app::<Views>().day, 0, "nothing before the first");
    ui.click_key("day-on").settle();
    ui.click_key("day-on").settle();
    assert_eq!(ui.app::<Views>().day, 2);
    ui.click_key("day-on").settle();
    assert_eq!(ui.app::<Views>().day, 2, "nothing after the last");
}

#[test]
fn brackets_and_arrows_step_and_down_opens_the_menu() {
    let mut ui = app();
    ui.tab();
    assert_eq!(ui.ui.focused_key(), Some("views".into()));
    ui.type_text("]");
    assert_eq!(ui.app::<Views>().view, 1);
    ui.settle().key(NamedKey::ArrowRight);
    assert_eq!(ui.app::<Views>().view, 2);
    ui.settle().type_text("]");
    assert_eq!(ui.app::<Views>().view, 0, "views go round");
    ui.settle().type_text("[");
    assert_eq!(ui.app::<Views>().view, 2);
    ui.settle().key(NamedKey::ArrowDown);
    assert!(ui.app::<Views>().open);
}

#[test]
fn a_step_past_either_end_is_greyed_out() {
    let mut ui = app();
    assert!(ui.inspect("day-back").disabled);
    assert!(!ui.inspect("day-on").disabled);
}

#[test]
fn ids_key_the_choices_and_menu_keeps_two_in_a_menu() {
    let mut ui = app();
    ui.click_key("named").settle();
    assert!(ui.app::<Views>().named_open, "two choices open a menu when asked to");
    ui.click_key("named-short").settle();
    assert_eq!(ui.app::<Views>().named, 1);
    assert!(!ui.app::<Views>().named_open);
}
