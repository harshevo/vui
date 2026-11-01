//! Rendering tests for vector meshes: exact pixels for the simple cases, layering with quads, clipping and
//! opacity, and a golden sheet of fills, strokes and curves to read by eye.

use std::rc::Rc;

use vui_core::vector::{Cap, Join, Path, Stroke, Transform, Vector};
use vui_core::{Color, MeshCmd, Quad, Scene, pt, rect, size};
use vui_render::testing::{render_scene, shared_gpu};
use vui_render::{Image, assert_golden};

fn render(scene: &Scene) -> Image {
    render_scene(scene, &vui_core::Fonts::bundled_only())
}

const RED: Color = Color::rgb(1.0, 0.0, 0.0);
const BLUE: Color = Color::rgb(0.1, 0.3, 0.9);
const INK: Color = Color::rgb(0.1, 0.1, 0.12);

fn mesh_of(scale: f32, f: impl FnOnce(&mut Vector)) -> Rc<vui_core::vector::Mesh> {
    let mut v = Vector::new(scale);
    f(&mut v);
    Rc::new(v.finish())
}

#[test]
fn a_filled_path_has_solid_inside_clean_edges_and_untouched_outside() {
    let mut s = Scene::new(size(40.0, 40.0), 1.0).with_clear(Color::WHITE);
    let mesh = mesh_of(1.0, |v| {
        v.fill(&Path::rect(rect(10.0, 10.0, 20.0, 20.0)), &Transform::IDENTITY, RED);
    });
    s.mesh(MeshCmd::new(mesh));
    let img = render(&s);
    assert_eq!(img.pixel(20, 20), [255, 0, 0, 255]);
    assert_eq!(img.pixel(10, 10), [255, 0, 0, 255], "the corner pixel is inside");
    assert_eq!(img.pixel(29, 29), [255, 0, 0, 255]);
    assert_eq!(img.pixel(8, 20), [255, 255, 255, 255], "two pixels away is clean");
    assert_eq!(img.pixel(31, 20), [255, 255, 255, 255]);
}

#[test]
fn a_diagonal_edge_is_antialiased() {
    let mut s = Scene::new(size(20.0, 20.0), 1.0).with_clear(Color::WHITE);
    let tri = Path::polyline(&[pt(0.0, 0.0), pt(20.0, 0.0), pt(0.0, 20.0)], true);
    s.mesh(MeshCmd::new(mesh_of(1.0, |v| {
        v.fill(&tri, &Transform::IDENTITY, Color::BLACK);
    })));
    let img = render(&s);
    // Along the hypotenuse the pixels step between white and black through greys.
    let edge: Vec<u8> = (0..20).map(|i| img.pixel(19 - i, i)[0]).collect();
    assert!(edge.iter().all(|c| *c > 20 && *c < 235), "every pixel on the edge is partly covered: {edge:?}");
    assert_eq!(img.pixel(3, 3)[0], 0);
    assert_eq!(img.pixel(17, 17)[0], 255);
}

#[test]
fn meshes_layer_with_quads_in_order() {
    let mut s = Scene::new(size(40.0, 20.0), 1.0).with_clear(Color::WHITE);
    s.quad(Quad::new(rect(0.0, 0.0, 30.0, 20.0)).fill(RED));
    s.mesh(MeshCmd::new(mesh_of(1.0, |v| {
        v.fill(&Path::rect(rect(10.0, 0.0, 30.0, 20.0)), &Transform::IDENTITY, BLUE);
    })));
    s.quad(Quad::new(rect(20.0, 5.0, 5.0, 10.0)).fill(Color::WHITE));
    let img = render(&s);
    assert_eq!(img.pixel(5, 10), [255, 0, 0, 255]);
    assert!(img.pixel(15, 2).iter().zip([26, 77, 230, 255]).all(|(a, b): (&u8, u8)| a.abs_diff(b) <= 1), "the mesh is over the first quad: {:?}", img.pixel(15, 2));
    assert_eq!(img.pixel(22, 10), [255, 255, 255, 255], "the last quad is over the mesh");
}

#[test]
fn a_mesh_is_cut_to_its_clip_and_faded_by_its_opacity() {
    let mut s = Scene::new(size(40.0, 20.0), 1.0).with_clear(Color::WHITE);
    let mesh = mesh_of(1.0, |v| {
        v.fill(&Path::rect(rect(0.0, 0.0, 40.0, 20.0)), &Transform::IDENTITY, Color::BLACK);
    });
    s.mesh(MeshCmd { mesh: mesh.clone(), clip: Some(rect(0.0, 0.0, 10.0, 20.0)), opacity: 1.0 });
    s.mesh(MeshCmd { mesh, clip: Some(rect(20.0, 0.0, 10.0, 20.0)), opacity: 0.5 });
    let img = render(&s);
    assert_eq!(img.pixel(5, 10)[0], 0);
    assert_eq!(img.pixel(15, 10)[0], 255, "outside both clips");
    assert!(img.pixel(25, 10)[0].abs_diff(128) <= 2, "half faded: {}", img.pixel(25, 10)[0]);
    assert_eq!(img.pixel(35, 10)[0], 255);
    // And nothing the first clip excluded leaked into the draw after it.
    assert_eq!(img.pixel(12, 10)[0], 255);
}

