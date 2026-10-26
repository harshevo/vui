//! The sidebar, its navigation items and list rows, and the form helpers.

use vui_core::{Color, Command, NamedKey, Quad, Scene, Theme, Tone};
use vui_icons::set;
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{AccessAction, Align, App, Builder, Cx, IntoElement, Parent, Role, Styled, column, row};
use vui_widgets::{
    avatar, field, list, list_row, nav_item, nav_section, separator_vertical, setting, sidebar, text_field, toggle,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Page {
    Ask,
    History,
    Memory,
    Settings,
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Go(Page),
    Open(usize),
    Wifi(bool),
}

struct Shell {
    page: Page,
    wifi: bool,
    log: Vec<Msg>,
}

impl App for Shell {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match &msg {
            Msg::Go(page) => self.page = *page,
            Msg::Wifi(on) => self.wifi = *on,
            Msg::Open(_) => {}
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let pages = [(Page::Ask, set::SPARKLES, "Ask"), (Page::History, set::CLOCK, "History"), (Page::Memory, set::LIBRARY, "Memory")];
        row().children((
            sidebar().key("side").children((
                list().key("nav").children(
                    pages.map(|(page, icon, name)| {
                        let item = nav_item(icon, name).key(name).selected(self.page == page).on_click(Msg::Go(page));
                        if page == Page::History { item.detail("12") } else { item }
                    }),
                ),
                nav_section("Recent").child(list().key("recent").children((
                    list_row("Notes on Rust ownership and borrowing in long form")
                        .key("r0")
                        .subtitle("Ownership, borrowing and lifetimes")
                        .trailing("9:41")
                        .leading(avatar("Rust Notes"))
                        .on_click(Msg::Open(0)),
                    list_row("Trip to Lisbon").key("r1").leading(avatar("Lisbon")).on_click(Msg::Open(1)).selected(true),
                ))),
                nav_item(set::SETTINGS, "Settings").key("settings").selected(self.page == Page::Settings).on_click(Msg::Go(Page::Settings)),
            )),
            separator_vertical(),
            column().grow(1.0).padding(24.0).gap(8.0).align_items(Align::Stretch).children((
                setting("Wi-Fi", "Join known networks", toggle(self.wifi).key("wifi").on_change(Msg::Wifi)),
                field("Name", text_field("Ada").key("name")),
            )),
        ))
    }
}

fn shell() -> TestUi<Msg> {
    TestUi::with_app(640.0, 480.0, Shell { page: Page::Ask, wifi: false, log: Vec::new() })
}

fn log(t: &TestUi<Msg>) -> Vec<Msg> {
    t.app::<Shell>().log.clone()
}

fn near(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01 && (a.a - b.a).abs() < 0.01
}

fn color(tone: Tone) -> Color {
    Theme::light().palette.get(tone)
}

fn fills(scene: &Scene, rect: vui_core::Rect, tone: Tone) -> Vec<&Quad> {
    quads(scene).into_iter().filter(|q| rect.contains(q.rect.center()) && near(q.fill, color(tone))).collect()
}

fn focus_name(t: &mut TestUi<Msg>) -> Option<String> {
    let tree = t.ui.access_tree();
    tree.node(tree.focus).and_then(|n| n.name.clone())
}

// ---- the sidebar -----------------------------------------------------------------------------

#[test]
fn the_sidebar_is_240_wide_on_the_side_colour_and_full_height() {
    let mut t = shell();
    let side = t.rect("side");
    assert_eq!((side.x, side.y, side.w, side.h), (0.0, 0.0, 240.0, 480.0));
    let scene = t.scene();
    assert!(fills(&scene, side, Tone::Side).iter().any(|q| q.rect == side));
}

#[test]
fn nav_items_are_32_high_and_line_up_with_the_sidebar_padding() {
    let mut t = shell();
    let ask = t.rect("Ask");
    assert_eq!(ask.h, 32.0);
    assert_eq!(ask.x, 12.0);
    assert_eq!(ask.w, 240.0 - 24.0);
}

#[test]
fn the_open_page_is_marked_with_a_wash_and_heavier_text() {
    let mut t = shell();
    t.settle();
    let ask = t.rect("Ask");
    let history = t.rect("History");
    let scene = t.scene();
    assert_eq!(fills(&scene, ask, Tone::Wash).len(), 1);
    assert_eq!(fills(&scene, history, Tone::Wash).len(), 0);
    // Heavier and darker text on the open page.
    let text_colour = |name: &str| {
        scene.commands.iter().find_map(|c| match c {
            Command::Text(c) if c.layout.text() == name => Some(c.color),
            _ => None,
        })
    };
    assert!(near(text_colour("Ask").unwrap(), color(Tone::Text)));
    assert!(near(text_colour("History").unwrap(), color(Tone::Text2)));
    let tree = t.ui.access_tree();
    assert_eq!(tree.find("Ask").unwrap().selected, Some(true));
    assert_eq!(tree.find("History").unwrap().selected, Some(false));
    assert_eq!(tree.find("Ask").unwrap().role, Role::ListItem);
}

#[test]
fn clicking_an_item_opens_its_page_and_the_wash_moves() {
    let mut t = shell();
    t.click_key("Memory");
    assert_eq!(log(&t), [Msg::Go(Page::Memory)]);
    t.settle();
    let memory = t.rect("Memory");
    let ask = t.rect("Ask");
    let scene = t.scene();
    assert_eq!(fills(&scene, memory, Tone::Wash).len(), 1);
    assert_eq!(fills(&scene, ask, Tone::Wash).len(), 0);
}

#[test]
fn hovering_an_item_tints_it() {
    let mut t = shell();
    let history = t.rect("History");
    t.move_to(history.center());
    t.settle();
    assert_eq!(fills(&t.scene(), history, Tone::Hover).len(), 1);
}

#[test]
fn an_item_can_carry_a_count() {
    let mut t = shell();
    let history = t.rect("History");
    let scene = t.scene();
    let count = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "12" => Some((c.origin, c.color)),
        _ => None,
    });
    let (origin, colour) = count.expect("the count is drawn");
    assert!(near(colour, color(Tone::Text3)));
    assert!(origin.x > history.center().x, "at the far end");
}

