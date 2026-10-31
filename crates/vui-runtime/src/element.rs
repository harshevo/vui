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

use vui_core::theme::PILL;
use vui_core::{Corners, CursorIcon, Fill, KeyEvent, Modifiers, Point, PointerButton, Spring, Theme};

use crate::access::{A11y, Role};
use crate::key::Key;
use crate::style::{Axis, Style, Styled};
use crate::widget::{Dirty, Erased, Focus, Widget};

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
    pub radius: Radius,
    /// Multiplies the element and everything inside it.
    pub opacity: f32,
    /// Draw the focus ring when the element has keyboard focus.
    pub focus_ring: bool,
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
            radius: Radius::None,
            opacity: 1.0,
            focus_ring: true,
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

pub(crate) type Handler<M> = Box<dyn Fn(&Click) -> M>;
pub(crate) type KeyHandler<M> = Box<dyn Fn(&KeyEvent) -> Option<M>>;

/// What an element does with input. Handlers build the message; the runtime delivers it.
pub(crate) struct Input<M> {
    pub click: Option<Handler<M>>,
    pub press: Option<Handler<M>>,
    pub double_click: Option<Handler<M>>,
    pub secondary: Option<Handler<M>>,
    pub hover: Option<Box<dyn Fn(bool) -> M>>,
    pub key: Option<KeyHandler<M>>,
    pub drop: Option<Box<dyn Fn(Vec<PathBuf>) -> M>>,
    pub focus: Option<Focus>,
    pub group: Option<FocusGroup>,
    pub tab_default: bool,
    pub cursor: Option<CursorIcon>,
    pub disabled: bool,
    pub pointer_events: bool,
    pub window_drag: bool,
}

impl<M> Default for Input<M> {
    fn default() -> Input<M> {
        Input {
            click: None,
            press: None,
            double_click: None,
            secondary: None,
            hover: None,
            key: None,
            drop: None,
            focus: None,
            group: None,
            tab_default: false,
            cursor: None,
            disabled: false,
            pointer_events: true,
            window_drag: false,
        }
    }
}

impl<M> Input<M> {
    /// Pressing it means something, so the runtime tracks the press.
    pub fn clickable(&self) -> bool {
        self.click.is_some() || self.press.is_some() || self.double_click.is_some()
    }
}

/// What every element has, whatever its widget: layout, decoration, input and accessibility.
pub struct Base<M> {
    pub(crate) style: Style,
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
}

