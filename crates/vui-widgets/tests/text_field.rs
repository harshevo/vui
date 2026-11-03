//! Text fields driven the way a person drives them: clicking, typing, dragging, shortcuts, an input
//! method, assistive technology. The app is a small form so the whole loop runs: the field sends its
//! text, the app keeps it, the next view hands it back.

use std::rc::Rc;

use vui_core::{Command, Event, Fonts, ImeEvent, Key as KeyCode, KeyEvent, Modifiers, NamedKey, PointerButton, Quad, Rect, Scene, TextCmd, TextStyle, TextWrap, Theme, pt};
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{AccessAction, AccessActionKind, AccessNode, App, Builder, Clipboard, Cx, Effect, IntoElement, MemoryClipboard, Parent, Role, Styled, column};
use vui_widgets::{TextField, password_field, search_field, text_area, text_field};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Text(String),
    Submit(String),
    Key(&'static str),
    Picture(Vec<u8>),
}

type Make = fn(&str) -> TextField<Msg>;

struct Form {
    text: String,
    submitted: Vec<String>,
    keys: Vec<&'static str>,
    pictures: Vec<Vec<u8>>,
    make: Make,
    digits_only: bool,
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        match msg {
            Msg::Text(t) => {
                if !self.digits_only || t.chars().all(|c| c.is_ascii_digit()) {
                    self.text = t;
                }
            }
            Msg::Submit(t) => self.submitted.push(t),
            Msg::Key(k) => self.keys.push(k),
            Msg::Picture(p) => self.pictures.push(p),
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column()
            .padding(20.0)
            .gap(12.0)
            .on_key(|k| match k.key {
                KeyCode::Named(NamedKey::Escape) => Some(Msg::Key("escape")),
                KeyCode::Character('k') if k.mods.cmd => Some(Msg::Key("cmd-k")),
                _ => None,
            })
            .children((
                (self.make)(&self.text).key("f").width(240.0).on_change(Msg::Text).on_submit(Msg::Submit).on_paste_image(Msg::Picture),
                text_field("second").key("g").width(240.0),
            ))
    }
}

fn plain(t: &str) -> TextField<Msg> {
    text_field(t).placeholder("Name")
}

fn area(t: &str) -> TextField<Msg> {
    text_area(t).placeholder("Notes")
}

fn secret(t: &str) -> TextField<Msg> {
    password_field(t).placeholder("Password")
}

fn find(t: &str) -> TextField<Msg> {
    search_field(t)
}

fn limited(t: &str) -> TextField<Msg> {
    text_field(t).max_chars(5).placeholder("Name")
}

fn locked(t: &str) -> TextField<Msg> {
    text_field(t).read_only(true).placeholder("Name")
}

fn grows(t: &str) -> TextField<Msg> {
    text_area(t).rows(2).max_rows(4)
}

fn select_all_on_focus(t: &str) -> TextField<Msg> {
    text_field(t).select_on_focus()
}

fn window_with(make: Make, text: &str, digits_only: bool) -> TestUi<Msg> {
    let form = Form { text: text.into(), submitted: Vec::new(), keys: Vec::new(), pictures: Vec::new(), make, digits_only };
    TestUi::with_app(320.0, 220.0, form)
}

fn window(make: Make) -> TestUi<Msg> {
    window_with(make, "", false)
}

/// A window whose field has been clicked.
fn focused(make: Make) -> TestUi<Msg> {
    let mut t = window(make);
    t.click_key("f");
    t
}

fn form(t: &TestUi<Msg>) -> &Form {
    t.app::<Form>()
}

fn text(t: &TestUi<Msg>) -> String {
    form(t).text.clone()
}

/// The first field as assistive technology sees it.
fn node(t: &mut TestUi<Msg>) -> AccessNode {
    let bounds = t.rect("f");
    t.ui.access_tree().nodes.into_iter().find(|n| n.bounds == bounds).expect("the field is in the tree")
}

fn selection(t: &mut TestUi<Msg>) -> (usize, usize) {
    node(t).selection.expect("a text field reports its selection")
}

thread_local! {
    static FONTS: Fonts = Fonts::bundled_only();
}

/// Where the text starts inside a field 240 points wide at (20, 20).
const INNER_X: f32 = 32.0;
const MID_Y: f32 = 36.0;

