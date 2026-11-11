//! What assistive technology sees and can do.

use super::*;

#[test]
fn assistive_technology_sees_a_named_field_with_its_value_and_selection() {
    let mut t = focused(plain);
    t.type_text("hello").key_with(NamedKey::ArrowLeft, Modifiers::SHIFT);
    let n = node(&mut t);
    assert_eq!(n.role, Role::TextInput);
    assert_eq!(n.name.as_deref(), Some("Name"), "the placeholder names it");
    assert_eq!((n.value.as_deref(), n.selection), (Some("hello"), Some((5, 4))));
    assert!(n.actions.contains(&AccessActionKind::SetValue) && n.actions.contains(&AccessActionKind::Focus));
    assert!(n.focusable && !n.disabled);
    let tree = t.ui.access_tree();
    assert_eq!(tree.focus, n.id, "and knows it has focus");
}

#[test]
fn the_app_can_name_the_field_itself() {
    let mut t = TestUi::<Msg>::new(320.0, 100.0);
    t.view(column().padding(20.0).child(text_field::<Msg>("").key("f").width(200.0).placeholder("you@example.com").name("Email")));
    let n = node_of(&mut t, "f");
    assert_eq!(n.name.as_deref(), Some("Email"));
}

#[test]
fn assistive_technology_can_set_the_value() {
    let mut t = window(plain);
    let id = node(&mut t).id;
    t.ui.do_action(id, AccessAction::SetValue("typed by voice".into()));
    assert_eq!(text(&t), "typed by voice");
    assert_eq!(node(&mut t).value.as_deref(), Some("typed by voice"));
    t.ui.do_action(id, AccessAction::Click);
    assert_eq!(t.ui.focused_key(), Some("f".into()), "and click it to focus it");
}

#[test]
fn a_read_only_field_cannot_be_set_and_offers_no_way_to() {
    let mut t = window_with(locked, "locked", false);
    let n = node(&mut t);
    assert!(!n.actions.contains(&AccessActionKind::SetValue));
    t.ui.do_action(n.id, AccessAction::SetValue("changed".into()));
    assert_eq!(text(&t), "locked");
}

#[test]
fn a_search_field_has_the_search_role_a_placeholder_and_a_clear_button() {
    let mut t = window(find);
    assert!(drawn(&mut t).contains(&"Search".to_string()));
    assert_eq!(node(&mut t).role, Role::SearchInput);
    t.click_key("f").type_text("abc");
    assert_eq!(text(&t), "abc");
    let field = t.rect("f");
    t.click(pt(field.right() - 20.0, field.y + field.h / 2.0));
    assert_eq!(text(&t), "", "the button at the end empties it");
    t.shortcut('z');
    assert_eq!(text(&t), "abc", "and that can be undone");
}

#[test]
fn a_search_field_starts_its_text_after_the_magnifier() {
    let mut t = focused(find);
    t.type_text("abc");
    let scene = t.scene();
    let x = text_cmd(&scene, "abc").origin.x;
    assert_eq!(x, INNER_X + 16.0 + 8.0);
}
