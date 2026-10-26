//! Buttons, the switch, the checkbox and radio buttons, driven by pointer, keyboard and assistive
//! technology. The app is a small settings window that logs every message it gets.

use vui_core::{Color, Command, NamedKey, Point, Quad, Scene, Theme, Tone, pt};
use vui_icons::set;
use vui_runtime::testing::{TestUi, at, quads, texts};
use vui_runtime::{AccessAction, AccessActionKind, AccessNode, Align, App, Builder, Cx, IntoElement, Parent, Role, Styled, column, row};
use vui_widgets::{CheckState, button, checkbox, icon_button, radio_group, toggle};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Pick {
    Small,
    Medium,
    Large,
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Save,
    Publish,
    More,
    Delete,
    Close,
    Sync,
    Wifi(bool),
    Agree(bool),
    Size(Pick),
}

struct Lab {
    log: Vec<Msg>,
    wifi: bool,
    agree: CheckState,
    size: Pick,
    syncing: bool,
    locked: bool,
}

impl Lab {
    fn new() -> Lab {
        Lab { log: Vec::new(), wifi: false, agree: CheckState::Off, size: Pick::Medium, syncing: false, locked: false }
    }
}

impl App for Lab {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match &msg {
            Msg::Wifi(on) => self.wifi = *on,
            Msg::Agree(on) => self.agree = (*on).into(),
            Msg::Size(size) => self.size = *size,
            _ => {}
        }
        self.log.push(msg);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let sizes = [(Pick::Small, "Small"), (Pick::Medium, "Medium"), (Pick::Large, "Large")];
        column().padding(20.0).gap(16.0).align_items(Align::Start).children((
            row().gap(8.0).align_items(Align::Center).children((
                button("Save").key("save").on_click(Msg::Save),
                button("Publish").primary().key("primary").on_click(Msg::Publish),
                button("More").ghost().key("ghost").on_click(Msg::More),
                button("Delete").danger().key("danger").on_click(Msg::Delete),
            )),
            row().gap(8.0).align_items(Align::Center).children((
                button("Tiny").small().key("small").on_click(Msg::Save),
                button("Huge").large().key("large").on_click(Msg::Save),
                icon_button(set::X, "Close").key("close").on_click(Msg::Close),
                button("Sync").icon(set::SEARCH).loading(self.syncing).key("sync").on_click(Msg::Sync),
                button("Locked").disabled(true).key("locked").on_click(Msg::Save),
            )),
            toggle(self.wifi).label("Wi-Fi").key("wifi").disabled(self.locked).on_change(Msg::Wifi),
            toggle(self.wifi).key("bare").on_change(Msg::Wifi),
            checkbox(self.agree).label("I agree").key("agree").on_change(Msg::Agree),
            radio_group(sizes.map(|(p, l)| (p, l.to_string())), &self.size, Msg::Size),
        ))
    }
}

fn lab() -> TestUi<Msg> {
    TestUi::with_app(420.0, 360.0, Lab::new())
}

fn log(t: &TestUi<Msg>) -> Vec<Msg> {
    t.app::<Lab>().log.clone()
}

fn node(t: &mut TestUi<Msg>, name: &str) -> AccessNode {
    t.ui.access_tree().find(name).unwrap_or_else(|| panic!("no node named {name}")).clone()
}

fn centre(t: &mut TestUi<Msg>, name: &str) -> Point {
    node(t, name).bounds.center()
}

fn click_named(t: &mut TestUi<Msg>, name: &str) {
    let p = centre(t, name);
    t.click(p);
}

fn palette() -> vui_core::Palette {
    Theme::light().palette
}

fn color(tone: Tone) -> Color {
    palette().get(tone)
}

fn near(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01 && (a.a - b.a).abs() < 0.01
}

/// The quads inside `rect` that are filled with `tone` (at full strength).
fn filled(scene: &Scene, rect: vui_core::Rect, tone: Tone) -> Vec<&Quad> {
    quads(scene).into_iter().filter(|q| rect.contains(q.rect.center()) && near(q.fill, color(tone))).collect()
}

