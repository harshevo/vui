//! Web, video and PDF views as frames drawn without a window show them: placeholders, the same on
//! every machine and renderer, light and dark.
//!
//! ```sh
//! VUI_BLESS=1 cargo test -p vui --test media   # after an intended visual change
//! ```

#[allow(dead_code)]
#[path = "../examples/media.rs"]
mod media;

use vui::core::{EmbedKind, Resources, Theme, size};
use vui::prelude::*;
use vui::runtime::Ui;
use vui::testing::{TestUi, assert_golden, render_with};

fn sheet() -> impl IntoElement<()> {
    column().padding(16.0).gap(12.0).children((
        web_view("https://www.example.com/guide").rounded(10.0).size(368.0, 120.0),
        row().gap(12.0).children((
            video("/media/launch-film.mp4").rounded(10.0).width(178.0),
            pdf_view("/docs/Quarterly Report.pdf").rounded(10.0).size(178.0, 100.0),
        )),
    ))
}

fn shot(theme: Theme) -> vui::render::Image {
    let res = Resources::bundled_only();
    let mut t = TestUi { ui: Ui::new(res.clone(), theme, size(400.0, 280.0), 1.0) };
    t.view(sheet());
    t.advance(0.5);
    let scene = t.scene();
    assert_eq!(t.ui.embeds().len(), 3, "a native view asked for each");
    render_with(&scene, &res)
}

#[test]
fn media_placeholders_light() {
    assert_golden!("media_light", shot(Theme::light()));
}

#[test]
fn media_placeholders_dark() {
    assert_golden!("media_dark", shot(Theme::dark()));
}

#[test]
fn the_example_plays_pauses_and_turns_pages_in_place() {
    let app = media::Media { web: "https://example.com".into(), video: "clip.mp4".into(), pdf: "doc.pdf".into(), playing: true, page: 0 };
    let mut t = TestUi { ui: Ui::with_app(Resources::bundled_only(), size(900.0, 720.0), 1.0, app) };
    t.advance(0.6);
    t.scene();
    let ids: Vec<u64> = t.ui.embeds().iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 3);
    assert!(t.ui.embeds().iter().all(|e| e.visible() == Some(e.rect)), "all in the window");

    t.click_key("play").click_key("page").advance(0.3);
    t.scene();
    let list = t.ui.embeds();
    assert_eq!(list.iter().map(|e| e.id).collect::<Vec<_>>(), ids, "the same views");
    assert!(matches!(list[1].kind, EmbedKind::Video { playing: false, looping: true, .. }));
    assert!(matches!(list[2].kind, EmbedKind::Pdf { page: 1, .. }));
}
