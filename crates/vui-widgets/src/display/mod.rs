//! Showing things: icons, cards, badges, avatars, keycaps, separators, chips, image tiles, rich text
//! and markdown, the glow and shimmer effects, and the shaded aurora, border beam and spotlight.

mod basics;
mod chip;
pub(crate) mod effects;
pub(crate) mod icon;
pub mod markdown;
mod shaded;
mod tile;

pub use basics::*;
pub use chip::chip;
pub use effects::{EffectState, Glow, GlowWidget, Shimmer, ShimmerWidget, glow, shimmer};
pub use icon::{ICON_SIZE, IconEl, IconWidget, SPINNER_PERIOD, icon, paint_icon, paint_spinner};
pub use markdown::{Markdown, RichText, markdown, rich_text};
pub use shaded::{
    Aurora, AuroraWidget, BeamWidget, BorderBeam, Spotlight, SpotlightState, SpotlightWidget, aurora, border_beam, spotlight,
};
pub use tile::{ImageTile, TileWidget, image_tile};
