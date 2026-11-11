//! Layers: commands drawn on their own and laid over the frame moved, scaled, turned and faded as
//! one, the same on the GPU and the CPU.

use vui_core::{Affine, Color, Command, Fonts, LayerCmd, Quad, Rect, Resources, Scene, Shadow, effects, pt, rect, size};
use vui_render::testing::{render_cpu, render_scene, try_shared_gpu, renderer_for};
use vui_render::{Image, assert_golden};

fn render(scene: &Scene) -> Image {
    render_scene(scene, &Fonts::bundled_only())
}

fn layer(bounds: Rect, transform: Affine, opacity: f32, commands: Vec<Command>) -> Command {
    Command::Layer(Box::new(LayerCmd { bounds, transform, opacity, clip: None, commands }))
}

fn square(r: Rect, color: Color) -> Command {
    Command::Quad(Quad::new(r).fill(color))
}

fn red(img: &Image, x: u32, y: u32) -> bool {
    let [r, g, b, _] = img.pixel(x, y);
    r > 200 && g < 60 && b < 60
}

#[test]
fn an_identity_layer_draws_what_its_commands_would() {
    let commands = vec![square(rect(10.0, 10.0, 30.0, 20.0), Color::rgb(0.9, 0.2, 0.1)), square(rect(20.0, 15.0, 30.0, 30.0), Color::rgba(0.1, 0.3, 0.9, 0.6))];
    let mut plain = Scene::new(size(64.0, 64.0), 2.0).with_clear(Color::WHITE);
    plain.commands = commands.clone();
    let mut layered = Scene::new(size(64.0, 64.0), 2.0).with_clear(Color::WHITE);
    layered.push(layer(rect(0.0, 0.0, 64.0, 64.0), Affine::IDENTITY, 1.0, commands));
    assert_eq!(render(&plain).rgba, render(&layered).rgba);
}

#[test]
fn a_faded_layer_fades_as_one() {
    // Two opaque squares overlapping: at half opacity the overlap is no darker than either.
    let mut s = Scene::new(size(80.0, 40.0), 1.0).with_clear(Color::WHITE);
    let black = Color::BLACK;
    s.push(layer(rect(0.0, 0.0, 80.0, 40.0), Affine::IDENTITY, 0.5, vec![square(rect(10.0, 10.0, 30.0, 20.0), black), square(rect(30.0, 10.0, 30.0, 20.0), black)]));
    let img = render(&s);
    let (alone, overlap) = (img.pixel(15, 20)[0], img.pixel(35, 20)[0]);
    assert!((alone as i32 - 128).abs() <= 2, "half grey: {alone}");
    assert_eq!(alone, overlap, "the overlap does not show through");
}

#[test]
fn a_layer_scales_and_turns_about_its_origin() {
    let r = rect(40.0, 40.0, 20.0, 20.0);
    let centre = pt(50.0, 50.0);
    let mut s = Scene::new(size(100.0, 100.0), 1.0).with_clear(Color::WHITE);
    s.push(layer(r.inflate(4.0, 4.0), Affine::scale(2.0, 2.0).around(centre), 1.0, vec![square(r, Color::rgb(1.0, 0.0, 0.0))]));
    let img = render(&s);
    assert!(red(&img, 32, 50) && red(&img, 67, 50), "twice as wide about its centre");
    assert!(!red(&img, 28, 50) && !red(&img, 72, 50));

    let tall = rect(45.0, 30.0, 10.0, 40.0);
    let mut s = Scene::new(size(100.0, 100.0), 1.0).with_clear(Color::WHITE);
    s.push(layer(tall.inflate(4.0, 4.0), Affine::rotate(90.0).around(centre), 1.0, vec![square(tall, Color::rgb(1.0, 0.0, 0.0))]));
    let img = render(&s);
    assert!(red(&img, 33, 50) && red(&img, 66, 50), "a quarter turn lays it on its side");
    assert!(!red(&img, 50, 33) && !red(&img, 50, 66));
}

