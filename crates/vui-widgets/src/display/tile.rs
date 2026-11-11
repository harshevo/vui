//! Picture tiles: a picture in a rounded box that shows a blurred stand-in until the picture is ready
//! and then fades the picture in over it.

use std::rc::Rc;

use vui_core::{Color, Fit, ImageCmd, ImageData, ImageId, Quad, Size, Spring, SpringValue, Tone, fit_image, size};
use vui_runtime::layout::{Avail, Pair};
use vui_runtime::{AccessInfo, Base, Dirty, Env, PaintCx, Role, TickCx, Widget, builder};

/// What the blurred stand-in is when the hash cannot be decoded.
const BROKEN: Color = Color { r: 0.5, g: 0.5, b: 0.5, a: 0.15 };

/// The widget behind [`ImageTile`].
pub struct TileWidget {
    image: Option<ImageId>,
    /// A ThumbHash of the picture: a few dozen bytes that decode to a blurred version of it.
    thumb: Option<Rc<[u8]>>,
    fit: Fit,
    ring: bool,
}

pub struct TileState {
    /// 0 while only the stand-in shows, 1 once the picture has faded in over it.
    shown: SpringValue,
}

impl TileWidget {
    /// The stand-in, decoded once per hash and kept in the image store.
    fn placeholder(&self, env: &Env) -> Option<ImageId> {
        let hash = self.thumb.as_ref()?;
        Some(env.images().get_or_insert_with(&("thumb-hash", &hash[..]), || {
            ImageData::from_thumb_hash(hash).unwrap_or_else(|_| ImageData::solid(1, 1, BROKEN))
        }))
    }

    fn natural(&self, env: &Env) -> Size {
        let images = env.images();
        self.image.and_then(|id| images.size(id)).or_else(|| self.placeholder(env).and_then(|id| images.size(id))).unwrap_or(Size::ZERO)
    }
}

impl<M: 'static> Widget<M> for TileWidget {
    type State = TileState;

    fn create_state(&self) -> TileState {
        // A picture that is there from the start is not faded in.
        TileState { shown: SpringValue::new(if self.image.is_some() { 1.0 } else { 0.0 }, Spring::SMOOTH) }
    }

    fn update(&self, old: &TileWidget, state: &mut TileState) -> Dirty {
        match self.image {
            Some(_) => state.shown.set_target(1.0),
            None => state.shown.snap_to(0.0),
        }
        if self.image != old.image || self.fit != old.fit {
            Dirty::Layout
        } else if self.thumb != old.thumb || self.ring != old.ring {
            Dirty::Paint
        } else {
            Dirty::None
        }
    }

    fn measure(&self, _state: &mut TileState, env: &Env, known: Pair<Option<f32>>, avail: Pair<Avail>) -> Size {
        let natural = self.natural(env);
        let ratio = if natural.h > 0.0 { natural.w / natural.h } else { 0.0 };
        match (known.x, known.y) {
            (Some(w), Some(h)) => size(w, h),
            (Some(w), None) => size(w, if ratio > 0.0 { w / ratio } else { 0.0 }),
            (None, Some(h)) => size(h * ratio, h),
            (None, None) => match avail.x {
                Avail::Definite(w) if natural.w > w && ratio > 0.0 => size(w, w / ratio),
                _ => natural,
            },
        }
    }

    fn paint(&self, state: &TileState, cx: &mut PaintCx) {
        let c = cx.content;
        let (radii, smooth) = (cx.radii, cx.smooth);
        let shown = state.shown.value().clamp(0.0, 1.0);
        let images = cx.env.images();

        if shown < 1.0 {
            match self.placeholder(cx.env) {
                Some(id) => {
                    let natural = images.size(id).unwrap_or(Size::ZERO);
                    let (dest, uv) = fit_image(natural, c, Fit::Cover);
                    cx.image(ImageCmd::new(id, dest).uv(uv).radii(radii).smooth(smooth));
                }
                None => cx.fill_rect(c, Tone::Wash),
            }
        }
        if let Some(id) = self.image
            && shown > 0.0
        {
            let natural = images.size(id).unwrap_or(Size::ZERO);
            let (dest, uv) = fit_image(natural, c, self.fit);
            let mut cmd = ImageCmd::new(id, dest).uv(uv).radii(radii).smooth(smooth);
            cmd.opacity = shown;
            cx.image(cmd);
        }
        if self.ring {
            let ring = cx.color(Tone::Ring);
            cx.quad(Quad::new(c).radii(radii).smooth(smooth).border(1.0, ring));
        }
    }

    fn tick(&self, state: &mut TileState, cx: &mut TickCx, dt: f32) -> bool {
        cx.step(&mut state.shown, dt)
    }

    fn animating(&self, state: &TileState) -> bool {
        !state.shown.is_settled()
    }

    fn access(&self, _state: &TileState, info: &mut AccessInfo) {
        info.role = Some(Role::Image);
    }
}

/// A picture that may still be on its way.
pub struct ImageTile<M: 'static> {
    base: Base<M>,
    widget: TileWidget,
}

builder!(ImageTile<M> => TileWidget);

impl<M: 'static> ImageTile<M> {
    /// A ThumbHash of the picture, shown blurred until the picture is there. Get one from
    /// `ImageData::thumb_hash` when the picture is first loaded and keep it with the picture's record.
    pub fn placeholder(mut self, thumb_hash: impl Into<Rc<[u8]>>) -> Self {
        self.widget.thumb = Some(thumb_hash.into());
        self
    }

    /// How the picture fills the tile: `Cover` unless said otherwise.
    pub fn fit(mut self, fit: Fit) -> Self {
        self.widget.fit = fit;
        self
    }

    /// A hairline just inside the edge, so pale pictures do not bleed into the page.
    pub fn ring(mut self) -> Self {
        self.widget.ring = true;
        self
    }
}

/// A picture tile, with the picture or `None` while it loads. Size it: `.size(160.0, 120.0)`, or give
/// it a width and `.aspect_ratio(..)`. Name it with `.name("Cover of ...")`.
pub fn image_tile<M: 'static>(image: Option<ImageId>) -> ImageTile<M> {
    ImageTile { base: Base::new(), widget: TileWidget { image, thumb: None, fit: Fit::Cover, ring: false } }
}
