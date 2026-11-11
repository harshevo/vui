//! Text: fonts, shaping, measuring and caret geometry.
//!
//! `Fonts` owns the font database and the shaping engine (cosmic-text). Layout turns a string and a
//! `TextStyle` into a `TextLayout`: shaped glyphs, a size for the layout engine to use, and the
//! geometry text fields need (caret positions, selection rectangles, hit testing). Layouts are cached
//! across frames, so a screenful of unchanged labels costs nothing to re-shape.

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use cosmic_text::{Align, Attrs, Buffer, Cursor, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping, Weight, Wrap, fontdb};

use crate::geom::{Point, Rect, Size, rect, size};
use crate::theme::TextToken;

/// Re-exported so the renderer and backends use the exact same cosmic-text.
pub use cosmic_text;

mod fonts;
mod layout;

pub use fonts::Fonts;
pub use layout::TextLayout;

/// How far past its width a line may run before it wraps: far below a visible pixel, far above
/// the rounding error of adding and removing padding.
const WIDTH_SLACK: f32 = 0.05;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    /// The interface font (Inter by default).
    Sans,
    /// The monospace font (JetBrains Mono by default).
    Mono,
    Named(Arc<str>),
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextWrap {
    /// Break at word boundaries, and inside a word only if it is wider than the line.
    #[default]
    Words,
    /// Break at word boundaries only: a word wider than the line overflows it.
    WordsOnly,
    /// One line, however long.
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub size: f32,
    pub line_height: f32,
    pub weight: u16,
    pub align: TextAlign,
    pub wrap: TextWrap,
    /// Cut off after this many lines with an ellipsis.
    pub max_lines: Option<usize>,
    /// Extra space between letters, in ems.
    pub letter_spacing: f32,
}

impl TextStyle {
    pub fn new(size: f32, line_height: f32) -> TextStyle {
        TextStyle {
            family: FontFamily::Sans,
            size,
            line_height,
            weight: 400,
            align: TextAlign::Start,
            wrap: TextWrap::Words,
            max_lines: None,
            letter_spacing: 0.0,
        }
    }

    pub fn from_token(token: TextToken) -> TextStyle {
        TextStyle::new(token.size, token.line_height).weight(token.weight)
    }

    pub fn weight(mut self, weight: u16) -> TextStyle {
        self.weight = weight;
        self
    }

    pub fn mono(mut self) -> TextStyle {
        self.family = FontFamily::Mono;
        self
    }

    pub fn family(mut self, family: FontFamily) -> TextStyle {
        self.family = family;
        self
    }

    pub fn align(mut self, align: TextAlign) -> TextStyle {
        self.align = align;
        self
    }

    pub fn no_wrap(mut self) -> TextStyle {
        self.wrap = TextWrap::None;
        self
    }

    pub fn max_lines(mut self, lines: usize) -> TextStyle {
        self.max_lines = Some(lines);
        self
    }

    pub fn letter_spacing(mut self, em: f32) -> TextStyle {
        self.letter_spacing = em;
        self
    }
}

/// A run of text styled apart from the rest of its paragraph: bold, italic, code or a link. Anything
/// left `None` follows the paragraph's [`TextStyle`] and colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub weight: Option<u16>,
    pub italic: bool,
    pub mono: bool,
    /// Its own colour; it then ignores the element's opacity.
    pub color: Option<crate::Color>,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Span {
        Span { text: text.into(), ..Span::default() }
    }

    pub fn weight(mut self, weight: u16) -> Span {
        self.weight = Some(weight);
        self
    }

    pub fn italic(mut self) -> Span {
        self.italic = true;
        self
    }

    pub fn mono(mut self) -> Span {
        self.mono = true;
        self
    }

    pub fn color(mut self, color: crate::Color) -> Span {
        self.color = Some(color);
        self
    }
}

#[cfg(test)]
mod tests;
