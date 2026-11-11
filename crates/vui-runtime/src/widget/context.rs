//! What a widget is handed: [`EventCx`](super::EventCx) to answer input, [`TickCx`](super::TickCx) to
//! animate and [`PaintCx`](super::PaintCx) to draw.

use super::*;

/// What a widget asked for during a callback. The runtime acts on it once the callback returns.
#[derive(Default)]
pub(crate) struct Requests {
    /// `Some(true)` takes focus, `Some(false)` gives it up.
    pub focus: Option<bool>,
    pub capture: Option<bool>,
    pub layout: bool,
    pub paint: bool,
    pub frame: bool,
    pub wake: Option<f32>,
    pub reveal: Option<Rect>,
}

pub(crate) struct Output<M> {
    pub messages: Vec<M>,
    pub effects: Vec<Effect>,
    pub req: Requests,
}

impl<M> Default for Output<M> {
    fn default() -> Output<M> {
        Output { messages: Vec::new(), effects: Vec::new(), req: Requests::default() }
    }
}

/// What a widget can see and do while handling an event.
pub struct EventCx<'a, M> {
    pub env: &'a Env,
    pub id: NodeId,
    /// The node's border box and content box, in window coordinates.
    pub rect: Rect,
    pub content: Rect,
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub focus_visible: bool,
    pub window_focused: bool,
    pub mods: Modifiers,
    /// Seconds since the UI started, advanced by `Ui::tick`.
    pub time: f32,
    pub(crate) out: &'a mut Output<M>,
    pub(crate) clipboard: &'a dyn Clipboard,
}

impl<M> EventCx<'_, M> {
    /// Hands a message to the app's `update`.
    pub fn emit(&mut self, message: M) {
        self.out.messages.push(message);
    }

    /// Takes keyboard focus.
    pub fn focus(&mut self) {
        self.out.req.focus = Some(true);
    }

    pub fn blur(&mut self) {
        self.out.req.focus = Some(false);
    }

    /// Receives every pointer event until the button is released, wherever the pointer goes: how
    /// sliders and text selection follow a drag out of their bounds.
    pub fn capture_pointer(&mut self) {
        self.out.req.capture = Some(true);
    }

    pub fn release_pointer(&mut self) {
        self.out.req.capture = Some(false);
    }

    /// Sizes changed: lay the tree out again.
    pub fn request_layout(&mut self) {
        self.out.req.layout = true;
    }

    /// Only what is drawn changed.
    pub fn request_paint(&mut self) {
        self.out.req.paint = true;
    }

    /// Keep calling `tick` every frame: an animation is running.
    pub fn request_frame(&mut self) {
        self.out.req.frame = true;
    }

    /// Call `tick` again in `seconds`, without running frames in between: a blinking caret.
    pub fn request_wake(&mut self, seconds: f32) {
        let wake = self.out.req.wake.map_or(seconds, |w| w.min(seconds));
        self.out.req.wake = Some(wake);
    }

    /// Where the text caret is, in window coordinates, so the input method can place its window.
    /// `None` when nothing is being edited.
    pub fn set_ime_area(&mut self, area: Option<Rect>) {
        self.out.effects.push(Effect::ImeArea(area));
    }

    /// Scrolls enclosing scroll containers until `rect` (window coordinates) is in view.
    pub fn scroll_into_view(&mut self, rect: Rect) {
        self.out.req.reveal = Some(rect);
    }

    pub fn clipboard_text(&self) -> Option<String> {
        self.clipboard.get()
    }

    pub fn set_clipboard_text(&self, text: &str) {
        self.clipboard.set(text);
    }

    /// A picture on the clipboard, as PNG bytes.
    pub fn clipboard_image(&self) -> Option<Vec<u8>> {
        self.clipboard.image()
    }

    /// A window position relative to the node's border box.
    pub fn to_local(&self, p: Point) -> Point {
        Point { x: p.x - self.rect.x, y: p.y - self.rect.y }
    }
}

/// What a widget can do while time passes.
pub struct TickCx<'a> {
    pub env: &'a Env,
    pub time: f32,
    pub window_focused: bool,
    pub focused: bool,
    pub(crate) layout: bool,
    pub(crate) paint: bool,
    pub(crate) wake: Option<f32>,
}

