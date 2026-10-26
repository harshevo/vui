mod common;

use common::*;
use vui_core::{pt, rect};
use vui_runtime::testing::TestUi;
use vui_runtime::{
    AccessAction, AccessActionKind, AccessInfo, AccessRange, AccessTree, Base, Builder, EventCx, Overflow, Parent, Role, Styled, Widget,
    WidgetEvent, builder, column, div, label, row, title,
};

fn tree_of(t: &mut TestUi<Msg>) -> AccessTree {
    t.ui.access_tree()
}

/// A widget that reports whatever it was given and answers every action by saying which one it was.
struct Reporter {
    info: AccessInfo,
}

impl Widget<Msg> for Reporter {
    type State = ();

    fn create_state(&self) {}

    fn access(&self, _: &(), info: &mut AccessInfo) {
        *info = self.info.clone();
    }

    fn event(&self, _: &mut (), cx: &mut EventCx<Msg>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::Action(action) => {
                cx.emit(Msg::Text(format!("{action:?}")));
                true
            }
            _ => false,
        }
    }
}

struct ReporterEl<M: 'static> {
    base: Base<M>,
    widget: Reporter,
}
builder!(ReporterEl<M> => Reporter);

fn reporter(info: AccessInfo) -> ReporterEl<Msg> {
    ReporterEl { base: Base::new(), widget: Reporter { info } }
}

// ---- the shape of the tree ---------------------------------------------------------------------

#[test]
fn the_window_leads_and_holds_everything_else() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((label("One"), label("Two"))));
    let tree = tree_of(&mut t);
    let window = &tree.nodes[0];
    assert_eq!((window.id, window.role), (0, Role::Window));
    assert_eq!(window.bounds, rect(0.0, 0.0, 300.0, 200.0));
    let names: Vec<_> = window.children.iter().map(|id| tree.node(*id).unwrap().name.clone().unwrap()).collect();
    assert_eq!(names, ["One", "Two"]);
    assert_eq!(tree.focus, 0, "nothing is focused, so the window is");
    let mut ids: Vec<_> = tree.nodes.iter().map(|n| n.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), tree.nodes.len(), "every id is used once");
}

#[test]
fn an_empty_ui_is_just_the_window() {
    let mut t = ui(300.0, 200.0);
    let tree = tree_of(&mut t);
    assert_eq!(tree.nodes.len(), 1);
    assert!(tree.nodes[0].children.is_empty());
}

#[test]
fn boxes_that_only_do_layout_are_looked_through() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(row().child(column().child(label("Deep")))));
    let tree = tree_of(&mut t);
    assert_eq!(tree.nodes.len(), 2);
    assert_eq!(tree.nodes[1].role, Role::Label);
    assert_eq!(tree.nodes[0].children, [tree.nodes[1].id]);
}

#[test]
fn text_is_a_label_and_a_title_is_a_heading() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((title("Settings").key("h"), label("Pick one").key("l"))));
    let (heading, text) = (t.rect("h"), t.rect("l"));
    let tree = tree_of(&mut t);
    let h = tree.find("Settings").expect("the title is listed");
    assert_eq!((h.role, h.bounds), (Role::Heading, heading));
    let l = tree.find("Pick one").expect("and the text");
    assert_eq!((l.role, l.bounds), (Role::Label, text));
}

#[test]
fn a_clickable_box_is_a_button_named_after_its_text() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(row().key("go").on_click(Msg::Click("go")).children((label("Open"), label("file")))));
    let go = t.rect("go");
    let tree = tree_of(&mut t);
    let button = tree.find("Open file").expect("named after what is inside");
    assert_eq!(button.role, Role::Button);
    assert_eq!(button.bounds, go);
    assert!(button.children.is_empty(), "the text is part of the button, not listed separately");
    assert!(button.actions.contains(&AccessActionKind::Click));
    assert!(tree.with_role(Role::Label).is_empty());
}

#[test]
fn an_explicit_name_and_description_beat_the_text_inside() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(row().on_click(Msg::Click("x")).name("Close").describe("Closes the window").child(label("x"))));
    let tree = tree_of(&mut t);
    let button = tree.find("Close").expect("the given name is used");
    assert_eq!(button.description.as_deref(), Some("Closes the window"));
    assert!(tree.find("x").is_none());
}

#[test]
fn the_app_can_choose_the_role() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        row().key("docs").role(Role::Link).name("Docs").on_click(Msg::Click("docs")),
        column().role(Role::List).name("Files").children((
            div().role(Role::ListItem).name("a.txt"),
            div().role(Role::ListItem).name("b.txt"),
        )),
    )));
    let tree = tree_of(&mut t);
    assert_eq!(tree.find("Docs").unwrap().role, Role::Link);
    let list = tree.find("Files").unwrap();
    assert_eq!(list.role, Role::List);
    let items: Vec<_> = list.children.iter().map(|id| tree.node(*id).unwrap().name.clone().unwrap()).collect();
    assert_eq!(items, ["a.txt", "b.txt"], "a list keeps its items as children, in order");
}

