//! Plain boxes: the `builder!` macro that gives a widget the whole vocabulary, and `div`, `row`,
//! `column` and `spacer`.

use super::*;

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
