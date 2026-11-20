//! A switcher: the view or filter that is showing, as one word. The other choices stay out of sight
//! until they are wanted, so a card's header holds a word, not a row of tabs.
//!
//! - Three or more choices: the word has a chevron, and a click opens them in a menu under it.
//! - Two choices: the word has an up-down mark, and a click flips to the other one.
//! - [`Switcher::steps`]: ordered values such as dates get `‹ ›` either side to step along them.
//!
//! When it has focus, `[` and `]` or the left and right arrows step to the choice before or after,
//! and Down opens the menu. The wheel does nothing, so scrolling a card past it never changes it.

use std::rc::Rc;

use vui_core::{CursorIcon, Fill, Key, KeyEvent, NamedKey, Spring, Tone};
use vui_icons::set;
use vui_runtime::{Align, Builder, Div, Element, Focus, IntoChildren, IntoElement, Justify, Parent, Radius, Role, Styled, div, row, small};

use crate::display::icon::icon;
use crate::overlay::{Side, menu, menu_check, popover};

/// How tall the word is, like every control in a row.
const HEIGHT: f32 = 28.0;

type Pick<M> = Rc<dyn Fn(usize) -> M>;

/// `(value, label)` for each choice, the chosen value, whether its menu is open, what to send to open
/// or close the menu, and what to send when another choice is made. Close the menu when handling
/// `on_select`. Key it, so the word is `key` and each choice in the menu `{key}-{index}`.
///
/// ```
/// # use vui_widgets::switcher;
/// # #[derive(Clone)] enum Msg { Open(bool), View(u8) }
/// let views = [(0, "Positions".to_string()), (1, "Changes".to_string()), (2, "Trend".to_string())];
/// let view = switcher(views, &1, false, Msg::Open, Msg::View).key("views");
/// ```
pub fn switcher<M: Clone + 'static, T: PartialEq + Clone + 'static>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: &T,
    open: bool,
    on_open: impl Fn(bool) -> M + 'static,
    on_select: impl Fn(T) -> M + 'static,
) -> Switcher<M> {
    let (values, labels): (Vec<T>, Vec<String>) = options.into_iter().unzip();
    let chosen = values.iter().position(|v| v == selected).unwrap_or(0);
    let pick: Pick<M> = Rc::new(move |i| on_select(values[i].clone()));
    Switcher { labels, chosen, open, on_open: Rc::new(on_open), pick, key: None, ids: Vec::new(), name: None, tones: Vec::new(), steps: false, menu: false, side: Side::Below }
}

/// See [`switcher`].
pub struct Switcher<M: 'static> {
    labels: Vec<String>,
    chosen: usize,
    open: bool,
    on_open: Rc<dyn Fn(bool) -> M>,
    pick: Pick<M>,
    key: Option<String>,
    ids: Vec<String>,
    name: Option<String>,
    tones: Vec<Option<Tone>>,
    steps: bool,
    menu: bool,
    side: Side,
}

