//! Transforms: an element and everything in it scaled, turned or moved as it is drawn, without
//! changing where the layout put it.

use vui_core::{Affine, Lerp, Rect, pt};

/// How an element is scaled, turned and moved when drawn, about `origin`. Layout ignores it, as CSS
/// `transform` does: the element keeps its place and its neighbours do not move.
///
/// Set it with [`Builder::scale`](super::Builder::scale), [`rotate`](super::Builder::rotate),
/// [`translate`](super::Builder::translate) and [`transform_origin`](super::Builder::transform_origin);
/// with a [`transition`](super::Builder::transition) changes are sprung.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Transform {
    /// Horizontal and vertical scale; 1 is the element's own size.
    pub scale: [f32; 2],
    /// Degrees, clockwise.
    pub rotate: f32,
    /// Points right and down.
    pub translate: [f32; 2],
    /// The point that stays still while scaling and turning, as fractions of the box: (0.5, 0.5) is
    /// the centre.
    pub origin: [f32; 2],
}

impl Transform {
    pub const IDENTITY: Transform = Transform { scale: [1.0, 1.0], rotate: 0.0, translate: [0.0, 0.0], origin: [0.5, 0.5] };

    pub fn is_identity(&self) -> bool {
        self.scale == [1.0, 1.0] && self.rotate == 0.0 && self.translate == [0.0, 0.0]
    }

    /// The transform for an element drawn in `rect`.
    pub fn affine(&self, rect: Rect) -> Affine {
        let origin = pt(rect.x + rect.w * self.origin[0], rect.y + rect.h * self.origin[1]);
        Affine::scale(self.scale[0], self.scale[1])
            .then(Affine::rotate(self.rotate))
            .around(origin)
            .then(Affine::translate(self.translate[0], self.translate[1]))
    }

    /// The values a transition springs: scale x and y, rotation, and translation x and y.
    pub(crate) fn values(&self) -> [f32; 5] {
        [self.scale[0], self.scale[1], self.rotate, self.translate[0], self.translate[1]]
    }

    pub(crate) fn with_values(self, v: [f32; 5]) -> Transform {
        Transform { scale: [v[0], v[1]], rotate: v[2], translate: [v[3], v[4]], ..self }
    }
}

/// What keyframes move: opacity, scale, rotation and translation, laid over the element's own
/// opacity and transform. See [`Builder::animate`](super::Builder::animate).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pose {
    pub opacity: f32,
    pub scale: f32,
    /// Degrees, clockwise.
    pub rotate: f32,
    pub translate: [f32; 2],
}

impl Pose {
    /// Fully opaque, at its own size, unturned and in place.
    pub const REST: Pose = Pose { opacity: 1.0, scale: 1.0, rotate: 0.0, translate: [0.0, 0.0] };

    pub fn opacity(self, opacity: f32) -> Pose {
        Pose { opacity, ..self }
    }

    pub fn scale(self, scale: f32) -> Pose {
        Pose { scale, ..self }
    }

    pub fn rotate(self, degrees: f32) -> Pose {
        Pose { rotate: degrees, ..self }
    }

    pub fn translate(self, x: f32, y: f32) -> Pose {
        Pose { translate: [x, y], ..self }
    }
}

impl Default for Pose {
    fn default() -> Pose {
        Pose::REST
    }
}

impl Lerp for Pose {
    fn lerp(self, to: Pose, t: f32) -> Pose {
        Pose {
            opacity: self.opacity.lerp(to.opacity, t),
            scale: self.scale.lerp(to.scale, t),
            rotate: self.rotate.lerp(to.rotate, t),
            translate: [self.translate[0].lerp(to.translate[0], t), self.translate[1].lerp(to.translate[1], t)],
        }
    }
}

impl Transform {
    /// This transform with `pose` laid over it.
    pub(crate) fn with_pose(self, pose: Pose) -> Transform {
        Transform {
            scale: [self.scale[0] * pose.scale, self.scale[1] * pose.scale],
            rotate: self.rotate + pose.rotate,
            translate: [self.translate[0] + pose.translate[0], self.translate[1] + pose.translate[1]],
            origin: self.origin,
        }
    }
}

impl Default for Transform {
    fn default() -> Transform {
        Transform::IDENTITY
    }
}
