//! The event loop: winit's events go to the window they belong to, what the UI asks back is carried
//! out, and frames are drawn on redraw requests, one per display refresh while something moves and
//! none while the UI is still.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use vui_core::{Event, Modifiers, Point, PointerButton, pt, size};
use vui_runtime::{Answer, Effect};
use vui_shell::frames::frame_step;
use vui_shell::host::Host;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Fullscreen, WindowId};

use crate::app::{Application, mode};
use crate::input::{self, Clicks};

/// A window and the UI in it.
pub(crate) struct Slot {
    pub(crate) window: Arc<winit::window::Window>,
    host: Box<dyn Host>,
    quit_on_close: bool,
    pointer: Point,
    mods: Modifiers,
    clicks: Clicks,
    composing: bool,
    ime_on: bool,
    /// When the last frame was drawn; `None` while frames are paused.
    last_frame: Option<Instant>,
    /// When the UI wants a tick though nothing moves: a caret blink or an app timer.
    wake_at: Option<Instant>,
    /// Covered or minimised: no frames until it shows again.
    hidden: bool,
    /// Files dragged over the window, and files dropped, which winit reports one at a time.
    hovered: Vec<PathBuf>,
    dropped: Vec<PathBuf>,
}

impl Slot {
    pub(crate) fn new(window: Arc<winit::window::Window>, host: Box<dyn Host>, quit_on_close: bool) -> Slot {
        Slot {
            window,
            host,
            quit_on_close,
            pointer: pt(0.0, 0.0),
            mods: Modifiers::NONE,
            clicks: Clicks::default(),
            composing: false,
            ime_on: false,
            last_frame: None,
            wake_at: None,
            hidden: false,
            hovered: Vec::new(),
            dropped: Vec::new(),
        }
    }

    fn scale(&self) -> f32 {
        self.window.scale_factor() as f32
    }

    fn send(&mut self, event: Event) -> Vec<Effect> {
        self.host.handle(event)
    }