/// The x of the edge before `byte` in a single line of `text`.
fn x_of(text: &str, byte: usize) -> f32 {
    FONTS.with(|fonts| {
        let mut style = TextStyle::from_token(Theme::light().types.body);
        style.wrap = TextWrap::None;
        INNER_X + fonts.layout(text, &style, None).caret_rect(byte).x
    })
}

fn at_byte(text: &str, byte: usize) -> vui_core::Point {
    pt(x_of(text, byte), MID_Y)
}

fn chord(t: &mut TestUi<Msg>, c: char, mods: Modifiers) {
    t.ui.handle(Event::Key(KeyEvent::press(KeyCode::Character(c), mods)));
}

fn with(base: Modifiers, extra: Modifiers) -> Modifiers {
    Modifiers { shift: base.shift || extra.shift, ctrl: base.ctrl || extra.ctrl, alt: base.alt || extra.alt, cmd: base.cmd || extra.cmd }
}

fn drawn(t: &mut TestUi<Msg>) -> Vec<String> {
    texts(&t.scene())
}

fn text_cmd<'a>(scene: &'a Scene, text: &str) -> &'a TextCmd {
    scene
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text(t) if t.layout.text() == text => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{text:?} is not drawn"))
}

fn caret(scene: &Scene) -> Option<Rect> {
    let accent = Theme::light().palette.accent;
    quads(scene).into_iter().find(|q: &&Quad| q.rect.w == 1.0 && q.fill == accent).map(|q| q.rect)
}

fn clipboard(t: &mut TestUi<Msg>) -> Rc<MemoryClipboard> {
    let clip = Rc::new(MemoryClipboard::default());
    t.ui.set_clipboard(clip.clone());
    clip
}

// ---- typing -------------------------------------------------------------------------------------

#[test]
fn typing_sends_the_new_text_and_the_app_hands_it_back() {
    let mut t = focused(plain);
    t.type_text("hello");
    assert_eq!(text(&t), "hello");
    let n = node(&mut t);
    assert_eq!((n.role, n.value.as_deref(), n.selection), (Role::TextInput, Some("hello"), Some((5, 5))));
}

#[test]
fn the_placeholder_shows_only_while_the_field_is_empty() {
    let mut t = window(plain);
    assert!(drawn(&mut t).contains(&"Name".to_string()));
    t.click_key("f");
    assert!(drawn(&mut t).contains(&"Name".to_string()), "focus alone does not hide it");
    t.type_text("a");
    let shown = drawn(&mut t);
    assert!(shown.contains(&"a".to_string()) && !shown.contains(&"Name".to_string()), "{shown:?}");
}

#[test]
fn typing_replaces_the_selection() {
    let mut t = focused(plain);
    t.type_text("hello world").shortcut('a');
    assert_eq!(selection(&mut t), (0, 11));
    t.type_text("x");
    assert_eq!(text(&t), "x");
}

#[test]
fn a_space_is_typed_and_activates_nothing() {
    let mut t = focused(plain);
    t.type_text("a b");
    assert_eq!(text(&t), "a b");
    assert!(form(&t).submitted.is_empty());
}

#[test]
fn return_submits_a_single_line() {
    let mut t = focused(plain);
    t.type_text("hi").key(NamedKey::Enter);
    assert_eq!(form(&t).submitted, ["hi"]);
    assert_eq!(text(&t), "hi", "and leaves the text alone");
}

#[test]
fn return_adds_a_line_in_a_text_area_and_command_return_submits() {
    let mut t = focused(area);
    t.type_text("a").key(NamedKey::Enter).type_text("b");
    assert_eq!(text(&t), "a\nb");
    assert!(form(&t).submitted.is_empty());
    t.key_with(NamedKey::Enter, Modifiers::CMD);
    assert_eq!(form(&t).submitted, ["a\nb"]);
}

#[test]
fn the_character_limit_holds_for_typing_and_pasting() {
    let mut t = focused(limited);
    let clip = clipboard(&mut t);
    t.type_text("abcdefgh");
    assert_eq!(text(&t), "abcde");
    clip.set("xyz");
    t.shortcut('v');
    assert_eq!(text(&t), "abcde");
    t.key(NamedKey::Backspace).key(NamedKey::Backspace).shortcut('v');
    assert_eq!(text(&t), "abcxy", "pasting fills what room there is");
}

// ---- moving and deleting ------------------------------------------------------------------------

