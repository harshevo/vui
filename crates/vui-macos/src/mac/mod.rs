//! The AppKit side: the application, a window, and the view that hosts a vui UI.
//!
//! The view is layer-backed; wgpu adds its Metal layer to it. Frames come from the view's display link,
//! which runs only while something moves and pauses when the UI is idle, so a still window costs
//! nothing. Keys go through the input method (`NSTextInputClient`) so dead keys, Japanese, Chinese and
//! Korean input, dictation and the emoji picker all type into vui's text fields.

mod pasteboard;
pub use pasteboard::Pasteboard;

mod menu;
mod system;
use menu::main_menu;

mod frames;
mod input;
mod view;
mod window;

use input::*;
use view::*;
use window::*;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::mpsc;

use block2::RcBlock;
use dispatch2::DispatchQueue;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy,
    NSBackingStoreType, NSColor, NSCursor, NSEvent, NSEventMask, NSEventModifierFlags, NSMainMenuWindowLevel, NSMenu, NSMenuItem, NSPanel,
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF, NSResponder, NSScreen, NSTextInputClient, NSTrackingArea, NSTrackingAreaOptions, NSView,
    NSWindow, NSWindowCollectionBehavior, NSWindowDelegate, NSWindowOcclusionState, NSWindowStyleMask, NSWindowTitleVisibility, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSNotification, NSPoint, NSRange, NSRect, NSRunLoop, NSRunLoopCommonModes, NSSize,
    NSString, NSTimer, NSUInteger,
};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
use vui_core::{
    Color, CursorIcon, Event, ImeEvent, Key, KeyEvent, Mode, Modifiers, PointerButton, Rect, Resources, Size, pt, size,
};
use vui_render::{CpuRenderer, CustomPass, Gpu, Renderer};
use vui_runtime::{App, Clipboard, Effect, Ui};

use crate::convert::{frame_step, scroll_delta, scroll_phase, utf16_len, utf16_range_to_bytes};
use crate::host::{Host, Output, Snapshot, Window};
use crate::software::LayerPresenter;
use crate::keys::{modifiers, named_key, shortcut_char, typed_text};
use crate::{Error, OverlayOptions, Place, Proxy, Screen, WindowOptions};

// ---- the application -----------------------------------------------------------------------------

thread_local! {
    /// The views of the windows opened so far, numbered as their `Proxy`s know them.
    static VIEWS: RefCell<Vec<Weak<VuiView>>> = const { RefCell::new(Vec::new()) };
}

/// From any thread: has the main thread hand window `id` the messages queued for it.
pub(crate) fn wake(id: usize) {
    DispatchQueue::main().exec_async(move || {
        if let Some(view) = VIEWS.with(|views| views.borrow().get(id).and_then(Weak::load)) {
            view.deliver();
        }
    });
}

type MakePass = Box<dyn Fn(&wgpu::Device, wgpu::TextureFormat) -> Box<dyn CustomPass>>;

/// The running app: its windows and overlays, which share one GPU device and one set of fonts and
/// pictures. Build it, open what the app shows, then `run`.
pub struct Application {
    mtm: MainThreadMarker,
    res: Resources,
    /// Until the first window picks a device to suit its surface.
    instance: Option<wgpu::Instance>,
    gpu: Option<Gpu>,
    /// Frames are drawn on the CPU: asked for with `VUI_RENDERER=cpu`, or because the GPU failed.
    cpu: bool,
    passes: Vec<(&'static str, MakePass)>,
    /// Kept alive for as long as the app runs.
    windows: Vec<(Retained<NSWindow>, Retained<VuiView>, Retained<WindowDelegate>)>,
    snapshot: Option<PathBuf>,
    snapshot_quit: bool,
    snapshots: Rc<Cell<usize>>,
    /// The pointer watchers of overlays that let clicks through their empty space.
    monitors: Vec<Retained<AnyObject>>,
}

impl Application {
    /// Sets the process up as an app called `name`, with the standard menus.
    pub fn new(name: &str) -> Result<Application, Error> {
        let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.setMainMenu(Some(&main_menu(mtm, name)));
        Ok(Application {
            mtm,
            res: Resources::new(),
            instance: Some(wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle())),
            gpu: None,
            cpu: std::env::var("VUI_RENDERER").is_ok_and(|v| v.eq_ignore_ascii_case("cpu")),
            passes: Vec::new(),
            windows: Vec::new(),
            snapshot: std::env::var_os("VUI_SNAPSHOT").map(PathBuf::from),
            snapshot_quit: std::env::var_os("VUI_SNAPSHOT_QUIT").is_some(),
            snapshots: Rc::new(Cell::new(0)),
            monitors: Vec::new(),
        })
    }