impl TickCx<'_> {
    pub fn request_layout(&mut self) {
        self.layout = true;
    }

    pub fn request_paint(&mut self) {
        self.paint = true;
    }

    /// Call `tick` again in `seconds`.
    pub fn request_wake(&mut self, seconds: f32) {
        self.wake = Some(self.wake.map_or(seconds, |w| w.min(seconds)));
    }
}

/// What a widget paints with: the scene to add to, where the node is, and how it is being interacted
/// with. Colours given as a [`Fill`] follow the theme and the element's opacity.
pub struct PaintCx<'a> {
    pub scene: &'a mut Scene,
    pub env: &'a Env,
    /// The node's border box and content box, in window coordinates, moved onto the device pixel grid
    /// so that quads and images have crisp edges.
    pub rect: Rect,
    pub content: Rect,
    /// The content box as layout left it, before moving onto the pixel grid. Measure and wrap text to
    /// this one: moving the edges can make a box that was sized to its text a fraction of a pixel too
    /// narrow to hold it.
    pub exact: Rect,
    /// Everything outside this rect is cut away.
    pub clip: Option<Rect>,
    /// The element's own corner radii, resolved against the theme, and how continuous they are.
    pub radii: Corners,
    pub smooth: f32,
    /// Opacity of the element and everything above it.
    pub alpha: f32,
    /// 0 to 1, animated: the pointer is over it, it is pressed, it shows a focus ring.
    pub hover: f32,
    pub press: f32,
    pub focus: f32,
    pub focused: bool,
    pub window_focused: bool,
    pub disabled: bool,
    /// Seconds since the UI started.
    pub time: f32,
}

impl PaintCx<'_> {
    pub fn theme(&self) -> &Theme {
        &self.env.theme
    }

    pub fn palette(&self) -> &Palette {
        &self.env.theme.palette
    }

    pub fn fonts(&self) -> &Fonts {
        &self.env.res.fonts
    }

    /// A fill resolved against the theme and faded by the element's opacity.
    pub fn color(&self, fill: impl Into<Fill>) -> Color {
        fill.into().resolve(self.palette()).mul_alpha(self.alpha)
    }

    /// Rounds a rect's edges to whole device pixels, so lines stay crisp.
    pub fn snap(&self, rect: Rect) -> Rect {
        rect.snap(self.env.scale)
    }

    /// Runs `f` with everything outside `rect` cut away as well: text that scrolls inside a field.
    pub fn clipped(&mut self, rect: Rect, f: impl FnOnce(&mut PaintCx)) {
        let saved = self.clip;
        self.clip = Some(saved.map_or(rect, |c| c.intersection(rect).unwrap_or(Rect::ZERO)));
        f(self);
        self.clip = saved;
    }

    /// Adds a quad, cut to the clip.
    pub fn quad(&mut self, quad: Quad) {
        self.scene.quad(quad.clip(self.clip));
    }

    /// Fills `rect` with the element's corner radii.
    pub fn fill_rect(&mut self, rect: Rect, fill: impl Into<Fill>) {
        self.fill_rounded(rect, self.radii, fill);
    }

    pub fn fill_rounded(&mut self, rect: Rect, radii: Corners, fill: impl Into<Fill>) {
        let color = self.color(fill);
        if color.a > 0.0 {
            self.quad(Quad::new(rect).fill(color).radii(radii).smooth(self.smooth));
        }
    }

    /// Adds an image, cut to the clip and faded by the element's opacity.
    pub fn image(&mut self, mut cmd: ImageCmd) {
        cmd.clip = self.clip;
        cmd.opacity *= self.alpha;
        self.scene.image(cmd);
    }

    /// Adds drawing for a custom shader pass, cut to the clip and faded by the element's opacity.
    pub fn custom(&mut self, mut cmd: CustomCmd) {
        cmd.clip = self.clip;
        cmd.opacity *= self.alpha;
        self.scene.push(Command::Custom(cmd));
    }

    /// Adds shaped text with its top-left corner at `origin`.
    pub fn text(&mut self, layout: Rc<TextLayout>, origin: Point, fill: impl Into<Fill>) {
        let color = self.color(fill);
        if color.a > 0.0 {
            self.scene.text(layout, origin, color, self.clip);
        }
    }
}