#[test]
fn arrows_home_and_end_move_the_caret_and_shift_extends() {
    let mut t = focused(plain);
    t.type_text("abc").key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (2, 2));
    t.key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (1, 1));
    t.key_with(NamedKey::ArrowRight, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (1, 2));
    t.key(NamedKey::Home);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::End);
    assert_eq!(selection(&mut t), (3, 3));
    t.key_with(NamedKey::Home, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (3, 0));
}

#[test]
fn a_plain_arrow_collapses_a_selection_to_its_edge() {
    let mut t = focused(plain);
    t.type_text("abcd").key_with(NamedKey::ArrowLeft, Modifiers::SHIFT).key_with(NamedKey::ArrowLeft, Modifiers::SHIFT);
    assert_eq!(selection(&mut t), (4, 2));
    t.key(NamedKey::ArrowLeft);
    assert_eq!(selection(&mut t), (2, 2));
}

#[test]
fn backspace_and_delete_take_a_letter_each_way() {
    let mut t = focused(plain);
    t.type_text("abcd").key(NamedKey::ArrowLeft).key(NamedKey::Backspace);
    assert_eq!((text(&t).as_str(), selection(&mut t)), ("abd", (2, 2)));
    t.key(NamedKey::Delete);
    assert_eq!(text(&t), "ab");
    t.key(NamedKey::End).key(NamedKey::Delete);
    assert_eq!(text(&t), "ab", "nothing after the end");
}

#[test]
fn option_moves_and_deletes_by_word() {
    let mut t = focused(plain);
    t.type_text("one two three");
    t.key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (8, 8));
    t.key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (4, 4));
    t.key_with(NamedKey::ArrowRight, Modifiers::ALT);
    assert_eq!(selection(&mut t), (7, 7));
    t.key_with(NamedKey::Backspace, Modifiers::ALT);
    assert_eq!(text(&t), "one  three");
    t.key_with(NamedKey::Delete, Modifiers::ALT);
    assert_eq!(text(&t), "one ");
}

#[test]
fn command_arrows_and_delete_go_to_the_ends_of_the_line() {
    let mut t = focused(plain);
    t.type_text("hello world").key_with(NamedKey::ArrowLeft, Modifiers::CMD);
    assert_eq!(selection(&mut t), (0, 0));
    t.key_with(NamedKey::ArrowRight, Modifiers::CMD);
    assert_eq!(selection(&mut t), (11, 11));
    t.key_with(NamedKey::Backspace, Modifiers::CMD);
    assert_eq!(text(&t), "");
    t.type_text("ab").key(NamedKey::ArrowLeft).key_with(NamedKey::Delete, Modifiers::CMD);
    assert_eq!(text(&t), "a");
}

#[test]
fn control_keys_edit_the_way_macos_text_fields_do() {
    let mut t = focused(plain);
    t.type_text("hello");
    chord(&mut t, 'a', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (0, 0));
    chord(&mut t, 'e', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (5, 5));
    chord(&mut t, 'b', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (4, 4));
    chord(&mut t, 'f', Modifiers::CTRL);
    assert_eq!(selection(&mut t), (5, 5));
    chord(&mut t, 'h', Modifiers::CTRL);
    assert_eq!(text(&t), "hell");
    chord(&mut t, 'a', Modifiers::CTRL);
    chord(&mut t, 'd', Modifiers::CTRL);
    assert_eq!(text(&t), "ell");
    chord(&mut t, 'k', Modifiers::CTRL);
    assert_eq!(text(&t), "");
}

#[test]
fn up_and_down_in_one_line_go_to_its_ends() {
    let mut t = focused(plain);
    t.type_text("abc").key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::ArrowDown);
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn keys_the_field_does_not_use_reach_the_elements_around_it() {
    let mut t = focused(plain);
    t.key(NamedKey::Escape).shortcut('k');
    assert_eq!(form(&t).keys, ["escape", "cmd-k"]);
    t.type_text("k");
    assert_eq!(form(&t).keys.len(), 2, "typing never gets past the field");
}

// ---- the mouse ----------------------------------------------------------------------------------

#[test]
fn clicking_places_the_caret_between_letters() {
    let mut t = focused(plain);
    t.type_text("hello");
    t.click(at_byte("hello", 2));
    assert_eq!(selection(&mut t), (2, 2));
    t.click(pt(240.0, MID_Y));
    assert_eq!(selection(&mut t), (5, 5), "past the end of the text is the end");
    t.click(pt(21.0, MID_Y));
    assert_eq!(selection(&mut t), (0, 0), "and in the padding before it, the start");
}

