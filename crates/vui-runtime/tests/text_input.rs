//! A custom text field written against the public `Widget` trait: focus, keys, the input method,
//! the clipboard and a blinking caret.

mod common;

use common::*;
use vui_core::{Event, ImeEvent, Key as KeyCode, NamedKey, Size, Tone, pt, rect, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::testing::{TestUi, quads};
use vui_runtime::{
    AccessInfo, Base, Builder, Effect, Env, EventCx, Focus, PaintCx, Parent, Role, TickCx, Widget, WidgetEvent, builder,
    column,
};


// ---- a text field: focus, keys, input method, clipboard, caret blink --------------------------

struct Field;

#[derive(Default)]
struct FieldState {
    caret_on: bool,
    next_blink: f32,
}

impl Widget<Msg> for Field {
    type State = FieldState;

    fn create_state(&self) -> FieldState {
        FieldState::default()
    }

    fn measure(&self, _: &mut FieldState, _: &Env, _: Pair<Option<f32>>, _: Pair<Avail>) -> Size {
        size(100.0, 24.0)
    }

    fn focusable(&self) -> Focus {
        Focus::Click
    }

    fn event(&self, state: &mut FieldState, cx: &mut EventCx<Msg>, event: &WidgetEvent) -> bool {
        match event {
            WidgetEvent::FocusChanged { focused: true, .. } => {
                state.caret_on = true;
                state.next_blink = cx.time + 0.5;
                cx.request_wake(0.5);
                cx.set_ime_area(Some(rect(cx.content.x, cx.content.y, 1.0, cx.content.h)));
                cx.request_paint();
                false
            }
            WidgetEvent::Input(Event::Key(k)) if k.pressed => match (&k.key, k.mods.cmd) {
                (KeyCode::Character('v'), true) => {
                    if let Some(text) = cx.clipboard_text() {
                        cx.emit(Msg::Text(text));
                    }
                    true
                }
                (KeyCode::Character('c'), true) => {
                    cx.set_clipboard_text("copied");
                    true
                }
                _ => match &k.text {
                    Some(text) => {
                        cx.emit(Msg::Text(text.clone()));
                        true
                    }
                    None => false,
                },
            },
            WidgetEvent::Input(Event::Ime(ImeEvent::Commit(text))) => {
                cx.emit(Msg::Text(text.clone()));
                true
            }
            _ => false,
        }
    }

    fn tick(&self, state: &mut FieldState, cx: &mut TickCx, _dt: f32) -> bool {
        if !cx.focused {
            return false;
        }
        if cx.time >= state.next_blink {
            state.caret_on = !state.caret_on;
            state.next_blink = cx.time + 0.5;
            cx.request_paint();
        }
        cx.request_wake((state.next_blink - cx.time).max(0.0));
        false
    }

    fn paint(&self, state: &FieldState, cx: &mut PaintCx) {
        if state.caret_on && cx.focused && cx.window_focused {
            let c = cx.content;
            cx.fill_rect(rect(c.x, c.y, 1.5, c.h), Tone::Accent);
        }
    }

    fn access(&self, _: &FieldState, info: &mut AccessInfo) {
        info.role = Some(Role::TextInput);
    }
}

struct FieldEl<M: 'static> {
    base: Base<M>,
    widget: Field,
}
builder!(FieldEl<M> => Field);

fn field() -> FieldEl<Msg> {
    FieldEl { base: Base::new(), widget: Field }
}

fn caret_count(t: &mut TestUi<Msg>) -> usize {
    let scene = t.scene();
    quads(&scene).iter().filter(|q| q.rect.w == 1.5).count()
}

/// The last place the input method was told about, if it was told at all.
fn ime_area(effects: &[Effect]) -> Option<Option<vui_core::Rect>> {
    effects.iter().rev().find_map(|e| match e {
        Effect::ImeArea(area) => Some(*area),
        _ => None,
    })
}

#[test]
fn clicking_a_field_focuses_it_and_tells_the_input_method_where_the_caret_is() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    t.effects();
    t.click_key("f");
    assert!(t.inspect("f").focused);
    assert!(!t.inspect("f").focus_visible, "the mouse put it there");
    assert_eq!(ime_area(&t.effects()), Some(Some(rect(0.0, 0.0, 1.0, 24.0))));

    t.click(pt(200.0, 90.0));
    assert_eq!(ime_area(&t.effects()), Some(None), "and the input method stands down when focus goes");
}

#[test]
fn typed_text_and_input_method_text_reach_the_focused_field() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    t.type_text("a b");
    assert!(t.messages().is_empty(), "nothing has focus yet");
    t.click_key("f");
    t.type_text("a b");
    assert_eq!(t.messages(), [Msg::Text("a".into()), Msg::Text(" ".into()), Msg::Text("b".into())]);
    t.ime(ImeEvent::Commit("日本".into()));
    assert_eq!(t.messages(), [Msg::Text("日本".into())]);
    t.ime(ImeEvent::Preedit { text: "に".into(), cursor: None });
    assert!(t.messages().is_empty());
}

#[test]
fn a_key_the_focused_widget_uses_does_not_also_activate_the_element() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f").on_click(Msg::Click("f"))));
    t.click_key("f");
    t.messages();
    t.type_text(" ");
    assert_eq!(t.messages(), [Msg::Text(" ".into())], "Space was text for the field, not a press");
    t.key(NamedKey::Enter);
    assert_eq!(t.messages(), [Msg::Click("f")], "the field has no use for Enter, so it presses the element");
}

#[test]
fn a_field_uses_the_clipboard() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    t.click_key("f");
    t.shortcut('v');
    assert!(t.messages().is_empty(), "nothing has been copied yet");
    t.shortcut('c');
    t.shortcut('v');
    assert_eq!(t.messages(), [Msg::Text("copied".into())]);
}

#[test]
fn a_blinking_caret_asks_to_be_woken_instead_of_running_frames() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    assert_eq!(t.ui.next_wake(), None);

    t.click_key("f");
    t.settle();
    assert!(!t.ui.needs_frame(), "nothing is animating");
    assert!(t.ui.next_wake().is_some(), "but the caret wants to blink");

    // A backend that sleeps until it is needed, and wakes a hair late.
    let mut seen = Vec::new();
    for _ in 0..6 {
        let wait = t.ui.next_wake().expect("it keeps asking");
        t.ui.tick(wait + 0.001);
        assert!(!t.ui.needs_frame());
        seen.push(caret_count(&mut t));
    }
    assert!(seen.windows(2).all(|w| w[0] != w[1]), "on, off, on, off: {seen:?}");
}

#[test]
fn the_caret_hides_when_the_window_is_not_active() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    t.click_key("f");
    assert_eq!(caret_count(&mut t), 1);
    t.window_focus(false);
    assert_eq!(caret_count(&mut t), 0);
    t.window_focus(true);
    assert_eq!(caret_count(&mut t), 1);
}

#[test]
fn the_blink_stops_when_the_field_loses_focus() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f")));
    t.click_key("f");
    t.click(pt(200.0, 90.0));
    t.advance(1.5);
    assert_eq!(t.ui.next_wake(), None);
    assert_eq!(caret_count(&mut t), 0);
}

// ---- what a widget tells assistive technology --------------------------------------------------

#[test]
fn what_a_widget_reports_to_assistive_technology_ends_up_in_the_tree() {
    let mut t = ui(300.0, 100.0);
    t.view(column().child(field().key("f").name("Search")));
    let tree = t.ui.access_tree();
    let node = tree.find("Search").expect("named by the element");
    assert_eq!(node.role, Role::TextInput, "the widget said what it is");
    assert!(node.focusable);
}
