mod common;

use common::*;
use vui_core::{Color, Command, Corners, ImageData, ImageMode, Theme, Tone, pt, rect, size, theme::PILL};
use vui_runtime::testing::{TestUi, quads, texts};
use vui_runtime::{Builder, Parent, Radius, Styled, column, div, img, label};

fn fills(t: &mut TestUi<Msg>) -> Vec<Color> {
    let scene = t.scene();
    quads(&scene).iter().map(|q| q.fill).collect()
}

#[test]
fn boxes_paint_before_their_children_and_text_comes_last() {
    let mut t = ui(200.0, 100.0);
    t.view(column().bg(Tone::Side).child(div().key("card").height(50.0).bg(Tone::Wash)).child(label("Hi")));
    let scene = t.scene();
    let p = t.ui.theme().palette.clone();
    assert_eq!(scene.clear, Some(p.bg));
    assert_eq!(scene.commands.len(), 3);
    match (&scene.commands[0], &scene.commands[1], &scene.commands[2]) {
        (Command::Quad(root), Command::Quad(card), Command::Text(text)) => {
            assert_eq!((root.rect, root.fill), (rect(0.0, 0.0, 200.0, 100.0), p.side));
            assert_eq!((card.rect, card.fill), (rect(0.0, 0.0, 200.0, 50.0), p.wash));
            assert_eq!(text.layout.text(), "Hi");
            assert_eq!(text.origin, pt(0.0, 50.0));
            assert_eq!(text.color, p.text);
        }
        other => panic!("unexpected scene {other:?}"),
    }
}

#[test]
fn an_invisible_box_draws_nothing() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(div().height(20.0)));
    assert!(t.scene().commands.is_empty());
}

#[test]
fn colours_follow_the_theme() {
    let mut t = ui(200.0, 100.0);
    t.view(column().bg(Tone::Side).child(label("Hi")));
    let light = Theme::light().palette;
    assert_eq!(fills(&mut t), [light.side]);
    t.ui.set_theme(Theme::dark());
    let scene = t.scene();
    let dark = Theme::dark().palette;
    assert_eq!(scene.clear, Some(dark.bg));
    assert_eq!(quads(&scene)[0].fill, dark.side);
    match scene.commands.last() {
        Some(Command::Text(text)) => assert_eq!(text.color, dark.text),
        other => panic!("{other:?}"),
    }
}

#[test]
fn fixed_colours_are_left_alone() {
    let mut t = ui(200.0, 100.0);
    let red = Color::hex(0xff0000);
    t.view(column().bg(red));
    t.ui.set_theme(Theme::dark());
    assert_eq!(fills(&mut t), [red]);
}

#[test]
fn corner_tokens_come_from_the_theme() {
    let theme = Theme::light();
    let mut t = ui(200.0, 400.0);
    t.view(column().children((
        div().height(20.0).bg(Tone::Wash).rounded(Radius::Control),
        div().height(20.0).bg(Tone::Wash).rounded(Radius::Card),
        div().height(20.0).bg(Tone::Wash).rounded(Radius::Panel),
        div().height(20.0).bg(Tone::Wash).rounded(Radius::Pill),
        div().height(20.0).bg(Tone::Wash).rounded(5.0),
        div().height(20.0).bg(Tone::Wash).rounded(Radius::Corners(Corners::top(8.0))),
    )));
    let scene = t.scene();
    let q = quads(&scene);
    assert_eq!(q[0].radii, Corners::all(theme.radii.control));
    assert_eq!(q[1].radii, Corners::all(theme.radii.card));
    assert_eq!(q[2].radii, Corners::all(theme.radii.panel));
    assert_eq!((q[3].radii, q[3].smooth), (Corners::all(PILL), 0.0), "a pill's ends are true semicircles");
    assert_eq!(q[4].radii, Corners::all(5.0));
    assert_eq!(q[5].radii, Corners::top(8.0));
    assert!(q.iter().enumerate().all(|(i, q)| i == 3 || q.smooth == theme.radii.smooth));
}

#[test]
fn a_border_is_drawn_inside_the_box() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(div().height(40.0).border(1.0, Tone::Line)));
    let scene = t.scene();
    let q = quads(&scene);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].fill.a, 0.0);
    let b = q[0].border.expect("a border");
    assert_eq!((b.width, b.color), (1.0, Theme::light().palette.line));
}

