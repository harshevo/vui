//! Elements: what the app's `view` returns.
//!
//! An element describes one box of the interface: its layout [`Style`], how it is decorated, what
//! happens when it is clicked, and the [`Widget`] that draws its content. The runtime compares each new
//! tree with the one it kept and updates in place, so building a whole tree on every change is cheap and
//! hover, focus and scroll state survive it.
//!
//! Builders make elements read like markup:
//!
//! ```
//! use vui_core::Tone;
//! use vui_runtime::testing::TestUi;
//! use vui_runtime::{Builder, Parent, Styled, column, label, row, title};
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Sort,
//! }
//!
//! let mut ui = TestUi::new(320.0, 200.0);
//! ui.view(column().gap(8.0).padding(16.0).bg(Tone::Side).children((
//!     title("Library"),
//!     row().key("sort").gap(8.0).on_click(Msg::Sort).child(label("Sort")).child(label("Newest")),
//! )));
//! ui.click_key("sort");
//! assert_eq!(ui.messages(), [Msg::Sort]);
//! ```

use std::path::PathBuf;
use std::rc::Rc;

use vui_core::theme::PILL;
use vui_core::{Corners, CursorIcon, Fill, KeyEvent, Modifiers, Point, PointerButton, Spring, Theme};

use crate::access::{A11y, Role};
use crate::key::Key;
use crate::responsive::{Restyle, Rule, Width};
use crate::style::{Axis, Style, Styled};
use crate::widget::{Dirty, Erased, Focus, Mapped, Plain, Widget};

mod builder;
mod container;
mod input;
mod lazy;
mod transform;

pub use builder::Builder;
pub use container::{Container, Div, column, div, grid, row, spacer};
pub(crate) use lazy::Lazy;
pub use lazy::{VirtualList, virtual_list};
pub use transform::Transform;
pub(crate) use input::*;

/// How round an element's corners are. Tokens follow the theme, so restyling the theme restyles every
/// card at once.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum Radius {
    #[default]
    None,
    Px(f32),
    /// Buttons, fields and rows.
    Control,
    Card,
    Panel,
    /// Fully round ends.
    Pill,
    Corners(Corners),
}

impl Radius {
    pub fn resolve(self, theme: &Theme) -> Corners {
        match self {
            Radius::None => Corners::ZERO,
            Radius::Px(r) => Corners::all(r),
            Radius::Control => Corners::all(theme.radii.control),
            Radius::Card => Corners::all(theme.radii.card),
            Radius::Panel => Corners::all(theme.radii.panel),
            Radius::Pill => Corners::all(PILL),
            Radius::Corners(c) => c,
        }
    }

    /// How continuous the corners are: the theme's, except on a pill, whose ends are true semicircles.
    pub fn smooth(self, theme: &Theme) -> f32 {
        match self {
            Radius::Pill => 0.0,
            _ => theme.radii.smooth,
        }
    }
}

impl From<f32> for Radius {
    fn from(r: f32) -> Radius {
        Radius::Px(r)
    }
}

impl From<Corners> for Radius {
    fn from(c: Corners) -> Radius {
        Radius::Corners(c)
    }
}

/// How an element's box is drawn, apart from what it contains.
#[derive(Clone, Debug, PartialEq)]
pub struct Decor {
    pub fill: Fill,
    /// Faded in over `fill` while the pointer is over the element.
    pub hover_fill: Fill,
    /// Faded in while it is pressed.
    pub press_fill: Fill,
    /// Faded in while files are dragged over it.
    pub drop_fill: Fill,
    pub border: Option<(f32, Fill)>,
    /// A soft shadow under the element's box, drawn by the renderer's effects pass.
    pub shadow: Option<vui_core::Shadow>,
    /// A gradient over the fill, drawn by the renderer's shading pass.
    pub gradient: Option<vui_core::shading::Gradient>,
    /// A border coloured by a gradient, this many points wide.
    pub gradient_border: Option<(f32, vui_core::shading::Gradient)>,
    pub radius: Radius,
    /// Multiplies the element and everything inside it.
    pub opacity: f32,
    /// Draw the focus ring when the element has keyboard focus.
    pub focus_ring: bool,
    /// Scales, turns and moves the element as it is drawn; layout ignores it.
    pub transform: Transform,
}

