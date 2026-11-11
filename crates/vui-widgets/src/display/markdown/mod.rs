//! Markdown, drawn with the theme: headings, paragraphs with bold, italic, code and links, lists,
//! quotes, code blocks, rules, tables and pictures. Written for what models write; the parser is
//! small and forgiving, and anything it does not understand shows as plain text, never as markup.

use std::rc::Rc;

use vui_core::{CursorIcon, Event, Fill, FontFamily, ImageId, PointerButton, Size, Span, TextRole, TextStyle, Theme, Tone, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{
    AccessInfo, Align, Base, Builder, Dirty, Div, Element, Env, EventCx, IntoElement, PaintCx, Parent, Radius, Role, Styled, Widget,
    WidgetEvent, builder, column, div, img, label, row,
};

mod parse;
mod render;
mod rich_text;

pub use parse::{Block, Inline, inlines, parse, plain, strip};
pub use render::{Markdown, markdown};
pub use rich_text::{RichText, RichTextState, RichTextWidget, rich_text};
use parse::*;
use rich_text::*;
