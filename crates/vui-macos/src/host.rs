//! What a window runs: the UI, the surface it presents to and the renderer, behind one object-safe
//! trait so the AppKit view does not need to know the app's message type.

use std::path::PathBuf;

use vui_core::{Color, Event, Mode, Point, Size, size};
use vui_render::Renderer;
use vui_runtime::{Effect, Ui};

/// What a frame did, so the view knows when to ask for the next one.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FrameResult {
    /// Something is moving: keep the display link running.
    pub animating: bool,
    /// The window is hidden or covered, so nothing could be shown. Wait for it to come into view
    /// instead of drawing frames nobody sees.
    pub hidden: bool,
    /// Nothing is moving, but a widget wants a tick in this many seconds (a blinking caret).
    pub wake: Option<f32>,
}

pub trait Host {
    /// Feeds one event in and returns what the window system should do about it.
    fn handle(&mut self, event: Event) -> Vec<Effect>;
    fn drags_window(&mut self, p: Point) -> bool;
    /// The view's size in points and its backing scale.
    fn resize(&mut self, size: Size, scale: f32);
    fn set_system_mode(&mut self, mode: Mode) -> Vec<Effect>;
    /// Advances time by `dt` and draws if anything changed.
    fn frame(&mut self, dt: f32) -> (FrameResult, Vec<Effect>);
    /// Draws now, without moving time: after a resize.
    fn redraw(&mut self);
    fn needs_paint(&self) -> bool;
    fn quit_requested(&self) -> bool;
    /// The colour around the content: what the window shows behind the view.
    fn canvas(&self) -> Color;
}

/// A UI on a wgpu surface.
pub struct Window<M: 'static> {
    pub ui: Ui<M>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    /// Write the first settled frame to this PNG: `VUI_SNAPSHOT`, for checking a window from scripts.
    snapshot: Option<PathBuf>,
    /// Quit once the snapshot is written: `VUI_SNAPSHOT_QUIT`.
    snapshot_quit: bool,
    quit: bool,
    /// The last frame could not be shown (the window was hidden or the surface was being rebuilt), so
    /// what is on screen is out of date.
    stale: bool,
}

impl<M: 'static> Window<M> {
    pub fn new(ui: Ui<M>, surface: wgpu::Surface<'static>, adapter: &wgpu::Adapter, renderer: Renderer) -> Window<M> {
        let caps = surface.get_capabilities(adapter);
        let (w, h) = pixel_size(ui.size(), ui.scale());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: renderer.format(),
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else {
                caps.alpha_modes[0]
            },
            view_formats: vec![],
        };
        surface.configure(renderer.device(), &config);
        Window {
            ui,
            surface,
            config,
            renderer,
            snapshot: std::env::var_os("VUI_SNAPSHOT").map(PathBuf::from),
            snapshot_quit: std::env::var_os("VUI_SNAPSHOT_QUIT").is_some(),
            quit: false,
            stale: true,
        }
    }

    /// The format to render in for a surface: the shaders blend in encoded sRGB, so a non-sRGB format.
    pub fn pick_format(surface: &wgpu::Surface<'_>, adapter: &wgpu::Adapter) -> wgpu::TextureFormat {
        let formats = surface.get_capabilities(adapter).formats;
        [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|f| formats.contains(f))
            .unwrap_or_else(|| formats.first().map_or(wgpu::TextureFormat::Bgra8Unorm, |f| f.remove_srgb_suffix()))
    }

    /// Draws the UI to the window. Returns false if the window could not take a frame: it is covered,
    /// minimised or its surface is being rebuilt.
    fn present(&mut self) -> bool {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(self.renderer.device(), &self.config);
                self.stale = true;
                return false;
            }
            _ => {
                self.stale = true;
                return false;
            }
        };
        self.stale = false;
        let scene = self.ui.paint();
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.render(&scene, &view, (self.config.width, self.config.height));
        self.renderer.queue().present(texture);
        true
    }

    /// Writes the snapshot, offscreen, so it works whether or not the window is in view.
    fn take_snapshot(&mut self) {
        let Some(path) = self.snapshot.take() else { return };
        let scene = self.ui.paint();
        match self.renderer.render_to_image(&scene).map(|image| image.save_png(&path)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("vui: could not write {}: {e}", path.display()),
            Err(e) => eprintln!("vui: could not render the snapshot: {e}"),
        }
        // The window still needs this frame.
        self.stale = true;
        self.quit |= self.snapshot_quit;
    }
}

fn pixel_size(points: Size, scale: f32) -> (u32, u32) {
    (((points.w * scale).round() as u32).max(1), ((points.h * scale).round() as u32).max(1))
}

impl<M: 'static> Host for Window<M> {
    fn handle(&mut self, event: Event) -> Vec<Effect> {
        self.ui.handle(event);
        self.ui.take_effects()
    }

    fn drags_window(&mut self, p: Point) -> bool {
        self.ui.drags_window(p)
    }

    fn resize(&mut self, points: Size, scale: f32) {
        let points = size(points.w.max(1.0), points.h.max(1.0));
        self.ui.set_size(points);
        self.ui.set_scale(scale);
        let (w, h) = pixel_size(points, scale);
        if (w, h) != (self.config.width, self.config.height) {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(self.renderer.device(), &self.config);
        }
    }

    fn set_system_mode(&mut self, mode: Mode) -> Vec<Effect> {
        self.ui.set_system_mode(mode);
        self.ui.take_effects()
    }

    fn frame(&mut self, dt: f32) -> (FrameResult, Vec<Effect>) {
        self.ui.tick(dt);
        if self.snapshot.is_some() && !self.ui.needs_frame() {
            self.take_snapshot();
        }
        // A pending snapshot keeps time moving even out of sight, so the UI can settle and be written.
        let hidden = self.needs_paint() && !self.present() && self.snapshot.is_none();
        let result = FrameResult { animating: self.ui.needs_frame() || self.snapshot.is_some(), hidden, wake: self.ui.next_wake() };
        (result, self.ui.take_effects())
    }

    fn redraw(&mut self) {
        self.present();
    }

    fn needs_paint(&self) -> bool {
        self.stale || self.ui.needs_paint()
    }

    fn quit_requested(&self) -> bool {
        self.quit || self.ui.quit_requested()
    }

    fn canvas(&self) -> Color {
        self.ui.theme().palette.side
    }
}