impl Default for Decor {
    fn default() -> Decor {
        Decor {
            fill: Fill::NONE,
            hover_fill: Fill::NONE,
            press_fill: Fill::NONE,
            drop_fill: Fill::NONE,
            border: None,
            shadow: None,
            gradient: None,
            gradient_border: None,
            radius: Radius::None,
            opacity: 1.0,
            focus_ring: true,
            transform: Transform::IDENTITY,
        }
    }
}

/// A click or press, as handlers see it.
#[derive(Clone, Debug, PartialEq)]
pub struct Click {
    /// Window coordinates.
    pub pos: Point,
    pub mods: Modifiers,
    pub button: PointerButton,
    /// 1 for a single click, 2 for the second click of a double click.
    pub count: u8,
}

/// Arrow keys move focus among the focusable elements inside an element, and Tab treats all of them as
/// one stop: a list, a sidebar, a radio group, a toolbar. Tab leaves the group; coming back lands on
/// the item that had focus last, or on the one marked [`Builder::tab_default`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FocusGroup {
    /// Up and down for `Y`, left and right for `X`, all four for `None`.
    pub axis: Option<Axis>,
    /// Going past the last item lands on the first, and the other way round.
    pub wrap: bool,
}

impl FocusGroup {
    pub const fn vertical() -> FocusGroup {
        FocusGroup { axis: Some(Axis::Y), wrap: false }
    }

    pub const fn horizontal() -> FocusGroup {
        FocusGroup { axis: Some(Axis::X), wrap: false }
    }

    pub const fn both() -> FocusGroup {
        FocusGroup { axis: None, wrap: false }
    }

    pub const fn wrapping(mut self) -> FocusGroup {
        self.wrap = true;
        self
    }
}

/// What every element has, whatever its widget: layout, decoration, input and accessibility.
pub struct Base<M> {
    pub(crate) style: Style,
    /// Restyles for ranges of window widths, applied over `style` by the UI.
    pub(crate) rules: Vec<Rule>,
    pub(crate) decor: Decor,
    pub(crate) input: Input<M>,
    pub(crate) a11y: A11y,
    pub(crate) key: Option<Key>,
    pub(crate) transition: Option<Spring>,
    pub(crate) appear: bool,
}

impl<M> Default for Base<M> {
    fn default() -> Base<M> {
        Base {
            style: Style::default(),
            rules: Vec::new(),
            decor: Decor::default(),
            input: Input::default(),
            a11y: A11y::default(),
            key: None,
            transition: None,
            appear: false,
        }
    }
}

impl<M> Base<M> {
    pub fn new() -> Base<M> {
        Base::default()
    }

    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }
}

/// One box of the interface, with its content and children.
pub struct Element<M: 'static> {
    pub(crate) base: Base<M>,
    pub(crate) widget: Box<dyn Erased<M>>,
    pub(crate) children: Vec<Element<M>>,
    /// Rows the UI builds as they scroll into view, in place of `children` (see [`virtual_list`]).
    pub(crate) lazy: Option<Lazy<M>>,
}

impl<M: 'static> Element<M> {
    pub fn new<W: Widget<M>>(widget: W, base: Base<M>, children: Vec<Element<M>>) -> Element<M> {
        Element { base, widget: Box::new(Plain(widget)), children, lazy: None }
    }

    /// Turns every message this element and its children send into another type, so a component
    /// with messages of its own sits inside a bigger app:
    ///
    /// ```
    /// use vui_runtime::{Builder, Element, IntoElement, Parent, column, label};
    ///
    /// #[derive(Clone)]
    /// enum CounterMsg { Add }
    /// fn counter(n: u32) -> Element<CounterMsg> {
    ///     column().on_click(CounterMsg::Add).child(label(n.to_string())).into_element()
    /// }
    ///
    /// enum Msg { Left(CounterMsg), Right(CounterMsg) }
    /// let both: Element<Msg> = column().children((counter(1).map(Msg::Left), counter(2).map(Msg::Right))).into_element();
    /// ```
    pub fn map<N: 'static>(self, f: impl Fn(M) -> N + 'static) -> Element<N> {
        let f: Rc<dyn Fn(M) -> N> = Rc::new(f);
        self.map_rc(&f)
    }

    /// The name assistive technology reads for the element, replacing any it had: for naming a
    /// built element, as a form row names the trigger of a select.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.base.a11y.label = Some(name.into());
        self
    }

    /// Names the element unless it was named already; see [`Builder::default_name`].
    pub fn default_name(mut self, name: impl Into<String>) -> Self {
        self.base.a11y.label.get_or_insert_with(|| name.into());
        self
    }

    /// A longer description, read after the name; see [`Builder::describe`].
    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.base.a11y.description = Some(description.into());
        self
    }

    fn map_rc<N: 'static>(self, f: &Rc<dyn Fn(M) -> N>) -> Element<N> {
        let Element { base, widget, children, lazy } = self;
        let Base { style, rules, decor, input, a11y, key, transition, appear } = base;
        let base = Base { style, rules, decor, input: input.map(f), a11y, key, transition, appear };
        let widget: Box<dyn Erased<N>> = Box::new(Mapped { inner: widget, f: f.clone() });
        let lazy = lazy.map(|l| l.map(f));
        Element { base, widget, children: children.into_iter().map(|c| c.map_rc(f)).collect(), lazy }
    }
}

