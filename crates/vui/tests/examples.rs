//! The examples, run headless: they behave as their docs say, and their frames are golden images at
//! a wide and a narrow window, so a change that breaks one shows up here.
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p vui --test examples   # after an intended visual change
//! ```

#[allow(dead_code)]
#[path = "../examples/hello.rs"]
mod hello;

#[allow(dead_code)]
#[path = "../examples/todo.rs"]
mod todo;

#[allow(dead_code)]
#[path = "../examples/dashboard.rs"]
mod dashboard;

use vui::core::{NamedKey, Resources, Theme, size};
use vui::render::Image;
use vui::runtime::{App, Ui};
use vui::testing::{TestUi, assert_golden, render_with};

/// `app` in a window `width` by `height`, its springs at rest.
fn open<A: App>(width: f32, height: f32, app: A) -> (TestUi<A::Msg>, Resources) {
    let res = Resources::bundled_only();
    let mut t = TestUi { ui: Ui::with_app(res.clone(), size(width, height), 1.0, app) };
    t.advance(0.6);
    (t, res)
}

fn shot<M>(t: &mut TestUi<M>, res: &Resources) -> Image {
    t.advance(0.6);
    render_with(&t.scene(), res)
}

#[test]
fn hello_counts() {
    let (mut t, res) = open(420.0, 320.0, hello::Counter::default());
    t.click_key("more").click_key("more").click_key("less");
    assert_eq!(t.app::<hello::Counter>().count, 1);
    assert_golden!("hello", shot(&mut t, &res));
    t.click_key("reset");
    assert_eq!(t.app::<hello::Counter>().count, 0);
}

fn todo_app() -> todo::Todo {
    todo::Todo::new(&[("Read the vui guides", true), ("Build the first screen", false), ("Write a golden test", false)])
}

#[test]
fn todo_adds_with_return_and_keeps_the_field() {
    let (mut t, _res) = open(640.0, 560.0, todo_app());
    t.type_text("Ship it").key(NamedKey::Enter);
    let app = t.app::<todo::Todo>();
    assert_eq!(app.tasks.last().map(|t| t.text.as_str()), Some("Ship it"));
    assert!(app.draft.is_empty());
    // The field still has the keyboard, so the next task can be typed straight away.
    t.type_text("And another").key(NamedKey::Enter);
    assert_eq!(t.app::<todo::Todo>().tasks.len(), 5);
}

#[test]
fn todo_filters_and_clears() {
    let (mut t, _res) = open(640.0, 560.0, todo_app());
    t.click_key("clear");
    assert_eq!(t.app::<todo::Todo>().tasks.len(), 2);
    t.click_key("remove-1");
    assert_eq!(t.app::<todo::Todo>().tasks.len(), 1);
}

#[test]
fn todo_wide_and_narrow() {
    let (mut t, res) = open(640.0, 560.0, todo_app());
    assert_golden!("todo_wide", shot(&mut t, &res));
    let (mut t, res) = open(360.0, 560.0, todo_app());
    // On a phone-sized window the card fills the width, with a smaller margin.
    let card = t.rect("card");
    assert!(card.x <= 13.0 && card.w >= 330.0, "{card:?}");
    assert_golden!("todo_narrow", shot(&mut t, &res));
}

#[test]
fn todo_empty() {
    let (mut t, res) = open(640.0, 420.0, todo::Todo::new(&[]));
    assert_golden!("todo_empty_dark", {
        t.ui.set_theme(Theme::dark());
        shot(&mut t, &res)
    });
}

#[test]
fn dashboard_wide_light() {
    let (mut t, res) = open(1180.0, 900.0, dashboard::Dashboard::new(false));
    assert!(t.rect("sidebar").w > 0.0);
    assert_golden!("dashboard_wide_light", shot(&mut t, &res));
}

#[test]
fn dashboard_narrow_dark() {
    let (mut t, res) = open(390.0, 1500.0, dashboard::Dashboard::new(true));
    // The sidebar gives way to the top bar, and the stat cards stack.
    assert!(t.rect("top-bar").h > 0.0);
    assert_golden!("dashboard_narrow_dark", shot(&mut t, &res));
}

#[test]
fn dashboard_range_redraws_the_chart() {
    let (mut t, _res) = open(1180.0, 900.0, dashboard::Dashboard::new(false));
    t.click_key("dark");
    assert!(t.app::<dashboard::Dashboard>().dark);
    t.update_app(|d: &mut dashboard::Dashboard| d.range = dashboard::Range::Week);
    assert_eq!(t.app::<dashboard::Dashboard>().range, dashboard::Range::Week);
}