#[test]
fn each_list_is_one_tab_stop_and_the_arrows_move_inside_it() {
    let mut t = shell();
    t.tab();
    assert_eq!(focus_name(&mut t).as_deref(), Some("Ask"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focus_name(&mut t).as_deref(), Some("History"));
    t.key(NamedKey::End);
    assert_eq!(focus_name(&mut t).as_deref(), Some("Memory"));
    t.key(NamedKey::ArrowDown);
    assert_eq!(focus_name(&mut t).as_deref(), Some("Memory"), "the list ends there");
    t.key(NamedKey::Enter);
    assert_eq!(log(&t), [Msg::Go(Page::Memory)]);
    // Tab leaves for the next list instead of visiting Ask and History.
    t.tab();
    let next = focus_name(&mut t);
    assert!(next.as_deref().is_some_and(|n| n.starts_with("Notes") || n.starts_with("Trip")), "{next:?}");
}

#[test]
fn the_selected_row_is_where_tab_lands_after_the_arrows_have_moved_on() {
    let mut t = shell();
    t.tab();
    t.key(NamedKey::ArrowDown);
    t.key(NamedKey::ArrowDown);
    t.tab();
    t.shift_tab();
    assert_eq!(focus_name(&mut t).as_deref(), Some("Memory"), "Shift-Tab comes back to where focus left the list");
}

#[test]
fn a_heading_is_nearer_its_own_items_than_the_group_above() {
    let mut t = shell();
    let scene = t.scene();
    let heading = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "Recent" => Some(c.origin.y),
        _ => None,
    });
    let heading = heading.expect("the heading is drawn");
    // Under the 18-high text come the heading's 4 of padding and the section's 2.
    assert_eq!(t.rect("r0").y - (heading + 18.0), 4.0 + 2.0);
    // Above it comes the sidebar's 16, past the heading's own 4 of padding.
    let nav = t.rect("nav");
    assert_eq!(heading - 4.0 - nav.bottom(), 16.0);
}

// ---- rows ------------------------------------------------------------------------------------

