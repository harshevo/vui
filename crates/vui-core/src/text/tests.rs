use super::*;
use crate::geom::pt;

fn body() -> TextStyle {
    TextStyle::new(14.0, 21.0)
}

fn face_family(fonts: &Fonts, layout: &TextLayout) -> (String, u16) {
    let id = layout.buffer().layout_runs().next().unwrap().glyphs[0].font_id;
    let fs = fonts.system();
    let face = fs.db().face(id).unwrap();
    (face.families[0].0.clone(), face.weight.0)
}

#[test]
fn inter_is_the_default_face_and_weights_pick_different_files() {
    let fonts = Fonts::bundled_only();
    let regular = fonts.layout("Hello", &body(), None);
    let medium = fonts.layout("Hello", &body().weight(500), None);
    let semibold = fonts.layout("Hello", &body().weight(600), None);
    let (name, w) = face_family(&fonts, &regular);
    assert!(name.starts_with("Inter"), "{name}");
    assert_eq!(w, 400);
    assert_eq!(face_family(&fonts, &medium).1, 500);
    assert_eq!(face_family(&fonts, &semibold).1, 600);
    assert!(semibold.size().w > regular.size().w, "semibold is wider than regular");
    assert!(medium.size().w > regular.size().w && medium.size().w < semibold.size().w);
}

#[test]
fn mono_uses_jetbrains_mono_with_equal_advances() {
    let fonts = Fonts::bundled_only();
    let a = fonts.layout("iiiiii", &body().mono(), None);
    let b = fonts.layout("WWWWWW", &body().mono(), None);
    assert!(face_family(&fonts, &a).0.contains("JetBrains"), "{:?}", face_family(&fonts, &a));
    assert!((a.size().w - b.size().w).abs() < 0.01);
    let prop_a = fonts.layout("iiiiii", &body(), None);
    let prop_b = fonts.layout("WWWWWW", &body(), None);
    assert!(prop_b.size().w > prop_a.size().w * 2.0, "the sans font is proportional");
}

#[test]
fn height_follows_line_height_and_wrapping_adds_lines() {
    let fonts = Fonts::bundled_only();
    let one = fonts.layout("Hello world", &body(), Some(400.0));
    assert_eq!(one.line_count(), 1);
    assert_eq!(one.size().h, 21.0);
    let wrapped = fonts.layout("Hello world, this sentence is long enough to wrap onto lines", &body(), Some(120.0));
    assert!(wrapped.line_count() >= 3, "{}", wrapped.line_count());
    assert!(wrapped.size().w <= 120.0 + 0.5, "{}", wrapped.size().w);
    assert_eq!(wrapped.size().h, 21.0 * wrapped.line_count() as f32);
}

#[test]
fn no_wrap_keeps_one_line() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("a very long single line of text that would otherwise wrap", &body().no_wrap(), Some(60.0));
    assert_eq!(l.line_count(), 1);
    assert!(l.size().w > 60.0);
}

#[test]
fn min_content_is_the_longest_unbreakable_word() {
    let fonts = Fonts::bundled_only();
    let one_line = |s: &str| fonts.layout(s, &body().no_wrap(), None).size().w;
    let min = fonts.min_content_width("a wonderful little world", &body());
    assert!((min - one_line("wonderful")).abs() < 0.5, "{min} vs {}", one_line("wonderful"));
    assert_eq!(fonts.min_content_width("", &body()), 0.0);
    // ellipsis limits do not make the text narrower than its words
    let capped = fonts.min_content_width("a wonderful little world", &body().max_lines(1));
    assert_eq!(capped, min);
}

#[test]
fn words_only_wrapping_never_cuts_a_word() {
    let fonts = Fonts::bundled_only();
    let mut style = body();
    style.wrap = TextWrap::WordsOnly;
    let long = "abcdefghijklmnopqrstuvwxyz";
    let l = fonts.layout(long, &style, Some(40.0));
    assert_eq!(l.line_count(), 1);
    assert!(l.size().w > 40.0, "the word overflows instead of breaking: {}", l.size().w);
    // the default still breaks a word that cannot fit
    assert!(fonts.layout(long, &body(), Some(40.0)).line_count() > 1);
}

#[test]
fn newlines_start_new_paragraphs() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("one\ntwo\nthree", &body(), None);
    assert_eq!(l.line_count(), 3);
    assert_eq!(l.size().h, 63.0);
}

#[test]
fn max_lines_ellipsizes() {
    let fonts = Fonts::bundled_only();
    let text = "The quick brown fox jumps over the lazy dog and keeps running across the field";
    let full = fonts.layout(text, &body(), Some(140.0));
    let cut = fonts.layout(text, &body().max_lines(2), Some(140.0));
    assert!(full.line_count() > 2);
    assert_eq!(cut.line_count(), 2);
    assert!(cut.size().w <= 140.5);
}

