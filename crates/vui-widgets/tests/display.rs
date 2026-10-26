//! Progress, spinners, segmented controls, badges, avatars, dividers and picture tiles.

use vui_core::{Color, Command, ImageCmd, ImageData, ImageId, NamedKey, Quad, Scene, Theme, Tone};
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{Align, App, Builder, Cx, IntoElement, Parent, Role, Styled, column};
use vui_widgets::{
    avatar, badge, card, image_tile, kbd, progress, progress_indeterminate, section_title, segmented, separator, separator_vertical,
    spinner,
};

fn near(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.01 && (a.g - b.g).abs() < 0.01 && (a.b - b.b).abs() < 0.01 && (a.a - b.a).abs() < 0.01
}

fn color(tone: Tone) -> Color {
    Theme::light().palette.get(tone)
}

fn images(scene: &Scene) -> Vec<&ImageCmd> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Image(i) => Some(i),
            _ => None,
        })
        .collect()
}

fn with_fill(scene: &Scene, tone: Tone) -> Vec<&Quad> {
    quads(scene).into_iter().filter(|q| near(q.fill, color(tone))).collect()
}

fn still(view: impl IntoElement<()>) -> TestUi<()> {
    let mut t = TestUi::new(320.0, 240.0);
    t.view(column().padding(20.0).gap(12.0).align_items(Align::Start).child(view));
    t
}

// ---- progress and spinner --------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Mode(Mode),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Mode {
    Light,
    Dark,
    System,
}

struct Board {
    done: f32,
    mode: Mode,
    picture: Option<ImageId>,
    hash: Vec<u8>,
    log: Vec<Mode>,
}

impl App for Board {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx) {
        let Msg::Mode(mode) = msg;
        self.mode = mode;
        self.log.push(mode);
    }

    fn view(&self) -> impl IntoElement<Msg> {
        let modes = [(Mode::Light, "Light"), (Mode::Dark, "Dark"), (Mode::System, "System")];
        column().padding(20.0).gap(16.0).align_items(Align::Start).children((
            progress(self.done).key("bar").width(200.0).name("Download"),
            segmented(modes.map(|(m, l)| (m, l.to_string())), &self.mode, Msg::Mode).key("seg").name("Appearance"),
            image_tile(self.picture).placeholder(self.hash.clone()).key("tile").size(120.0, 80.0).name("Cover"),
        ))
    }
}

fn picture(t: &TestUi<Msg>) -> ImageId {
    t.ui.resources().images.insert(ImageData::from_fn(60, 40, |x, y| [(x * 4) as u8, (y * 6) as u8, 160, 255]))
}

fn board() -> TestUi<Msg> {
    let board = Board { done: 0.4, mode: Mode::System, picture: None, hash: Vec::new(), log: Vec::new() };
    TestUi::with_app(340.0, 300.0, board)
}

#[test]
fn a_progress_bar_is_filled_to_its_share_of_the_track() {
    let mut t = board();
    let r = t.rect("bar");
    assert_eq!((r.w, r.h), (200.0, 4.0));
    assert!(!t.ui.needs_frame(), "a bar shown at its value is at rest");
    let scene = t.scene();
    let fill = with_fill(&scene, Tone::Accent);
    assert_eq!(fill.len(), 1);
    assert!((fill[0].rect.w - 80.0).abs() < 0.6, "{:?}", fill[0].rect);
    assert_eq!(with_fill(&scene, Tone::Wash).len(), 2, "the track, and the segmented control's own track");
}

#[test]
fn a_progress_bar_eases_to_a_new_value() {
    let mut t = board();
    t.update_app(|b: &mut Board| b.done = 0.9);
    t.advance(0.05);
    let w = with_fill(&t.scene(), Tone::Accent)[0].rect.w;
    assert!(w > 80.5 && w < 180.0, "in flight: {w}");
    t.settle();
    assert!((with_fill(&t.scene(), Tone::Accent)[0].rect.w - 180.0).abs() < 0.6);
}

#[test]
fn a_progress_bar_reports_how_far_along_it_is() {
    let mut t = board();
    let n = t.ui.access_tree().find("Download").unwrap().clone();
    assert_eq!(n.role, Role::ProgressBar);
    assert_eq!(n.value.as_deref(), Some("40%"));
    let range = n.range.expect("a range");
    assert_eq!((range.min, range.max), (0.0, 1.0));
    assert!((range.value - 0.4).abs() < 1e-6);
}