#[test]
fn a_row_shows_title_subtitle_and_time_and_cuts_a_long_title_short() {
    let mut t = shell();
    let r0 = t.rect("r0");
    let scene = t.scene();
    let shown = texts(&scene);
    assert!(shown.contains(&"Ownership, borrowing and lifetimes".to_string()));
    assert!(shown.contains(&"9:41".to_string()));
    // However long the title, the row keeps its two lines: 8 + 21 + 18 + 8.
    assert_eq!(r0.h, 55.0, "{r0:?}");
    // The time stays inside the row.
    let time = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "9:41" => Some(c.origin.x + c.layout.size().w),
        _ => None,
    });
    assert!(time.unwrap() <= r0.right() - 11.9, "{time:?} vs {}", r0.right());
}

#[test]
fn a_row_with_one_line_is_shorter() {
    let mut t = shell();
    let r1 = t.rect("r1");
    assert_eq!(r1.h, 8.0 + 32.0 + 8.0, "the avatar is the tallest thing in it");
}

#[test]
fn rows_open_things_and_read_as_list_items() {
    let mut t = shell();
    t.click_key("r0");
    assert_eq!(log(&t), [Msg::Open(0)]);
    let tree = t.ui.access_tree();
    let rows = tree.with_role(Role::ListItem);
    let row = rows.iter().find(|n| n.name.as_deref() == Some("Notes on Rust ownership and borrowing in long form")).expect("named after its title");
    assert!(row.focusable);
    assert_eq!(tree.with_role(Role::List).len(), 2);
}

#[test]
fn the_open_row_has_the_wash_too() {
    let mut t = shell();
    t.settle();
    let r1 = t.rect("r1");
    assert_eq!(fills(&t.scene(), r1, Tone::Wash).iter().filter(|q| q.rect == r1).count(), 1);
}

// ---- forms -----------------------------------------------------------------------------------

#[test]
fn a_setting_has_its_title_on_the_left_and_its_control_on_the_right() {
    let mut t = shell();
    let wifi = t.rect("wifi");
    assert_eq!((wifi.w, wifi.h), (36.0, 20.0));
    assert_eq!(wifi.right(), 640.0 - 24.0, "the control ends at the content's padding");
    let scene = t.scene();
    let title = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "Wi-Fi" => Some(c.origin.x),
        _ => None,
    });
    assert_eq!(title, Some(241.0 + 24.0), "after the sidebar, its divider and the padding");
    assert!(texts(&scene).contains(&"Join known networks".to_string()));
}

#[test]
fn a_field_puts_its_name_over_the_control() {
    let mut t = shell();
    let name = t.rect("name");
    let scene = t.scene();
    let label = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "Name" => Some(c.origin),
        _ => None,
    });
    let label = label.expect("the name is drawn");
    assert!(label.y + 18.0 + 8.0 <= name.y + 0.5, "{label:?} over {name:?}");
    assert_eq!(label.x, name.x);
}

#[test]
fn a_setting_names_its_control_after_its_title_and_describes_it_with_its_detail() {
    let mut t = shell();
    let tree = t.ui.access_tree();
    let named: Vec<_> = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Wi-Fi")).collect();
    assert_eq!(named.len(), 1, "read out once");
    assert_eq!(named[0].role, Role::Switch);
    assert_eq!(named[0].description.as_deref(), Some("Join known networks"));
}

#[test]
fn a_field_names_its_control_after_its_label_and_is_read_out_once() {
    let mut t = shell();
    let tree = t.ui.access_tree();
    let named: Vec<_> = tree.nodes.iter().filter(|n| n.name.as_deref() == Some("Name")).map(|n| n.role).collect();
    assert_eq!(named, [Role::TextInput]);
}

#[test]
fn flipping_a_setting_sends_the_new_state() {
    let mut t = shell();
    t.click_key("wifi");
    assert_eq!(log(&t), [Msg::Wifi(true)]);
    let id = t.ui.access_tree().nodes.iter().find(|n| n.role == Role::Switch).unwrap().id;
    t.ui.do_action(id, AccessAction::Click);
    assert_eq!(log(&t), [Msg::Wifi(true), Msg::Wifi(false)]);
}
