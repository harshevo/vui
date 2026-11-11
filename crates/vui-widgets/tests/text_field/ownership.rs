//! Who owns the text: the app changing it from outside while the field keeps its caret.

use super::*;

#[test]
fn the_app_can_refuse_an_edit() {
    let mut t = window_with(plain, "", true);
    t.click_key("f").type_text("12a3");
    assert_eq!(text(&t), "123");
    assert_eq!(node(&mut t).value.as_deref(), Some("123"), "the field shows what the app kept");
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn the_app_can_change_the_text_from_outside() {
    let mut t = focused(plain);
    t.type_text("abc");
    t.update_app(|f: &mut Form| f.text = "reset".into());
    let n = node(&mut t);
    assert_eq!((n.value.as_deref(), n.selection), (Some("reset"), Some((5, 5))), "the caret stays at the end");
    t.type_text("!");
    assert_eq!(text(&t), "reset!");
}

#[test]
fn a_field_without_a_handler_keeps_what_is_typed_across_rebuilds() {
    let free = |value: &'static str| column::<Msg>().padding(20.0).child(text_field(value).key("f").width(240.0));
    let mut t = TestUi::<Msg>::new(320.0, 100.0);
    t.view(free("init"));
    t.click_key("f").type_text("xyz");
    let value = |t: &mut TestUi<Msg>| node_of(t, "f").value;
    assert_eq!(value(&mut t).as_deref(), Some("initxyz"));
    t.view(free("init"));
    assert_eq!(value(&mut t).as_deref(), Some("initxyz"), "the same value from the app changes nothing");
    t.view(free("new"));
    assert_eq!(value(&mut t).as_deref(), Some("new"), "a different one replaces the text");
}

#[test]
fn a_read_only_field_selects_and_copies_but_does_not_change() {
    let mut t = window_with(locked, "locked", false);
    let clip = clipboard(&mut t);
    t.click_key("f").type_text("x").key(NamedKey::Backspace).key(NamedKey::Delete).shortcut('v');
    assert_eq!(text(&t), "locked");
    t.shortcut('a').shortcut('c');
    assert_eq!(clip.get().as_deref(), Some("locked"));
    assert_eq!(selection(&mut t), (0, 6));
}