    /// The fonts and pictures every window draws with, for registering images.
    pub fn resources(&self) -> &Resources {
        &self.res
    }

    /// Gives windows opened from now on a custom shader pass, which `PaintCx::custom` commands named
    /// `name` draw with.
    pub fn pass(&mut self, name: &'static str, make: impl Fn(&wgpu::Device, wgpu::TextureFormat) -> Box<dyn CustomPass> + 'static) {
        self.passes.push((name, Box::new(make)));
    }

    /// The screen overlays go on, measured now: displays come and go.
    pub fn screen(&self) -> Screen {
        describe(&overlay_screen(self.mtm))
    }

    /// Every display's frame in points, in the coordinates screenshots use: the origin at the top left
    /// of the main display, y growing down.
    pub fn displays(&self) -> Vec<Rect> {
        let screens = NSScreen::screens(self.mtm);
        let main = screens.firstObject().map_or(0.0, |s| s.frame().size.height);
        screens
            .iter()
            .map(|s| {
                let f = s.frame();
                vui_core::rect(f.origin.x as f32, (main - f.origin.y - f.size.height) as f32, f.size.width as f32, f.size.height as f32)
            })
            .collect()
    }

    /// Opens a window running `app`, centred on the screen and in front.
    pub fn window<A: App>(&mut self, options: WindowOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(options.size.w as f64, options.size.h as f64));
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(NSWindow::alloc(mtm), content, style, NSBackingStoreType::Buffered, false)
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&options.title));
        window.setTitlebarAppearsTransparent(true);
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        window.setContentMinSize(NSSize::new(options.min_size.w as f64, options.min_size.h as f64));
        window.setAcceptsMouseMovedEvents(true);
        window.center();

        let proxy = self.attach(window, content, options.size, options.quit_on_close, false, app)?;
        let (window, view, _) = self.windows.last().expect("just attached");
        window.makeKeyAndOrderFront(None);
        window.makeFirstResponder(Some(&**view));
        view.start_display_link();
        view.request_frame();
        Ok(proxy)
    }

    /// Opens an overlay running `app`: see [`OverlayOptions`].
    pub fn overlay<A: App>(&mut self, options: OverlayOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let ns_screen = match options.place {
            Place::Display(n) => NSScreen::screens(mtm).iter().nth(n).unwrap_or_else(|| overlay_screen(mtm)),
            _ => overlay_screen(mtm),
        };
        let screen = ns_screen.frame();
        let (w, h) = (options.size.w as f64, options.size.h as f64);
        let origin = match options.place {
            Place::Top => NSPoint::new(screen.origin.x + ((screen.size.width - w) / 2.0).round(), screen.origin.y + screen.size.height - h),
            Place::Right => {
                let visible = ns_screen.visibleFrame();
                NSPoint::new(screen.origin.x + screen.size.width - w, visible.origin.y + visible.size.height - h)
            }
            Place::Display(_) => NSPoint::new(screen.origin.x, screen.origin.y + screen.size.height - h),
        };
        let frame = NSRect::new(origin, NSSize::new(w, h));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel = if options.keyboard {
            // SAFETY: NSPanel's designated initialiser, sent to a fresh instance of its subclass.
            let panel: Retained<KeyPanel> = unsafe {
                msg_send![KeyPanel::alloc(mtm), initWithContentRect: frame, styleMask: style, backing: NSBackingStoreType::Buffered, defer: false]
            };
            panel.into_super()
        } else {
            NSPanel::initWithContentRect_styleMask_backing_defer(NSPanel::alloc(mtm), frame, style, NSBackingStoreType::Buffered, false)
        };
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setHidesOnDeactivate(false);
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setMovable(false);
        panel.setIgnoresMouseEvents(options.click_through);
        // Above the menu bar, so it can cover the notch.
        panel.setLevel(NSMainMenuWindowLevel + 3);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        panel.setBackgroundColor(Some(&NSColor::clearColor()));

        let content = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
        let proxy = self.attach(panel.into_super(), content, options.size, false, true, app)?;
        let (window, view, _) = self.windows.last().expect("just attached");
        if options.through_empty {
            self.monitors.extend(follow_pointer(window, view));
        }
        let (window, view, _) = self.windows.last().expect("just attached");
        window.orderFrontRegardless();
        view.start_display_link();
        view.request_frame();
        Ok(proxy)
    }

    /// Puts a view running `app` in `window`, with a surface and a renderer.
    fn attach<A: App>(
        &mut self,
        window: Retained<NSWindow>,
        content: NSRect,
        size: Size,
        quit_on_close: bool,
        transparent: bool,
        app: A,
    ) -> Result<Proxy<A::Msg>, Error> {
        let mtm = self.mtm;
        let view = VuiView::new(mtm, content);
        window.setContentView(Some(&view));
        let delegate = WindowDelegate::new(mtm, &view, quit_on_close);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let scale = window.backingScaleFactor() as f32;
        let mut ui = Ui::with_app(self.res.clone(), size, scale, app);
        ui.set_clipboard(Rc::new(Pasteboard));
        ui.set_transparent(transparent);
        ui.set_system_mode(view.system_mode());
        ui.set_preferences(frames::system_preferences());
        if !transparent {
            window.setBackgroundColor(Some(&ns_color(ui.theme().palette.side)));
        }

        let output = self.output(&view, transparent, size, scale);

        let n = self.windows.len();
        let path = self.snapshot.as_ref().map(|p| if n == 0 { p.clone() } else { numbered(p, n) });
        if path.is_some() {
            self.snapshots.set(self.snapshots.get() + 1);
        }
        let snapshot = Snapshot { path, pending: self.snapshots.clone(), quit: self.snapshot_quit };
        let (tx, inbox) = mpsc::channel();
        let host = Window::new(ui, output, inbox, snapshot);
        view.ivars().host.replace(Some(Box::new(host)));
        view.resized();

        let id = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.push(Weak::from_retained(&view));
            views.len() - 1
        });
        view.with_host(|h| h.set_waker(std::sync::Arc::new(move || wake(id))));
        self.windows.push((window, view, delegate));
        Ok(Proxy { tx, window: id })
    }

    /// Where a new view's frames go: the GPU when one can be had, else the CPU renderer, which is also
    /// what `VUI_RENDERER=cpu` asks for. Once the GPU has failed, later windows go straight to the CPU.
    fn output(&mut self, view: &VuiView, transparent: bool, size: Size, scale: f32) -> Output {
        if !self.cpu {
            match self.gpu_output(view, transparent, size, scale) {
                Ok(output) => return output,
                Err(e) => {
                    eprintln!("vui: {e}; drawing on the CPU instead");
                    self.cpu = true;
                }
            }
        }
        let presenter = LayerPresenter::new(view);
        Output::Cpu { renderer: CpuRenderer::new(self.res.clone()), show: Box::new(move |canvas, scale| presenter.show(canvas, scale)) }
    }

    /// The surface on the view, and, for the first window, a device that can present to it.
    fn gpu_output(&mut self, view: &VuiView, transparent: bool, size: Size, scale: f32) -> Result<Output, Error> {
        let instance = match (&self.gpu, &self.instance) {
            (Some(gpu), _) => &gpu.instance,
            (None, Some(instance)) => instance,
            (None, None) => return Err(Error::Gpu("the GPU could not be set up".into())),
        };
        let handle = AppKitWindowHandle::new(NonNull::from(view).cast());
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(handle),
        };
        // SAFETY: the view outlives the surface: the app keeps every view until it exits, and the
        // surface is dropped with the host, which the view owns.
        let surface = unsafe { instance.create_surface_unsafe(target) }.map_err(|e| Error::Surface(e.to_string()))?;
        if self.gpu.is_none() {
            let instance = self.instance.take().expect("no device yet");
            self.gpu = Some(Gpu::for_surface(instance, &surface).map_err(|e| Error::Gpu(e.to_string()))?);
        }
        let gpu = self.gpu.as_ref().expect("made above");
        let format = Window::<()>::pick_format(&surface, &gpu.adapter);
        let mut renderer = Renderer::new(gpu.device.clone(), gpu.queue.clone(), format, self.res.clone());
        for (name, make) in &self.passes {
            renderer.add_pass(name, make(&gpu.device, format));
        }
        Ok(Output::gpu(surface, &gpu.adapter, renderer, transparent, size, scale))
    }

    /// Brings the app forward and runs until it quits.
    pub fn run(self) {
        let app = NSApplication::sharedApplication(self.mtm);
        app.activate();
        app.run();
    }
}