#[test]
fn an_indeterminate_bar_keeps_running_inside_its_track() {
    let mut t = still(progress_indeterminate().width(200.0).name("Loading"));
    assert!(t.ui.needs_frame());
    let runner = |t: &mut TestUi<()>| {
        let scene = t.scene();
        with_fill(&scene, Tone::Accent)[0].clone()
    };
    let first = runner(&mut t);
    t.advance(0.4);
    let second = runner(&mut t);
    assert!(second.rect.x > first.rect.x, "{} then {}", first.rect.x, second.rect.x);
    assert!(second.clip.is_some(), "cut to the track so it enters and leaves cleanly");
    assert!(t.ui.needs_frame());
    let n = t.ui.access_tree().find("Loading").unwrap().clone();
    assert!(n.range.is_none(), "unknown length");
}

#[test]
fn a_spinner_turns_and_asks_for_frames() {
    let mut t = still(spinner().key("s").name("Loading"));
    assert_eq!(t.rect("s").w, 16.0);
    assert!(t.ui.needs_frame());
    let frame = |t: &mut TestUi<()>| images(&t.scene())[0].id;
    let first = frame(&mut t);
    t.advance(0.2);
    assert_ne!(frame(&mut t), first);
    assert_eq!(t.ui.access_tree().find("Loading").unwrap().role, Role::ProgressBar);
}

// ---- segmented control -----------------------------------------------------------------------

fn segment_rect(t: &mut TestUi<Msg>, name: &str) -> vui_core::Rect {
    t.ui.access_tree().find(name).unwrap_or_else(|| panic!("no segment {name}")).bounds
}

#[test]
fn a_segmented_control_is_a_grey_track_with_the_chosen_segment_lifted() {
    let mut t = board();
    t.settle();
    let track = t.rect("seg");
    assert_eq!(track.h, 32.0);
    let system = segment_rect(&mut t, "System");
    assert_eq!(system.h, 28.0);
    let scene = t.scene();
    let lifted: Vec<_> = with_fill(&scene, Tone::Field).into_iter().filter(|q| track.contains(q.rect.center())).collect();
    assert_eq!(lifted.len(), 1);
    assert!((lifted[0].rect.center().x - system.center().x).abs() < 1.0);
}

#[test]
fn clicking_a_segment_chooses_it_and_the_lift_slides_over() {
    let mut t = board();
    t.settle();
    let dark = segment_rect(&mut t, "Dark");
    t.click(dark.center());
    assert_eq!(t.app::<Board>().log, [Mode::Dark]);
    t.settle();
    let track = t.rect("seg");
    let scene = t.scene();
    let lifted: Vec<_> = with_fill(&scene, Tone::Field).into_iter().filter(|q| track.contains(q.rect.center())).collect();
    assert_eq!(lifted.len(), 1);
    assert!((lifted[0].rect.center().x - dark.center().x).abs() < 1.0);
}

#[test]
fn segments_are_one_tab_stop_and_the_arrows_choose_as_they_go() {
    let mut t = board();
    // Tab reaches the control on the chosen segment.
    t.tab();
    let focus = t.ui.access_tree().focus;
    assert_eq!(t.ui.access_tree().node(focus).and_then(|n| n.name.clone()).as_deref(), Some("System"));
    t.key(NamedKey::ArrowLeft);
    t.key(NamedKey::ArrowLeft);
    assert_eq!(t.app::<Board>().log, [Mode::Dark, Mode::Light]);
    t.key(NamedKey::ArrowLeft);
    assert_eq!(t.app::<Board>().log.len(), 2, "no wrapping past the first segment");
    t.key(NamedKey::End);
    assert_eq!(t.app::<Board>().mode, Mode::System);
    t.tab();
    assert!(t.ui.access_tree().node(t.ui.access_tree().focus).is_none_or(|n| n.name.as_deref() != Some("Light")));
}

#[test]
fn segments_read_as_radio_buttons_in_a_group() {
    let mut t = board();
    let tree = t.ui.access_tree();
    let group = tree.find("Appearance").expect("the group is named");
    assert_eq!(group.role, Role::Group);
    let radios = tree.with_role(Role::RadioButton);
    assert_eq!(radios.len(), 3);
    let on: Vec<_> = radios.iter().filter(|n| n.toggled == Some(true)).filter_map(|n| n.name.clone()).collect();
    assert_eq!(on, ["System"]);
}

// ---- badge, keycap, avatar, dividers ---------------------------------------------------------

#[test]
fn a_badge_is_a_small_pill() {
    let mut t = still(badge("Beta").key("b"));
    let r = t.rect("b");
    assert_eq!(r.h, 20.0);
    let scene = t.scene();
    let pill = quads(&scene).into_iter().find(|q| q.rect.h == 20.0).expect("a pill");
    assert!(pill.radii.tl >= 10.0);
    // A little of the text colour, so the pill stays visible on a row that is hovered or open.
    assert!(near(Color { a: 1.0, ..pill.fill }, color(Tone::Text)) && (pill.fill.a - 0.07).abs() < 0.01, "{:?}", pill.fill);
    assert_eq!(texts(&scene), ["Beta"]);
}