fn images_in(scene: &Scene, rect: vui_core::Rect) -> usize {
    scene.commands.iter().filter(|c| matches!(c, Command::Image(i) if rect.contains(i.rect.center()))).count()
}

// ---- buttons ---------------------------------------------------------------------------------

#[test]
fn a_click_sends_the_message() {
    let mut t = lab();
    t.click_key("save");
    assert_eq!(log(&t), [Msg::Save]);
}

#[test]
fn a_press_that_slides_off_the_button_sends_nothing() {
    let mut t = lab();
    let r = t.rect("save");
    t.press(r.center()).move_to(pt(r.x + 400.0, r.bottom() + 100.0)).release(pt(r.x + 400.0, r.bottom() + 100.0));
    assert!(log(&t).is_empty());
}

#[test]
fn clicking_a_button_does_not_leave_a_ring_on_it_but_tab_does() {
    let mut t = lab();
    let r = t.rect("save");
    let ring = |scene: &Scene| quads(scene).iter().any(|q| q.border.is_some_and(|b| b.width == 2.0 && near(b.color, color(Tone::Accent))));
    t.click_key("save");
    t.settle();
    assert!(!ring(&t.scene()), "a click is not keyboard focus");
    t.tab();
    t.settle();
    let scene = t.scene();
    assert!(ring(&scene), "Tab shows where focus is");
    assert_eq!(t.ui.focused_key(), Some("save".into()), "{r:?}");
}

#[test]
fn space_and_enter_press_a_focused_button() {
    let mut t = lab();
    t.tab();
    t.key(NamedKey::Enter);
    t.key(NamedKey::Space);
    assert_eq!(log(&t), [Msg::Save, Msg::Save]);
    t.tab();
    t.key(NamedKey::Enter);
    assert_eq!(log(&t).last(), Some(&Msg::Publish));
}

#[test]
fn a_disabled_button_does_nothing_and_is_skipped_by_tab() {
    let mut t = lab();
    t.click_key("locked");
    assert!(log(&t).is_empty());
    assert!(node(&mut t, "Locked").disabled);
    // Tab visits every enabled control and never lands on the disabled one.
    let mut visited = Vec::new();
    for _ in 0..8 {
        t.tab();
        visited.push(t.ui.focused_key());
    }
    assert!(!visited.contains(&Some("locked".into())), "{visited:?}");
}

#[test]
fn a_loading_button_ignores_presses_and_keeps_turning() {
    let mut t = lab();
    t.update_app(|lab: &mut Lab| lab.syncing = true);
    let r = t.rect("sync");
    t.click(r.center());
    assert!(!log(&t).contains(&Msg::Sync), "a click while busy does nothing");
    let frame = |t: &mut TestUi<Msg>| {
        let scene = t.scene();
        scene.commands.iter().find_map(|c| match c {
            Command::Image(i) if r.contains(i.rect.center()) => Some(i.id),
            _ => None,
        })
    };
    let first = frame(&mut t);
    assert!(first.is_some(), "a spinner is drawn");
    assert!(t.ui.needs_frame(), "and it asks for frames");
    t.advance(0.2);
    assert_ne!(frame(&mut t), first, "it turns");
    // Keyboard presses are ignored as well.
    t.tab();
    for _ in 0..4 {
        if t.ui.focused_key() == Some("sync".into()) {
            break;
        }
        t.tab();
    }
    t.key(NamedKey::Enter);
    assert!(!log(&t).contains(&Msg::Sync));
}

