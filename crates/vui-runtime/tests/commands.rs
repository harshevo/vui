//! What `update` can start beyond changing state: follow-up messages, timers, background tasks, and
//! child components whose messages are mapped into the app's.

use std::time::Duration;

use vui_runtime::testing::{TestUi, texts};
use vui_runtime::{App, Builder, Cx, Element, IntoElement, Parent, Styled, column, div, label, row};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Chain,
    Second,
    Toast,
    HideToast,
    Load,
    Loaded(u32),
    Stream,
    Left(Counter),
    Right(Counter),
}

#[derive(Clone, Debug, PartialEq)]
enum Counter {
    Add,
}

#[derive(Default)]
struct Commands {
    log: Vec<&'static str>,
    toast: bool,
    loaded: Vec<u32>,
    left: u32,
    right: u32,
}

/// A component with its own message type, used twice.
fn counter(key: &str, n: u32) -> Element<Counter> {
    div().key(key.to_string()).size(40.0, 20.0).on_click(Counter::Add).child(label(n.to_string())).into_element()
}

impl App for Commands {
    type Msg = Msg;

    fn update(&mut self, msg: Msg, cx: &mut Cx<Msg>) {
        match msg {
            Msg::Chain => {
                self.log.push("first");
                cx.send(Msg::Second);
            }
            Msg::Second => self.log.push("second"),
            Msg::Toast => {
                self.toast = true;
                cx.after(Duration::from_secs(2), Msg::HideToast);
            }
            Msg::HideToast => self.toast = false,
            Msg::Load => cx.spawn(|| Msg::Loaded(6 * 7)),
            Msg::Loaded(n) => self.loaded.push(n),
            Msg::Stream => {
                let tx = cx.sender();
                std::thread::spawn(move || {
                    for n in 1..=3 {
                        tx.send(Msg::Loaded(n));
                    }
                });
            }
            Msg::Left(Counter::Add) => self.left += 1,
            Msg::Right(Counter::Add) => self.right += 1,
        }
    }

    fn view(&self) -> impl IntoElement<Msg> {
        column().children((
            row().children((
                div().key("chain").size(40.0, 20.0).on_click(Msg::Chain),
                div().key("toast").size(40.0, 20.0).on_click(Msg::Toast),
                div().key("load").size(40.0, 20.0).on_click(Msg::Load),
                div().key("stream").size(40.0, 20.0).on_click(Msg::Stream),
            )),
            row().children((counter("left", self.left).map(Msg::Left), counter("right", self.right).map(Msg::Right))),
            self.toast.then(|| label("Saved")),
        ))
    }
}

fn app() -> TestUi<Msg> {
    TestUi::with_app(400.0, 200.0, Commands::default())
}

#[test]
fn a_follow_up_message_is_handled_after_the_first() {
    let mut ui = app();
    ui.click_key("chain");
    assert_eq!(ui.app::<Commands>().log, ["first", "second"]);
}

#[test]
fn a_timer_fires_after_its_delay_and_not_before() {
    let mut ui = app();
    ui.click_key("toast");
    assert!(texts(&ui.scene()).contains(&"Saved".to_string()));
    let wake = ui.ui.next_wake().expect("the timer asks to be woken");
    assert!((wake - 2.0).abs() < 0.01, "woken in {wake} s");
    ui.advance(1.9);
    assert!(ui.app::<Commands>().toast);
    ui.advance(0.1);
    assert!(!ui.app::<Commands>().toast);
    assert!(!texts(&ui.scene()).contains(&"Saved".to_string()));
    assert_eq!(ui.ui.next_wake(), None);
}

#[test]
fn a_background_task_answers_through_update() {
    let mut ui = app();
    ui.click_key("load");
    assert_eq!(ui.ui.pending_tasks(), 1);
    assert!(ui.ui.wait(Duration::from_secs(5)));
    assert_eq!(ui.app::<Commands>().loaded, [42]);
    assert_eq!(ui.ui.pending_tasks(), 0);
}

#[test]
fn a_sender_streams_messages_from_another_thread() {
    let mut ui = app();
    ui.click_key("stream");
    while ui.app::<Commands>().loaded.len() < 3 {
        assert!(ui.ui.wait(Duration::from_secs(5)), "the stream stopped early");
    }
    assert_eq!(ui.app::<Commands>().loaded, [1, 2, 3]);
}

#[test]
fn the_waker_is_called_when_a_task_finishes() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut ui = app();
    let woken = Arc::new(AtomicUsize::new(0));
    let w = woken.clone();
    ui.ui.set_waker(Arc::new(move || {
        w.fetch_add(1, Ordering::SeqCst);
    }));
    ui.click_key("load");
    let start = std::time::Instant::now();
    while woken.load(Ordering::SeqCst) == 0 && start.elapsed() < Duration::from_secs(5) {
        std::thread::yield_now();
    }
    assert_eq!(woken.load(Ordering::SeqCst), 1);
    ui.ui.poll();
    assert_eq!(ui.app::<Commands>().loaded, [42]);
}

#[test]
fn mapped_components_send_the_parents_messages() {
    let mut ui = app();
    ui.click_key("left").click_key("right").click_key("right");
    let app = ui.app::<Commands>();
    assert_eq!((app.left, app.right), (1, 2));
}

#[test]
fn mapped_widgets_keep_their_state_across_updates() {
    // Text inside a mapped component is still measured and painted, and survives a re-render.
    let mut ui = app();
    ui.click_key("left");
    ui.click_key("left");
    let shown = texts(&ui.scene());
    assert!(shown.contains(&"2".to_string()), "{shown:?}");
}
