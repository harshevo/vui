//! Markdown: what the parser makes of what models write, plain text for speech, and clickable links.

use vui_core::pt;
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Parent, Styled};
use vui_widgets::markdown::{Block, Inline, inlines, parse, plain, strip};
use vui_widgets::{markdown, rich_text};

#[test]
fn inline_markup_becomes_runs_and_never_shows_asterisks() {
    assert_eq!(
        inlines("That is **Aarush Yadav** (@AarushYadav47), *co-founder* of `Subvocal`."),
        [
            Inline::Text("That is ".into()),
            Inline::Strong("Aarush Yadav".into()),
            Inline::Text(" (@AarushYadav47), ".into()),
            Inline::Emphasis("co-founder".into()),
            Inline::Text(" of ".into()),
            Inline::Code("Subvocal".into()),
            Inline::Text(".".into()),
        ]
    );
    assert_eq!(strip("see [Hindustan Times](https://example.com/a-b) and snake_case_name"), "see Hindustan Times and snake_case_name");
    assert_eq!(strip("2 * 3 * 4 and a lone * star"), "2 * 3 * 4 and a lone * star", "spaced stars are arithmetic, not emphasis");
}

#[test]
fn blocks_are_found() {
    let doc = "# Plan\n\nIntro line\ncontinues.\n\n- one\n- **two**\n\n1. first\n2. second\n\n> a quote\n\n```rust\nfn main() {}\n```\n\n---\n\n![shot](/tmp/a.png)\n\n| a | b |\n|---|---|\n| 1 | 2 |";
    let blocks = parse(doc);
    assert!(matches!(&blocks[0], Block::Heading(1, _)));
    assert_eq!(blocks[1], Block::Paragraph(vec![Inline::Text("Intro line continues.".into())]));
    assert!(matches!(&blocks[2], Block::List { start: None, items } if items.len() == 2));
    assert!(matches!(&blocks[3], Block::List { start: Some(1), items } if items.len() == 2));
    assert!(matches!(&blocks[4], Block::Quote(_)));
    assert_eq!(blocks[5], Block::Code { lang: "rust".into(), text: "fn main() {}".into() });
    assert_eq!(blocks[6], Block::Rule);
    assert_eq!(blocks[7], Block::Image { alt: "shot".into(), src: "/tmp/a.png".into() });
    assert!(matches!(&blocks[8], Block::Table { head, rows } if head.len() == 2 && rows.len() == 1));
    assert_eq!(plain("**Done.** Open [the page](http://x)."), "Done. Open the page.");
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open(String),
}

#[test]
fn a_link_sends_its_address_and_plain_words_do_not() {
    let mut t = TestUi::new(600.0, 200.0);
    t.view(rich_text("Go to [docs](https://docs.example) now").key("p").width(600.0).on_link(Msg::Open));
    let r = t.rect("p");
    t.click(pt(r.x + 2.0, r.y + 10.0));
    assert!(t.messages().is_empty(), "plain words");
    // "Go to " is about 40 points at body size; the link follows it.
    t.click(pt(r.x + 56.0, r.y + 10.0));
    assert_eq!(t.messages(), [Msg::Open("https://docs.example".into())]);
}

#[test]
fn a_document_builds_a_child_per_block() {
    let mut t = TestUi::new(600.0, 400.0);
    t.view(markdown::<Msg>("# Title\n\nText\n\n- a\n- b").with(|d| d.key("doc")));
    assert!(t.rect("doc").h > 60.0);
}


const LONG: &str = "Streaming text arrives in bursts of a few words at a time, and drawn as it comes it jumps. \
Typed in at a steady pace with its newest words fading in, it reads as one smooth flow instead.";

fn texts_drawn(t: &mut TestUi<Msg>) -> usize {
    vui_runtime::testing::texts(&t.scene()).len()
}

#[test]
fn a_revealed_paragraph_types_in_a_line_at_a_time_and_ends_whole() {
    let mut t = TestUi::new(240.0, 400.0);
    t.view(vui_runtime::column().child(rich_text::<Msg>(LONG).reveal(true).key("p")));
    let first = t.rect("p").h;
    assert!(texts_drawn(&mut t) <= 8, "nothing typed yet: at most the fading bands of the first line");
    t.advance(0.3);
    let middle = t.rect("p").h;
    assert!(middle > first, "taller as the reveal reaches later lines ({first} then {middle})");
    t.settle();
    let whole = t.rect("p").h;
    assert!(whole > middle, "all of it in the end ({middle} then {whole})");
    assert_eq!(texts_drawn(&mut t), 1, "once typed in, the paragraph is drawn in one piece");
}

#[test]
fn a_paragraph_without_reveal_shows_whole_at_once() {
    let mut t = TestUi::new(240.0, 400.0);
    t.view(vui_runtime::column().child(rich_text::<Msg>(LONG).reveal(true).key("p")));
    t.settle();
    let whole = t.rect("p").h;
    let mut t = TestUi::new(240.0, 400.0);
    t.view(vui_runtime::column().child(rich_text::<Msg>(LONG).key("p")));
    assert_eq!(t.rect("p").h, whole);
    assert_eq!(texts_drawn(&mut t), 1);
}

#[test]
fn text_added_to_a_revealed_paragraph_types_on_from_where_it_was() {
    let mut t = TestUi::new(240.0, 400.0);
    let half = &LONG[..LONG.len() / 2];
    t.view(vui_runtime::column().child(rich_text::<Msg>(half).reveal(true).key("p")));
    t.settle();
    let before = t.rect("p").h;
    t.view(vui_runtime::column().child(rich_text::<Msg>(LONG).reveal(true).key("p")));
    t.advance(1.0 / 60.0);
    assert!(t.rect("p").h <= before + 20.0, "the new text does not appear all at once");
    assert!(texts_drawn(&mut t) > 1, "the old text stays whole while the new types in after it");
    // The stream ending does not cut the reveal short: what was typing finishes.
    t.view(vui_runtime::column().child(rich_text::<Msg>(LONG).reveal(false).key("p")));
    assert!(texts_drawn(&mut t) > 1);
    t.settle();
    assert_eq!(texts_drawn(&mut t), 1);
}