#[test]
fn hidden_elements_and_everything_in_them_are_left_out() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        label("Visible"),
        column().a11y_hidden(true).child(label("Decoration")),
        label("Gone").hidden(true),
        row().on_click(Msg::Click("save")).children((label("Save"), label("shortcut text").a11y_hidden(true))),
    )));
    let tree = tree_of(&mut t);
    let mut names: Vec<_> = tree.nodes.iter().filter_map(|n| n.name.as_deref()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Save", "Visible"]);
}

#[test]
fn a_box_that_can_be_focused_is_listed_even_if_nobody_named_it() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(div().key("f").height(20.0).focusable(true)));
    t.tab();
    let tree = tree_of(&mut t);
    assert_ne!(tree.focus, 0);
    let node = tree.node(tree.focus).expect("the focus is a real node");
    assert_eq!(node.role, Role::Group);
    assert!(node.focusable);
}

#[test]
fn a_scrolling_box_is_a_scroll_view_that_can_bring_its_rows_into_view() {
    let rows = |n: u32| {
        column().key("list").overflow(Overflow::Scroll).children(
            (0..n)
                .map(|i| div().key(i).height(30.0).shrink(0.0).role(Role::ListItem).name(format!("row {i}")))
                .collect::<Vec<_>>(),
        )
    };
    let mut t = ui(200.0, 100.0);
    t.view(rows(10));
    let tree = tree_of(&mut t);
    let views = tree.with_role(Role::ScrollView);
    assert_eq!(views.len(), 1);
    assert!(views[0].actions.contains(&AccessActionKind::ScrollIntoView));
    assert_eq!(views[0].children.len(), 10);

    let row9 = tree.find("row 9").unwrap().id;
    t.ui.do_action(row9, AccessAction::ScrollIntoView);
    t.settle();
    assert!(t.rect(9u32).bottom() <= 100.0, "{:?}", t.rect(9u32));

    t.trackpad(pt(100.0, 50.0), 0.0, 5000.0);
    t.trackpad(pt(100.0, 50.0), 0.0, -60.0);
    let tree = tree_of(&mut t);
    assert_eq!(tree.find("row 2").unwrap().bounds.y, 0.0, "bounds follow the scrolled content");
}

// ---- state ------------------------------------------------------------------------------------

#[test]
fn focus_is_reported_and_focusable_things_say_so() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        div().key("a").height(20.0).name("A").focusable(true).on_click(Msg::Click("a")),
        div().key("b").height(20.0).name("B").on_click(Msg::Click("b")),
    )));
    let tree = tree_of(&mut t);
    let (a, b) = (tree.find("A").unwrap(), tree.find("B").unwrap());
    assert!(a.focusable && a.actions.contains(&AccessActionKind::Focus));
    assert!(!b.focusable && !b.actions.contains(&AccessActionKind::Focus));
    assert_eq!(tree.focus, 0);

    t.tab();
    let tree = tree_of(&mut t);
    assert_eq!(tree.focus, tree.find("A").unwrap().id);
}

#[test]
fn a_disabled_box_disables_what_is_inside_it() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        column().disabled(true).child(div().height(20.0).name("Inner").focusable(true).on_click(Msg::Click("inner"))),
        div().height(20.0).name("Outer").on_click(Msg::Click("outer")),
    )));
    let tree = tree_of(&mut t);
    let inner = tree.find("Inner").unwrap();
    assert!(inner.disabled && !inner.focusable);
    assert!(inner.actions.is_empty(), "nothing to do with a disabled button: {:?}", inner.actions);
    let outer = tree.find("Outer").unwrap();
    assert!(!outer.disabled);
}

#[test]
fn what_a_widget_reports_is_what_assistive_technology_gets() {
    let mut t = ui(300.0, 200.0);
    let volume = AccessInfo {
        role: Some(Role::Slider),
        value: Some("40%".into()),
        range: Some(AccessRange { min: 0.0, max: 100.0, step: 5.0, value: 40.0 }),
        actions: vec![AccessActionKind::Increment, AccessActionKind::Decrement],
        ..AccessInfo::default()
    };
    let notes = AccessInfo {
        role: Some(Role::Checkbox),
        toggled: Some(true),
        selected: Some(false),
        expanded: Some(true),
        ..AccessInfo::default()
    };
    t.view(column().children((reporter(volume.clone()).height(20.0).name("Volume"), reporter(notes).height(20.0).name("Notes"))));
    let tree = tree_of(&mut t);

    let v = tree.find("Volume").unwrap();
    assert_eq!(v.role, Role::Slider);
    assert_eq!(v.value.as_deref(), Some("40%"));
    assert_eq!(v.range, volume.range);
    assert_eq!(v.actions, [AccessActionKind::Increment, AccessActionKind::Decrement]);

    let n = tree.find("Notes").unwrap();
    assert_eq!((n.role, n.toggled, n.selected, n.expanded), (Role::Checkbox, Some(true), Some(false), Some(true)));
}