#[test]
fn clicking_into_a_field_does_not_select_what_is_in_it() {
    let mut t = window_with(plain, "hello", false);
    t.click(at_byte("hello", 3));
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn dragging_selects_either_way() {
    let mut t = focused(plain);
    let s = "hello world";
    t.type_text(s);
    t.press(at_byte(s, 2)).move_to(at_byte(s, 8)).release(at_byte(s, 8));
    assert_eq!(selection(&mut t), (2, 8));
    t.press(at_byte(s, 8)).move_to(at_byte(s, 3)).release(at_byte(s, 3));
    assert_eq!(selection(&mut t), (8, 3));
}

#[test]
fn dragging_out_of_the_field_keeps_selecting() {
    let mut t = focused(plain);
    t.type_text("hello world");
    t.press(at_byte("hello world", 2)).move_to(pt(300.0, 150.0));
    assert_eq!(selection(&mut t), (2, 11), "the pointer is far away, and the end is as far as the text goes");
    t.move_to(pt(0.0, 0.0)).release(pt(0.0, 0.0));
    assert_eq!(selection(&mut t), (2, 0));
}

#[test]
fn shift_click_extends_the_selection() {
    let mut t = focused(plain);
    let s = "hello world";
    t.type_text(s).click(at_byte(s, 2));
    let p = at_byte(s, 7);
    t.ui.handle(Event::PointerDown { pos: p, button: PointerButton::Primary, mods: Modifiers::SHIFT, click_count: 1 });
    t.ui.handle(Event::PointerUp { pos: p, button: PointerButton::Primary, mods: Modifiers::SHIFT });
    assert_eq!(selection(&mut t), (2, 7));
}

#[test]
fn double_click_selects_a_word_and_triple_click_everything() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    t.double_click(at_byte(s, 7));
    assert_eq!(selection(&mut t), (6, 9));
    let p = at_byte(s, 7);
    t.press_button(p, PointerButton::Primary, 3).release(p);
    assert_eq!(selection(&mut t), (0, 15));
}

#[test]
fn double_click_on_the_right_half_of_a_words_last_letter_picks_that_word() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    let p = pt(x_of(s, 5) - 1.0, MID_Y);
    t.double_click(p);
    assert_eq!(selection(&mut t), (0, 5));
}

#[test]
fn dragging_after_a_double_click_selects_whole_words() {
    let mut t = focused(plain);
    let s = "hello big world";
    t.type_text(s);
    let p = at_byte(s, 7);
    t.press_button(p, PointerButton::Primary, 1).release(p).press_button(p, PointerButton::Primary, 2);
    assert_eq!(selection(&mut t), (6, 9));
    let q = at_byte(s, 13);
    t.move_to(q).release(q);
    assert_eq!(selection(&mut t), (6, 15), "into the middle of world takes all of it");
    t.press_button(p, PointerButton::Primary, 1).release(p).press_button(p, PointerButton::Primary, 2);
    let r = at_byte(s, 2);
    t.move_to(r).release(r);
    assert_eq!(selection(&mut t), (9, 0), "and backwards the same way");
}

#[test]
fn the_pointer_over_a_field_is_an_i_beam() {
    let mut t = window(plain);
    t.move_to(at_byte("", 0));
    assert!(t.effects().contains(&Effect::Cursor(vui_core::CursorIcon::Text)));
}

// ---- clipboard and history ----------------------------------------------------------------------

#[test]
fn copy_cut_and_paste_use_the_clipboard() {
    let mut t = focused(plain);
    let clip = clipboard(&mut t);
    t.type_text("hello world").key(NamedKey::Home);
    for _ in 0..5 {
        t.key_with(NamedKey::ArrowRight, Modifiers::SHIFT);
    }
    t.shortcut('c');
    assert_eq!((clip.get().as_deref(), text(&t).as_str()), (Some("hello"), "hello world"));
    t.key(NamedKey::End).shortcut('v');
    assert_eq!(text(&t), "hello worldhello");
    t.shortcut('a').shortcut('x');
    assert_eq!((clip.get().as_deref(), text(&t).as_str()), (Some("hello worldhello"), ""));
}

/// A clipboard holding only a picture, as after a screenshot.
struct PictureOnly;