/// Anything that can stand for an element in a tree.
pub trait IntoElement<M: 'static> {
    fn into_element(self) -> Element<M>;
}

impl<M: 'static> IntoElement<M> for Element<M> {
    fn into_element(self) -> Element<M> {
        self
    }
}

/// A child that is only sometimes there, inside a tuple of children: `(title, editing.then(|| field))`.
/// When absent it is a hidden, empty element, so the children after it keep their places and state.
impl<M: 'static, E: IntoElement<M>> IntoElement<M> for Option<E> {
    fn into_element(self) -> Element<M> {
        match self {
            Some(e) => e.into_element(),
            None => div().hidden(true).a11y_hidden(true).into_element(),
        }
    }
}

/// A group of elements that can be added as children together: a tuple (any mix of builders), a `Vec`,
/// an array, or an `Option` for a child that is only sometimes there.
pub trait IntoChildren<M: 'static> {
    fn append_to(self, out: &mut Vec<Element<M>>);
}

/// No children.
impl<M: 'static> IntoChildren<M> for () {
    fn append_to(self, _out: &mut Vec<Element<M>>) {}
}

impl<M: 'static> IntoChildren<M> for Element<M> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.push(self);
    }
}

impl<M: 'static, E: IntoElement<M>> IntoChildren<M> for Vec<E> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.extend(self.into_iter().map(IntoElement::into_element));
    }
}

impl<M: 'static, E: IntoElement<M>, const N: usize> IntoChildren<M> for [E; N] {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.extend(self.into_iter().map(IntoElement::into_element));
    }
}

impl<M: 'static, E: IntoElement<M>> IntoChildren<M> for Option<E> {
    fn append_to(self, out: &mut Vec<Element<M>>) {
        out.extend(self.map(IntoElement::into_element));
    }
}

macro_rules! tuple_children {
    ($($t:ident),+) => {
        impl<M: 'static, $($t: IntoElement<M>),+> IntoChildren<M> for ($($t,)+) {
            #[allow(non_snake_case)]
            fn append_to(self, out: &mut Vec<Element<M>>) {
                let ($($t,)+) = self;
                $(out.push($t.into_element());)+
            }
        }
    };
}

tuple_children!(A);
tuple_children!(A, B);
tuple_children!(A, B, C);
tuple_children!(A, B, C, D);
tuple_children!(A, B, C, D, E);
tuple_children!(A, B, C, D, E, F);
tuple_children!(A, B, C, D, E, F, G);
tuple_children!(A, B, C, D, E, F, G, H);
tuple_children!(A, B, C, D, E, F, G, H, I);
tuple_children!(A, B, C, D, E, F, G, H, I, J);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K);
tuple_children!(A, B, C, D, E, F, G, H, I, J, K, L);

/// Builders that hold children.
pub trait Parent<M: 'static>: Sized {
    fn child_list(&mut self) -> &mut Vec<Element<M>>;

    fn child(mut self, child: impl IntoElement<M>) -> Self {
        self.child_list().push(child.into_element());
        self
    }

    fn children(mut self, children: impl IntoChildren<M>) -> Self {
        children.append_to(self.child_list());
        self
    }

    /// One child per item: `column().each(items.iter().map(row_for))`.
    fn each<E: IntoElement<M>>(mut self, items: impl IntoIterator<Item = E>) -> Self {
        let list = self.child_list();
        list.extend(items.into_iter().map(IntoElement::into_element));
        self
    }

    fn child_if<E: IntoElement<M>>(self, condition: bool, build: impl FnOnce() -> E) -> Self {
        if condition { self.child(build()) } else { self }
    }
}