#[test]
fn layers_nest_and_clip() {
    let r = rect(20.0, 20.0, 20.0, 20.0);
    let inner = layer(r, Affine::translate(30.0, 0.0), 1.0, vec![square(r, Color::rgb(1.0, 0.0, 0.0))]);
    let mut s = Scene::new(size(100.0, 60.0), 1.0).with_clear(Color::WHITE);
    s.push(layer(rect(0.0, 0.0, 100.0, 60.0), Affine::translate(0.0, 10.0), 1.0, vec![inner]));
    let img = render(&s);
    assert!(red(&img, 60, 40), "both moves add up");
    assert!(!red(&img, 30, 30));

    let mut s = Scene::new(size(100.0, 60.0), 1.0).with_clear(Color::WHITE);
    s.push(Command::Layer(Box::new(LayerCmd {
        bounds: r,
        transform: Affine::IDENTITY,
        opacity: 1.0,
        clip: Some(rect(0.0, 0.0, 30.0, 60.0)),
        commands: vec![square(r, Color::rgb(1.0, 0.0, 0.0))],
    })));
    let img = render(&s);
    assert!(red(&img, 25, 30) && !red(&img, 35, 30), "cut at the clip");
}

#[test]
fn a_flat_or_offscreen_layer_draws_nothing() {
    let r = rect(10.0, 10.0, 20.0, 20.0);
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    s.push(layer(r, Affine::scale(0.0, 1.0), 1.0, vec![square(r, Color::BLACK)]));
    s.push(layer(rect(100.0, 100.0, 20.0, 20.0), Affine::translate(-90.0, -90.0), 1.0, vec![square(r, Color::BLACK)]));
    assert!(render(&s).rgba.iter().all(|&v| v == 255));
}

fn sheet() -> Scene {
    let fonts = Fonts::bundled_only();
    let mut s = Scene::new(size(360.0, 140.0), 2.0).with_clear(Color::rgb(0.96, 0.96, 0.97));
    for (i, (transform, opacity)) in [(Affine::IDENTITY, 1.0), (Affine::rotate(-8.0), 1.0), (Affine::scale(0.85, 0.85), 0.6), (Affine::rotate(12.0).then(Affine::translate(0.0, 6.0)), 0.9)]
        .into_iter()
        .enumerate()
    {
        let card = rect(16.0 + i as f32 * 86.0, 30.0, 70.0, 80.0);
        let centre = pt(card.x + card.w / 2.0, card.y + card.h / 2.0);
        let label = std::rc::Rc::new(fonts.layout(&format!("Card {}", i + 1), &vui_core::TextStyle::new(13.0, 18.0), None));
        let commands = vec![
            Command::Custom(effects::shadow(card, 10.0, Shadow::RAISED)),
            Command::Quad(Quad::new(card).fill(Color::WHITE).radii(10.0).border(1.0, Color::rgb(0.85, 0.85, 0.88))),
            Command::Quad(Quad::new(rect(card.x + 10.0, card.y + 10.0, 50.0, 30.0)).fill(Color::rgb(0.25, 0.45, 0.95)).radii(6.0)),
            Command::Text(vui_core::TextCmd { layout: label, origin: pt(card.x + 10.0, card.y + 50.0), color: Color::rgb(0.1, 0.1, 0.12), clip: None }),
        ];
        s.push(layer(card.inflate(24.0, 24.0), transform.around(centre), opacity, commands));
    }
    s
}

#[test]
fn layers_sheet() {
    assert_golden!("layers_sheet", render(&sheet()));
}

#[test]
fn the_gpu_and_the_cpu_agree_on_layers() {
    if try_shared_gpu().is_none() {
        return;
    }
    let res = Resources { fonts: Fonts::bundled_only(), images: Default::default() };
    let scene = sheet();
    let gpu = renderer_for(&res).render_to_image(&scene).unwrap();
    let cpu = render_cpu(&scene, &res);
    let worst = gpu.rgba.iter().zip(&cpu.rgba).map(|(a, b)| (*a as i32 - *b as i32).abs()).max().unwrap();
    assert!(worst <= 2, "largest difference {worst}");
}
