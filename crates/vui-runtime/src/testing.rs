//! Driving a [`Ui`] from tests: no window, no GPU, deterministic fonts and a clock you move by hand.

use vui_core::{
    Command, Event, ImeEvent, Key as KeyCode, KeyEvent, Modifiers, NamedKey, Point, PointerButton, Rect, Resources, Scene, ScrollPhase,
    Size, Theme, pt, size, vec2,
};

use crate::app::App;
use crate::effects::Effect;
use crate::element::IntoElement;
use crate::key::Key;
use crate::ui::{Inspect, Ui};

/// A [`Ui`] with helpers that read like a user: click here, type that, wait a moment.
pub struct TestUi<M: 'static> {
    pub ui: Ui<M>,
}

impl<M: 'static> TestUi<M> {
    /// A light-themed window of this size at scale 1, with only the bundled fonts.
    pub fn new(width: f32, height: f32) -> TestUi<M> {
        TestUi::with_theme(size(width, height), Theme::light(), 1.0)
    }

    pub fn with_theme(size: Size, theme: Theme, scale: f32) -> TestUi<M> {
        TestUi { ui: Ui::new(Resources::bundled_only(), theme, size, scale) }
    }

    /// A window that runs `app` at scale 1, in whatever theme the app asks for.
    pub fn with_app<A: App<Msg = M>>(width: f32, height: f32, app: A) -> TestUi<M> {
        TestUi::with_app_scaled(size(width, height), 1.0, app)
    }

    pub fn with_app_scaled<A: App<Msg = M>>(size: Size, scale: f32, app: A) -> TestUi<M> {
        TestUi { ui: Ui::with_app(Resources::bundled_only(), size, scale, app) }
    }

    /// The hosted app. Panics if there is none or it is another type, which is what a test wants.
    pub fn app<A: App<Msg = M>>(&self) -> &A {
        self.ui.app::<A>().expect("the window does not run an app of this type")
    }

    /// Changes the hosted app's state as the host would, and shows the result.
    pub fn update_app<A: App<Msg = M>, R>(&mut self, f: impl FnOnce(&mut A) -> R) -> R {
        self.ui.update_app(f).expect("the window does not run an app of this type")
    }

    /// Shows this tree, as the app would after an update.
    pub fn view(&mut self, view: impl IntoElement<M>) -> &mut Self {
        self.ui.set_view(view);
        self
    }

    pub fn messages(&mut self) -> Vec<M> {
        self.ui.take_messages()
    }

    pub fn effects(&mut self) -> Vec<Effect> {
        self.ui.take_effects()
    }

    pub fn scene(&mut self) -> Scene {
        self.ui.paint()
    }

    /// The border box of a keyed element. Panics if there is none, which is what a test wants.
    pub fn rect(&mut self, key: impl Into<Key>) -> Rect {
        let key = key.into();
        self.ui.rect_of(key.clone()).unwrap_or_else(|| panic!("no element with key {key:?}"))
    }

    pub fn inspect(&mut self, key: impl Into<Key>) -> Inspect {
        let key = key.into();
        self.ui.inspect(key.clone()).unwrap_or_else(|| panic!("no element with key {key:?}"))
    }

    /// The middle of a keyed element.
    pub fn center(&mut self, key: impl Into<Key>) -> Point {
        self.rect(key).center()
    }

    // ---- pointer ------------------------------------------------------------------------------

    pub fn move_to(&mut self, p: Point) -> &mut Self {
        self.ui.handle(Event::PointerMoved { pos: p, mods: Modifiers::NONE });
        self
    }

    pub fn move_to_key(&mut self, key: impl Into<Key>) -> &mut Self {
        let p = self.center(key);
        self.move_to(p)
    }

    pub fn leave(&mut self) -> &mut Self {
        self.ui.handle(Event::PointerLeft);
        self
    }

    pub fn press(&mut self, p: Point) -> &mut Self {
        self.press_button(p, PointerButton::Primary, 1)
    }

    pub fn press_button(&mut self, p: Point, button: PointerButton, count: u8) -> &mut Self {
        self.ui.handle(Event::PointerMoved { pos: p, mods: Modifiers::NONE });
        self.ui.handle(Event::PointerDown { pos: p, button, mods: Modifiers::NONE, click_count: count });
        self
    }

    pub fn release(&mut self, p: Point) -> &mut Self {
        self.release_button(p, PointerButton::Primary)
    }

    pub fn release_button(&mut self, p: Point, button: PointerButton) -> &mut Self {
        self.ui.handle(Event::PointerUp { pos: p, button, mods: Modifiers::NONE });
        self
    }

    pub fn click(&mut self, p: Point) -> &mut Self {
        self.press(p).release(p)
    }

    pub fn click_key(&mut self, key: impl Into<Key>) -> &mut Self {
        let p = self.center(key);
        self.click(p)
    }

    pub fn double_click(&mut self, p: Point) -> &mut Self {
        self.press_button(p, PointerButton::Primary, 1).release(p);
        self.press_button(p, PointerButton::Primary, 2).release(p)
    }

    pub fn right_click(&mut self, p: Point) -> &mut Self {
        self.press_button(p, PointerButton::Secondary, 1).release_button(p, PointerButton::Secondary)
    }

    /// A mouse wheel tick: `dy` is how far the content should move, positive down.
    pub fn wheel(&mut self, p: Point, dx: f32, dy: f32) -> &mut Self {
        self.ui.handle(Event::Scroll {
            pos: p,
            delta: vec2(dx, dy),
            precise: false,
            phase: ScrollPhase::None,
            momentum: false,
        });
        self
    }

    /// A trackpad scroll, which moves the content exactly as far as the fingers did.
    pub fn trackpad(&mut self, p: Point, dx: f32, dy: f32) -> &mut Self {
        self.ui.handle(Event::Scroll {
            pos: p,
            delta: vec2(dx, dy),
            precise: true,
            phase: ScrollPhase::Changed,
            momentum: false,
        });
        self
    }

    pub fn drag_over(&mut self, p: Point, paths: &[&str]) -> &mut Self {
        self.ui.handle(Event::DragOver { pos: p, paths: paths.iter().map(Into::into).collect() });
        self
    }

    pub fn drop_files(&mut self, p: Point, paths: &[&str]) -> &mut Self {
        self.ui.handle(Event::Drop { pos: p, paths: paths.iter().map(Into::into).collect() });
        self
    }

    // ---- keyboard -----------------------------------------------------------------------------

    pub fn key(&mut self, key: NamedKey) -> &mut Self {
        self.ui.handle(Event::Key(KeyEvent::named(key)));
        self
    }

    pub fn key_with(&mut self, key: NamedKey, mods: Modifiers) -> &mut Self {
        self.ui.handle(Event::Key(KeyEvent::named(key).with_mods(mods)));
        self
    }

    pub fn tab(&mut self) -> &mut Self {
        self.key(NamedKey::Tab)
    }

    pub fn shift_tab(&mut self) -> &mut Self {
        self.key_with(NamedKey::Tab, Modifiers::SHIFT)
    }

    /// Command plus a letter.
    pub fn shortcut(&mut self, c: char) -> &mut Self {
        self.ui.handle(Event::Key(KeyEvent::shortcut(c)));
        self
    }

    /// Types text one character at a time.
    pub fn type_text(&mut self, text: &str) -> &mut Self {
        for c in text.chars() {
            let event = match c {
                ' ' => KeyEvent { key: KeyCode::Named(NamedKey::Space), text: Some(" ".into()), ..KeyEvent::named(NamedKey::Space) },
                c => KeyEvent::character(c),
            };
            self.ui.handle(Event::Key(event));
        }
        self
    }

    pub fn ime(&mut self, event: ImeEvent) -> &mut Self {
        self.ui.handle(Event::Ime(event));
        self
    }

    pub fn window_focus(&mut self, focused: bool) -> &mut Self {
        self.ui.handle(Event::WindowFocus(focused));
        self
    }

    // ---- time ---------------------------------------------------------------------------------

    /// Lets `seconds` pass, in 60 Hz frames.
    pub fn advance(&mut self, seconds: f32) -> &mut Self {
        let frame = 1.0 / 60.0;
        let mut left = seconds;
        while left > 0.0 {
            let dt = left.min(frame);
            self.ui.tick(dt);
            left -= dt;
        }
        self
    }

    /// Runs frames until nothing is animating, for at most `limit` seconds.
    pub fn settle(&mut self) -> &mut Self {
        let mut spent = 0.0;
        while self.ui.needs_frame() && spent < 5.0 {
            self.ui.tick(1.0 / 60.0);
            spent += 1.0 / 60.0;
        }
        self
    }
}

/// Everything the scene draws as text, in paint order, inside layers too.
pub fn texts(scene: &Scene) -> Vec<String> {
    scene
        .walk()
        .filter_map(|c| match c {
            Command::Text(t) => Some(t.layout.text().to_string()),
            _ => None,
        })
        .collect()
}

/// The filled quads, in paint order.
pub fn quads(scene: &Scene) -> Vec<&vui_core::Quad> {
    scene
        .walk()
        .filter_map(|c| match c {
            Command::Quad(q) => Some(q),
            _ => None,
        })
        .collect()
}

/// The point at fractions of a rect, for aiming at a part of an element.
pub fn at(rect: Rect, fx: f32, fy: f32) -> Point {
    pt(rect.x + rect.w * fx, rect.y + rect.h * fy)
}
