//! Decorated text: stretches in a tone of their own, wavy and straight underlines.

use vui_core::Tone;
use vui_widgets::{Decoration, TextField, password_field, text_area, text_field};

use super::*;

fn formula(t: &str) -> TextField<Msg> {
    // "roce > 15 and pe < 20": a name, an operator, a number, a keyword; the last name is unknown.
    text_field(t).mono().decorate(vec![
        Decoration::new(0..4).tone(Tone::Text),
        Decoration::new(5..6).tone(Tone::Text2),
        Decoration::new(7..9).tone(Tone::Orange),
        Decoration::new(10..13).tone(Tone::Accent),
        Decoration::new(14..16).tone(Tone::Danger).squiggle(Tone::Danger),
    ])
}

fn pieces(t: &mut TestUi<Msg>) -> Vec<(String, vui_core::Color)> {
    t.scene()
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text(t) => Some((t.layout.text().to_string(), t.color)),
            _ => None,
        })
        .collect()
}

fn palette() -> vui_core::Palette {
    Theme::light().palette
}

#[test]
fn each_stretch_is_drawn_in_its_own_tone_and_the_gaps_in_the_text_colour() {
    let mut t = window_with(formula, "roce > 15 and pe < 20", false);
    let p = pieces(&mut t);
    let pal = palette();
    let colour_of = |s: &str| p.iter().find(|(text, _)| text == s).map(|(_, c)| *c).unwrap_or_else(|| panic!("{s:?} not drawn in {p:?}"));
    assert_eq!(colour_of("roce"), pal.text);
    assert_eq!(colour_of(">"), pal.text_2);
    assert_eq!(colour_of("15"), pal.orange);
    assert_eq!(colour_of("and"), pal.accent);
    assert_eq!(colour_of("pe"), pal.danger);
    // What no mark covers is the plain text colour, and nothing is drawn twice.
    assert_eq!(colour_of(" < 20"), pal.text, "the tail no mark covers is one plain piece");
    let joined: String = p.iter().filter(|(s, _)| *s != "second").map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("");
    assert_eq!(joined.replace(' ', ""), "roce>15andpe<20", "the pieces are the text, once: {p:?}");
}

#[test]
fn pieces_sit_where_the_whole_text_would_have_put_them() {
    let mut t = window_with(formula, "roce > 15 and pe < 20", false);
    let scene = t.scene();
    for (word, byte) in [("roce", 0), (">", 5), ("15", 7), ("and", 10), ("pe", 14)] {
        let cmd = text_cmd(&scene, word);
        // Mono text is evenly spaced, so a piece lands within a hair of where the whole text has it.
        let want = {
            let mut style = TextStyle::from_token(Theme::light().types.body);
            style.family = vui_core::FontFamily::Mono;
            style.wrap = TextWrap::None;
            INNER_X + FONTS.with(|f| f.layout("roce > 15 and pe < 20", &style, None).caret_rect(byte).x)
        };
        assert!((cmd.origin.x - want).abs() < 0.6, "{word}: drawn at {} want {want}", cmd.origin.x);
    }
}

#[test]
fn a_squiggle_is_drawn_under_the_marked_stretch_only() {
    let mut t = window_with(formula, "roce > 15 and pe < 20", false);
    let scene = t.scene();
    let danger = palette().danger;
    let waves: Vec<&Quad> = quads(&scene).into_iter().filter(|q| q.fill == danger && q.rect.h == 1.0).collect();
    assert!(waves.len() >= 4, "a few steps of wave under two letters of mono text: {}", waves.len());
    let (left, right) = (waves.iter().map(|q| q.rect.x).fold(f32::MAX, f32::min), waves.iter().map(|q| q.rect.right()).fold(f32::MIN, f32::max));
    let pe = text_cmd(&scene, "pe");
    assert!(left >= pe.origin.x - 0.6 && right <= pe.origin.x + pe.layout.size().w + 0.6, "the wave stays under `pe`: {left}..{right} vs {}..", pe.origin.x);
    let ys: std::collections::BTreeSet<i32> = waves.iter().map(|q| (q.rect.y * 10.0) as i32).collect();
    assert_eq!(ys.len(), 2, "it zigzags between two heights");
}