#[test]
fn buttons_are_as_tall_as_the_theme_says_and_as_wide_as_their_content() {
    let mut t = lab();
    let theme = Theme::light();
    assert_eq!(t.rect("save").h, theme.control);
    assert_eq!(t.rect("small").h, 24.0);
    assert_eq!(t.rect("large").h, theme.control_large);
    // The icon button is square, and a labelled button is its label plus 12 on each side.
    let close = t.rect("close");
    assert_eq!((close.w, close.h), (theme.control, theme.control));
    let save = t.rect("save");
    let label_w = {
        let scene = t.scene();
        let label = scene.commands.iter().find_map(|c| match c {
            Command::Text(c) if c.layout.text() == "Save" => Some(c.layout.size().w),
            _ => None,
        });
        label.expect("the label is drawn")
    };
    assert!((save.w - (label_w + 24.0)).abs() < 0.01, "{} vs {}", save.w, label_w);
}

#[test]
fn variants_are_drawn_in_the_palette() {
    let mut t = lab();
    let scene = t.scene();
    let primary = t.rect("primary");
    assert_eq!(filled(&scene, primary, Tone::Accent).len(), 1);
    let danger = t.rect("danger");
    assert_eq!(filled(&scene, danger, Tone::Danger).len(), 1);
    // A secondary button is an outline; a ghost button has no box at rest.
    let save = t.rect("save");
    assert!(quads(&scene).iter().any(|q| (q.rect.center().x - save.center().x).abs() < 1.0 && q.rect.h == save.h && q.border.is_some_and(|b| near(b.color, color(Tone::Line)))));
    let ghost = t.rect("ghost");
    assert!(!quads(&scene).iter().any(|q| ghost.contains(q.rect.center())));
    // Text on the primary button is the "on" colour.
    let on_accent = scene.commands.iter().any(|c| matches!(c, Command::Text(c) if c.layout.text() == "Publish" && near(c.color, color(Tone::OnAccent))));
    assert!(on_accent);
}

#[test]
fn a_ghost_button_shows_its_box_under_the_pointer_and_darker_when_pressed() {
    let mut t = lab();
    let ghost = t.rect("ghost");
    t.move_to(ghost.center());
    t.settle();
    let scene = t.scene();
    assert_eq!(filled(&scene, ghost, Tone::Hover).len(), 1);
    t.press(ghost.center());
    t.settle();
    let scene = t.scene();
    assert!(!filled(&scene, ghost, Tone::Wash).is_empty());
}

#[test]
fn buttons_read_as_buttons() {
    let mut t = lab();
    for name in ["Save", "Publish", "More", "Delete", "Tiny", "Close"] {
        let n = node(&mut t, name);
        assert_eq!(n.role, Role::Button, "{name}");
        assert!(n.actions.contains(&AccessActionKind::Click), "{name}");
        assert!(n.focusable, "{name}");
    }
    // An icon button is named by the app, not by its picture.
    assert_eq!(node(&mut t, "Close").name.as_deref(), Some("Close"));
}

#[test]
fn assistive_technology_can_press_a_button() {
    let mut t = lab();
    let id = node(&mut t, "Save").id;
    t.ui.do_action(id, AccessAction::Click);
    assert_eq!(log(&t), [Msg::Save]);
}

// ---- the switch ------------------------------------------------------------------------------

#[test]
fn a_switch_asks_for_the_other_state() {
    let mut t = lab();
    t.click_key("wifi");
    assert_eq!(log(&t), [Msg::Wifi(true)]);
    t.click_key("wifi");
    assert_eq!(log(&t), [Msg::Wifi(true), Msg::Wifi(false)]);
}

#[test]
fn a_switch_is_36_by_20_and_its_label_sits_beside_it() {
    let mut t = lab();
    let bare = t.rect("bare");
    assert_eq!((bare.w, bare.h), (36.0, 20.0));
    let wifi = t.rect("wifi");
    assert!(wifi.w > 36.0 + 8.0);
    let scene = t.scene();
    let label = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) if c.layout.text() == "Wi-Fi" => Some(c.origin),
        _ => None,
    });
    assert!((label.expect("the label is drawn").x - (wifi.x + 36.0 + 8.0)).abs() < 0.6);
}