    /// One display refresh, in seconds.
    fn refresh(&self) -> f64 {
        let hz = self.window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).unwrap_or(60_000);
        1000.0 / hz.max(1) as f64
    }

    /// Carries out what the UI asked for, then has the window drawn if anything changed.
    fn apply(&mut self, effects: Vec<Effect>) {
        let mut queue = effects;
        while !queue.is_empty() {
            for effect in std::mem::take(&mut queue) {
                match effect {
                    Effect::Cursor(icon) => self.window.set_cursor(input::cursor(icon)),
                    Effect::ImeArea(area) => {
                        if area.is_some() != self.ime_on {
                            self.ime_on = area.is_some();
                            self.window.set_ime_allowed(self.ime_on);
                        }
                        if let Some(r) = area {
                            self.window.set_ime_cursor_area(LogicalPosition::new(r.x as f64, r.y as f64), LogicalSize::new(r.w.max(1.0) as f64, r.h.max(1.0) as f64));
                        }
                    }
                    // No native dialogs here yet: answer as if cancelled, so the app does not wait.
                    Effect::Open { id, .. } | Effect::Save { id, .. } => {
                        warn_once("open and save dialogs are not available on this platform yet");
                        queue.extend(self.host.answer(id, Answer::Paths(Vec::new())));
                    }
                    Effect::Notify { notification, .. } => {
                        warn_once("notifications are not available on this platform yet");
                        let _ = notification;
                    }
                    Effect::FullScreen(on) => {
                        let is = self.window.fullscreen().is_some();
                        if on.is_none_or(|on| on != is) {
                            self.window.set_fullscreen((!is).then_some(Fullscreen::Borderless(None)));
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(through) = self.host.passes_clicks() {
            let _ = self.window.set_cursor_hittest(!through);
        }
        if self.host.takes_keys() == Some(true) && !self.window.has_focus() {
            self.window.focus_window();
        }
        if self.host.needs_paint() {
            self.window.request_redraw();
        } else if let Some(wake) = self.host.next_wake() {
            self.wake_at = Some(Instant::now() + Duration::from_secs_f32(wake.max(0.0)));
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let refresh = self.refresh();
        let dt = match self.last_frame {
            Some(last) => frame_step(Some(0.0), now.duration_since(last).as_secs_f64(), refresh),
            None => frame_step(None, 0.0, refresh),
        };
        self.window.pre_present_notify();
        let (result, effects) = self.host.frame(dt);
        self.hidden = result.hidden;
        self.wake_at = result.wake.map(|s| now + Duration::from_secs_f32(s.max(0.0)));
        if result.animating && !result.hidden {
            self.last_frame = Some(now);
            self.window.request_redraw();
        } else {
            self.last_frame = None;
        }
        self.apply(effects);
    }

    fn event(&mut self, event: WindowEvent, exit: &mut bool) {
        let scale = self.scale();
        let effects = match event {
            WindowEvent::RedrawRequested => {
                if !self.hidden {
                    self.frame();
                }
                return;
            }
            WindowEvent::Resized(px) => {
                self.host.resize(size(px.width as f32 / scale, px.height as f32 / scale), scale);
                self.hidden = px.width == 0 || px.height == 0;
                self.host.redraw();
                self.window.request_redraw();
                return;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let px = self.window.inner_size();
                let s = scale_factor as f32;
                self.host.resize(size(px.width as f32 / s, px.height as f32 / s), s);
                self.window.request_redraw();
                return;
            }
            WindowEvent::CloseRequested => {
                if self.quit_on_close {
                    *exit = true;
                } else {
                    self.window.set_visible(false);
                }
                return;
            }
            WindowEvent::Occluded(covered) => {
                self.hidden = covered;
                if !covered {
                    self.window.request_redraw();
                }
                return;
            }
            WindowEvent::ThemeChanged(theme) => self.host.set_system_mode(mode(Some(theme))),
            WindowEvent::Focused(on) => {
                if on {
                    self.hidden = false;
                }
                self.send(Event::WindowFocus(on))
            }
            WindowEvent::ModifiersChanged(m) => {
                self.mods = input::modifiers(m.state());
                return;
            }
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
                let unmodified = {
                    use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
                    Some(event.key_without_modifiers())
                };
                #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
                let unmodified: Option<winit::keyboard::Key> = None;
                let pressed = event.state == ElementState::Pressed;
                match input::key_event(&event.logical_key, unmodified.as_ref(), event.text.as_deref(), pressed, event.repeat, self.mods) {
                    Some(key) => self.send(Event::Key(key)),
                    None => return,
                }
            }
            WindowEvent::Ime(ime) => match input::ime(ime, &mut self.composing) {
                Some(e) => self.send(Event::Ime(e)),
                None => return,
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = pt(position.x as f32 / scale, position.y as f32 / scale);
                let mut effects = self.send(Event::PointerMoved { pos: self.pointer, mods: self.mods });
                if !self.hovered.is_empty() {
                    effects.extend(self.send(Event::DragOver { pos: self.pointer, paths: self.hovered.clone() }));
                }
                effects
            }
            WindowEvent::CursorLeft { .. } => self.send(Event::PointerLeft),
            WindowEvent::MouseInput { state, button, .. } => {
                let button = input::button(button);
                let pos = self.pointer;
                if state == ElementState::Released {
                    self.send(Event::PointerUp { pos, button, mods: self.mods })
                } else if button == PointerButton::Primary && self.host.drags_window(pos) {
                    let _ = self.window.drag_window();
                    return;
                } else {
                    let click_count = self.clicks.press(Instant::now(), pos, button);
                    self.send(Event::PointerDown { pos, button, mods: self.mods, click_count })
                }
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let (delta, precise, phase) = input::scroll(delta, phase, scale);
                self.send(Event::Scroll { pos: self.pointer, delta, precise, phase, momentum: false })
            }
            WindowEvent::PinchGesture { delta, phase, .. } if delta.is_finite() => {
                self.send(Event::Magnify { pos: self.pointer, delta: delta as f32, phase: input::gesture_phase(phase) })
            }
            WindowEvent::HoveredFile(path) => {
                self.hovered.push(path);
                self.send(Event::DragOver { pos: self.pointer, paths: self.hovered.clone() })
            }
            WindowEvent::HoveredFileCancelled => {
                self.hovered.clear();
                self.send(Event::DragLeft)
            }
            WindowEvent::DroppedFile(path) => {
                self.hovered.clear();
                self.dropped.push(path);
                return;
            }
            _ => return,
        };
        self.apply(effects);
    }

    /// Hands over the files dropped since the last time: winit reports them one by one.
    fn flush_drops(&mut self) {
        if !self.dropped.is_empty() {
            let paths = std::mem::take(&mut self.dropped);
            let effects = self.send(Event::Drop { pos: self.pointer, paths });
            self.apply(effects);
        }
    }
}

fn warn_once(what: &str) {
    use std::sync::Mutex;
    static WARNED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    if let Ok(mut warned) = WARNED.lock()
        && !warned.iter().any(|w| w == what)
    {
        eprintln!("vui: {what}");
        warned.push(what.to_string());
    }
}

impl Application {
    fn quit_if_asked(&self, event_loop: &ActiveEventLoop, exit: bool) {
        if exit || self.slots.iter().any(|s| s.host.quit_requested()) {
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<usize> for Application {
    /// Windows are opened before the loop starts, so there is nothing to make here.
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause {
            let now = Instant::now();
            for slot in &mut self.slots {
                if slot.wake_at.is_some_and(|t| t <= now) {
                    slot.wake_at = None;
                    slot.window.request_redraw();
                }
            }
        }
    }

    /// A `Proxy`, a background task or a timer has sent window `n` something.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, n: usize) {
        if let Some(slot) = self.slots.get_mut(n) {
            let effects = slot.host.deliver();
            slot.apply(effects);
        }
        self.quit_if_asked(event_loop, false);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let mut exit = false;
        if let Some(slot) = self.slots.iter_mut().find(|s| s.window.id() == id) {
            slot.event(event, &mut exit);
        }
        self.quit_if_asked(event_loop, exit);
    }

    /// Before sleeping: deliver dropped files, then sleep until the nearest timer, or until the next
    /// event if there is none. A window that is drawing asks for its next frame itself.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        for slot in &mut self.slots {
            slot.flush_drops();
        }
        let next = self.slots.iter().filter_map(|s| s.wake_at).min();
        event_loop.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
        self.quit_if_asked(event_loop, false);
    }
}