#[test]
fn multi_line_truncation_cuts_at_a_word_and_ends_with_an_ellipsis() {
    let fonts = Fonts::bundled_only();
    let text = "Each permission row ticks when macOS reports the grant, and Continue stays grey until the required ones are done.";
    let cut = fonts.layout(text, &body().max_lines(3), Some(220.0));
    assert_eq!(cut.line_count(), 3);
    let shown = cut.text();
    assert!(shown.ends_with('\u{2026}'), "{shown}");
    let kept = shown.trim_end_matches('\u{2026}');
    assert!(text.starts_with(kept), "the kept text is a prefix of the original: {kept:?}");
    assert!(text[kept.len()..].starts_with(' '), "the cut falls on a word boundary: {kept:?}");
    // text that already fits is left alone
    let short = fonts.layout("Short text", &body().max_lines(3), Some(220.0));
    assert_eq!(short.text(), "Short text");
}

#[test]
fn single_line_truncation_uses_the_ellipsis_too() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("Invoice-0423-final-revised-copy.docx", &body().max_lines(1), Some(120.0));
    assert_eq!(l.line_count(), 1);
    assert!(l.size().w <= 120.5, "{}", l.size().w);
}

#[test]
fn empty_text_still_has_a_line() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("", &body(), Some(100.0));
    assert_eq!(l.size().h, 21.0);
    assert_eq!(l.caret_rect(0).h, 21.0);
    assert_eq!(l.hit(pt(50.0, 5.0)), 0);
}

#[test]
fn caret_moves_right_with_the_index_and_ends_at_the_text_width() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("Hello world", &body(), None);
    let mut last = -1.0;
    for i in 0..=l.text().len() {
        let x = l.caret_rect(i).x;
        assert!(x >= last, "caret went backwards at {i}: {x} < {last}");
        last = x;
    }
    assert_eq!(l.caret_rect(0).x, 0.0);
    assert!((l.caret_rect(11).x - l.size().w).abs() < 0.5);
}

#[test]
fn hit_testing_inverts_caret_positions() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("Hello world", &body(), None);
    for i in [0, 1, 4, 5, 6, 10, 11] {
        let c = l.caret_rect(i);
        assert_eq!(l.hit(pt(c.x + 0.5, c.y + 5.0)), i, "index {i}");
    }
    assert_eq!(l.hit(pt(-20.0, 5.0)), 0);
    assert_eq!(l.hit(pt(1000.0, 5.0)), 11);
}

#[test]
fn selection_covers_the_selected_glyphs() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("Hello world", &body(), None);
    let rects = l.selection_rects(0, 5);
    assert_eq!(rects.len(), 1);
    assert!((rects[0].x - 0.0).abs() < 0.5);
    assert!((rects[0].right() - l.caret_rect(5).x).abs() < 0.5);
    assert!(l.selection_rects(3, 3).is_empty());
    // a selection across a wrap produces one rect per visual line
    let w = fonts.layout("alpha beta gamma delta epsilon", &body(), Some(70.0));
    assert!(w.line_count() >= 3);
    assert!(w.selection_rects(0, w.text().len()).len() >= 3);
}

#[test]
fn caret_after_a_newline_is_on_the_next_line() {
    let fonts = Fonts::bundled_only();
    let l = fonts.layout("ab\ncd", &body(), None);
    assert_eq!(l.caret_rect(2).y, 0.0);
    assert_eq!(l.caret_rect(3).y, 21.0);
    assert_eq!(l.caret_rect(3).x, 0.0);
}

#[test]
fn centered_text_is_offset_inside_the_wrap_width() {
    let fonts = Fonts::bundled_only();
    let left = fonts.layout("Hi", &body(), Some(200.0));
    let centered = fonts.layout("Hi", &body().align(TextAlign::Center), Some(200.0));
    assert!(centered.caret_rect(0).x > 80.0, "{}", centered.caret_rect(0).x);
    assert_eq!(left.caret_rect(0).x, 0.0);
}

#[test]
fn the_cache_returns_the_same_layout_and_evicts_stale_ones() {
    let fonts = Fonts::bundled_only();
    let a = fonts.layout_cached("cached", &body(), Some(100.0));
    let b = fonts.layout_cached("cached", &body(), Some(100.0));
    assert!(Rc::ptr_eq(&a, &b));
    let c = fonts.layout_cached("cached", &body(), Some(200.0));
    assert!(!Rc::ptr_eq(&a, &c), "a different wrap width is a different layout");
    assert_eq!(fonts.cached_layouts(), 2);
    for _ in 0..4 {
        fonts.end_frame();
    }
    assert_eq!(fonts.cached_layouts(), 0);
}

#[test]
fn the_title_token_is_bigger_and_heavier() {
    let t = crate::theme::Theme::light().types;
    let fonts = Fonts::bundled_only();
    let title = fonts.layout("Allow three things", &TextStyle::from_token(t.title), None);
    let body_l = fonts.layout("Allow three things", &TextStyle::from_token(t.body), None);
    assert!(title.size().w > body_l.size().w * 1.5);
    assert_eq!(title.size().h, 32.0);
}

#[test]
fn an_added_font_names_its_family_and_text_can_ask_for_it() {
    let fonts = Fonts::bundled_only();
    let mut families = Vec::new();
    for bytes in vui_fonts::all() {
        families.extend(fonts.add_font(bytes.to_vec()));
    }
    assert!(families.iter().any(|f| f == "JetBrains Mono"), "{families:?}");
    let style = body().family(FontFamily::Named("JetBrains Mono".into()));
    let layout = fonts.layout("Hello", &style, None);
    assert_eq!(face_family(&fonts, &layout).0, "JetBrains Mono");
}