impl Clipboard for PictureOnly {
    fn get(&self) -> Option<String> {
        None
    }
    fn set(&self, _: &str) {}
    fn image(&self) -> Option<Vec<u8>> {
        Some(b"png".to_vec())
    }
}

#[test]
fn a_picture_on_the_clipboard_is_handed_to_the_app() {
    let mut t = focused(plain);
    t.ui.set_clipboard(Rc::new(PictureOnly));
    t.shortcut('v');
    assert_eq!((form(&t).pictures.clone(), text(&t)), (vec![b"png".to_vec()], String::new()));
    // Text on the clipboard pastes as text, and no picture.
    let clip = clipboard(&mut t);
    clip.set("hi");
    t.shortcut('v');
    assert_eq!((form(&t).pictures.len(), text(&t)), (1, "hi".to_string()));
}

#[test]
fn pasting_into_a_single_line_flattens_line_breaks() {
    let mut t = focused(plain);
    let clip = clipboard(&mut t);
    clip.set("a\nb\r\nc");
    t.shortcut('v');
    assert_eq!(text(&t), "a b c");
}

#[test]
fn pasting_into_a_text_area_keeps_line_breaks() {
    let mut t = focused(area);
    let clip = clipboard(&mut t);
    clip.set("a\r\nb\nc");
    t.shortcut('v');
    assert_eq!(text(&t), "a\nb\nc");
}

#[test]
fn undo_and_redo_step_through_typing() {
    let mut t = focused(plain);
    t.type_text("abc").shortcut('z');
    assert_eq!(text(&t), "", "a run of typing is one step");
    t.ui.handle(Event::Key(KeyEvent::shortcut('z').with_mods(with(Modifiers::CMD, Modifiers::SHIFT))));
    assert_eq!(text(&t), "abc");
    t.type_text("d").key(NamedKey::ArrowLeft).type_text("X");
    assert_eq!(text(&t), "abcXd");
    t.shortcut('z');
    assert_eq!(text(&t), "abcd", "moving the caret ended the run");
}

// ---- focus --------------------------------------------------------------------------------------

#[test]
fn tabbing_into_a_line_selects_it_and_tabbing_out_leaves_the_field() {
    let mut t = focused(plain);
    t.type_text("x").tab();
    assert_eq!(t.ui.focused_key(), Some("g".into()));
    let bounds = t.rect("g");
    let g = t.ui.access_tree().nodes.into_iter().find(|n| n.bounds == bounds).unwrap();
    assert_eq!(g.selection, Some((0, 6)), "all of `second`, ready to be typed over");
    t.shift_tab();
    assert_eq!(t.ui.focused_key(), Some("f".into()));
    assert_eq!(selection(&mut t), (0, 1));
}

#[test]
fn tabbing_into_a_text_area_keeps_the_caret_where_it_was() {
    let mut t = focused(area);
    t.type_text("abc").tab().shift_tab();
    assert_eq!(selection(&mut t), (3, 3));
}

#[test]
fn a_field_can_ask_to_be_selected_whenever_focus_comes_by_keyboard_or_code() {
    let mut t = window_with(select_all_on_focus, "hello", false);
    t.ui.focus_key("f");
    assert_eq!(selection(&mut t), (0, 5));
    t.click(at_byte("hello", 3));
    assert_eq!(selection(&mut t), (3, 3), "a click still places the caret");
}

#[test]
fn a_disabled_field_ignores_the_pointer_and_the_keys() {
    let mut t = TestUi::<Msg>::new(320.0, 220.0);
    t.view(column().padding(20.0).child(text_field::<Msg>("x").key("f").width(240.0).on_change(Msg::Text).disabled(true)));
    t.click_key("f").type_text("abc");
    assert_eq!(t.ui.focused_key(), None);
    assert!(t.messages().is_empty());
    let n = node_of(&mut t, "f");
    assert!(n.disabled && !n.focusable && n.actions.is_empty());
}

fn node_of(t: &mut TestUi<Msg>, key: &str) -> AccessNode {
    let bounds = t.rect(key);
    t.ui.access_tree().nodes.into_iter().find(|n| n.bounds == bounds).expect("the field is in the tree")
}

// ---- the caret and the border -------------------------------------------------------------------