#[test]
fn the_knob_slides_across_and_the_track_takes_the_accent() {
    let mut t = lab();
    let r = t.rect("bare");
    let scene = t.scene();
    let knob = |scene: &Scene| quads(scene).into_iter().find(|q| near(q.fill, color(Tone::Knob)) && r.contains(q.rect.center())).map(|q| q.rect);
    let off = knob(&scene).expect("a knob");
    assert_eq!((off.x, off.w, off.h), (r.x + 2.0, 16.0, 16.0));
    assert_eq!(filled(&scene, r, Tone::TrackOff).len(), 1);

    t.click_key("bare");
    t.advance(0.04);
    let mid = knob(&t.scene()).expect("a knob").x;
    assert!(mid > off.x && mid < r.x + 18.0, "in flight: {mid}");

    t.settle();
    let scene = t.scene();
    let on = knob(&scene).expect("a knob");
    assert!((on.x - (r.right() - 2.0 - 16.0)).abs() < 0.01, "{on:?}");
    assert_eq!(filled(&scene, r, Tone::Accent).len(), 1);
    assert_eq!(filled(&scene, r, Tone::TrackOff).len(), 0);
}

#[test]
fn the_knob_widens_under_the_finger() {
    let mut t = lab();
    let r = t.rect("bare");
    t.press(r.center());
    t.settle();
    let scene = t.scene();
    let knob = quads(&scene).into_iter().find(|q| near(q.fill, color(Tone::Knob)) && r.contains(q.rect.center())).expect("a knob").rect;
    assert!((knob.w - 20.0).abs() < 0.01, "{knob:?}");
    t.release(r.center());
    t.settle();
    assert_eq!(log(&t), [Msg::Wifi(true)]);
}

#[test]
fn space_and_enter_flip_a_focused_switch() {
    let mut t = lab();
    for _ in 0..20 {
        t.tab();
        if t.ui.focused_key() == Some("wifi".into()) {
            break;
        }
    }
    assert_eq!(t.ui.focused_key(), Some("wifi".into()));
    t.key(NamedKey::Space);
    t.key(NamedKey::Enter);
    assert_eq!(log(&t), [Msg::Wifi(true), Msg::Wifi(false)]);
}

#[test]
fn a_switch_reads_as_a_switch() {
    let mut t = lab();
    let n = node(&mut t, "Wi-Fi");
    assert_eq!((n.role, n.toggled), (Role::Switch, Some(false)));
    t.ui.do_action(n.id, AccessAction::Click);
    assert_eq!(node(&mut t, "Wi-Fi").toggled, Some(true));
}

#[test]
fn a_disabled_switch_stays_as_it_is() {
    let mut t = lab();
    t.update_app(|lab: &mut Lab| lab.locked = true);
    t.click_key("wifi");
    assert!(log(&t).is_empty());
    assert!(node(&mut t, "Wi-Fi").disabled);
}

// ---- the checkbox ----------------------------------------------------------------------------

#[test]
fn a_checkbox_draws_its_tick_only_when_it_is_on() {
    let mut t = lab();
    let r = t.rect("agree");
    let boxed = vui_core::Rect::new(r.x, r.y, 18.0, 18.0);
    assert_eq!(images_in(&t.scene(), boxed), 0);
    assert!(quads(&t.scene()).iter().any(|q| q.rect.w == 18.0 && q.border.is_some_and(|b| b.width == 1.5)));
    t.click_key("agree");
    t.settle();
    assert_eq!(log(&t), [Msg::Agree(true)]);
    let scene = t.scene();
    assert_eq!(images_in(&scene, boxed), 1, "the tick");
    assert_eq!(filled(&scene, boxed, Tone::Accent).len(), 1);
}

#[test]
fn clicking_the_label_clicks_the_box() {
    let mut t = lab();
    let r = t.rect("agree");
    t.click(at(r, 0.95, 0.5));
    assert_eq!(log(&t), [Msg::Agree(true)]);
}