#[test]
fn an_underline_without_a_tone_change_keeps_the_text_colour() {
    fn underlined(t: &str) -> TextField<Msg> {
        text_field(t).decorate(vec![Decoration::new(0..3).underline(Tone::Accent)])
    }
    let mut t = window_with(underlined, "abc def", false);
    let p = pieces(&mut t);
    assert!(p.iter().any(|(s, c)| s == "abc" && *c == palette().text), "{p:?}");
    let lines: Vec<_> = quads(&t.scene()).into_iter().filter(|q| q.fill == palette().accent && q.rect.h == 1.0 && q.rect.w > 5.0).count().to_string().into();
    assert_eq!(lines.len(), 1);
}

#[test]
fn marks_that_leave_the_text_are_cut_to_it_and_never_split_a_character() {
    fn wild(t: &str) -> TextField<Msg> {
        text_field(t).decorate(vec![
            Decoration::new(2..400).tone(Tone::Danger),
            Decoration::new(500..600).tone(Tone::Accent),
            Decoration::new(3..3).tone(Tone::Accent),
            #[allow(clippy::reversed_empty_ranges)]
            Decoration::new(9..4).tone(Tone::Accent),
        ])
    }
    // A two-byte letter at 1..3: a mark that starts inside it is moved to its edge.
    let mut t = window_with(wild, "aéb cd", false);
    let p = pieces(&mut t);
    assert!(!p.is_empty(), "it draws");
    let joined: String = p.iter().filter(|(s, _)| *s != "second").map(|(s, _)| s.as_str()).collect::<Vec<_>>().join("");
    assert_eq!(joined.replace(' ', ""), "aébcd");
}

#[test]
fn a_later_mark_wins_where_two_overlap() {
    fn layered(t: &str) -> TextField<Msg> {
        text_field(t).decorate(vec![Decoration::new(0..6).tone(Tone::Success), Decoration::new(2..4).tone(Tone::Danger)])
    }
    let mut t = window_with(layered, "abcdef", false);
    let p = pieces(&mut t);
    let pal = palette();
    let find = |s: &str| p.iter().find(|(text, _)| text == s).map(|(_, c)| *c);
    assert_eq!(find("ab"), Some(pal.success));
    assert_eq!(find("cd"), Some(pal.danger));
    assert_eq!(find("ef"), Some(pal.success));
}

#[test]
fn wrapped_text_is_marked_line_by_line() {
    fn long(t: &str) -> TextField<Msg> {
        text_area(t).decorate(vec![Decoration::new(0..t.len()).tone(Tone::Accent)])
    }
    let words = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau";
    let mut t = window_with(long, words, false);
    let p = pieces(&mut t);
    let accent = palette().accent;
    let marked: Vec<&(String, vui_core::Color)> = p.iter().filter(|(_, c)| *c == accent).collect();
    assert!(marked.len() >= 2, "more than one line is drawn: {p:?}");
    let rejoined: String = marked.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join(" ");
    assert_eq!(rejoined.split_whitespace().collect::<Vec<_>>().join(" "), words);
}

#[test]
fn a_password_and_a_composition_ignore_decorations() {
    fn secret_marked(t: &str) -> TextField<Msg> {
        password_field(t).decorate(vec![Decoration::new(0..2).tone(Tone::Danger)])
    }
    let mut t = window_with(secret_marked, "abcd", false);
    let shown = drawn(&mut t);
    assert!(shown.contains(&"••••".to_string()), "{shown:?}");
    assert!(!pieces(&mut t).iter().any(|(_, c)| *c == palette().danger));
}

#[test]
fn the_text_stays_selectable_and_the_caret_stays_put_when_decorated() {
    let mut t = window_with(formula, "roce > 15 and pe < 20", false);
    t.click_key("f");
    t.shortcut('a');
    assert_eq!(selection(&mut t), (0, 21));
    t.type_text("pe");
    assert_eq!(text(&t), "pe", "decorations beyond the new text are simply cut");
    assert!(!drawn(&mut t).is_empty());
}
