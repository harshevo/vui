//! Images: decoded pixels, the registry the renderer reads from, resizing, decoding and ThumbHash
//! placeholders.
//!
//! Pixels are 8-bit RGBA with straight alpha, sRGB-encoded. Decoding happens on the caller's thread
//! (use a worker thread for anything large) and the result is registered with [`Images`], which hands
//! back an [`ImageId`] that scenes refer to. The renderer uploads textures lazily and evicts them
//! under a memory budget; the CPU copy stays here until you remove it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use fast_image_resize::images::Image as ResizeImage;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};

use crate::color::{Color, linear_to_srgb, srgb_to_linear};
use crate::geom::{Rect, Size, rect, size};

/// Lookup tables for the sRGB transfer function, so converting a large picture needs no `powf`.
struct Transfer {
    decode: [f32; 256],
    encode: Vec<u8>,
}

fn transfer_tables() -> &'static Transfer {
    static TABLES: OnceLock<Transfer> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut decode = [0.0; 256];
        for (i, d) in decode.iter_mut().enumerate() {
            *d = srgb_to_linear(i as f32 / 255.0);
        }
        let encode = (0..=65535u32).map(|i| (linear_to_srgb(i as f32 / 65535.0) * 255.0 + 0.5) as u8).collect();
        Transfer { decode, encode }
    })
}

#[derive(Debug)]
pub enum ImageError {
    /// The bytes are not a supported image, or are damaged.
    Decode(String),
    /// Zero-sized, or the pixel buffer does not match the size.
    Size,
    ThumbHash,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::Decode(e) => write!(f, "could not decode the image: {e}"),
            ImageError::Size => write!(f, "the image has no pixels or the wrong buffer size"),
            ImageError::ThumbHash => write!(f, "invalid ThumbHash"),
        }
    }
}

impl std::error::Error for ImageError {}

/// Decoded pixels: row-major RGBA8, straight alpha, sRGB-encoded.
#[derive(Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl fmt::Debug for ImageData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ImageData({}x{})", self.width, self.height)
    }
}