#[test]
fn a_mixed_checkbox_turns_everything_on_and_says_it_is_mixed() {
    let mut t = lab();
    t.update_app(|lab: &mut Lab| lab.agree = CheckState::Mixed);
    let n = node(&mut t, "I agree");
    assert_eq!((n.role, n.mixed), (Role::Checkbox, true));
    t.click_key("agree");
    assert_eq!(log(&t), [Msg::Agree(true)]);
    assert!(!node(&mut t, "I agree").mixed);
    assert_eq!(node(&mut t, "I agree").toggled, Some(true));
    t.click_key("agree");
    assert_eq!(log(&t).last(), Some(&Msg::Agree(false)));
}

// ---- radio buttons ---------------------------------------------------------------------------

fn chosen(t: &mut TestUi<Msg>) -> Vec<String> {
    let tree = t.ui.access_tree();
    tree.with_role(Role::RadioButton).into_iter().filter(|n| n.toggled == Some(true)).filter_map(|n| n.name.clone()).collect()
}

#[test]
fn a_radio_group_has_one_chosen_button() {
    let mut t = lab();
    assert_eq!(chosen(&mut t), ["Medium"]);
    assert_eq!(t.ui.access_tree().with_role(Role::RadioButton).len(), 3);
    click_named(&mut t, "Large");
    assert_eq!(log(&t), [Msg::Size(Pick::Large)]);
    assert_eq!(chosen(&mut t), ["Large"]);
}

#[test]
fn arrow_keys_move_between_radio_buttons_and_choose_as_they_go() {
    let mut t = lab();
    // The group is one Tab stop, and it lands on the chosen button.
    let mut at_group = false;
    for _ in 0..12 {
        t.tab();
        let focus = t.ui.access_tree().focus;
        if t.ui.access_tree().node(focus).is_some_and(|n| n.name.as_deref() == Some("Medium")) {
            at_group = true;
            break;
        }
    }
    assert!(at_group, "Tab reaches the chosen radio button");
    t.key(NamedKey::ArrowDown);
    assert_eq!(log(&t), [Msg::Size(Pick::Large)]);
    assert_eq!(chosen(&mut t), ["Large"]);
    t.key(NamedKey::ArrowUp);
    t.key(NamedKey::ArrowUp);
    assert_eq!(log(&t), [Msg::Size(Pick::Large), Msg::Size(Pick::Medium), Msg::Size(Pick::Small)]);
    t.key(NamedKey::End);
    assert_eq!(chosen(&mut t), ["Large"]);
    // One more Tab leaves the group instead of visiting every button.
    t.tab();
    let focus = t.ui.access_tree().focus;
    let name = t.ui.access_tree().node(focus).and_then(|n| n.name.clone());
    assert_ne!(name.as_deref(), Some("Small"));
    assert_ne!(name.as_deref(), Some("Medium"));
}

#[test]
fn a_chosen_radio_button_has_a_dot_and_the_others_do_not() {
    let mut t = lab();
    t.settle();
    let scene = t.scene();
    let dots = |name: &str, t: &mut TestUi<Msg>| {
        let r = node(t, name).bounds;
        quads(&scene).iter().filter(|q| near(q.fill, color(Tone::Accent)) && q.rect.w < 12.0 && r.contains(q.rect.center())).count()
    };
    assert_eq!(dots("Medium", &mut t), 1);
    assert_eq!(dots("Small", &mut t), 0);
    assert_eq!(dots("Large", &mut t), 0);
}

#[test]
fn texts_in_the_window_are_the_labels_and_nothing_else() {
    let mut t = lab();
    let scene = t.scene();
    let mut all = texts(&scene);
    all.sort();
    let mut want = ["Save", "Publish", "More", "Delete", "Tiny", "Huge", "Sync", "Locked", "Wi-Fi", "I agree", "Small", "Medium", "Large"].map(String::from).to_vec();
    want.sort();
    assert_eq!(all, want);
}