#[test]
fn the_tree_follows_the_latest_view() {
    let mut t = ui(300.0, 200.0);
    let view = |on: bool| {
        let info = AccessInfo { role: Some(Role::Switch), toggled: Some(on), ..AccessInfo::default() };
        column().child(reporter(info).key("sw").height(20.0).name("Wi-Fi"))
    };
    t.view(view(false));
    assert_eq!(tree_of(&mut t).find("Wi-Fi").unwrap().toggled, Some(false));
    t.view(view(true));
    assert_eq!(tree_of(&mut t).find("Wi-Fi").unwrap().toggled, Some(true));
}

#[test]
fn elements_keep_their_ids_while_they_live() {
    let mut t = ui(300.0, 200.0);
    let item = |name: &'static str| div().key(name).height(20.0).name(name).on_click(Msg::Click(name));
    t.view(column().child(item("A")));
    let before = tree_of(&mut t).find("A").unwrap().id;
    t.view(column().children((item("N"), item("A"))));
    let tree = tree_of(&mut t);
    assert_eq!(tree.find("A").unwrap().id, before, "A is the same node, only moved down");
    let new = tree.find("N").unwrap().id;
    assert_ne!(new, before);
    assert_ne!(new, 0);
}

// ---- carrying out requests ---------------------------------------------------------------------

#[test]
fn a_click_from_assistive_technology_presses_the_element() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(div().height(20.0).name("Go").on_press(Msg::Press("go")).on_click(Msg::Click("go"))));
    let id = tree_of(&mut t).find("Go").unwrap().id;
    t.ui.do_action(id, AccessAction::Click);
    assert_eq!(t.messages(), [Msg::Press("go"), Msg::Click("go")]);
}

#[test]
fn a_widget_answers_the_actions_it_understands() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(reporter(AccessInfo::default()).height(20.0).name("Thing").role(Role::Slider)));
    let id = tree_of(&mut t).find("Thing").unwrap().id;
    t.ui.do_action(id, AccessAction::Increment);
    t.ui.do_action(id, AccessAction::SetNumeric(0.5));
    t.ui.do_action(id, AccessAction::SetValue("hi".into()));
    assert_eq!(
        t.messages(),
        [Msg::Text("Increment".into()), Msg::Text("SetNumeric(0.5)".into()), Msg::Text("SetValue(\"hi\")".into())]
    );
}

#[test]
fn the_widget_hears_a_click_before_the_element_does() {
    let mut t = ui(300.0, 200.0);
    t.view(column().child(reporter(AccessInfo::default()).height(20.0).name("Mine").on_click(Msg::Click("element"))));
    let id = tree_of(&mut t).find("Mine").unwrap().id;
    t.ui.do_action(id, AccessAction::Click);
    assert_eq!(t.messages(), [Msg::Text("Click".into())], "it used the action, so the click handler did not run");
}

#[test]
fn asking_for_focus_moves_it_and_shows_the_ring() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        div().key("a").height(20.0).name("A").focusable(true),
        div().key("b").height(20.0).name("B"),
    )));
    let tree = tree_of(&mut t);
    let (a, b) = (tree.find("A").unwrap().id, 0);
    t.ui.do_action(a, AccessAction::Focus);
    assert!(t.inspect("a").focused && t.inspect("a").focus_visible);
    assert_eq!(tree_of(&mut t).focus, a);

    t.ui.do_action(b, AccessAction::Focus);
    assert!(t.inspect("a").focused, "the window cannot be focused like an element");
}

#[test]
fn focusing_something_out_of_view_scrolls_it_in() {
    let mut t = ui(200.0, 100.0);
    t.view(column().key("list").overflow(Overflow::Scroll).children(
        (0..10u32).map(|i| div().key(i).height(30.0).shrink(0.0).name(format!("row {i}")).focusable(true)).collect::<Vec<_>>(),
    ));
    let id = tree_of(&mut t).find("row 8").unwrap().id;
    t.ui.do_action(id, AccessAction::Focus);
    t.settle();
    let r = t.rect(8u32);
    assert!(r.y >= 0.0 && r.bottom() <= 100.0, "{r:?}");
}

#[test]
fn requests_for_things_that_cannot_be_acted_on_are_ignored() {
    let mut t = ui(300.0, 200.0);
    let view = |disabled: bool, hidden: bool| {
        column().children((
            column().disabled(disabled).child(div().height(20.0).name("Inner").on_click(Msg::Click("inner")).focusable(true)),
            div().height(20.0).name("Plain").on_click(Msg::Click("plain")).a11y_hidden(hidden),
        ))
    };
    t.view(view(false, false));
    let tree = tree_of(&mut t);
    let (inner, plain) = (tree.find("Inner").unwrap().id, tree.find("Plain").unwrap().id);

    t.view(view(true, true));
    t.ui.do_action(inner, AccessAction::Click);
    t.ui.do_action(inner, AccessAction::Focus);
    t.ui.do_action(plain, AccessAction::Click);
    assert!(t.messages().is_empty());
    assert_eq!(tree_of(&mut t).focus, 0);

    t.ui.do_action(0, AccessAction::Click);
    t.ui.do_action(9999, AccessAction::Click);
    assert!(t.messages().is_empty());
}
