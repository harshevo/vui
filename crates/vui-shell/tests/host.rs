//! The host every backend drives, with CPU output and no window: frames are drawn when something
//! changes and not while the UI is still, messages from a `Proxy` arrive, and snapshots are written.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::Arc;

use vui_core::{Event, Resources, Theme, pt, size};
use vui_render::CpuRenderer;
use vui_runtime::{App, Cx, IntoElement, Parent, Styled, Ui};
use vui_shell::Proxy;
use vui_shell::host::{Host, Output, Snapshot, Window};

#[derive(Clone, Debug)]
enum Msg {
    Bump,
}

#[derive(Default)]
struct Counter {
    n: u32,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, _cx: &mut Cx<Msg>) {
        match msg {
            Msg::Bump => self.n += 1,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        vui_runtime::column().padding(8.0).child(vui_runtime::label(format!("{} bumps", self.n)))
    }
}

/// The sizes of the frames shown, in pixels.
type Shown = Rc<RefCell<Vec<(u32, u32)>>>;

/// A host whose frames are counted, with the sizes they came in.
fn host(snapshot: Snapshot) -> (Window<Msg>, Shown, mpsc::Sender<Msg>) {
    let res = Resources::bundled_only();
    let mut ui = Ui::with_app(res.clone(), size(120.0, 60.0), 1.0, Counter::default());
    ui.set_system_mode(Theme::light().mode);
    let shown = Rc::new(RefCell::new(Vec::new()));
    let log = shown.clone();
    let output = Output::Cpu { renderer: CpuRenderer::new(res), show: Box::new(move |canvas, _scale| log.borrow_mut().push((canvas.width, canvas.height))) };
    let (tx, inbox) = mpsc::channel();
    (Window::new(ui, output, inbox, snapshot), shown, tx)
}

/// Runs frames until the UI is still, as a backend's frame loop does.
fn settle(h: &mut Window<Msg>) {
    for _ in 0..600 {
        let (result, _) = h.frame(1.0 / 60.0);
        if !result.animating {
            return;
        }
    }
    panic!("the UI never settled");
}

#[test]
fn frames_are_drawn_when_something_changes_and_not_while_still() {
    let (mut h, shown, _tx) = host(Snapshot::none());
    h.resize(size(120.0, 60.0), 2.0);
    settle(&mut h);
    assert!(!shown.borrow().is_empty(), "the first frame");
    assert_eq!(*shown.borrow().last().unwrap(), (240, 120), "drawn at the backing scale");
    let drawn = shown.borrow().len();
    for _ in 0..10 {
        let (result, _) = h.frame(1.0 / 60.0);
        assert!(!result.animating);
    }
    assert_eq!(shown.borrow().len(), drawn, "a still UI costs nothing");
    assert!(!h.needs_paint());

    h.handle(Event::PointerMoved { pos: pt(-10.0, -10.0), mods: Default::default() });
    h.resize(size(100.0, 50.0), 1.0);
    assert!(h.needs_paint(), "a resize needs a frame");
    settle(&mut h);
    assert_eq!(*shown.borrow().last().unwrap(), (100, 50));
}

#[test]
fn proxy_messages_reach_the_app() {
    let (mut h, shown, tx) = host(Snapshot::none());
    settle(&mut h);
    let woken = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = woken.clone();
    let proxy = Proxy::new(tx, Arc::new(move || flag.store(true, std::sync::atomic::Ordering::SeqCst)));
    std::thread::spawn(move || proxy.send(Msg::Bump)).join().unwrap();
    assert!(woken.load(std::sync::atomic::Ordering::SeqCst), "the backend is woken");
    let before = shown.borrow().len();
    h.deliver();
    assert!(h.needs_paint(), "the new state needs drawing");
    settle(&mut h);
    assert!(shown.borrow().len() > before);
    assert!(h.ui.app::<Counter>().is_some_and(|c| c.n == 1));
}

#[test]
fn the_snapshot_is_written_once_settled_and_asks_to_quit() {
    let dir = std::env::temp_dir().join(format!("vui-shell-host-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("frame.png");
    let pending = Rc::new(Cell::new(1));
    let (mut h, _shown, _tx) = host(Snapshot { path: Some(path.clone()), pending: pending.clone(), quit: true });
    assert!(!h.quit_requested());
    settle(&mut h);
    assert!(path.exists(), "written");
    assert_eq!(pending.get(), 0);
    assert!(h.quit_requested(), "the last snapshot quits");
    let _ = std::fs::remove_dir_all(dir);
}
