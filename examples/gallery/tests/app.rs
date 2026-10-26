//! The gallery as an app: clicks and keys do what a person expects, and every page holds together.
//! None of this needs a GPU.

use gallery::{Choice, Headless, Mode, Page};
use vui::core::Theme;
use vui::prelude::*;
use vui::runtime::testing::{quads, texts};

fn open(page: Page) -> Headless {
    Headless::new(page, false, 1.0)
}

fn both_themes() -> [bool; 2] {
    [false, true]
}

/// The bounds of the control with this role and name, anywhere on the page.
fn bounds(h: &mut Headless, role: Role, name: &str) -> Rect {
    let tree = h.ui.ui.access_tree();
    let found = tree.with_role(role).into_iter().find(|n| n.name.as_deref() == Some(name));
    found.unwrap_or_else(|| panic!("no {role:?} named {name}")).bounds
}

fn focus_name(h: &mut Headless) -> Option<String> {
    let tree = h.ui.ui.access_tree();
    tree.node(tree.focus).and_then(|n| n.name.clone())
}

// ---- the shell -------------------------------------------------------------------------------

#[test]
fn clicking_a_page_in_the_sidebar_opens_it() {
    let mut h = open(Page::Foundations);
    h.ui.click_key("Buttons");
    assert_eq!(h.gallery().page, Page::Buttons);
    h.ui.settle();
    assert!(texts(&h.ui.scene()).contains(&"Variants".to_string()), "the page itself changes");
    let tree = h.ui.ui.access_tree();
    let selected = |name: &str| {
        tree.with_role(Role::ListItem).into_iter().find(|n| n.name.as_deref() == Some(name)).map(|n| n.selected)
    };
    assert_eq!(selected("Buttons"), Some(Some(true)));
    assert_eq!(selected("Foundations"), Some(Some(false)));
}

#[test]
fn the_sidebar_is_one_tab_stop_and_the_arrow_keys_walk_it() {
    let mut h = open(Page::Foundations);
    h.ui.tab();
    assert_eq!(focus_name(&mut h).as_deref(), Some("Foundations"), "Tab lands on the open page");
    h.ui.key(NamedKey::ArrowDown);
    h.ui.key(NamedKey::ArrowDown);
    assert_eq!(focus_name(&mut h).as_deref(), Some("Controls"));
    h.ui.key(NamedKey::Enter);
    assert_eq!(h.gallery().page, Page::Controls);
    h.ui.tab();
    let next = focus_name(&mut h);
    assert!(!matches!(next.as_deref(), Some("Foundations" | "Buttons" | "Inputs")), "Tab leaves the list: {next:?}");
}

#[test]
fn the_dark_switch_changes_the_theme_and_the_sidebar_colour_with_it() {
    let mut h = open(Page::Foundations);
    assert_eq!(h.ui.ui.theme().palette.bg, Theme::light().palette.bg);
    h.ui.click_key("dark");
    assert!(h.gallery().dark);
    assert_eq!(h.ui.ui.theme().palette.bg, Theme::dark().palette.bg);
    h.ui.settle();
    let side = h.ui.rect("sidebar");
    let scene = h.ui.scene();
    assert!(quads(&scene).iter().any(|q| q.rect == side && q.fill == Theme::dark().palette.side), "the sidebar is painted dark");
    h.ui.click_key("dark");
    assert_eq!(h.ui.ui.theme().palette.bg, Theme::light().palette.bg);
}

#[test]
fn choosing_dark_in_the_appearance_control_darkens_the_window() {
    let mut h = open(Page::Controls);
    let dark = bounds(&mut h, Role::RadioButton, "Dark");
    h.ui.click(dark.center());
    assert_eq!(h.gallery().mode, Mode::Dark);
    assert!(h.gallery().dark);
    assert_eq!(h.ui.ui.theme().palette.bg, Theme::dark().palette.bg);
    // System follows whatever the window already is.
    let system = bounds(&mut h, Role::RadioButton, "System");
    h.ui.click(system.center());
    assert_eq!(h.gallery().mode, Mode::System);
    assert!(h.gallery().dark);
}

// ---- the pages -------------------------------------------------------------------------------

