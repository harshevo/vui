//! Sliders: one thumb ([`slider`]) or two ([`range_slider`]), across or upright, with marks at the
//! steps and the value shown over the thumb. Drag a thumb, click the track, or use the keys.
//!
//! Like every widget here they are controlled: they show the values the app gives them and send the
//! values a drag or a key asks for.

mod range;
mod single;

pub use range::{RangeSlider, RangeSliderWidget, RangeState, range_slider};
pub use single::{Slider, SliderState, SliderWidget, slider};

use std::rc::Rc;

use vui_core::{Corners, Key, KeyEvent, NamedKey, Palette, Point, Quad, Rect, Size, TextStyle, pt, size};
use vui_runtime::PaintCx;
use vui_runtime::layout::{Avail, Pair};

pub(crate) const THUMB: f32 = 16.0;
const THUMB_BORDER: f32 = 2.0;
const TRACK: f32 = 4.0;
const THICKNESS: f32 = 20.0;
const DEFAULT_LENGTH: f32 = 160.0;
/// A thumb under the pointer, and one being dragged, get a flat halo this much bigger than they are.
const HALO: f32 = 6.0;
/// How far the keyboard focus ring stands off the thumb: the runtime's ring has a gap and a width.
const RING_GROW: f32 = 3.0;
const RING_WIDTH: f32 = 2.0;
const MARK: f32 = 4.0;
/// Marks closer than this are left out: they would read as a dotted line.
const MARK_SPACING: f32 = 6.0;

pub(crate) type Format = Rc<dyn Fn(f64) -> String>;

/// Where marks go along the track.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Marks {
    None,
    /// At every step.
    Steps,
    At(Vec<f64>),
}

/// What both sliders share: the range, the steps, which way the track runs, and how it is marked.
#[derive(Clone)]
pub(crate) struct Track {
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
    pub vertical: bool,
    pub marks: Marks,
    /// Shows the value over the thumb while it is touched, hovered or focused.
    pub bubble: bool,
    pub format: Option<Format>,
}

impl PartialEq for Track {
    fn eq(&self, other: &Track) -> bool {
        self.min == other.min
            && self.max == other.max
            && self.step == other.step
            && self.vertical == other.vertical
            && self.marks == other.marks
            && self.bubble == other.bubble
    }
}

impl Track {
    pub fn new(min: f64, max: f64) -> Track {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        Track { min, max, step: None, vertical: false, marks: Marks::None, bubble: false, format: None }
    }

    pub fn span(&self) -> f64 {
        (self.max - self.min).max(f64::EPSILON)
    }

    pub fn fraction(&self, v: f64) -> f32 {
        (((v - self.min) / self.span()) as f32).clamp(0.0, 1.0)
    }

    /// What one arrow key press changes: a step, or a hundredth of the range.
    pub fn small_step(&self) -> f64 {
        self.step.unwrap_or(self.span() / 100.0)
    }

    pub fn large_step(&self) -> f64 {
        self.step.map_or(self.span() / 10.0, |s| s * 10.0).max(self.small_step())
    }

    /// The value nearest `v` that the slider can have.
    pub fn snapped(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        match self.step {
            Some(step) if step > 0.0 => {
                let snapped = self.min + ((v - self.min) / step).round() * step;
                // Steps like 0.1 do not add up exactly: keep what is left after rounding to 9 places.
                ((snapped * 1e9).round() / 1e9).clamp(self.min, self.max)
            }
            _ => v,
        }
    }

    /// The thumb's centre `fraction` of the way along: left to right, or bottom to top.
    pub fn at(&self, content: Rect, fraction: f32) -> Point {
        if self.vertical {
            let (bottom, len) = (content.bottom() - THUMB / 2.0, (content.h - THUMB).max(1.0));
            pt(content.x + content.w / 2.0, bottom - len * fraction)
        } else {
            let (left, len) = (content.x + THUMB / 2.0, (content.w - THUMB).max(1.0));
            pt(left + len * fraction, content.y + content.h / 2.0)
        }
    }

    /// The fraction of the way along at `p`, less `grab`: how far off the thumb's centre it was taken.
    pub fn fraction_at(&self, content: Rect, p: Point, grab: f32) -> f32 {
        let f = if self.vertical {
            (content.bottom() - THUMB / 2.0 - (p.y - grab)) / (content.h - THUMB).max(1.0)
        } else {
            (p.x - grab - content.x - THUMB / 2.0) / (content.w - THUMB).max(1.0)
        };
        f.clamp(0.0, 1.0)
    }

