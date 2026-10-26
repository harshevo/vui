//! A CPU-side RGBA image with PNG input and output and a pixel diff. Used for headless
//! screenshots and golden-image tests.

use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;

/// 8-bit RGBA with straight (not premultiplied) alpha, row-major, top row first.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// How two images differ.
#[derive(Clone, Debug, PartialEq)]
pub struct Diff {
    pub size_mismatch: bool,
    /// Largest difference in any single channel.
    pub max_channel_diff: u8,
    /// Pixels where some channel differs by more than the tolerance.
    pub differing_pixels: usize,
    pub total_pixels: usize,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, rgba: vec![0; (width * height * 4) as usize] }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, p: [u8; 4]) {
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&p);
    }

    pub fn save_png(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(to_io)?;
        writer.write_image_data(&self.rgba).map_err(to_io)
    }

    pub fn load_png(path: &Path) -> io::Result<Image> {
        let mut decoder = png::Decoder::new(BufReader::new(File::open(path)?));
        decoder.set_transformations(png::Transformations::EXPAND);
        let mut reader = decoder.read_info().map_err(to_io)?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| io::Error::other("image too large"))?];
        let info = reader.next_frame(&mut buf).map_err(to_io)?;
        if info.bit_depth != png::BitDepth::Eight || info.color_type != png::ColorType::Rgba {
            return Err(io::Error::other(format!("expected 8-bit RGBA, found {:?} {:?}", info.color_type, info.bit_depth)));
        }
        buf.truncate(info.buffer_size());
        Ok(Image { width: info.width, height: info.height, rgba: buf })
    }

    /// Compares with `other`; channels closer than `tolerance` count as equal.
    pub fn diff(&self, other: &Image, tolerance: u8) -> Diff {
        if self.width != other.width || self.height != other.height {
            return Diff { size_mismatch: true, max_channel_diff: 255, differing_pixels: 0, total_pixels: 0 };
        }
        let mut max = 0u8;
        let mut bad = 0usize;
        let (mine, _) = self.rgba.as_chunks::<4>();
        let (theirs, _) = other.rgba.as_chunks::<4>();
        for (a, b) in mine.iter().zip(theirs) {
            let d = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
            max = max.max(d);
            if d > tolerance {
                bad += 1;
            }
        }
        Diff { size_mismatch: false, max_channel_diff: max, differing_pixels: bad, total_pixels: (self.width * self.height) as usize }
    }

    /// An image that highlights where `self` and `other` differ, for reading a failed golden test.
    pub fn diff_image(&self, other: &Image, tolerance: u8) -> Image {
        let mut out = Image::new(self.width.max(other.width), self.height.max(other.height));
        for y in 0..out.height {
            for x in 0..out.width {
                let inside = x < self.width && y < self.height && x < other.width && y < other.height;
                let p = if !inside {
                    [255, 0, 255, 255]
                } else {
                    let (a, b) = (self.pixel(x, y), other.pixel(x, y));
                    let d = a.iter().zip(&b).map(|(p, q)| p.abs_diff(*q)).max().unwrap_or(0);
                    if d > tolerance { [255, 0, 0, 255] } else { [a[0] / 3 + 170, a[1] / 3 + 170, a[2] / 3 + 170, 255] }
                };
                out.set_pixel(x, y, p);
            }
        }
        out
    }
}

fn to_io(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let mut img = Image::new(3, 2);
        img.set_pixel(1, 1, [10, 20, 30, 255]);
        img.set_pixel(2, 0, [255, 0, 0, 128]);
        let path = std::env::temp_dir().join("vui-image-roundtrip.png");
        img.save_png(&path).unwrap();
        let back = Image::load_png(&path).unwrap();
        assert_eq!(img, back);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn diff_counts_pixels_over_tolerance() {
        let a = Image::new(2, 2);
        let mut b = Image::new(2, 2);
        b.set_pixel(0, 0, [2, 0, 0, 0]);
        b.set_pixel(1, 1, [10, 0, 0, 0]);
        let d = a.diff(&b, 3);
        assert_eq!((d.max_channel_diff, d.differing_pixels, d.total_pixels), (10, 1, 4));
        assert!(a.diff(&Image::new(3, 2), 0).size_mismatch);
    }
}
