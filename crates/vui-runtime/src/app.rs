//! Hosting an Elm-style app inside a [`Ui`](crate::Ui): the app owns its state, `view` draws it,
//! `update` answers the messages widgets send, and the UI runs that loop itself.

use std::any::Any;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use vui_core::{Mode, Size, Theme};

use crate::element::{Element, IntoElement};
use crate::key::Key;
use crate::platform::{FileDialog, Notification};

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
///     fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
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

    /// Handles one message from a widget, a timer, a background task or another thread. Change the
    /// state; the UI asks for a fresh `view` afterwards. `cx` reaches the rest of the world: focus,
    /// timers, background work and the window.
    fn update(&mut self, msg: Self::Msg, cx: &mut Cx<Self::Msg>);

    /// What the window shows for the current state.
    fn view(&self) -> impl IntoElement<Self::Msg>;

    /// The look of the window, given whether the system is in light or dark mode. Asked again after
    /// every update and whenever the system mode changes; the default follows the system.
    fn theme(&self, system: Mode) -> Theme {
        Theme::for_mode(system)
    }

    /// The window's size, when it is first shown and whenever it changes. An app that needs the size
    /// in its view answers with a message to keep it; the default ignores it.
    fn resized(&self, size: Size) -> Option<Self::Msg> {
        let _ = size;
        None
    }

    /// The app's own menus in the menu bar, with their shortcuts: "File" after the app menu, the
    /// rest between Edit and Window. Asked
    /// again after every update, so items can be enabled, ticked or renamed from state.
    fn menus(&self) -> Vec<crate::Menu<Self::Msg>> {
        Vec::new()
    }

    /// An item in the system menu bar for as long as the app wants one.
    fn status_item(&self) -> Option<crate::StatusItem<Self::Msg>> {
        None
    }
}

/// Wakes the window's event loop from another thread, so it collects what background tasks sent.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// What `update` can ask of the UI beyond changing state: focus, timers, background work, follow-up
/// messages and the window.
pub struct Cx<M: 'static> {
    pub(crate) focus: Option<Key>,
    pub(crate) quit: bool,
    pub(crate) pass_clicks: Option<bool>,
    pub(crate) take_keys: Option<bool>,
    pub(crate) then: Vec<M>,
    pub(crate) timers: Vec<(f32, M)>,
    pub(crate) tasks: Tasks<M>,
    pub(crate) requests: Vec<Request<M>>,
}

/// What `update` asked of the system, waiting to go out as effects.
pub(crate) enum Request<M> {
    Open(FileDialog, Box<dyn FnOnce(Vec<PathBuf>) -> M>),
    Save(FileDialog, Box<dyn FnOnce(Option<PathBuf>) -> M>),
    Notify(Notification),
    FullScreen(Option<bool>),
}

/// Where background tasks send their answers.
pub(crate) struct Tasks<M> {
    pub tx: Sender<M>,
    pub waker: Option<Waker>,
    /// Tasks started and not yet answered.
    pub pending: Rc<Cell<usize>>,
}

impl<M> Clone for Tasks<M> {
    fn clone(&self) -> Tasks<M> {
        Tasks { tx: self.tx.clone(), waker: self.waker.clone(), pending: self.pending.clone() }
    }
}

