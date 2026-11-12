//! The application: its windows and overlays, which share one GPU device and one set of fonts and
//! pictures, opened before the event loop runs.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};

use vui_core::{Mode, Rect, Resources, Size, rect, size};
use vui_render::{CpuRenderer, CustomPass, Gpu, Renderer};
use vui_runtime::{App, Ui};
use vui_shell::frames::{cpu_requested, snapshot_path};
use vui_shell::host::{Host, Output, Snapshot, Window};
use vui_shell::{Error, OverlayOptions, Place, Proxy, Screen, WindowOptions};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::EventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Theme, WindowAttributes, WindowLevel};

use crate::clipboard::SystemClipboard;
use crate::present::Presenter;
use crate::runner::Slot;

type MakePass = Box<dyn Fn(&wgpu::Device, wgpu::TextureFormat) -> Box<dyn CustomPass>>;

/// The running app. Build it, open what the app shows, then `run`.
pub struct Application {
    pub(crate) event_loop: Option<EventLoop<usize>>,
    res: Resources,
    /// Until the first window picks a device to suit its surface.
    instance: Option<wgpu::Instance>,
    gpu: Option<Gpu>,
    /// Frames are drawn on the CPU: asked for with `VUI_RENDERER=cpu`, or because the GPU failed.
    cpu: bool,
    passes: Vec<(&'static str, MakePass)>,
    /// The windows, numbered as their `Proxy`s know them.
    pub(crate) slots: Vec<Slot>,
    snapshot: Option<PathBuf>,
    snapshot_quit: bool,
    snapshots: Rc<Cell<usize>>,
}

impl Application {
    /// Connects to the window system. `name` is the app's name; window titles come from their options.
    pub fn new(name: &str) -> Result<Application, Error> {
        let _ = name;
        let event_loop = EventLoop::<usize>::with_user_event().build().map_err(|e| Error::Platform(e.to_string()))?;
        let display = Box::new(event_loop.owned_display_handle());
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(display));
        Ok(Application {
            event_loop: Some(event_loop),
            res: Resources::new(),
            instance: Some(instance),
            gpu: None,
            cpu: cpu_requested(),
            passes: Vec::new(),
            slots: Vec::new(),
            snapshot: std::env::var_os("VUI_SNAPSHOT").map(PathBuf::from),
            snapshot_quit: std::env::var_os("VUI_SNAPSHOT_QUIT").is_some(),
            snapshots: Rc::new(Cell::new(0)),
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

    /// The screen overlays go on: the primary monitor. Off the Mac there is no notch, and the menu
    /// bar is the app's own.
    pub fn screen(&self) -> Screen {
        let monitor = self.monitors().into_iter().next();
        let size = monitor.map_or(size(1280.0, 800.0), |m| bounds(&m).size());
        Screen { size, notch: None, menu_bar: 0.0 }
    }

    /// Every display's frame in points, the primary one first.
    pub fn displays(&self) -> Vec<Rect> {
        self.monitors().iter().map(bounds).collect()
    }

    /// The monitors, the primary first, as a window sees them; before any window is open, a hidden
    /// one is made to ask.
    fn monitors(&self) -> Vec<MonitorHandle> {
        let probe;
        let window = match self.slots.first() {
            Some(slot) => &slot.window,
            None => {
                #[allow(deprecated)]
                let made = self.event_loop.as_ref().and_then(|el| el.create_window(WindowAttributes::default().with_visible(false)).ok());
                let Some(made) = made else { return Vec::new() };
                probe = Arc::new(made);
                &probe
            }
        };
        let primary = window.primary_monitor();
        primary.iter().cloned().chain(window.available_monitors().filter(|m| Some(m) != primary.as_ref())).collect()
    }

    /// Opens a window running `app`, in front.
    pub fn window<A: App>(&mut self, options: WindowOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let attributes = WindowAttributes::default()
            .with_title(options.title.clone())
            .with_inner_size(LogicalSize::new(options.size.w as f64, options.size.h as f64))
            .with_min_inner_size(LogicalSize::new(options.min_size.w as f64, options.min_size.h as f64));
        let window = self.create(attributes)?;
        self.attach(window, options.size, options.quit_on_close, false, app)
    }

    /// Opens an overlay running `app`: a borderless, see-through window above the others that does
    /// not take focus when it opens. Clicks pass through it if asked; `through_empty` is treated as
    /// taking clicks everywhere, since winit cannot pass them through part of a window.
    pub fn overlay<A: App>(&mut self, options: OverlayOptions, app: A) -> Result<Proxy<A::Msg>, Error> {
        let (w, h) = (options.size.w as f64, options.size.h as f64);
        let attributes = WindowAttributes::default()
            .with_title("")
            .with_inner_size(LogicalSize::new(w, h))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_active(false)
            .with_visible(false)
            .with_window_level(WindowLevel::AlwaysOnTop);
        let window = self.create(attributes)?;
        let monitors = window.available_monitors().collect::<Vec<_>>();
        let monitor = match options.place {
            Place::Display(n) => monitors.get(n).cloned(),
            _ => window.primary_monitor(),
        }
        .or_else(|| window.current_monitor());
        if let Some(m) = monitor {
            let (at, scale) = (m.position(), m.scale_factor());
            let (mw, ow) = (m.size().width as f64, w * scale);
            let x = match options.place {
                Place::Top => at.x as f64 + ((mw - ow) / 2.0).round(),
                Place::Right => at.x as f64 + mw - ow,
                Place::Display(_) => at.x as f64,
            };
            window.set_outer_position(PhysicalPosition::new(x, at.y as f64));
        }
        let _ = window.set_cursor_hittest(!options.click_through);
        window.set_visible(true);
        self.attach(window, options.size, false, true, app)
    }

    fn create(&self, attributes: WindowAttributes) -> Result<winit::window::Window, Error> {
        let event_loop = self.event_loop.as_ref().ok_or_else(|| Error::Platform("the app is already running".into()))?;
        // Windows are opened before the loop runs, as on the Mac; winit keeps this for desktops.
        #[allow(deprecated)]
        event_loop.create_window(attributes).map_err(|e| Error::Platform(e.to_string()))
    }

    /// Puts a UI running `app` in `window`, with a surface and a renderer.
    fn attach<A: App>(&mut self, window: winit::window::Window, size: Size, quit_on_close: bool, transparent: bool, app: A) -> Result<Proxy<A::Msg>, Error> {
        let window = Arc::new(window);
        let scale = window.scale_factor() as f32;
        let mut ui = Ui::with_app(self.res.clone(), size, scale, app);
        ui.set_clipboard(Rc::new(SystemClipboard::new()));
        ui.set_transparent(transparent);
        ui.set_system_mode(mode(window.theme()));

        let output = self.output(&window, transparent, size, scale)?;
        let n = self.slots.len();
        let path = self.snapshot.as_ref().map(|p| snapshot_path(p, n));
        if path.is_some() {
            self.snapshots.set(self.snapshots.get() + 1);
        }
        let snapshot = Snapshot { path, pending: self.snapshots.clone(), quit: self.snapshot_quit };
        let (tx, inbox) = mpsc::channel();
        let mut host = Window::new(ui, output, inbox, snapshot);
        let inner = window.inner_size();
        host.resize(vui_core::size(inner.width as f32 / scale, inner.height as f32 / scale), scale);

        let loop_proxy = Mutex::new(self.event_loop.as_ref().expect("checked in create").create_proxy());
        let wake: vui_shell::Wake = Arc::new(move || {
            if let Ok(p) = loop_proxy.lock() {
                let _ = p.send_event(n);
            }
        });
        host.set_waker(wake.clone());
        window.request_redraw();
        self.slots.push(Slot::new(window, Box::new(host), quit_on_close));
        Ok(Proxy::new(tx, wake))
    }

    /// Where a new window's frames go: the GPU when one can be had, else the CPU renderer, which is
    /// also what `VUI_RENDERER=cpu` asks for. Once the GPU has failed, later windows go straight to
    /// the CPU.
    fn output(&mut self, window: &Arc<winit::window::Window>, transparent: bool, size: Size, scale: f32) -> Result<Output, Error> {
        if !self.cpu {
            match self.gpu_output(window, transparent, size, scale) {
                Ok(output) => return Ok(output),
                Err(e) => {
                    eprintln!("vui: {e}; drawing on the CPU instead");
                    self.cpu = true;
                }
            }
        }
        let surface = self.surface(window)?;
        if self.gpu.is_none() {
            let gpu = Gpu::for_surface(self.instance.clone().expect("no device yet"), &surface)
                .or_else(|_| Gpu::software_for_surface(self.instance.clone().expect("no device yet"), &surface))
                .map_err(|e| Error::Gpu(format!("nothing can show frames drawn on the CPU: {e}")))?;
            self.gpu = Some(gpu);
        }
        let gpu = self.gpu.as_ref().expect("made above");
        let mut presenter = Presenter::new(gpu.device.clone(), gpu.queue.clone(), surface, &gpu.adapter, transparent);
        Ok(Output::Cpu { renderer: CpuRenderer::new(self.res.clone()), show: Box::new(move |canvas, _scale| presenter.show(canvas)) })
    }

    fn surface(&self, window: &Arc<winit::window::Window>) -> Result<wgpu::Surface<'static>, Error> {
        let instance = match (&self.gpu, &self.instance) {
            (Some(gpu), _) => &gpu.instance,
            (None, Some(instance)) => instance,
            (None, None) => return Err(Error::Gpu("the GPU could not be set up".into())),
        };
        instance.create_surface(window.clone()).map_err(|e| Error::Surface(e.to_string()))
    }

    /// The surface on the window, and, for the first window, a device that can present to it.
    fn gpu_output(&mut self, window: &Arc<winit::window::Window>, transparent: bool, size: Size, scale: f32) -> Result<Output, Error> {
        let surface = self.surface(window)?;
        if self.gpu.is_none() {
            let instance = self.instance.clone().ok_or_else(|| Error::Gpu("the GPU could not be set up".into()))?;
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

    /// Runs until the app quits.
    pub fn run(mut self) {
        let Some(event_loop) = self.event_loop.take() else { return };
        if let Err(e) = event_loop.run_app(&mut self) {
            eprintln!("vui: the event loop stopped: {e}");
        }
    }
}

/// A monitor's frame in points.
fn bounds(m: &MonitorHandle) -> Rect {
    let (at, px, scale) = (m.position(), m.size(), m.scale_factor().max(0.1));
    rect((at.x as f64 / scale) as f32, (at.y as f64 / scale) as f32, (px.width as f64 / scale) as f32, (px.height as f64 / scale) as f32)
}

pub(crate) fn mode(theme: Option<Theme>) -> Mode {
    match theme {
        Some(Theme::Dark) => Mode::Dark,
        _ => Mode::Light,
    }
}
