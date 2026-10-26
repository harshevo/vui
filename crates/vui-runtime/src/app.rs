//! Hosting an Elm-style app inside a [`Ui`](crate::Ui): the app owns its state, `view` draws it,
//! `update` answers the messages widgets send, and the UI runs that loop itself.

use std::any::Any;

use vui_core::Theme;

use crate::element::{Element, IntoElement};
use crate::key::Key;

/// An application: some state, a view of it, and how messages change it.
///
/// ```
/// use vui_runtime::{App, Builder, Cx, IntoElement, Parent, column, label};
///
/// struct Counter(u32);
///
/// #[derive(Clone)]
/// enum Msg {
///     Add,
/// }
///
/// impl App for Counter {
///     type Msg = Msg;
///
///     fn update(&mut self, msg: Msg, _cx: &mut Cx) {
///         match msg {
///             Msg::Add => self.0 += 1,
///         }
///     }
///
///     fn view(&self) -> impl IntoElement<Msg> {
///         column().key("add").on_click(Msg::Add).child(label(self.0.to_string()))
///     }
/// }
/// ```
pub trait App: 'static {
    type Msg: 'static;

    /// Handles one message from a widget. Change the state; the UI asks for a fresh `view` afterwards.
    fn update(&mut self, msg: Self::Msg, cx: &mut Cx);

    /// What the window shows for the current state.
    fn view(&self) -> impl IntoElement<Self::Msg>;

    /// The look of the window. Asked again after every update, so switching between light and dark is
    /// a matter of changing state.
    fn theme(&self) -> Theme {
        Theme::light()
    }
}

/// What `update` can ask of the UI beyond changing state.
#[derive(Default)]
pub struct Cx {
    pub(crate) focus: Option<Key>,
    pub(crate) quit: bool,
}

impl Cx {
    /// Moves focus to the keyed element once the new view is in place.
    pub fn focus(&mut self, key: impl Into<Key>) {
        self.focus = Some(key.into());
    }

    /// Asks the window system to close the app. The backend reads [`Ui::quit_requested`](crate::Ui::quit_requested).
    pub fn quit(&mut self) {
        self.quit = true;
    }
}

/// The type-erased app a `Ui` runs.
pub(crate) trait Driver<M> {
    fn update(&mut self, msg: M, cx: &mut Cx);
    fn view(&self) -> Element<M>;
    fn theme(&self) -> Theme;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<A: App> Driver<A::Msg> for A {
    fn update(&mut self, msg: A::Msg, cx: &mut Cx) {
        App::update(self, msg, cx);
    }

    fn view(&self) -> Element<A::Msg> {
        App::view(self).into_element()
    }

    fn theme(&self) -> Theme {
        App::theme(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