#[test]
fn an_avatar_and_a_neutral_badge_stay_visible_on_a_row_of_the_same_grey() {
    let mut t = still(
        column().bg(Tone::Wash).children((avatar("Ada Lovelace").key("ada"), badge("Beta").key("b"))),
    );
    let scene = t.scene();
    // The row is the wash over the window background.
    let wash = color(Tone::Wash).over(color(Tone::Bg));
    let text = color(Tone::Text);
    for key in ["ada", "b"] {
        let at = t.rect(key).center();
        let chip = quads(&scene).into_iter().rev().find(|q| q.rect.contains(at)).expect("a chip");
        // What the eye sees is the chip over the row: 7% of the text colour mixed into the wash.
        let seen = chip.fill.over(wash);
        assert!(seen.r < wash.r - 0.02, "{key}: {seen:?} against {wash:?}");
        assert!((seen.r - wash.lerp(text, 0.07).r).abs() < 0.01);
    }
}

#[test]
fn an_accent_badge_is_a_tint_of_the_accent() {
    let mut t = still(badge("Live").accent().key("b"));
    let scene = t.scene();
    let pill = quads(&scene).into_iter().find(|q| q.rect.h == 20.0).expect("a pill");
    assert!(near(Color { a: 1.0, ..pill.fill }, color(Tone::Accent)) && (pill.fill.a - 0.12).abs() < 0.01, "{:?}", pill.fill);
    let text = scene.commands.iter().find_map(|c| match c {
        Command::Text(c) => Some(c.color),
        _ => None,
    });
    assert!(near(text.unwrap(), color(Tone::Accent)));
}

#[test]
fn a_keycap_has_a_hairline_border() {
    let mut t = still(kbd("⌘K").key("k"));
    let scene = t.scene();
    let cap = quads(&scene).into_iter().find(|q| q.border.is_some()).expect("a keycap");
    let border = cap.border.unwrap();
    assert_eq!(border.width, 1.0);
    assert!(near(border.color, color(Tone::Line)));
    assert_eq!(texts(&scene), ["⌘K"]);
}

#[test]
fn an_avatar_shows_initials_and_is_named_in_full() {
    let mut t = still(column().children((avatar("Ada Lovelace").key("ada"), avatar("plato").key("p"))));
    assert_eq!((t.rect("ada").w, t.rect("ada").h), (32.0, 32.0));
    let mut shown = texts(&t.scene());
    shown.sort();
    assert_eq!(shown, ["AL", "P"]);
    let tree = t.ui.access_tree();
    let n = tree.find("Ada Lovelace").expect("named in full");
    assert_eq!(n.role, Role::Image);
}

#[test]
fn separators_are_one_point_lines() {
    let mut t = TestUi::<()>::new(200.0, 100.0);
    t.view(column().padding(20.0).children((separator().key("h"), vui_runtime::row().height(40.0).child(separator_vertical().key("v")))));
    let h = t.rect("h");
    assert_eq!((h.w, h.h), (160.0, 1.0));
    let v = t.rect("v");
    assert_eq!((v.w, v.h), (1.0, 40.0));
    let scene = t.scene();
    assert_eq!(with_fill(&scene, Tone::Line).len(), 2);
    assert_eq!(t.ui.access_tree().with_role(Role::Separator).len(), 2);
}

#[test]
fn a_card_has_a_border_and_rounded_corners() {
    let mut t = still(card().child(section_title("Recent")).key("c"));
    let scene = t.scene();
    let q = quads(&scene).into_iter().find(|q| q.border.is_some()).expect("the card");
    assert_eq!(q.radii.tl, t.ui.theme().radii.card);
    assert_eq!(q.border.unwrap().width, 1.0);
    assert!(near(q.fill, color(Tone::Bg)));
    let heading = t.ui.access_tree().with_role(Role::Heading).len();
    assert_eq!(heading, 1);
}

// ---- picture tiles ---------------------------------------------------------------------------

fn hash_of(t: &TestUi<Msg>, id: ImageId) -> Vec<u8> {
    t.ui.resources().images.get(id).expect("the picture is stored").thumb_hash()
}

fn tile_window(with_picture: bool) -> TestUi<Msg> {
    let mut t = board();
    let id = picture(&t);
    let hash = hash_of(&t, id);
    t.update_app(|b: &mut Board| {
        b.hash = hash;
        b.picture = with_picture.then_some(id);
    });
    t
}

