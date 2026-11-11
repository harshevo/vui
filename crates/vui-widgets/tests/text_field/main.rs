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

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
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

fn node_of(t: &mut TestUi<Msg>, key: &str) -> AccessNode {
    let bounds = t.rect(key);
    t.ui.access_tree().nodes.into_iter().find(|n| n.bounds == bounds).expect("the field is in the tree")
}

mod access;
mod caret;
mod clipboard;
mod composition;
mod editing;
mod focus;
mod mouse;
mod ownership;
mod passwords;
mod scrolling;
mod typing;
