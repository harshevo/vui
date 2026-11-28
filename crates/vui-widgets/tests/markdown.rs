//! Markdown: what the parser makes of what models write, plain text for speech, and clickable links.

use vui_core::pt;
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Parent, Styled};
use vui_widgets::markdown::{Block, Inline, inlines, parse, parse_streaming, plain, plain_streaming, strip};
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

#[test]
fn streaming_text_never_shows_half_written_markup() {
    let text = |t: &str| Inline::Text(t.into());
    let para = |runs: Vec<Inline>| Block::Paragraph(runs);
    // Bold, emphasis and code still open run to the end, without a closing marker half typed; a
    // marker with nothing after it yet shows nothing.
    assert_eq!(parse_streaming("INFY is **up 1.0"), [para(vec![text("INFY is "), Inline::Strong("up 1.0".into())])]);
    assert_eq!(parse_streaming("It is *very* **clo*"), [para(vec![text("It is "), Inline::Emphasis("very".into()), text(" "), Inline::Strong("clo".into())])]);
    assert_eq!(parse_streaming("Run `cargo te"), [para(vec![text("Run "), Inline::Code("cargo te".into())])]);
    assert_eq!(parse_streaming("It is **"), [para(vec![text("It is ")])]);
    assert_eq!(parse_streaming("2 * 3"), [para(vec![text("2 * 3")])], "spaced stars stay arithmetic");
    // A link shows its words before its address.
    assert_eq!(parse_streaming("See [the fil"), [para(vec![text("See the fil")])]);
    assert_eq!(parse_streaming("See [the filing](https://ex"), [para(vec![text("See the filing")])]);
    // A line that only starts a block waits for words of its own.
    assert_eq!(parse_streaming("Levels:\n\n-"), [para(vec![text("Levels:")])]);
    assert_eq!(parse_streaming("Levels:\n\n- **1,8"), [para(vec![text("Levels:")]), Block::List { start: None, items: vec![vec![Inline::Strong("1,8".into())]] }]);
    assert_eq!(parse_streaming("1. first\n2."), [Block::List { start: Some(1), items: vec![vec![text("first")]] }]);
    assert_eq!(parse_streaming("Intro\n\n## "), [para(vec![text("Intro")])]);
    assert_eq!(parse_streaming("Intro\n\n```rust"), [para(vec![text("Intro")])]);
    // A table's head waits for the line under it; an open code block shows its lines so far.
    assert_eq!(parse_streaming("| Stock | Price |"), []);
    assert_eq!(parse_streaming("| Stock | Price |\n|"), []);
    assert!(matches!(&parse_streaming("| Stock | Price |\n|---|")[..], [Block::Table { head, rows }] if head.len() == 2 && rows.is_empty()));
    assert_eq!(parse_streaming("```\nlet a = 1;\n``"), [Block::Code { lang: String::new(), text: "let a = 1;".into() }]);
    assert_eq!(plain_streaming("**Checking** the `look"), "Checking the look");
    // Finished text keeps what it never closed as it is.
    assert_eq!(parse("It is **"), [para(vec![text("It is **")])]);
    assert_eq!(parse_streaming("Done.\n\n- one\n- two"), parse("Done.\n\n- one\n- two"), "finished markup parses the same streaming or not");
}

#[test]
fn a_burst_of_paragraphs_types_in_one_at_a_time_in_reading_order() {
    let first = "The first paragraph of an answer that came in a single burst, long enough to wrap.";
    let doc = format!("{first}\n\nThen a second one.\n\n- an item\n- another item");
    let height = |text: &str| {
        let mut t = TestUi::new(240.0, 600.0);
        t.view(vui_runtime::column().child(markdown::<Msg>(text).with(|d| d.key("doc"))));
        t.rect("doc").h
    };
    let (alone, whole) = (height(first), height(&doc));
    let mut t = TestUi::new(240.0, 600.0);
    t.view(vui_runtime::column().child(markdown::<Msg>(&doc).reveal(true).with(|d| d.key("doc"))));
    t.advance(1.0 / 60.0);
    let mut last = t.rect("doc").h;
    assert!(last > 0.0 && last <= 22.0, "one line has begun and what waits takes no room, gaps included: {last}");
    for _ in 0..300 {
        t.advance(1.0 / 60.0);
        let h = t.rect("doc").h;
        assert!(h + 0.01 >= last, "the document only grows as it types ({last} then {h})");
        if h > alone + 0.5 {
            let drawn = vui_runtime::testing::texts(&t.scene());
            assert_eq!(drawn.iter().filter(|s| *s == first).count(), 1, "the second paragraph starts once the first is typed whole");
        }
        last = h;
    }
    assert!((last - whole).abs() < 0.01, "it ends as tall as the finished document ({last}, {whole})");
}