impl<M: 'static> Element<M> {
    pub fn new<W: Widget<M>>(widget: W, base: Base<M>, children: Vec<Element<M>>) -> Element<M> {
        Element { base, widget: Box::new(widget), children }
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

/// A group of elements that can be added as children together: a tuple (any mix of builders), a `Vec`,
/// an array, or an `Option` for a child that is only sometimes there.
pub trait IntoChildren<M: 'static> {
    fn append_to(self, out: &mut Vec<Element<M>>);
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

/// The chainable vocabulary every element builder shares, on top of [`Styled`]: decoration, input
/// handlers, identity, accessibility and transitions. Implement `base_mut`, or use [`builder!`](crate::builder).
pub trait Builder<M: 'static>: Styled {
    fn base_mut(&mut self) -> &mut Base<M>;

    /// Names the element so it keeps its state when its siblings reorder, and so the app can find
    /// its rectangle with `Ui::rect_of`.
    fn key(mut self, key: impl Into<Key>) -> Self {
        self.base_mut().key = Some(key.into());
        self
    }

    /// The fill behind the element.
    fn bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.fill = fill.into();
        self
    }

    /// Faded over the fill while the pointer is over the element.
    fn hover_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.hover_fill = fill.into();
        self
    }

    /// Faded over the fill while the element is pressed.
    fn press_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.press_fill = fill.into();
        self
    }

    /// Faded over the fill while files are dragged over the element.
    fn drop_bg(mut self, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.drop_fill = fill.into();
        self
    }

    /// A line `width` thick drawn inside the element's edge. Layout ignores it: pad to make room.
    fn border(mut self, width: f32, fill: impl Into<Fill>) -> Self {
        self.base_mut().decor.border = Some((width, fill.into()));
        self
    }

    /// A soft shadow under the element, following its rounded corners: `.shadow(Shadow::RAISED)`.
    /// It is stronger in dark mode, where a faint one would not show.
    fn shadow(mut self, shadow: vui_core::Shadow) -> Self {
        self.base_mut().decor.shadow = Some(shadow);
        self
    }

    fn rounded(mut self, radius: impl Into<Radius>) -> Self {
        self.base_mut().decor.radius = radius.into();
        self
    }

    /// Fades the element and everything inside it.
    fn opacity(mut self, opacity: f32) -> Self {
        self.base_mut().decor.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Turns the keyboard focus ring off, for widgets that show focus themselves.
    fn focus_ring(mut self, on: bool) -> Self {
        self.base_mut().decor.focus_ring = on;
        self
    }

    /// Animates changes to the element's size, insets, fill and opacity with `spring`. Insets make an
    /// absolute element slide: a selection marker whose `top` follows the selected row.
    fn transition(mut self, spring: Spring) -> Self {
        self.base_mut().transition = Some(spring);
        self
    }

    /// Animates changes with the theme's usual spring.
    fn animated(self) -> Self {
        self.transition(Spring::SNAPPY)
    }

    /// Fades the element in and lifts it into place when it first shows up, with the `transition`
    /// spring or a smooth one. Key it by what it shows, a page or a row, so that new content appears
    /// afresh instead of updating the old element in place.
    fn appear(mut self) -> Self {
        self.base_mut().appear = true;
        self
    }

    /// Sent when the element is clicked: pressed and released while over it.
    fn on_click(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_click_at(move |_| message.clone())
    }

    fn on_click_with(self, f: impl Fn() -> M + 'static) -> Self {
        self.on_click_at(move |_| f())
    }

    fn on_click_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.click = Some(Box::new(f));
        self
    }

    /// Sent the moment the primary button goes down on the element.
    fn on_press(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_press_at(move |_| message.clone())
    }

    fn on_press_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.press = Some(Box::new(f));
        self
    }

    /// Sent after the second click of a double click. The first click still sends `on_click`.
    fn on_double_click(self, message: M) -> Self
    where
        M: Clone,
    {
        self.on_double_click_at(move |_| message.clone())
    }

    fn on_double_click_at(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.double_click = Some(Box::new(f));
        self
    }

    /// Sent on a right click or control-click: where a context menu opens.
    fn on_secondary(mut self, f: impl Fn(&Click) -> M + 'static) -> Self {
        self.base_mut().input.secondary = Some(Box::new(f));
        self
    }

    /// Sent when the pointer enters (`true`) or leaves (`false`) the element or anything inside it.
    fn on_hover(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.base_mut().input.hover = Some(Box::new(f));
        self
    }

    /// Sees key presses that reach the element, from itself or anything focused inside it. Return
    /// `None` to let the key carry on to the next handler.
    fn on_key(mut self, f: impl Fn(&KeyEvent) -> Option<M> + 'static) -> Self {
        self.base_mut().input.key = Some(Box::new(f));
        self
    }

    /// Accepts files dropped on the element.
    fn on_drop(mut self, f: impl Fn(Vec<PathBuf>) -> M + 'static) -> Self {
        self.base_mut().input.drop = Some(Box::new(f));
        self
    }

    /// Makes the element focusable by Tab and by click, or not focusable at all.
    fn focusable(self, on: bool) -> Self {
        self.focus_mode(if on { Focus::Click } else { Focus::Never })
    }

    fn focus_mode(mut self, focus: Focus) -> Self {
        self.base_mut().input.focus = Some(focus);
        self
    }

    /// Makes the element a focus group: Tab visits it once, and the arrow keys move between the
    /// focusable elements inside it.
    fn focus_group(mut self, group: FocusGroup) -> Self {
        self.base_mut().input.group = Some(group);
        self
    }

    /// Inside a focus group, the item Tab lands on when focus comes into the group: the selected radio
    /// button, the current page. Without one, Tab returns to the item that had focus last.
    fn tab_default(mut self, on: bool) -> Self {
        self.base_mut().input.tab_default = on;
        self
    }

    /// The pointer shape over the element.
    fn cursor(mut self, cursor: CursorIcon) -> Self {
        self.base_mut().input.cursor = Some(cursor);
        self
    }

    /// Dims the element and stops it, and everything inside it, from taking part in input.
    fn disabled(mut self, disabled: bool) -> Self {
        self.base_mut().input.disabled = disabled;
        self
    }

    /// `false` makes the element see through to what is behind it; its children still get input.
    fn pointer_events(mut self, on: bool) -> Self {
        self.base_mut().input.pointer_events = on;
        self
    }

    /// Dragging the element moves the window, as a title bar does, unless the press lands on something
    /// inside it that takes clicks or focus. For the empty canvas around a window's content.
    fn window_drag(mut self, on: bool) -> Self {
        self.base_mut().input.window_drag = on;
        self
    }

    /// What the element is, to assistive technology.
    fn role(mut self, role: Role) -> Self {
        self.base_mut().a11y.role = Some(role);
        self
    }

    /// The name assistive technology reads out, when the visible text is not enough.
    fn name(mut self, name: impl Into<String>) -> Self {
        self.base_mut().a11y.label = Some(name.into());
        self
    }

    /// A name for an element that has none yet: `name` called later or earlier still wins. Widgets
    /// that sit beside a visible label use it to name the control after that label.
    fn default_name(mut self, name: impl Into<String>) -> Self {
        self.base_mut().a11y.label.get_or_insert_with(|| name.into());
        self
    }

    fn describe(mut self, description: impl Into<String>) -> Self {
        self.base_mut().a11y.description = Some(description.into());
        self
    }

    /// Leaves the element and what is inside it out of the accessibility tree.
    fn a11y_hidden(mut self, hidden: bool) -> Self {
        self.base_mut().a11y.hidden = hidden;
        self
    }
}