impl<M: 'static> Cx<M> {
    pub(crate) fn new(tasks: Tasks<M>) -> Cx<M> {
        Cx { focus: None, quit: false, pass_clicks: None, take_keys: None, then: Vec::new(), timers: Vec::new(), tasks, requests: Vec::new() }
    }

    /// Moves focus to the keyed element once the new view is in place.
    pub fn focus(&mut self, key: impl Into<Key>) {
        self.focus = Some(key.into());
    }

    /// Handles `msg` right after this one, once the view is up to date: for one step that leads to
    /// another.
    pub fn send(&mut self, msg: M) {
        self.then.push(msg);
    }

    /// Sends `msg` to `update` after `delay`: a toast that hides itself, a debounced search, a
    /// clock. Timers run on UI time, so tests move them with `TestUi::advance`. To repeat, set
    /// another from the update that handles it.
    pub fn after(&mut self, delay: Duration, msg: M) {
        self.timers.push((delay.as_secs_f32(), msg));
    }

    /// Asks the window system to close the app. The backend reads [`Ui::quit_requested`](crate::Ui::quit_requested).
    pub fn quit(&mut self) {
        self.quit = true;
    }

    /// Lets clicks through the window to whatever is under it (true), or has the window take them, such
    /// as while an overlay shows something to click. The backend reads [`Ui::passes_clicks`](crate::Ui::passes_clicks).
    pub fn pass_clicks(&mut self, through: bool) {
        self.pass_clicks = Some(through);
    }

    /// Has an overlay take the keyboard (true) while the person types into it, or give it back to
    /// the app they were in. Only an overlay opened to take keys does; a window has them whenever
    /// it is in front. The backend reads [`Ui::takes_keys`](crate::Ui::takes_keys).
    pub fn take_keys(&mut self, take: bool) {
        self.take_keys = Some(take);
    }

    /// Shows an open dialog and sends `reply` with what was picked: no paths if it was cancelled.
    ///
    /// ```
    /// # use std::path::PathBuf;
    /// # use vui_runtime::{Cx, FileDialog};
    /// # enum Msg { Opened(Vec<PathBuf>) }
    /// # fn open(cx: &mut Cx<Msg>) {
    /// cx.open_files(FileDialog::new().extensions(["md", "txt"]).multiple(), Msg::Opened);
    /// # }
    /// ```
    pub fn open_files(&mut self, dialog: FileDialog, reply: impl FnOnce(Vec<PathBuf>) -> M + 'static) {
        self.requests.push(Request::Open(dialog, Box::new(reply)));
    }

    /// Shows a save dialog and sends `reply` with the path chosen, or `None` if it was cancelled.
    pub fn save_file(&mut self, dialog: FileDialog, reply: impl FnOnce(Option<PathBuf>) -> M + 'static) {
        self.requests.push(Request::Save(dialog, Box::new(reply)));
    }

    /// Shows a system notification.
    pub fn notify(&mut self, notification: Notification) {
        self.requests.push(Request::Notify(notification));
    }

    /// Takes the window full screen, or brings it back.
    pub fn full_screen(&mut self, on: bool) {
        self.requests.push(Request::FullScreen(Some(on)));
    }

    pub fn toggle_full_screen(&mut self) {
        self.requests.push(Request::FullScreen(None));
    }
}

impl<M: Send + 'static> Cx<M> {
    /// Runs `work` on a thread of its own and hands what it returns to `update`: reading a file,
    /// a network request, anything that would stall the window. The view stays responsive meanwhile;
    /// show progress from state set before spawning.
    ///
    /// ```
    /// # use vui_runtime::Cx;
    /// # enum Msg { Loaded(String) }
    /// # fn start(cx: &mut Cx<Msg>) {
    /// cx.spawn(|| Msg::Loaded(std::fs::read_to_string("notes.txt").unwrap_or_default()));
    /// # }
    /// ```
    pub fn spawn(&mut self, work: impl FnOnce() -> M + Send + 'static) {
        let Tasks { tx, waker, pending } = self.tasks.clone();
        pending.set(pending.get() + 1);
        std::thread::Builder::new()
            .name("vui-task".into())
            .spawn(move || {
                let msg = work();
                if tx.send(msg).is_ok()
                    && let Some(wake) = waker
                {
                    wake();
                }
            })
            .expect("the system could not start a thread");
    }

    /// A handle that sends the app messages from anywhere, for as long as the window lives: a
    /// stream of events from a socket, progress from a long job.
    pub fn sender(&self) -> AppSender<M> {
        AppSender { tx: self.tasks.tx.clone(), waker: self.tasks.waker.clone() }
    }
}

/// Sends an app messages from any thread; see [`Cx::sender`].
pub struct AppSender<M> {
    tx: Sender<M>,
    waker: Option<Waker>,
}

impl<M> Clone for AppSender<M> {
    fn clone(&self) -> AppSender<M> {
        AppSender { tx: self.tx.clone(), waker: self.waker.clone() }
    }
}

impl<M: Send> AppSender<M> {
    /// Queues `msg` for `update`. False once the window is gone.
    pub fn send(&self, msg: M) -> bool {
        if self.tx.send(msg).is_err() {
            return false;
        }
        if let Some(wake) = &self.waker {
            wake();
        }
        true
    }
}

/// The type-erased app a `Ui` runs.
pub(crate) trait Driver<M> {
    fn update(&mut self, msg: M, cx: &mut Cx<M>);
    fn view(&self) -> Element<M>;
    fn theme(&self, system: Mode) -> Theme;
    fn resized(&self, size: Size) -> Option<M>;
    fn menus(&self) -> Vec<crate::Menu<M>>;
    fn status_item(&self) -> Option<crate::StatusItem<M>>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<A: App> Driver<A::Msg> for A {
    fn update(&mut self, msg: A::Msg, cx: &mut Cx<A::Msg>) {
        App::update(self, msg, cx);
    }

    fn view(&self) -> Element<A::Msg> {
        App::view(self).into_element()
    }

    fn theme(&self, system: Mode) -> Theme {
        App::theme(self, system)
    }

    fn resized(&self, size: Size) -> Option<A::Msg> {
        App::resized(self, size)
    }

    fn menus(&self) -> Vec<crate::Menu<A::Msg>> {
        App::menus(self)
    }

    fn status_item(&self) -> Option<crate::StatusItem<A::Msg>> {
        App::status_item(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