#[test]
fn the_caret_blinks_and_typing_brings_it_back() {
    let mut t = focused(plain);
    assert!(caret(&t.scene()).is_some());
    t.advance(0.6);
    assert!(caret(&t.scene()).is_none(), "off for the second half second");
    t.advance(0.5);
    assert!(caret(&t.scene()).is_some(), "and on again");
    t.advance(0.5);
    assert!(caret(&t.scene()).is_none());
    t.type_text("a");
    assert!(caret(&t.scene()).is_some(), "typing shows it at once");
}

#[test]
fn the_caret_sits_after_the_last_letter() {
    let mut t = focused(plain);
    t.type_text("hello");
    let c = caret(&t.scene()).expect("a caret");
    assert!((c.x - x_of("hello", 5)).abs() <= 0.5, "{} vs {}", c.x, x_of("hello", 5));
    assert!(c.y >= t.rect("f").y && c.bottom() <= t.rect("f").bottom());
}

#[test]
fn a_selection_replaces_the_caret_and_a_read_only_field_has_none() {
    let mut t = focused(plain);
    t.type_text("hello").shortcut('a');
    assert!(caret(&t.scene()).is_none());
    let mut r = window_with(locked, "locked", false);
    r.click_key("f");
    assert!(caret(&r.scene()).is_none());
}

#[test]
fn the_caret_hides_while_the_window_is_not_active() {
    let mut t = focused(plain);
    t.window_focus(false);
    assert!(caret(&t.scene()).is_none());
    t.window_focus(true);
    assert!(caret(&t.scene()).is_some());
}

#[test]
fn the_border_goes_over_to_the_accent_with_focus() {
    let theme = Theme::light();
    let border_of = |t: &mut TestUi<Msg>| {
        let field = t.rect("f");
        let scene = t.scene();
        quads(&scene).into_iter().find(|q| q.rect == field && q.border.is_some()).and_then(|q| q.border).expect("a border").color
    };
    let near = |a: vui_core::Color, b: vui_core::Color| a.to_array().iter().zip(b.to_array()).all(|(x, y)| (x - y).abs() < 1e-3);
    let mut t = window(plain);
    assert!(near(border_of(&mut t), theme.palette.line));
    t.click_key("f").settle();
    assert!(near(border_of(&mut t), theme.palette.accent));
    t.click(pt(300.0, 200.0)).settle();
    assert!(near(border_of(&mut t), theme.palette.line), "it fades back when focus goes");
}

// ---- scrolling ----------------------------------------------------------------------------------

#[test]
fn a_line_scrolls_to_keep_the_caret_in_view() {
    let mut t = focused(plain);
    let long = "w".repeat(40);
    t.type_text(&long);
    let scene = t.scene();
    let drawn = text_cmd(&scene, &long);
    let right = drawn.origin.x + drawn.layout.size().w;
    assert!(drawn.origin.x < INNER_X, "scrolled left");
    assert!((right - 247.0).abs() < 1.0, "the end of the text is at the right edge: {right}");
    assert!(caret(&scene).is_some_and(|c| c.x >= INNER_X && c.right() <= 248.0));
    t.key(NamedKey::Home);
    let scene = t.scene();
    assert!((text_cmd(&scene, &long).origin.x - INNER_X).abs() < 0.01, "back to the start");
}

#[test]
fn moving_the_caret_left_scrolls_it_back_into_view() {
    let mut t = focused(plain);
    let long = format!("{}{}", "a".repeat(10), "w".repeat(40));
    t.type_text(&long);
    for _ in 0..45 {
        t.key(NamedKey::ArrowLeft);
    }
    let scene = t.scene();
    let c = caret(&scene).expect("a caret");
    assert!(c.x >= INNER_X - 0.5 && c.x <= 248.0, "{}", c.x);
}

#[test]
fn a_text_area_wraps_and_scrolls_with_the_wheel() {
    let mut t = focused(area);
    for line in 0..8 {
        t.type_text(&format!("line {line}")).key(NamedKey::Enter);
    }
    let all: String = (0..8).map(|i| format!("line {i}\n")).collect();
    let scene = t.scene();
    let end = text_cmd(&scene, &all).origin.y;
    assert!(end < 28.0, "scrolled up to show the caret on the last line: {end}");
    t.wheel(pt(100.0, 60.0), 0.0, 1000.0);
    let scene = t.scene();
    assert_eq!(text_cmd(&scene, &all).origin.y, 28.0, "the wheel can scroll it back to the top");
    t.wheel(pt(100.0, 60.0), 0.0, -30.0);
    let scene = t.scene();
    assert_eq!(text_cmd(&scene, &all).origin.y, -2.0);
}

