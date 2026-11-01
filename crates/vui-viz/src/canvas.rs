//! A drawing surface for data pictures: vector shapes, rounded quads and text, kept in the order they
//! were drawn so later marks sit over earlier ones. It produces scene commands and touches nothing else,
//! so everything drawn through it can be checked without a window.

use std::rc::Rc;

use vui_core::vector::{Mesh, Vector};
use vui_core::{
    Color, Command, Fonts, MeshCmd, Point, Quad, Rect, Scene, TextCmd, TextStyle, pt, rect,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HAnchor {
    Left,
    Center,
    Right,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VAnchor {
    Top,
    Middle,
    Bottom,
}

pub struct Canvas<'a> {
    fonts: &'a Fonts,
    scale: f32,
    items: Vec<Command>,
    vector: Vector,
    clip: Option<Rect>,
}

impl<'a> Canvas<'a> {
    /// `scale` is the device pixels per point, which sets how fine edges and curves are cut.
    pub fn new(fonts: &'a Fonts, scale: f32) -> Canvas<'a> {
        Canvas {
            fonts,
            scale,
            items: Vec::new(),
            vector: Vector::new(scale),
            clip: None,
        }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    pub fn fonts(&self) -> &Fonts {
        self.fonts
    }

    /// The vector builder for shapes drawn from here until the next quad or text.
    pub fn vector(&mut self) -> &mut Vector {
        &mut self.vector
    }

    fn flush(&mut self) {
        if !self.vector.mesh.is_empty() {
            let mesh = std::mem::replace(&mut self.vector.mesh, Mesh::new());
            let mut cmd = MeshCmd::new(Rc::new(mesh));
            cmd.clip = self.clip;
            self.items.push(Command::Mesh(cmd));
        }
    }

    /// Cuts everything drawn from here on to `clip`, until it is changed again.
    pub fn set_clip(&mut self, clip: Option<Rect>) {
        self.flush();
        self.clip = clip;
    }

    pub fn quad(&mut self, quad: Quad) {
        self.flush();
        self.items.push(Command::Quad(quad.clip(self.clip)));
    }

    /// A line one device pixel thick, lying on the pixel grid so it stays crisp.
    pub fn hairline(&mut self, from: Point, to: Point, color: Color) {
        let px = 1.0 / self.scale;
        let snap = |v: f32| (v * self.scale).floor() / self.scale;
        let r = if (from.y - to.y).abs() < (from.x - to.x).abs() {
            let y = snap(from.y);
            rect(from.x.min(to.x), y, (to.x - from.x).abs(), px)
        } else {
            let x = snap(from.x);
            rect(x, from.y.min(to.y), px, (to.y - from.y).abs())
        };
        self.quad(Quad::new(r).fill(color));
    }

    /// A filled circle drawn as a signed-distance quad: crisper than a tessellated one at small sizes.
    pub fn dot(&mut self, center: Point, radius: f32, color: Color) {
        self.quad(
            Quad::new(rect(
                center.x - radius,
                center.y - radius,
                radius * 2.0,
                radius * 2.0,
            ))
            .fill(color)
            .radii(radius),
        );
    }

    /// A ring: a dot with a hollow middle.
    pub fn ring(&mut self, center: Point, radius: f32, width: f32, fill: Color, line: Color) {
        self.quad(
            Quad::new(rect(
                center.x - radius,
                center.y - radius,
                radius * 2.0,
                radius * 2.0,
            ))
            .fill(fill)
            .radii(radius)
            .border(width, line),
        );
    }

    /// Writes `text` anchored at `at`. Returns the box the text occupies.
    pub fn text(
        &mut self,
        text: &str,
        style: &TextStyle,
        at: Point,
        h: HAnchor,
        v: VAnchor,
        color: Color,
    ) -> Rect {
        if text.is_empty() || color.a <= 0.0 {
            return Rect::ZERO;
        }
        let layout = self.fonts.layout_cached(text, style, None);
        let size = layout.size();
        let x = match h {
            HAnchor::Left => at.x,
            HAnchor::Center => at.x - size.w / 2.0,
            HAnchor::Right => at.x - size.w,
        };
        let y = match v {
            VAnchor::Top => at.y,
            VAnchor::Middle => at.y - size.h / 2.0,
            VAnchor::Bottom => at.y - size.h,
        };
        self.flush();
        self.items.push(Command::Text(TextCmd {
            layout,
            origin: pt(x, y),
            color,
            clip: self.clip,
        }));
        rect(x, y, size.w, size.h)
    }

    pub fn measure(&self, text: &str, style: &TextStyle) -> vui_core::Size {
        self.fonts.measure(text, style, None)
    }

    pub fn finish(mut self) -> Vec<Command> {
        self.flush();
        self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.vector.mesh.is_empty()
    }
}

/// Puts drawn commands into a scene, cut to `clip` and faded by `alpha`.
pub fn paint(scene: &mut Scene, commands: Vec<Command>, clip: Option<Rect>, alpha: f32) {
    for c in commands {
        match c {
            Command::Quad(mut q) => {
                q.fill = q.fill.mul_alpha(alpha);
                if let Some(b) = q.border.as_mut() {
                    b.color = b.color.mul_alpha(alpha);
                }
                q.clip = both(q.clip, clip);
                scene.quad(q);
            }
            Command::Mesh(mut m) => {
                m.clip = both(m.clip, clip);
                m.opacity *= alpha;
                scene.mesh(m);
            }
            Command::Text(mut t) => {
                t.color = t.color.mul_alpha(alpha);
                t.clip = both(t.clip, clip);
                scene.push(Command::Text(t));
            }
            other => scene.push(other),
        }
    }
}

/// Where two clips overlap. Nothing at all is drawn when they do not.
fn both(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.intersection(b).unwrap_or(Rect::ZERO)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vui_core::Fonts;
    use vui_core::vector::Stroke;

    #[test]
    fn items_keep_the_order_they_were_drawn_in() {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 2.0);
        c.vector().polyline(
            &[pt(0.0, 0.0), pt(10.0, 0.0)],
            &Stroke::new(1.0),
            Color::BLACK,
        );
        c.dot(pt(5.0, 5.0), 3.0, Color::BLACK);
        c.vector().polyline(
            &[pt(0.0, 9.0), pt(10.0, 9.0)],
            &Stroke::new(1.0),
            Color::BLACK,
        );
        let area = c.text(
            "12",
            &TextStyle::new(12.0, 16.0),
            pt(0.0, 20.0),
            HAnchor::Left,
            VAnchor::Top,
            Color::BLACK,
        );
        let kinds: Vec<&str> = c
            .finish()
            .iter()
            .map(|i| match i {
                Command::Mesh(_) => "mesh",
                Command::Quad(_) => "quad",
                Command::Text(_) => "text",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, ["mesh", "quad", "mesh", "text"]);
        assert!(area.w > 0.0 && area.h > 0.0);
    }

    #[test]
    fn text_anchors_place_the_box() {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 1.0);
        let style = TextStyle::new(12.0, 16.0);
        let left = c.text(
            "Hello",
            &style,
            pt(100.0, 50.0),
            HAnchor::Left,
            VAnchor::Top,
            Color::BLACK,
        );
        let right = c.text(
            "Hello",
            &style,
            pt(100.0, 50.0),
            HAnchor::Right,
            VAnchor::Bottom,
            Color::BLACK,
        );
        let centre = c.text(
            "Hello",
            &style,
            pt(100.0, 50.0),
            HAnchor::Center,
            VAnchor::Middle,
            Color::BLACK,
        );
        assert_eq!((left.x, left.y), (100.0, 50.0));
        assert!((right.right() - 100.0).abs() < 1e-3 && (right.bottom() - 50.0).abs() < 1e-3);
        assert!(
            (centre.center().x - 100.0).abs() < 1e-3 && (centre.center().y - 50.0).abs() < 1e-3
        );
        assert_eq!(
            c.text(
                "",
                &style,
                pt(0.0, 0.0),
                HAnchor::Left,
                VAnchor::Top,
                Color::BLACK
            ),
            Rect::ZERO
        );
    }

    #[test]
    fn a_set_clip_follows_every_item_and_meets_the_outer_one() {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 1.0);
        let inner = rect(0.0, 0.0, 10.0, 10.0);
        c.set_clip(Some(inner));
        c.dot(pt(5.0, 5.0), 3.0, Color::BLACK);
        c.vector().polyline(
            &[pt(0.0, 0.0), pt(10.0, 0.0)],
            &Stroke::new(1.0),
            Color::BLACK,
        );
        c.set_clip(None);
        c.dot(pt(5.0, 5.0), 3.0, Color::BLACK);
        let items = c.finish();
        let clips: Vec<Option<Rect>> = items
            .iter()
            .map(|i| match i {
                Command::Quad(q) => q.clip,
                Command::Mesh(m) => m.clip,
                _ => None,
            })
            .collect();
        assert_eq!(clips, [Some(inner), Some(inner), None]);
        let mut scene = Scene::new(vui_core::size(20.0, 20.0), 1.0);
        paint(&mut scene, items, Some(rect(5.0, 5.0, 20.0, 20.0)), 1.0);
        match &scene.commands[0] {
            Command::Quad(q) => assert_eq!(q.clip, Some(rect(5.0, 5.0, 5.0, 5.0))),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn hairlines_are_one_device_pixel_on_the_grid() {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 2.0);
        c.hairline(pt(0.0, 10.3), pt(40.0, 10.3), Color::BLACK);
        c.hairline(pt(7.2, 0.0), pt(7.2, 30.0), Color::BLACK);
        let items = c.finish();
        let rects: Vec<Rect> = items
            .iter()
            .filter_map(|i| {
                if let Command::Quad(q) = i {
                    Some(q.rect)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(rects[0], rect(0.0, 10.0, 40.0, 0.5));
        assert_eq!(rects[1], rect(7.0, 0.0, 0.5, 30.0));
    }

    #[test]
    fn painting_clips_and_fades_everything() {
        let fonts = Fonts::bundled_only();
        let mut c = Canvas::new(&fonts, 1.0);
        c.dot(pt(5.0, 5.0), 3.0, Color::BLACK);
        c.vector().polyline(
            &[pt(0.0, 0.0), pt(10.0, 0.0)],
            &Stroke::new(1.0),
            Color::BLACK,
        );
        let mut scene = Scene::new(vui_core::size(20.0, 20.0), 1.0);
        let clip = rect(0.0, 0.0, 8.0, 8.0);
        paint(&mut scene, c.finish(), Some(clip), 0.5);
        assert_eq!(scene.commands.len(), 2);
        match (&scene.commands[0], &scene.commands[1]) {
            (Command::Quad(q), Command::Mesh(m)) => {
                assert_eq!(q.clip, Some(clip));
                assert_eq!(q.fill.a, 0.5);
                assert_eq!((m.clip, m.opacity), (Some(clip), 0.5));
            }
            other => panic!("{other:?}"),
        }
    }
}