/// `frame.png` → `frame-2.png`.
fn numbered(path: &std::path::Path, n: usize) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}-{n}.{}", ext.to_string_lossy()),
        None => format!("{stem}-{n}"),
    };
    path.with_file_name(name)
}

/// Watches the pointer, inside the app and out, and has `window` take clicks only while the pointer is
/// over something in it that takes them.
fn follow_pointer(window: &Retained<NSWindow>, view: &Retained<VuiView>) -> Vec<Retained<AnyObject>> {
    let check = {
        let (window, view) = (window.clone(), view.clone());
        move || {
            let at = NSEvent::mouseLocation();
            let frame = window.frame();
            let inside = at.x >= frame.origin.x
                && at.x < frame.origin.x + frame.size.width
                && at.y >= frame.origin.y
                && at.y < frame.origin.y + frame.size.height;
            let p = pt((at.x - frame.origin.x) as f32, (frame.origin.y + frame.size.height - at.y) as f32);
            let takes = inside && view.with_host(|h| h.takes_pointer(p)).unwrap_or(false);
            if window.ignoresMouseEvents() == takes {
                window.setIgnoresMouseEvents(!takes);
            }
        }
    };
    let mask = NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged | NSEventMask::ScrollWheel;
    let outside = {
        let check = check.clone();
        RcBlock::new(move |_: NonNull<NSEvent>| check())
    };
    let inside = RcBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        check();
        event.as_ptr()
    });
    let mut monitors = Vec::new();
    if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &outside) {
        monitors.push(m);
    }
    if let Some(m) = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &inside) } {
        monitors.push(m);
    }
    monitors
}

/// The built-in display if it has a notch, else the one with the menu bar.
fn overlay_screen(mtm: MainThreadMarker) -> Retained<NSScreen> {
    let screens = NSScreen::screens(mtm);
    let notched = screens.iter().find(|s| s.safeAreaInsets().top > 0.0);
    notched.or_else(|| screens.firstObject()).or_else(|| NSScreen::mainScreen(mtm)).expect("a Mac has a screen")
}

fn describe(screen: &NSScreen) -> Screen {
    let frame = screen.frame();
    let top = screen.safeAreaInsets().top;
    let notch = (top > 0.0).then(|| {
        let (left, right) = (screen.auxiliaryTopLeftArea(), screen.auxiliaryTopRightArea());
        size((frame.size.width - left.size.width - right.size.width) as f32, top as f32)
    });
    let visible = screen.visibleFrame();
    let menu_bar = (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
    Screen { size: size(frame.size.width as f32, frame.size.height as f32), notch, menu_bar: menu_bar.max(0.0) as f32 }
}

fn ns_color(c: Color) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c.r as f64, c.g as f64, c.b as f64, c.a as f64)
}
