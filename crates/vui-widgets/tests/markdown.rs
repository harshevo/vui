//! Markdown: what the parser makes of what models write, plain text for speech, and clickable links.

use vui_core::pt;
use vui_runtime::testing::TestUi;
use vui_runtime::{Builder, Styled};
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

