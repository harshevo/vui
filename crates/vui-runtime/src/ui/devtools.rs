//! Tools for looking inside a running UI: an inspector that outlines the element under the pointer
//! and says what it is, and frame stats in the corner.
//!
//! Cmd-Option-I switches the inspector and Cmd-Option-S the stats, in any vui window. Set
//! `VUI_INSPECT=1` or `VUI_STATS=1` to start with them on.

use std::time::Instant;

use vui_core::{Command, KeyEvent, Quad, TextStyle, rect};

use super::*;

/// Frames the stats average over.
const WINDOW: usize = 60;

pub(crate) struct DevTools {
    pub inspect: bool,
    pub stats: bool,
    /// The last frames' steps, in seconds.
    steps: Vec<f32>,
    /// How long the last paint took, layout included, in milliseconds.
    pub paint_ms: f32,
}

impl Default for DevTools {
    fn default() -> DevTools {
        let on = |name: &str| std::env::var(name).is_ok_and(|v| v == "1");
        DevTools { inspect: on("VUI_INSPECT"), stats: on("VUI_STATS"), steps: Vec::new(), paint_ms: 0.0 }
    }
}

impl DevTools {
    pub(crate) fn step(&mut self, dt: f32) {
        if self.stats {
            if self.steps.len() == WINDOW {
                self.steps.remove(0);
            }
            self.steps.push(dt);
        }
    }

    fn fps(&self) -> Option<f32> {
        let total: f32 = self.steps.iter().sum();
        (total > 0.0).then(|| self.steps.len() as f32 / total)
    }
}

impl<M: 'static> Ui<M> {
    /// Shows or hides the inspector: the element under the pointer outlined, its padding shaded, and
    /// a card with its widget, key, size, padding and role.
    pub fn set_inspector(&mut self, on: bool) {
        self.devtools.inspect = on;
        self.core.paint_dirty = true;
    }

    pub fn inspector(&self) -> bool {
        self.devtools.inspect
    }

    /// Shows or hides frame stats in the top right corner: frames per second while something moves,
    /// the last paint's time, and how many elements there are.
    pub fn set_frame_stats(&mut self, on: bool) {
        self.devtools.stats = on;
        self.devtools.steps.clear();
        self.core.paint_dirty = true;
    }

    pub fn frame_stats(&self) -> bool {
        self.devtools.stats
    }

    /// The dev tools' own keys, before the app sees them. True if the event was one.
    pub(crate) fn devtools_key(&mut self, key: &KeyEvent) -> bool {
        let chord = key.pressed && key.mods.cmd && key.mods.alt && !key.mods.ctrl;
        match key.key {
            vui_core::Key::Character('i') if chord => self.set_inspector(!self.devtools.inspect),
            vui_core::Key::Character('s') if chord => self.set_frame_stats(!self.devtools.stats),
            _ => return false,
        }
        true
    }

    /// Paints the scene and times it for the stats.
    pub(crate) fn timed_paint(&mut self, paint: impl FnOnce(&mut Ui<M>) -> Scene) -> Scene {
        let start = Instant::now();
        let mut scene = paint(self);
        self.devtools.paint_ms = start.elapsed().as_secs_f32() * 1000.0;
        if self.devtools.inspect {
            self.paint_inspector(&mut scene);
        }
        if self.devtools.stats {
            self.paint_stats(&mut scene);
        }
        scene
    }

    fn paint_inspector(&self, scene: &mut Scene) {
        let (Some(root), Some(p)) = (self.root.as_ref(), self.core.pointer) else { return };
        let Some(path) = hit(root, p) else { return };
        let node = path.iter().fold(root, |n, &i| &n.children[i]);
        let env = &self.core.env;
        let accent = env.theme.palette.accent;
        let r = node.rect;
        let content = node.content_rect(r);
        // Padding shaded, the content box tinted, the border box outlined.
        scene.quad(Quad::new(r).fill(Color::rgba(0.2, 0.75, 0.4, 0.18)));
        scene.quad(Quad::new(content).fill(accent.with_alpha(0.16)));
        scene.quad(Quad::new(r).border(1.0, accent));

        let mut info = crate::access::AccessInfo::default();
        node.widget.access(&*node.state, &mut info);
        let role = node.a11y.role.or(info.role);
        let mut lines = vec![match &node.key {
            Some(Key::Name(k)) => format!("{}  \"{k}\"", node.widget.widget_name()),
            Some(Key::Num(k)) => format!("{}  #{k}", node.widget.widget_name()),
            None => node.widget.widget_name().to_string(),
        }];
        lines.push(format!("{} × {}  at {}, {}", round(r.w), round(r.h), round(r.x), round(r.y)));
        let pad = [content.y - r.y, r.x + r.w - content.x - content.w, r.y + r.h - content.y - content.h, content.x - r.x];
        if pad.iter().any(|&v| v > 0.0) {
            lines.push(format!("padding {} {} {} {}", round(pad[0]), round(pad[1]), round(pad[2]), round(pad[3])));
        }
        if let Some(role) = role {
            lines.push(format!("role {role:?}"));
        }
        lines.push(format!("{} deep, {} inside", path.len(), count(node) - 1));
        // Below the element if there is room, else above it, kept inside the window.
        self.card(scene, &lines, |w, h| {
            let size = self.core.size;
            let x = r.x.clamp(4.0, (size.w - w - 4.0).max(4.0));
            let below = r.y + r.h + 6.0;
            let y = if below + h <= size.h - 4.0 { below } else { (r.y - h - 6.0).max(4.0) };
            (x, y)
        });
    }

    fn paint_stats(&self, scene: &mut Scene) {
        let fps = match self.devtools.fps() {
            Some(fps) if self.core.animating => format!("{fps:.0} fps"),
            _ => "idle".to_string(),
        };
        let nodes = self.root.as_ref().map_or(0, count);
        let lines = [format!("{fps}  ·  paint {:.1} ms  ·  {nodes} elements", self.devtools.paint_ms)];
        let size = self.core.size;
        self.card(scene, &lines, |w, _| (size.w - w - 8.0, 8.0));
    }

    /// A small dark card of monospaced lines, placed by `at(width, height)`.
    fn card(&self, scene: &mut Scene, lines: &[String], at: impl FnOnce(f32, f32) -> (f32, f32)) {
        let fonts = &self.core.env.res.fonts;
        let style = TextStyle::new(11.0, 15.0).mono();
        let layouts: Vec<_> = lines.iter().map(|l| fonts.layout_cached(l, &style, None)).collect();
        let w = layouts.iter().map(|l| l.size().w).fold(0.0, f32::max) + 16.0;
        let h = layouts.len() as f32 * 15.0 + 12.0;
        let (x, y) = at(w, h);
        scene.push(Command::Quad(Quad::new(rect(x, y, w, h)).fill(Color::rgba(0.07, 0.07, 0.09, 0.92)).radii(6.0)));
        for (i, layout) in layouts.into_iter().enumerate() {
            scene.text(layout, vui_core::pt(x + 8.0, y + 6.0 + i as f32 * 15.0), Color::rgba(0.95, 0.95, 0.97, 1.0), None);
        }
    }
}

fn round(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn count<M: 'static>(node: &Node<M>) -> usize {
    1 + node.children.iter().map(count).sum::<usize>()
}