#[test]
fn a_retina_frame_keeps_lines_one_device_pixel_fine() {
    let mut s = Scene::new(size(20.0, 20.0), 2.0).with_clear(Color::WHITE);
    s.mesh(MeshCmd::new(mesh_of(2.0, |v| {
        v.polyline(&[pt(2.0, 10.25), pt(18.0, 10.25)], &Stroke::new(0.5), Color::BLACK);
    })));
    let img = render(&s);
    // Half a point is one device pixel at 2×: one solid-ish row, clean rows beside it.
    let col = 20;
    let rows: Vec<u8> = (18..24).map(|y| img.pixel(col, y)[0]).collect();
    assert!(rows.iter().filter(|c| **c < 100).count() >= 1, "a dark row: {rows:?}");
    assert_eq!(rows[0], 255);
    assert_eq!(rows[5], 255);
}

#[test]
fn a_sheet_of_fills_strokes_and_curves() {
    let scale = 2.0;
    let mut s = Scene::new(size(360.0, 240.0), scale).with_clear(Color::rgb(0.98, 0.98, 0.98));
    let id = Transform::IDENTITY;
    let mesh = mesh_of(scale, |v| {
        // Row 1: a circle, a rounded rect, a donut, a star.
        v.fill(&Path::circle(pt(40.0, 40.0), 26.0), &id, INK);
        v.fill(&Path::rounded_rect(rect(80.0, 14.0, 70.0, 52.0), 14.0), &id, BLUE);
        let mut donut = Path::circle(pt(195.0, 40.0), 26.0);
        for seg in Path::circle(pt(195.0, 40.0), 12.0).segments() {
            match *seg {
                vui_core::vector::Seg::Move(a) => donut.move_to(a),
                vui_core::vector::Seg::Line(a) => donut.line_to(a),
                vui_core::vector::Seg::Quad(c, a) => donut.quad_to(c, a),
                vui_core::vector::Seg::Cubic(a, b, c) => donut.cubic_to(a, b, c),
                vui_core::vector::Seg::Close => donut.close(),
            };
        }
        v.fill(&donut, &id, INK);
        let star: Vec<_> = (0..10)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { 28.0 } else { 12.0 };
                pt(300.0 + r * a.cos(), 40.0 + r * a.sin())
            })
            .collect();
        v.fill(&Path::polyline(&star, true), &id, BLUE);

        // Row 2: caps, joins and dashes.
        let zig = [pt(20.0, 140.0), pt(50.0, 100.0), pt(80.0, 140.0), pt(110.0, 100.0)];
        for (i, (cap, join)) in [(Cap::Butt, Join::Miter), (Cap::Round, Join::Round), (Cap::Square, Join::Bevel)].into_iter().enumerate() {
            let dy = i as f32 * 0.0;
            let pts: Vec<_> = zig.iter().map(|p| pt(p.x + i as f32 * 115.0, p.y + dy)).collect();
            v.polyline(&pts, &Stroke::new(10.0).cap(cap).join(join), INK);
            v.polyline(&pts, &Stroke::new(1.0), Color::WHITE);
        }

        // Row 3: dashes, a curve, an area with its line.
        v.polyline(&[pt(16.0, 175.0), pt(140.0, 175.0)], &Stroke::new(3.0).dashed(&[8.0, 6.0], 0.0).cap(Cap::Round), INK);
        let mut curve = Path::new();
        curve.move_to(pt(16.0, 215.0)).cubic_to(pt(60.0, 160.0), pt(100.0, 250.0), pt(140.0, 200.0));
        v.stroke(&curve, &id, &Stroke::new(2.5).cap(Cap::Round), BLUE);
        let top: Vec<_> = (0..=40).map(|i| pt(170.0 + i as f32 * 4.5, 205.0 - 22.0 * (i as f32 / 5.0).sin().abs() - i as f32 * 0.5)).collect();
        let base: Vec<_> = top.iter().map(|p| pt(p.x, 228.0)).collect();
        v.band(&top, &base, Color::rgba(0.1, 0.3, 0.9, 0.18));
        v.polyline(&top, &Stroke::new(2.0).join(Join::Round), BLUE);
    });
    s.mesh(MeshCmd::new(mesh));
    let img = render(&s);
    assert_golden!("vector_sheet", img);
}

#[test]
fn the_mesh_pass_runs_on_a_real_gpu() {
    assert_ne!(shared_gpu().info().device_type, wgpu::DeviceType::Cpu);
}
