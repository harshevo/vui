//! Rendering tests for pictures: colour accuracy, filtering in linear light, crops and corner shapes,
//! transparency, the GPU texture cache, layering with shapes, and a golden sheet to read by eye.

use vui_core::theme::PILL;
use vui_core::{
    Color, Corners, Fit, ImageCmd, ImageData, Quad, Resources, Sampling, Scene, TextStyle, Theme, fit_image, pt, rect, size,
};
use vui_render::testing::{render_with, renderer_for};
use vui_render::{Image, assert_golden};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const ACCENT: Color = Color::rgb(37.0 / 255.0, 99.0 / 255.0, 235.0 / 255.0);

fn near(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a.iter().zip(&b).all(|(x, y)| x.abs_diff(*y) <= tol)
}

fn solid(r: u8, g: u8, b: u8, a: u8) -> ImageData {
    ImageData::from_fn(1, 1, |_, _| [r, g, b, a])
}

mod drawing;
mod sampling;
mod sheet;
mod textures;

use sheet::photo;
