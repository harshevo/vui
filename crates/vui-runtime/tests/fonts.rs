//! The theme's fonts: `Theme::font_sans` and `font_mono` decide what sans and mono text are set in,
//! and changing them lays the text out again.

use vui_core::{Mode, Theme};
use vui_runtime::testing::TestUi;
use vui_runtime::{Align, App, Builder, Cx, IntoElement, Parent, Styled, column, label};

struct Page {
    mono_ui: bool,
}

impl App for Page {
    type Msg = bool;

    fn update(&mut self, mono: bool, _cx: &mut Cx<bool>) {
        self.mono_ui = mono;
    }

    fn view(&self) -> impl IntoElement<bool> {
        column().padding(10.0).align_items(Align::Start).child(label("illicit iii").key("text"))
    }

    fn theme(&self, _system: Mode) -> Theme {
        let mut theme = Theme::light();
        if self.mono_ui {
            theme.font_sans = "JetBrains Mono";
        }
        theme
    }
}

#[test]
fn the_theme_sets_the_interface_font_and_switching_it_lays_text_out_again() {
    let mut t = TestUi::with_app(400.0, 100.0, Page { mono_ui: false });
    let proportional = t.rect("text").w;
    assert_eq!(t.ui.resources().fonts.families().0, "Inter");

    t.update_app(|p: &mut Page| p.mono_ui = true);
    t.ui.send(true);
    t.settle();
    let mono = t.rect("text").w;
    assert_eq!(t.ui.resources().fonts.families().0, "JetBrains Mono");
    assert!(mono > proportional + 10.0, "narrow letters take a whole cell in a monospace face: {proportional} → {mono}");

    t.ui.send(false);
    t.settle();
    assert_eq!(t.rect("text").w, proportional, "and back");
}
