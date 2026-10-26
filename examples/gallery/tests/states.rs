//! A control in each state a person can put it in, shot small at scale 2: under the pointer, pressed,
//! focused from the keyboard, with text selected. Re-bless after an intended change and look first:
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p gallery --test states
//! ```

use vui::core::Resources;
use vui::prelude::*;
use vui::render::testing::render_with;
use vui::render::{Image, assert_golden};
use vui::runtime::Ui;
use vui::runtime::testing::TestUi;

const SCALE: f32 = 2.0;

/// A little window holding `view`, in one theme, driven by hand.
struct Bench {
    t: TestUi<()>,
    res: Resources,
}

impl Bench {
    fn new(dark: bool, width: f32, height: f32, view: impl IntoElement<()>) -> Bench {
        let res = Resources::bundled_only();
        let theme = if dark { Theme::dark() } else { Theme::light() };
        let mut t = TestUi { ui: Ui::new(res.clone(), theme, size(width, height), SCALE) };
        t.view(view);
        Bench { t, res }
    }

    /// What is on screen once every transition has finished.
    fn image(&mut self) -> Image {
        self.t.settle();
        render_with(&self.t.scene(), &self.res)
    }
}

/// The four weights of button side by side.
fn buttons() -> Div<()> {
    row().padding(16.0).gap(12.0).align_items(Align::Center).children((
        button("Primary").primary().key("primary"),
        button("Secondary").key("secondary"),
        button("Ghost").ghost().key("ghost"),
        button("Danger").danger().key("danger"),
    ))
}

fn buttons_bench(dark: bool) -> Bench {
    Bench::new(dark, 400.0, 64.0, buttons())
}

#[test]
fn a_button_under_the_pointer() {
    let mut b = buttons_bench(false);
    b.t.move_to_key("primary");
    assert_golden!("button_hover_light", b.image());
}

#[test]
fn a_button_being_pressed() {
    let mut b = buttons_bench(false);
    let at = b.t.center("secondary");
    b.t.move_to(at).press(at);
    assert_golden!("button_pressed_light", b.image());
}

#[test]
fn a_button_with_keyboard_focus_in_light() {
    let mut b = buttons_bench(false);
    b.t.tab();
    assert_golden!("button_focus_light", b.image());
}

#[test]
fn a_button_with_keyboard_focus_in_dark() {
    let mut b = buttons_bench(true);
    b.t.tab();
    assert_golden!("button_focus_dark", b.image());
}

fn field_bench(dark: bool) -> Bench {
    let field = row().padding(16.0).child(text_field("Ada Lovelace").key("name").width(240.0));
    let mut b = Bench::new(dark, 272.0, 64.0, field);
    b.t.click_key("name");
    b.t.shortcut('a');
    b
}

#[test]
fn text_selected_in_a_field_in_light() {
    let mut b = field_bench(false);
    assert_golden!("field_selection_light", b.image());
}

#[test]
fn text_selected_in_a_field_in_dark() {
    let mut b = field_bench(true);
    assert_golden!("field_selection_dark", b.image());
}

#[test]
fn a_slider_with_keyboard_focus() {
    let mut b = Bench::new(false, 272.0, 64.0, row().padding(16.0).align_items(Align::Center).child(slider(40.0, 0.0, 100.0).name("Volume").width(240.0)));
    b.t.tab();
    assert_golden!("slider_focus_light", b.image());
}

#[test]
fn a_switch_with_keyboard_focus() {
    let switches = row().padding(16.0).gap(24.0).align_items(Align::Center).children((
        toggle(true).label("Wi-Fi"),
        toggle(false).label("Alerts"),
    ));
    let mut b = Bench::new(true, 272.0, 64.0, switches);
    b.t.tab();
    assert_golden!("switch_focus_dark", b.image());
}