impl<M: 'static> Styled for Element<M> {
    fn style_mut(&mut self) -> &mut Style {
        &mut self.base.style
    }
}

impl<M: 'static> Builder<M> for Element<M> {
    fn base_mut(&mut self) -> &mut Base<M> {
        &mut self.base
    }
}

impl<M: 'static> Parent<M> for Element<M> {
    fn child_list(&mut self) -> &mut Vec<Element<M>> {
        &mut self.children
    }
}

/// Gives a widget's builder struct the whole builder vocabulary.
///
/// The struct needs a `base: Base<M>` field and a `widget` field holding the widget; containers also
/// need `children: Vec<Element<M>>` and are declared with `, children`:
///
/// ```
/// use vui_runtime::{Base, Builder, Parent, Styled, Widget, builder, column};
///
/// struct BadgeWidget;
///
/// impl<M: 'static> Widget<M> for BadgeWidget {
///     type State = ();
///     fn create_state(&self) {}
/// }
///
/// pub struct Badge<M: 'static> {
///     base: Base<M>,
///     widget: BadgeWidget,
/// }
///
/// builder!(Badge<M> => BadgeWidget);
///
/// fn badge<M: 'static>() -> Badge<M> {
///     Badge { base: Base::new(), widget: BadgeWidget }
/// }
///
/// // It now has the whole vocabulary: size, decoration, input, accessibility.
/// let view = column::<()>().child(badge().width(24.0).key("badge"));
/// ```
#[macro_export]
macro_rules! builder {
    ($name:ident<$m:ident> => $widget:ty) => {
        impl<$m: 'static> $crate::Styled for $name<$m> {
            fn style_mut(&mut self) -> &mut $crate::Style {
                self.base.style_mut()
            }
        }

        impl<$m: 'static> $crate::Builder<$m> for $name<$m> {
            fn base_mut(&mut self) -> &mut $crate::Base<$m> {
                &mut self.base
            }
        }

        impl<$m: 'static> $crate::IntoElement<$m> for $name<$m>
        where
            $widget: $crate::Widget<$m>,
        {
            fn into_element(self) -> $crate::Element<$m> {
                $crate::Element::new(self.widget, self.base, ::std::vec::Vec::new())
            }
        }
    };
    ($name:ident<$m:ident> => $widget:ty, children) => {
        impl<$m: 'static> $crate::Styled for $name<$m> {
            fn style_mut(&mut self) -> &mut $crate::Style {
                self.base.style_mut()
            }
        }

        impl<$m: 'static> $crate::Builder<$m> for $name<$m> {
            fn base_mut(&mut self) -> &mut $crate::Base<$m> {
                &mut self.base
            }
        }

        impl<$m: 'static> $crate::Parent<$m> for $name<$m> {
            fn child_list(&mut self) -> &mut ::std::vec::Vec<$crate::Element<$m>> {
                &mut self.children
            }
        }

        impl<$m: 'static> $crate::IntoElement<$m> for $name<$m>
        where
            $widget: $crate::Widget<$m>,
        {
            fn into_element(self) -> $crate::Element<$m> {
                $crate::Element::new(self.widget, self.base, self.children)
            }
        }
    };
}

/// The widget of a plain box: it has no content of its own, only its children.
pub struct Container;

impl<M: 'static> Widget<M> for Container {
    type State = ();

    fn create_state(&self) {}

    fn update(&self, _old: &Container, _state: &mut ()) -> Dirty {
        Dirty::None
    }
}

/// A box that holds children and decorates them.
pub struct Div<M: 'static> {
    base: Base<M>,
    widget: Container,
    children: Vec<Element<M>>,
}

builder!(Div<M> => Container, children);

/// A box. Children stack top to bottom unless `.row()` says otherwise.
pub fn div<M: 'static>() -> Div<M> {
    Div { base: Base::new(), widget: Container, children: Vec::new() }
}

/// A box whose children run left to right.
pub fn row<M: 'static>() -> Div<M> {
    div().row()
}

/// A box whose children stack top to bottom.
pub fn column<M: 'static>() -> Div<M> {
    div()
}

/// An empty box that takes the room left over in its row or column.
pub fn spacer<M: 'static>() -> Div<M> {
    div().grow(1.0)
}