    pub fn value_at(&self, content: Rect, p: Point, grab: f32) -> f64 {
        self.min + self.span() * f64::from(self.fraction_at(content, p, grab))
    }

    /// The distance along the track from `centre` to `p`, signed; the grab offset when taking a thumb.
    pub fn along(&self, centre: Point, p: Point) -> f32 {
        if self.vertical { p.y - centre.y } else { p.x - centre.x }
    }

    pub fn on_thumb(centre: Point, p: Point) -> bool {
        (p.x - centre.x).abs() <= THUMB / 2.0 + 2.0 && (p.y - centre.y).abs() <= THUMB / 2.0 + 2.0
    }

    /// Where an arrow, page or Home/End key takes `value`, if it is one of those.
    pub fn key(&self, value: f64, key: &KeyEvent) -> Option<f64> {
        if !key.pressed || key.mods.cmd || key.mods.ctrl || key.mods.alt {
            return None;
        }
        Some(match key.key {
            Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => value + self.small_step(),
            Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => value - self.small_step(),
            Key::Named(NamedKey::PageUp) => value + self.large_step(),
            Key::Named(NamedKey::PageDown) => value - self.large_step(),
            Key::Named(NamedKey::Home) => self.min,
            Key::Named(NamedKey::End) => self.max,
            _ => return None,
        })
    }

    pub fn measure(&self, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let length = |a: Avail| match a {
            Avail::Definite(l) => l.clamp(THUMB * 2.0, DEFAULT_LENGTH),
            _ => DEFAULT_LENGTH,
        };
        if self.vertical {
            size(known.x.unwrap_or(THICKNESS), known.y.unwrap_or(DEFAULT_LENGTH))
        } else {
            size(known.x.unwrap_or(length(avail.x)), known.y.unwrap_or(THICKNESS))
        }
    }

    pub fn text(&self, v: f64) -> String {
        self.format.as_ref().map_or_else(|| trimmed(v), |f| f(v))
    }

    fn mark_values(&self) -> Vec<f64> {
        match &self.marks {
            Marks::None => Vec::new(),
            Marks::At(at) => at.clone(),
            Marks::Steps => match self.step {
                Some(step) if step > 0.0 && self.span() / step <= 200.0 => {
                    let n = (self.span() / step).round() as usize;
                    (0..=n).map(|i| self.min + step * i as f64).collect()
                }
                _ => Vec::new(),
            },
        }
    }

    /// The track, the filled part between `from` and `to` (fractions), and the marks.
    pub fn paint_track(&self, cx: &mut PaintCx, from: f32, to: f32) {
        let palette = cx.palette().clone();
        let c = cx.content;
        let (a, b) = (self.at(c, from), self.at(c, to));
        let round = Corners::all(TRACK / 2.0);
        let (track, filled) = if self.vertical {
            let x = c.x + c.w / 2.0 - TRACK / 2.0;
            (Rect::new(x, c.y, TRACK, c.h), Rect::new(x, b.y, TRACK, (a.y - b.y).max(0.0)))
        } else {
            let y = c.y + c.h / 2.0 - TRACK / 2.0;
            (Rect::new(c.x, y, c.w, TRACK), Rect::new(a.x, y, (b.x - a.x).max(0.0), TRACK))
        };
        // A single slider's fill starts at the track's end, not at the thumb's centre.
        let filled = match (from == 0.0, self.vertical) {
            (true, false) => Rect::new(track.x, filled.y, filled.right() - track.x, TRACK),
            (true, true) => Rect::new(filled.x, filled.y, TRACK, track.bottom() - filled.y),
            _ => filled,
        };
        cx.quad(Quad::new(cx.snap(track)).fill(palette.track_off.mul_alpha(cx.alpha)).radii(round));
        cx.quad(Quad::new(cx.snap(filled)).fill(palette.accent.mul_alpha(cx.alpha)).radii(round));

        let marks = self.mark_values();
        let len = if self.vertical { c.h } else { c.w } - THUMB;
        if marks.len() < 2 || len / ((marks.len() - 1) as f32) < MARK_SPACING {
            return;
        }
        for v in marks {
            let f = self.fraction(v);
            let on = f >= from.min(to) - 1e-4 && f <= to.max(from) + 1e-4;
            let colour = if on { palette.on_accent.mul_alpha(0.75) } else { palette.text_3.mul_alpha(0.7) };
            let p = self.at(c, f);
            cx.quad(Quad::new(Rect::new(p.x - MARK / 2.0, p.y - MARK / 2.0, MARK, MARK)).fill(colour.mul_alpha(cx.alpha)).radii(MARK / 2.0));
        }
    }