impl ImageData {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<ImageData, ImageError> {
        if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
            return Err(ImageError::Size);
        }
        Ok(ImageData { width, height, rgba: rgba.into() })
    }

    pub fn solid(width: u32, height: u32, color: Color) -> ImageData {
        let px = color.to_rgba8();
        let rgba: Vec<u8> = (0..width as usize * height as usize).flat_map(|_| px).collect();
        ImageData::new(width, height, rgba).expect("solid image has a valid size")
    }

    pub fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> ImageData {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        ImageData::new(width, height, rgba).expect("generated image has a valid size")
    }

    pub fn size(&self) -> Size {
        size(self.width as f32, self.height as f32)
    }

    pub fn byte_len(&self) -> usize {
        self.rgba.len()
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    pub fn has_alpha(&self) -> bool {
        self.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
    }

    /// Decodes PNG, JPEG, GIF (first frame), WebP, BMP or TIFF, applies the EXIF orientation, and, if
    /// `max_dim` is given, shrinks the result so neither side exceeds it.
    pub fn decode(bytes: &[u8], max_dim: Option<u32>) -> Result<ImageData, ImageError> {
        let decode_err = |e: image::ImageError| ImageError::Decode(e.to_string());
        let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| ImageError::Decode(e.to_string()))?;
        let mut decoder = reader.into_decoder().map_err(decode_err)?;
        let orientation = image::ImageDecoder::orientation(&mut decoder).unwrap_or(image::metadata::Orientation::NoTransforms);
        let mut img = image::DynamicImage::from_decoder(decoder).map_err(decode_err)?;
        img.apply_orientation(orientation);
        let rgba = img.into_rgba8();
        let data = ImageData::new(rgba.width(), rgba.height(), rgba.into_raw())?;
        Ok(match max_dim {
            Some(m) => data.fit_within(m),
            None => data,
        })
    }

    pub fn load(path: &Path, max_dim: Option<u32>) -> Result<ImageData, ImageError> {
        let bytes = std::fs::read(path).map_err(|e| ImageError::Decode(e.to_string()))?;
        ImageData::decode(&bytes, max_dim)
    }

    /// A Lanczos-resampled copy at exactly `width` by `height`.
    pub fn resized(&self, width: u32, height: u32) -> ImageData {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == (self.width, self.height) {
            return self.clone();
        }
        let src = ResizeImage::from_vec_u8(self.width, self.height, self.rgba.to_vec(), PixelType::U8x4).expect("buffer matches size");
        let mut dst = ResizeImage::new(width, height, PixelType::U8x4);
        let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
        Resizer::new().resize(&src, &mut dst, &options).expect("same pixel type");
        ImageData::new(width, height, dst.into_vec()).expect("resized buffer matches size")
    }

    /// Shrinks to fit inside `max_dim` on both sides, keeping the aspect ratio. Never enlarges.
    pub fn fit_within(&self, max_dim: u32) -> ImageData {
        let longest = self.width.max(self.height);
        if longest <= max_dim {
            return self.clone();
        }
        let k = max_dim as f32 / longest as f32;
        self.resized(((self.width as f32 * k).round() as u32).max(1), ((self.height as f32 * k).round() as u32).max(1))
    }

    /// The same image with colour multiplied by alpha, which is what the GPU filters and blends.
    pub fn premultiplied(&self) -> Vec<u8> {
        let mut out = self.rgba.to_vec();
        for p in out.as_chunks_mut::<4>().0.iter_mut() {
            let a = p[3] as u32;
            if a != 255 {
                for c in &mut p[..3] {
                    *c = ((*c as u32 * a + 127) / 255) as u8;
                }
            }
        }
        out
    }

    /// Colour premultiplied by alpha in linear light, then re-encoded as sRGB. Uploaded as an sRGB
    /// texture, the GPU filters premultiplied linear values, so transparent edges do not fringe and
    /// minified pictures keep their brightness; the shader converts back for blending.
    pub fn premultiplied_linear(&self) -> Vec<u8> {
        let tables = transfer_tables();
        let mut out = self.rgba.to_vec();
        for p in out.as_chunks_mut::<4>().0.iter_mut() {
            let a = p[3];
            if a == 255 {
                continue;
            }
            let alpha = a as f32 / 255.0;
            for c in &mut p[..3] {
                let light = tables.decode[*c as usize] * alpha;
                *c = tables.encode[(light * 65535.0 + 0.5) as usize];
            }
        }
        out
    }

    pub fn average_color(&self) -> Color {
        let n = (self.width * self.height) as f32;
        let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for p in self.rgba.as_chunks::<4>().0 {
            let al =p[3] as f32 / 255.0;
            r += p[0] as f32 * al;
            g += p[1] as f32 * al;
            b += p[2] as f32 * al;
            a += al;
        }
        if a <= 0.0 {
            return Color::TRANSPARENT;
        }
        Color::rgba(r / a / 255.0, g / a / 255.0, b / a / 255.0, a / n)
    }

    /// A ThumbHash: a few dozen bytes that decode to a blurred stand-in for the image.
    pub fn thumb_hash(&self) -> Vec<u8> {
        let small = self.fit_within(100);
        thumbhash::rgba_to_thumb_hash(small.width as usize, small.height as usize, &small.rgba)
    }

    /// The blurred placeholder a ThumbHash describes: at most about 32 pixels on the long side. Scale
    /// it up when drawing.
    pub fn from_thumb_hash(hash: &[u8]) -> Result<ImageData, ImageError> {
        let (w, h, rgba) = thumbhash::thumb_hash_to_rgba(hash).map_err(|_| ImageError::ThumbHash)?;
        ImageData::new(w as u32, h as u32, rgba)
    }
}

/// How an image fills a box.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// Stretch to the box.
    Fill,
    /// The whole image, centred, with space left over.
    #[default]
    Contain,
    /// The box is filled; the overflowing edges are cropped.
    Cover,
}

