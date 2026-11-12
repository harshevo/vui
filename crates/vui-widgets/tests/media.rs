//! Native views: they take their place in layout, paint a placeholder, and the window is told where
//! each goes (cut to a scroll view) only when that changes, with ids that last while they are shown.

use vui_core::{EmbedKind, size};
use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{Builder, Effect, IntoElement, Overflow, Parent, Styled, column};
use vui_widgets::{pdf_view, video, web_view};

fn page(playing: bool, with_pdf: bool) -> impl IntoElement<()> {
    let mut items: Vec<vui_runtime::Element<()>> = vec![
        web_view("https://www.example.com/docs").key("web").size(300.0, 150.0).into_element(),
        video("/media/clip.mp4").key("video").playing(playing).muted(true).looping().into_element(),
    ];
    if with_pdf {
        items.push(pdf_view("/docs/Report.pdf").page(2).key("pdf").into_element());
    }
    column().key("scroller").padding(10.0).gap(10.0).height(400.0).overflow(Overflow::Scroll).children(items)
}

fn embeds(t: &mut TestUi<()>) -> Vec<Effect> {
    t.scene();
    t.effects().into_iter().filter(|e| matches!(e, Effect::Embeds(_))).collect()
}

#[test]
fn each_view_goes_where_layout_puts_it() {
    let mut t = TestUi::new(320.0, 400.0);
    t.view(page(false, true));
    let sent = embeds(&mut t);
    assert_eq!(sent.len(), 1, "sent once: {sent:?}");
    let list = t.ui.embeds().to_vec();
    assert_eq!(list.len(), 3);
    assert!(matches!(&list[0].kind, EmbedKind::Web { url } if url == "https://www.example.com/docs"));
    assert_eq!(list[0].rect, t.rect("web"), "sized by the app");
    assert_eq!(t.rect("video").size(), size(300.0, 300.0 * 9.0 / 16.0), "16:9 across the width given");
    assert!(matches!(&list[1].kind, EmbedKind::Video { playing: false, looping: true, muted: true, .. }));
    assert!(matches!(&list[2].kind, EmbedKind::Pdf { page: 2, .. }));
    assert_eq!(list[2].clip, Some(t.rect("scroller")), "cut to the scroll view");
    assert!(list[2].visible().is_some_and(|v| v.bottom() <= 400.0), "only the part in view shows");

    let words = texts(&t.scene());
    for label in ["example.com", "clip.mp4", "Report.pdf"] {
        assert!(words.iter().any(|w| w.contains(label)), "the placeholder names {label}: {words:?}");
    }
}

#[test]
fn the_window_hears_only_about_changes_and_ids_last() {
    let mut t = TestUi::new(320.0, 400.0);
    t.view(page(false, true));
    embeds(&mut t);
    let ids: Vec<u64> = t.ui.embeds().iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids[0] != ids[1] && ids[1] != ids[2]);

    t.view(page(false, true));
    assert!(embeds(&mut t).is_empty(), "nothing moved, nothing sent");

    t.view(page(true, true));
    assert_eq!(embeds(&mut t).len(), 1, "playing is news");
    let now: Vec<u64> = t.ui.embeds().iter().map(|e| e.id).collect();
    assert_eq!(now, ids, "the same views, changed in place");
    assert!(matches!(t.ui.embeds()[1].kind, EmbedKind::Video { playing: true, .. }));

    let at = t.center("scroller");
    t.wheel(at, 0.0, -120.0).advance(1.0);
    assert_eq!(embeds(&mut t).len(), 1, "scrolling moves them");
    assert!(t.ui.embeds()[0].rect.y < 10.0);

    t.view(page(true, false));
    let sent = embeds(&mut t);
    assert!(matches!(&sent[..], [Effect::Embeds(list)] if list.len() == 2), "the PDF is gone: {sent:?}");
}

#[test]
fn placeholders_are_named_for_assistive_tech() {
    let mut t = TestUi::new(320.0, 400.0);
    t.view(page(false, true));
    let tree = t.ui.access_tree();
    let labels: Vec<String> = tree.nodes.iter().filter_map(|n| n.name.clone()).collect();
    assert!(labels.iter().any(|l| l == "example.com") && labels.iter().any(|l| l == "Report.pdf"), "{labels:?}");
}