    /// A thumb at `centre`, with its halo at `halo` (0 to 1) and its focus ring at `focus`.
    pub fn paint_thumb(cx: &mut PaintCx, centre: Point, halo: f32, focus: f32) {
        let palette = cx.palette().clone();
        let thumb = cx.snap(Rect::new(centre.x - THUMB / 2.0, centre.y - THUMB / 2.0, THUMB, THUMB));
        if halo > 0.0 {
            let outer = thumb.inflate(HALO / 2.0, HALO / 2.0);
            cx.quad(Quad::new(outer).fill(palette.accent.mul_alpha(halo * cx.alpha)).radii(Corners::all(outer.h / 2.0)));
        }
        let quad = Quad::new(thumb)
            .fill(palette.bg.mul_alpha(cx.alpha))
            .radii(Corners::all(THUMB / 2.0))
            .border(THUMB_BORDER, palette.accent.mul_alpha(cx.alpha));
        cx.quad(quad);
        if focus > 0.01 {
            let ring = thumb.inflate(RING_GROW, RING_GROW);
            let quad = Quad::new(ring).radii(Corners::all(ring.h / 2.0)).border(RING_WIDTH, palette.accent.mul_alpha(focus * cx.alpha));
            cx.quad(quad);
        }
    }

    /// The value in a small dark pill above the thumb (or beside an upright one), at `shown` opacity.
    pub fn paint_bubble(&self, cx: &mut PaintCx, centre: Point, value: f64, shown: f32) {
        if !self.bubble || shown <= 0.01 {
            return;
        }
        let palette: Palette = cx.palette().clone();
        let style = TextStyle::new(11.0, 14.0).weight(500);
        let layout = cx.fonts().layout_cached(&self.text(value), &style, None);
        let text = layout.size();
        let (w, h) = (text.w + 12.0, text.h + 6.0);
        let pill = if self.vertical {
            Rect::new(centre.x - THUMB / 2.0 - 8.0 - w, centre.y - h / 2.0, w, h)
        } else {
            Rect::new(centre.x - w / 2.0, centre.y - THUMB / 2.0 - 8.0 - h, w, h)
        };
        let alpha = shown * cx.alpha;
        cx.quad(Quad::new(cx.snap(pill)).fill(palette.text.mul_alpha(alpha)).radii(Corners::all(6.0)));
        cx.text(layout, pt(pill.x + 6.0, pill.y + 3.0), palette.bg.mul_alpha(alpha));
    }
}

/// A number the way a person reads it: `40`, not `40.000000`, and `0.25`, not `0.2500000001`.
pub(crate) fn trimmed(v: f64) -> String {
    let s = format!("{:.4}", (v * 1e4).round() / 1e4);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The options both sliders take, on their builders.
macro_rules! track_options {
    ($ty:ident) => {
        impl<M: 'static> $ty<M> {
            /// Moves in whole multiples of `step` from the minimum, and the arrow keys move by one.
            pub fn step(mut self, step: f64) -> Self {
                self.widget.track.step = Some(step);
                self
            }

            /// Runs bottom to top instead of left to right.
            pub fn vertical(mut self) -> Self {
                self.widget.track.vertical = true;
                self
            }

            /// A mark on the track at every step.
            pub fn marks(mut self) -> Self {
                self.widget.track.marks = super::Marks::Steps;
                self
            }

            /// Marks at these values.
            pub fn marks_at(mut self, values: &[f64]) -> Self {
                self.widget.track.marks = super::Marks::At(values.to_vec());
                self
            }

            /// Shows the value over the thumb while it is dragged, hovered or focused.
            pub fn show_value(mut self) -> Self {
                self.widget.track.bubble = true;
                self
            }

            /// How the value is written, for the bubble and for assistive technology: `|v| format!("{v:.0}%")`.
            pub fn format(mut self, f: impl Fn(f64) -> String + 'static) -> Self {
                self.widget.track.format = Some(std::rc::Rc::new(f));
                self
            }
        }
    };
}

pub(crate) use track_options;