/// Where to draw an image of `image` size inside `bounds`, and which part of it to show.
/// Returns `(destination rect, source rect in normalised 0..1 coordinates)`.
pub fn fit_image(image: Size, bounds: Rect, fit: Fit) -> (Rect, Rect) {
    let full = rect(0.0, 0.0, 1.0, 1.0);
    if image.is_empty() || bounds.is_empty() {
        return (bounds, full);
    }
    let (ia, ba) = (image.w / image.h, bounds.w / bounds.h);
    match fit {
        Fit::Fill => (bounds, full),
        Fit::Contain => {
            let (w, h) = if ia > ba { (bounds.w, bounds.w / ia) } else { (bounds.h * ia, bounds.h) };
            (rect(bounds.x + (bounds.w - w) / 2.0, bounds.y + (bounds.h - h) / 2.0, w, h), full)
        }
        Fit::Cover => {
            if ia > ba {
                // image is wider than the box: crop left and right
                let visible = ba / ia;
                (bounds, rect((1.0 - visible) / 2.0, 0.0, visible, 1.0))
            } else {
                let visible = ia / ba;
                (bounds, rect(0.0, (1.0 - visible) / 2.0, 1.0, visible))
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageId(u64);

struct Store {
    next: u64,
    images: HashMap<ImageId, ImageData>,
}

/// The registry of decoded images. Cheap to clone; clones share the same images.
#[derive(Clone)]
pub struct Images {
    store: Rc<RefCell<Store>>,
}

impl Images {
    pub fn new() -> Images {
        Images { store: Rc::new(RefCell::new(Store { next: 1, images: HashMap::new() })) }
    }

    pub fn insert(&self, data: ImageData) -> ImageId {
        let mut s = self.store.borrow_mut();
        let id = ImageId(s.next);
        s.next += 1;
        s.images.insert(id, data);
        id
    }

    pub fn get(&self, id: ImageId) -> Option<ImageData> {
        self.store.borrow().images.get(&id).cloned()
    }

    pub fn size(&self, id: ImageId) -> Option<Size> {
        self.store.borrow().images.get(&id).map(ImageData::size)
    }

    pub fn remove(&self, id: ImageId) -> Option<ImageData> {
        self.store.borrow_mut().images.remove(&id)
    }

    pub fn len(&self) -> usize {
        self.store.borrow().images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn total_bytes(&self) -> usize {
        self.store.borrow().images.values().map(ImageData::byte_len).sum()
    }
}

impl Default for Images {
    fn default() -> Images {
        Images::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(w: u32, h: u32) -> ImageData {
        ImageData::from_fn(w, h, |x, y| [(x * 255 / (w - 1)) as u8, (y * 255 / (h - 1)) as u8, 128, 255])
    }

    #[test]
    fn new_validates_the_buffer() {
        assert!(ImageData::new(2, 2, vec![0; 16]).is_ok());
        assert!(matches!(ImageData::new(2, 2, vec![0; 15]), Err(ImageError::Size)));
        assert!(matches!(ImageData::new(0, 2, vec![]), Err(ImageError::Size)));
    }

    #[test]
    fn resizing_keeps_the_average_colour() {
        let big = gradient(200, 100);
        let small = big.resized(40, 20);
        assert_eq!((small.width, small.height), (40, 20));
        let (a, b) = (big.average_color(), small.average_color());
        assert!((a.r - b.r).abs() < 0.02 && (a.g - b.g).abs() < 0.02, "{a:?} vs {b:?}");
    }

    #[test]
    fn fit_within_keeps_the_aspect_and_never_enlarges() {
        let img = gradient(400, 200);
        let s = img.fit_within(100);
        assert_eq!((s.width, s.height), (100, 50));
        let same = img.fit_within(1000);
        assert_eq!((same.width, same.height), (400, 200));
    }

    #[test]
    fn png_round_trips_through_decode() {
        let img = gradient(16, 8);
        let mut png_bytes = Vec::new();
        image::write_buffer_with_format(
            &mut Cursor::new(&mut png_bytes),
            &img.rgba,
            16,
            8,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .unwrap();
        let back = ImageData::decode(&png_bytes, None).unwrap();
        assert_eq!((back.width, back.height), (16, 8));
        assert_eq!(back.pixel(3, 2), img.pixel(3, 2));
        let shrunk = ImageData::decode(&png_bytes, Some(8)).unwrap();
        assert_eq!((shrunk.width, shrunk.height), (8, 4));
        assert!(ImageData::decode(b"not an image", None).is_err());
    }

    #[test]
    fn premultiplying_scales_colour_by_alpha() {
        let img = ImageData::new(1, 2, vec![200, 100, 50, 128, 200, 100, 50, 255]).unwrap();
        let p = img.premultiplied();
        assert_eq!(&p[..4], &[100, 50, 25, 128]);
        assert_eq!(&p[4..], &[200, 100, 50, 255]);
        assert!(img.has_alpha());
        assert!(!ImageData::solid(2, 2, Color::WHITE).has_alpha());
    }

    #[test]
    fn linear_premultiplying_scales_light_rather_than_code_values() {
        let img = ImageData::new(1, 3, vec![255, 255, 255, 128, 255, 0, 0, 255, 10, 20, 30, 0]).unwrap();
        let p = img.premultiplied_linear();
        // half the light of white is code value 188 once encoded, not 128
        assert_eq!(p[3], 128);
        assert!((187..=188).contains(&p[0]) && p[0] == p[1] && p[1] == p[2], "{:?}", &p[..3]);
        // opaque pixels are untouched, and fully transparent ones carry no colour
        assert_eq!(&p[4..8], &[255, 0, 0, 255]);
        assert_eq!(&p[8..], &[0, 0, 0, 0]);
    }

    #[test]
    fn thumb_hash_is_small_and_decodes_to_a_similar_blur() {
        let img = gradient(160, 120);
        let hash = img.thumb_hash();
        assert!(hash.len() < 40, "a thumb hash is a few dozen bytes, got {}", hash.len());
        let blur = ImageData::from_thumb_hash(&hash).unwrap();
        assert!(blur.width.max(blur.height) <= 32);
        // the placeholder keeps the picture's overall colour and aspect
        let (a, b) = (img.average_color(), blur.average_color());
        assert!((a.r - b.r).abs() < 0.08 && (a.g - b.g).abs() < 0.08 && (a.b - b.b).abs() < 0.08, "{a:?} vs {b:?}");
        let (ar, br) = (160.0 / 120.0, blur.width as f32 / blur.height as f32);
        assert!((ar - br).abs() < 0.2, "{ar} vs {br}");
        assert!(ImageData::from_thumb_hash(&[1, 2]).is_err());
    }

    #[test]
    fn cover_crops_and_contain_letterboxes() {
        let wide = size(200.0, 100.0);
        let square = rect(0.0, 0.0, 100.0, 100.0);
        let (dst, src) = fit_image(wide, square, Fit::Cover);
        assert_eq!(dst, square);
        assert!((src.w - 0.5).abs() < 1e-6 && (src.x - 0.25).abs() < 1e-6 && src.h == 1.0, "{src:?}");
        let (dst, src) = fit_image(wide, square, Fit::Contain);
        assert_eq!(src, rect(0.0, 0.0, 1.0, 1.0));
        assert_eq!((dst.w, dst.h, dst.y), (100.0, 50.0, 25.0));
        let (dst, src) = fit_image(wide, square, Fit::Fill);
        assert_eq!((dst, src), (square, rect(0.0, 0.0, 1.0, 1.0)));
        // taller than the box: crop top and bottom
        let (_, src) = fit_image(size(100.0, 200.0), square, Fit::Cover);
        assert!((src.h - 0.5).abs() < 1e-6 && (src.y - 0.25).abs() < 1e-6);
    }

    #[test]
    fn registry_hands_out_distinct_ids_and_tracks_size() {
        let images = Images::new();
        let a = images.insert(ImageData::solid(4, 4, Color::BLACK));
        let b = images.insert(ImageData::solid(8, 2, Color::WHITE));
        assert_ne!(a, b);
        assert_eq!(images.size(b), Some(size(8.0, 2.0)));
        assert_eq!(images.total_bytes(), 4 * 4 * 4 + 8 * 2 * 4);
        let other = images.clone();
        assert!(other.remove(a).is_some());
        assert_eq!(images.len(), 1, "clones share the registry");
        assert!(images.get(a).is_none());
    }
}