fn tile_images<'a>(scene: &'a Scene, t: &mut TestUi<Msg>) -> Vec<&'a ImageCmd> {
    let r = t.rect("tile");
    images(scene).into_iter().filter(|i| r.contains(i.rect.center())).collect()
}

#[test]
fn a_tile_waiting_for_its_picture_shows_the_blurred_stand_in() {
    let mut t = tile_window(false);
    let scene = t.scene();
    let shown = tile_images(&scene, &mut t);
    assert_eq!(shown.len(), 1);
    let r = t.rect("tile");
    assert_eq!(shown[0].rect, r);
    assert_eq!(shown[0].opacity, 1.0);
    assert!(shown[0].radii.tl == 0.0, "square unless the tile is rounded");
}

#[test]
fn a_tile_that_starts_with_its_picture_does_not_fade() {
    let mut t = TestUi::<()>::new(200.0, 200.0);
    let id = t.ui.resources().images.insert(ImageData::from_fn(60, 40, |x, y| [(x * 4) as u8, (y * 6) as u8, 160, 255]));
    let hash = t.ui.resources().images.get(id).unwrap().thumb_hash();
    t.view(column().align_items(Align::Start).child(image_tile::<()>(Some(id)).placeholder(hash).key("tile").size(120.0, 80.0)));
    assert!(!t.ui.needs_frame());
    let scene = t.scene();
    let shown = images(&scene);
    assert_eq!(shown.len(), 1, "only the picture");
    assert_eq!((shown[0].id, shown[0].opacity), (id, 1.0));
    // 60 by 40 into 120 by 80 is the same shape: the whole picture shows.
    assert_eq!((shown[0].uv.w, shown[0].uv.h), (1.0, 1.0));
}

#[test]
fn the_picture_fades_in_over_the_stand_in_when_it_arrives() {
    let mut t = tile_window(false);
    let id = picture(&t);
    t.update_app(|b: &mut Board| b.picture = Some(id));
    assert!(t.ui.needs_frame(), "the fade asks for frames at once");
    t.advance(0.05);
    let scene = t.scene();
    let shown = tile_images(&scene, &mut t);
    assert_eq!(shown.len(), 2, "stand-in underneath, picture over it");
    let over = shown.iter().find(|i| i.id == id).expect("the picture is drawn");
    assert!(over.opacity > 0.0 && over.opacity < 1.0, "{}", over.opacity);
    t.settle();
    let scene = t.scene();
    let shown = tile_images(&scene, &mut t);
    assert_eq!(shown.len(), 1, "the stand-in is gone");
    assert_eq!((shown[0].id, shown[0].opacity), (id, 1.0));
}

#[test]
fn a_picture_covers_its_tile_and_crops_the_overflow() {
    let mut t = TestUi::<()>::new(200.0, 200.0);
    let id = t.ui.resources().images.insert(ImageData::solid(100, 50, Color::WHITE));
    t.view(column().align_items(Align::Start).child(image_tile::<()>(Some(id)).key("sq").size(60.0, 60.0)));
    let scene = t.scene();
    let shown = images(&scene);
    // Twice as wide as tall into a square: the middle half of the width shows.
    assert!((shown[0].uv.w - 0.5).abs() < 1e-5 && (shown[0].uv.x - 0.25).abs() < 1e-5, "{:?}", shown[0].uv);
}

#[test]
fn a_tile_with_neither_is_a_grey_box_and_a_ring_is_a_hairline() {
    let mut t = TestUi::<()>::new(200.0, 200.0);
    t.view(column().align_items(Align::Start).child(image_tile::<()>(None).key("t").size(60.0, 60.0).ring()));
    let scene = t.scene();
    assert!(images(&scene).is_empty());
    assert_eq!(with_fill(&scene, Tone::Wash).len(), 1);
    let ring = quads(&scene).into_iter().find(|q| q.border.is_some()).expect("the ring");
    assert_eq!(ring.border.unwrap().width, 1.0);
}

#[test]
fn a_tile_takes_its_height_from_the_picture_when_only_the_width_is_set() {
    let mut t = TestUi::<()>::new(200.0, 200.0);
    let id = t.ui.resources().images.insert(ImageData::solid(80, 40, Color::WHITE));
    t.view(column().align_items(Align::Start).child(image_tile(Some(id)).key("t").width(120.0)));
    let r = t.rect("t");
    assert_eq!((r.w, r.h), (120.0, 60.0));
}

#[test]
fn badges_keycaps_and_avatars_are_text_and_boxes_with_no_pictures() {
    let mut t = still(column().children((badge("A"), kbd("B"), avatar("C D"))));
    assert!(images(&t.scene()).is_empty());
}