#[test]
fn a_radio_choice_reaches_the_app() {
    let mut h = open(Page::Controls);
    let thorough = bounds(&mut h, Role::RadioButton, "Thorough");
    h.ui.click(thorough.center());
    assert_eq!(h.gallery().choice, Choice::Thorough);
}

#[test]
fn switches_and_checkboxes_flip_when_clicked() {
    let mut h = open(Page::Controls);
    assert!(h.gallery().wifi);
    h.ui.click_key("wifi");
    assert!(!h.gallery().wifi);
    h.ui.click_key("alerts");
    assert!(h.gallery().alerts);
    // A checkbox that is neither on nor off goes to on first.
    h.ui.click_key("agree");
    assert_eq!(h.gallery().agree, CheckState::On);
}

#[test]
fn dragging_a_slider_sets_the_volume() {
    let mut h = open(Page::Controls);
    let track = h.ui.rect("volume");
    let from = pt(track.x + track.w * 0.4, track.center().y);
    let to = pt(track.x + track.w * 0.9, track.center().y);
    h.ui.move_to(from).press(from).move_to(to).release(to);
    let volume = h.gallery().volume;
    assert!((volume - 90.0).abs() < 3.0, "{volume}");
}

#[test]
fn typing_in_a_field_reaches_the_app_and_comes_back_into_the_field() {
    let mut h = open(Page::Inputs);
    h.ui.click_key("name");
    h.ui.shortcut('a');
    h.ui.type_text("Grace Hopper");
    assert_eq!(h.gallery().name, "Grace Hopper");
    assert!(texts(&h.ui.scene()).contains(&"Grace Hopper".to_string()));
}

#[test]
fn the_erase_button_counts_its_clicks() {
    let mut h = open(Page::Settings);
    h.ui.click_key("erase");
    h.ui.click_key("erase");
    assert_eq!(h.gallery().erased, 2);
}

#[test]
fn a_row_of_the_list_is_chosen_by_clicking_it() {
    let mut h = open(Page::Lists);
    assert_eq!(h.gallery().row, 1);
    h.ui.click_key("Invoice 1042");
    assert_eq!(h.gallery().row, 2);
}

// ---- every page ------------------------------------------------------------------------------

#[test]
fn every_page_has_its_title_as_a_heading_and_names_everything_you_can_focus() {
    for page in Page::ALL {
        let mut h = open(page);
        let tree = h.ui.ui.access_tree();
        assert!(
            tree.with_role(Role::Heading).iter().any(|n| n.name.as_deref() == Some(page.name())),
            "{page:?} does not announce its title"
        );
        let nameless: Vec<_> =
            tree.nodes.iter().filter(|n| n.focusable && n.name.as_deref().is_none_or(str::is_empty)).map(|n| n.role).collect();
        assert!(nameless.is_empty(), "{page:?} has focusable things with no name: {nameless:?}");
    }
}

#[test]
fn every_page_fits_its_window_and_nothing_is_painted_outside_it() {
    for page in Page::ALL {
        for dark in both_themes() {
            let mut h = Headless::new(page, dark, 1.0);
            let window = h.ui.ui.size();
            let page_box = h.ui.rect("page");
            assert!(page_box.bottom() <= window.h + 0.5, "{page:?}: the page is {} tall in a window of {}", page_box.bottom(), window.h);
            let scene = h.ui.scene();
            for q in quads(&scene) {
                assert!(
                    q.rect.x >= -0.5 && q.rect.y >= -0.5 && q.rect.right() <= window.w + 0.5 && q.rect.bottom() <= window.h + 0.5,
                    "{page:?} ({}): {:?} leaves the {window:?} window",
                    if dark { "dark" } else { "light" },
                    q.rect
                );
            }
        }
    }
}

#[test]
fn the_theme_changes_colours_and_not_layout() {
    for page in Page::ALL {
        let rects = |dark: bool| {
            let mut h = Headless::new(page, dark, 1.0);
            // The dark switch is the one thing that does differ: its knob is at the other end.
            let switch = h.ui.rect("dark");
            let scene = h.ui.scene();
            quads(&scene).iter().map(|q| q.rect).filter(|r| !switch.contains(r.center())).collect::<Vec<_>>()
        };
        assert_eq!(rects(false), rects(true), "{page:?} is laid out differently in the dark");
    }
}
