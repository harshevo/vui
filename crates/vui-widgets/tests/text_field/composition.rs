//! Input methods: composing text inside the field.

use super::*;

#[test]
fn a_composition_shows_inside_the_text_and_commits_into_it() {
    let mut t = focused(plain);
    t.type_text("ab").key(NamedKey::ArrowLeft);
    t.ime(ImeEvent::Preedit { text: "か".into(), cursor: Some((0, 3)) });
    assert!(drawn(&mut t).contains(&"aかb".to_string()));
    assert_eq!(text(&t), "ab", "the app hears nothing until it is settled");
    t.ime(ImeEvent::Commit("漢".into()));
    assert_eq!(text(&t), "a漢b");
    assert!(drawn(&mut t).contains(&"a漢b".to_string()));
    t.ime(ImeEvent::Preedit { text: "x".into(), cursor: None }).ime(ImeEvent::Cancel);
    assert!(drawn(&mut t).contains(&"a漢b".to_string()), "cancelling leaves the text as it was");
}

#[test]
fn the_composition_is_underlined() {
    let mut t = focused(plain);
    let text_color = Theme::light().palette.text;
    let underlines = |t: &mut TestUi<Msg>| quads(&t.scene()).into_iter().filter(|q| q.rect.h == 1.0 && q.fill == text_color).count();
    assert_eq!(underlines(&mut t), 0);
    t.ime(ImeEvent::Preedit { text: "かな".into(), cursor: None });
    assert_eq!(underlines(&mut t), 1);
}

#[test]
fn focus_leaving_keeps_the_text_being_composed() {
    let mut t = focused(plain);
    t.type_text("a").ime(ImeEvent::Preedit { text: "か".into(), cursor: None });
    t.click(pt(300.0, 200.0));
    assert_eq!(text(&t), "aか");
}

#[test]
fn the_input_method_is_told_where_the_caret_is() {
    let mut t = window(plain);
    t.click_key("f");
    let area_of = |t: &mut TestUi<Msg>| {
        t.effects().into_iter().rev().find_map(|e| match e {
            Effect::ImeArea(Some(r)) => Some(r),
            _ => None,
        })
    };
    let first = area_of(&mut t).expect("the field says where it is as soon as it has focus");
    let field = t.rect("f");
    assert!(field.contains(first.origin()) && first.bottom() <= field.bottom(), "{first:?} in {field:?}");
    t.type_text("abc");
    let later = area_of(&mut t).expect("and again after typing");
    assert!(later.x > first.x + 10.0, "{later:?} is after {first:?}");
    t.click(pt(300.0, 200.0));
    assert!(t.effects().iter().all(|e| !matches!(e, Effect::ImeArea(Some(_)))), "nothing once focus has left");
}