#[test]
fn a_line_does_not_use_a_vertical_wheel() {
    let mut t = focused(plain);
    t.type_text(&"w".repeat(40));
    let before = text_cmd(&t.scene(), &"w".repeat(40)).origin.x;
    t.wheel(pt(100.0, MID_Y), 0.0, 40.0);
    assert_eq!(text_cmd(&t.scene(), &"w".repeat(40)).origin.x, before);
    t.trackpad(pt(100.0, MID_Y), 30.0, 0.0);
    assert!(text_cmd(&t.scene(), &"w".repeat(40)).origin.x > before, "but a sideways swipe moves it");
}

#[test]
fn a_text_area_moves_between_lines_and_keeps_its_column() {
    let mut t = focused(area);
    t.type_text("mm").key(NamedKey::Enter).type_text("mm").key(NamedKey::Enter).type_text("mm");
    assert_eq!(selection(&mut t), (8, 8));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (5, 5));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (2, 2));
    t.key(NamedKey::ArrowUp);
    assert_eq!(selection(&mut t), (0, 0));
    t.key(NamedKey::ArrowDown).key(NamedKey::ArrowDown);
    assert_eq!(selection(&mut t), (8, 8), "the column it left is the column it comes back to");
    t.key_with(NamedKey::ArrowUp, Modifiers::ALT);
    assert_eq!(selection(&mut t), (6, 6), "option goes to the start of the paragraph");
    t.key_with(NamedKey::ArrowUp, Modifiers::CMD);
    assert_eq!(selection(&mut t), (0, 0));
}

#[test]
fn a_text_area_grows_with_its_text_up_to_its_limit() {
    let mut t = focused(grows);
    let height = |t: &mut TestUi<Msg>| t.rect("f").h;
    assert_eq!(height(&mut t), 58.0, "two lines and the padding");
    t.type_text("a").key(NamedKey::Enter).type_text("b").key(NamedKey::Enter).type_text("c");
    assert_eq!(height(&mut t), 79.0);
    for _ in 0..4 {
        t.key(NamedKey::Enter).type_text("x");
    }
    assert_eq!(height(&mut t), 100.0, "four lines at most");
}

// ---- input methods ------------------------------------------------------------------------------

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

// ---- accessibility ------------------------------------------------------------------------------

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

// ---- passwords ----------------------------------------------------------------------------------

#[test]
fn a_password_field_draws_bullets_and_keeps_the_text_off_the_clipboard() {
    let mut t = focused(secret);
    let clip = clipboard(&mut t);
    t.type_text("secret");
    assert_eq!(text(&t), "secret", "the app gets the real text");
    let shown = drawn(&mut t);
    assert!(shown.contains(&"••••••".to_string()) && !shown.contains(&"secret".to_string()), "{shown:?}");
    t.shortcut('a').shortcut('c').shortcut('x');
    assert_eq!(clip.get(), None);
    assert_eq!(text(&t), "secret", "cut does nothing either");
    let n = node(&mut t);
    assert_eq!((n.value.as_deref(), n.selection), (Some("••••••"), Some((0, 6))));
}

#[test]
fn a_password_field_moves_and_selects_without_showing_where_words_end() {
    let mut t = focused(secret);
    t.type_text("two words").key_with(NamedKey::ArrowLeft, Modifiers::ALT);
    assert_eq!(selection(&mut t), (0, 0), "a word is the whole thing");
    t.double_click(pt(60.0, MID_Y));
    assert_eq!(selection(&mut t), (0, 9));
}

#[test]
fn a_password_caret_and_clicks_line_up_with_the_bullets() {
    let mut t = focused(secret);
    t.type_text("abcd");
    let bullets = "••••";
    let step = (x_of(bullets, bullets.len()) - x_of(bullets, 0)) / 4.0;
    t.click(pt(INNER_X + step * 2.0 + 1.0, MID_Y));
    assert_eq!(selection(&mut t), (2, 2));
    let scene = t.scene();
    let c = caret(&scene).expect("a caret");
    assert!((c.x - (INNER_X + step * 2.0)).abs() < 0.6);
}

// ---- who owns the text --------------------------------------------------------------------------

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