#[test]
fn text_sits_inside_the_padding() {
    let mut t = ui(200.0, 100.0);
    t.view(column().padding(10.0).child(label("Hi").padding(4.0)));
    let scene = t.scene();
    match &scene.commands[0] {
        Command::Text(text) => assert_eq!(text.origin, pt(14.0, 14.0)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn opacity_multiplies_into_everything_inside() {
    let mut t = ui(200.0, 100.0);
    t.view(column().opacity(0.5).bg(Tone::Wash).child(label("x")).child(div().height(10.0).bg(Tone::Side).opacity(0.5)));
    let scene = t.scene();
    let q = quads(&scene);
    assert_eq!(q[0].fill.a, 0.5);
    assert_eq!(q[1].fill.a, 0.25, "opacities of nested elements multiply");
    match &scene.commands[1] {
        Command::Text(text) => assert_eq!(text.color.a, 0.5),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_disabled_element_is_dimmed() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(div().height(20.0).bg(Tone::Wash).disabled(true).child(label("x"))));
    let scene = t.scene();
    assert!((quads(&scene)[0].fill.a - 0.45).abs() < 1e-6);
    match &scene.commands[1] {
        Command::Text(text) => assert!((text.color.a - 0.45).abs() < 1e-6),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_clipping_container_cuts_its_own_text_too() {
    let mut t = ui(200.0, 100.0);
    t.view(column().child(
        div().key("box").size(60.0, 30.0).overflow(vui_runtime::Overflow::Hidden).child(label("a long line of text").no_wrap()),
    ));
    let scene = t.scene();
    match &scene.commands[0] {
        Command::Text(text) => assert_eq!(text.clip, Some(rect(0.0, 0.0, 60.0, 30.0))),
        other => panic!("{other:?}"),
    }
}

#[test]
fn edges_snap_to_device_pixels() {
    let mut t = TestUi::<Msg>::with_theme(size(100.0, 100.0), Theme::light(), 2.0);
    t.view(column().padding(0.3).child(div().height(10.1).bg(Tone::Wash)));
    let scene = t.scene();
    assert_eq!(scene.scale, 2.0);
    let r = quads(&scene)[0].rect;
    assert_eq!((r.x, r.y), (0.5, 0.5), "0.3 rounds to the nearest half pixel at 2x");
    assert_eq!(r.bottom(), 10.5, "10.4 rounds to 10.5");
}

#[test]
fn an_image_fills_its_box_with_the_element_corners() {
    let mut t = ui(200.0, 100.0);
    let id = t.ui.resources().images.insert(ImageData::solid(40, 40, Color::WHITE));
    t.view(column().child(img(id).size(100.0, 50.0).rounded(Radius::Card).ring()));
    let scene = t.scene();
    let theme = Theme::light();
    match &scene.commands[0] {
        Command::Image(cmd) => {
            assert_eq!(cmd.id, id);
            assert_eq!(cmd.rect, rect(0.0, 0.0, 100.0, 50.0));
            assert_eq!(cmd.radii, Corners::all(theme.radii.card));
            assert_eq!(cmd.uv, rect(0.0, 0.25, 1.0, 0.5), "a square picture cropped to a wide box keeps its middle");
            assert_eq!(cmd.mode, ImageMode::Color);
            assert_eq!(cmd.ring.map(|r| r.width), Some(1.0));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_icon_is_a_tinted_stencil() {
    let mut t = ui(200.0, 100.0);
    let id = t.ui.resources().images.insert(ImageData::solid(16, 16, Color::BLACK));
    t.view(column().child(img(id).tint(Tone::Accent).size(16.0, 16.0)));
    let scene = t.scene();
    match &scene.commands[0] {
        Command::Image(cmd) => {
            assert_eq!(cmd.mode, ImageMode::Mask);
            assert_eq!(cmd.tint, Theme::light().palette.accent);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_image_with_no_size_is_as_big_as_the_picture_and_shrinks_to_fit() {
    let mut t = ui(100.0, 100.0);
    let id = t.ui.resources().images.insert(ImageData::solid(300, 150, Color::WHITE));
    t.view(column().align_items(vui_runtime::Align::Start).child(img(id).key("pic")));
    let r = t.rect("pic");
    assert_eq!((r.w, r.h), (100.0, 50.0), "300x150 squeezed into 100 wide keeps its shape");
}

#[test]
fn the_painted_text_uses_the_labels_own_style() {
    let mut t = ui(300.0, 200.0);
    t.view(column().children((
        label("body"),
        vui_runtime::title("title"),
        vui_runtime::small("small"),
        label("mono").mono(),
    )));
    assert_eq!(texts(&t.scene()), ["body", "title", "small", "mono"]);
    let scene = t.scene();
    let heights: Vec<f32> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text(t) => Some(t.layout.size().h),
            _ => None,
        })
        .collect();
    let types = Theme::light().types;
    assert_eq!(heights, [types.body.line_height, types.title.line_height, types.small.line_height, types.body.line_height]);
}