impl<M: Clone + 'static> Switcher<M> {
    /// Keys the word `key` and each choice in its menu `{key}-{index}`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Keys the choices in the menu `{key}-{id}` by these ids, in order, instead of by index.
    pub fn ids(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.ids = ids.into_iter().map(Into::into).collect();
        self
    }

    /// What the choice is about, such as "Leg": read before the word by assistive tools, and shown
    /// in its tooltip.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// A colour for each choice's word, in order; `None` keeps the text colour. Buy and Sell are
    /// green and red.
    pub fn tones(mut self, tones: impl IntoIterator<Item = Option<Tone>>) -> Self {
        self.tones = tones.into_iter().collect();
        self
    }

    /// `‹ ›` either side of the word, for values in an order such as dates or counts. They stop at
    /// either end instead of going round.
    pub fn steps(mut self) -> Self {
        self.steps = true;
        self
    }

    /// Opens a menu even with two choices, instead of flipping between them.
    pub fn menu(mut self) -> Self {
        self.menu = true;
        self
    }

    /// Which side of the word the menu opens on.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Whether a click flips to the other choice rather than opening the menu.
    fn flips(&self) -> bool {
        self.labels.len() == 2 && !self.menu && !self.steps
    }

    /// The choice `by` places along, going round unless it steps along an order.
    fn along(&self, by: i32) -> Option<usize> {
        let n = self.labels.len() as i32;
        let to = self.chosen as i32 + by;
        if n < 2 {
            None
        } else if self.steps {
            (0..n).contains(&to).then_some(to as usize)
        } else {
            Some(to.rem_euclid(n) as usize)
        }
    }

    fn word(&self) -> Div<M> {
        let text = self.labels.get(self.chosen).cloned().unwrap_or_default();
        let tone = self.tones.get(self.chosen).copied().flatten().unwrap_or(Tone::Text);
        let mark = if self.flips() { set::CHEVRONS_UP_DOWN } else { set::CHEVRON_DOWN };
        let click = if self.flips() { self.along(1).map(|i| (self.pick)(i)) } else { (self.labels.len() > 1).then(|| (self.on_open)(!self.open)) };
        let keys = {
            let (back, on, down) = (self.along(-1).map(|i| (self.pick)(i)), self.along(1).map(|i| (self.pick)(i)), (!self.flips() && !self.open).then(|| (self.on_open)(true)));
            move |k: &KeyEvent| {
                if !k.pressed || (k.mods.cmd || k.mods.ctrl || k.mods.alt) {
                    return None;
                }
                match k.key {
                    Key::Character('[') | Key::Named(NamedKey::ArrowLeft) => back.clone(),
                    Key::Character(']') | Key::Named(NamedKey::ArrowRight) => on.clone(),
                    Key::Named(NamedKey::ArrowDown) => down.clone(),
                    _ => None,
                }
            }
        };
        let said = match &self.name {
            Some(name) => format!("{name}: {text}"),
            None => text.clone(),
        };
        let word = row::<M>()
            .height(HEIGHT)
            .padding_x(8.0)
            .gap(4.0)
            .min_width(0.0)
            .rounded(Radius::Control)
            .align_items(Align::Center)
            .bg(if self.open { Fill::from(Tone::Hover) } else { Fill::NONE })
            .hover_bg(Tone::Hover)
            .transition(Spring::SNAPPY)
            .role(Role::Button)
            .focus_mode(Focus::Keyboard)
            .name(said)
            .cursor(CursorIcon::Pointer)
            .on_key(keys)
            .children((small(text).weight(600).color(tone).max_lines(1).min_width(0.0), (self.labels.len() > 1).then(|| icon(mark).square(12.0).color(Tone::Text3))));
        let word = match click {
            Some(click) => word.on_click(click),
            None => word,
        };
        let word = match &self.name {
            Some(name) => word.tooltip(name.clone()),
            None => word,
        };
        match &self.key {
            Some(key) => word.key(key.clone()),
            None => word,
        }
    }

    fn arrow(&self, glyph: vui_icons::Icon, name: &str, by: i32) -> Div<M> {
        let to = self.along(by);
        let arrow = div::<M>()
            .width(20.0)
            .height(HEIGHT)
            .rounded(Radius::Control)
            .align_items(Align::Center)
            .justify(Justify::Center)
            .role(Role::Button)
            .name(name.to_string())
            .disabled(to.is_none())
            .child(icon(glyph).square(14.0).color(if to.is_some() { Fill::from(Tone::Text2) } else { Fill::Tone(Tone::Text3, 0.5) }));
        let arrow = match &self.key {
            Some(key) => arrow.key(format!("{key}-{}", if by < 0 { "back" } else { "on" })),
            None => arrow,
        };
        match to {
            Some(i) => arrow.hover_bg(Tone::Hover).cursor(CursorIcon::Pointer).on_click((self.pick)(i)),
            None => arrow,
        }
    }

    fn build(self) -> Element<M> {
        let word = self.word();
        let anchor = if self.steps {
            row().align_items(Align::Center).children((self.arrow(set::CHEVRON_LEFT, "Before", -1), word, self.arrow(set::CHEVRON_RIGHT, "After", 1)))
        } else {
            row().align_items(Align::Center).child(word)
        };
        if self.flips() || self.labels.len() < 2 {
            return anchor.into_element();
        }
        let items: Vec<Element<M>> = self
            .labels
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let item = menu_check(text.clone(), i == self.chosen).on_click((self.pick)(i));
                match &self.key {
                    Some(key) => item.key(format!("{key}-{}", self.ids.get(i).cloned().unwrap_or_else(|| i.to_string()))).into_element(),
                    None => item.into_element(),
                }
            })
            .collect();
        popover(anchor, self.open, (self.on_open)(false), menu(items)).side(self.side).into_element()
    }
}

/// In a form row, the word takes the row's name.
impl<M: Clone + 'static> crate::Control<M> for Switcher<M> {
    fn labelled(mut self, name: String, _detail: Option<String>) -> Element<M> {
        self.name = Some(name);
        self.build()
    }
}

impl<M: Clone + 'static> IntoChildren<M> for Switcher<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self.build());
    }
}

impl<M: Clone + 'static> IntoElement<M> for Switcher<M> {
    fn into_element(self) -> Element<M> {
        self.build()
    }
}
