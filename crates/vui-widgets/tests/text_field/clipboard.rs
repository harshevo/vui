//! The clipboard, pictures on it, and undo.

use super::*;

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
