use color_eyre::{eyre::Context, Result};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use serde_bytes::Bytes;
use serialize_hierarchy::SerializeHierarchy;
use std::{
    fmt::{Debug, Error, Formatter},
    mem::{size_of, ManuallyDrop},
    ops::Index,
    path::Path,
    sync::Arc,
};

use image::{codecs::jpeg::JpegEncoder, io::Reader, RgbImage};
use nalgebra::Point2;

use crate::{Rgb, YCbCr444};

use super::color::YCbCr422;

const SERIALIZATION_JPEG_QUALITY: u8 = 40;

#[derive(Clone, Default, Deserialize, SerializeHierarchy)]
pub struct Image {
    buffer: Arc<Vec<YCbCr422>>,
    width_422: u32,
    height: u32,
}

impl Debug for Image {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), Error> {
        formatter
            .debug_struct("Image")
            .field("buffer", &"...")
            .field("width_422", &self.width_422)
            .field("height", &self.height)
            .finish()
    }
}

impl Image {
    pub fn zero(width: u32, height: u32) -> Self {
        assert!(
            width % 2 == 0,
            "the Image type does not support odd widths. Dimensions were {width}x{height}",
        );
        Self::from_ycbcr_buffer(
            vec![YCbCr422::default(); width as usize / 2 * height as usize],
            width / 2,
            height,
        )
    }

    pub fn from_ycbcr_buffer(buffer: Vec<YCbCr422>, width_422: u32, height: u32) -> Self {
        Self {
            buffer: Arc::new(buffer),
            width_422,
            height,
        }
    }

    pub fn from_raw_buffer(buffer: Vec<u8>, width_422: u32, height: u32) -> Self {
        let mut buffer = ManuallyDrop::new(buffer);

        let u8_pointer = buffer.as_mut_ptr();
        let u8_length = buffer.len();
        let u8_capacity = buffer.capacity();

        assert_eq!(u8_length % size_of::<YCbCr422>(), 0);
        assert_eq!(u8_capacity % size_of::<YCbCr422>(), 0);

        let ycbcr_pointer = u8_pointer as *mut YCbCr422;
        let ycbcr_length = u8_length / size_of::<YCbCr422>();
        let ycbcr_capacity = u8_capacity / size_of::<YCbCr422>();

        let buffer = unsafe { Vec::from_raw_parts(ycbcr_pointer, ycbcr_length, ycbcr_capacity) };

        Self {
            buffer: Arc::new(buffer),
            width_422,
            height,
        }
    }

    pub fn load_from_444_png(path: impl AsRef<Path>) -> Result<Self> {
        let png = Reader::open(path)?.decode()?.into_rgb8();

        let width = png.width();
        let height = png.height();
        let rgb_pixels = png.into_vec();

        let pixels = rgb_pixels
            .chunks(6)
            .map(|x| YCbCr422 {
                y1: x[0],
                cb: ((x[1] as u16 + x[4] as u16) / 2) as u8,
                y2: x[3],
                cr: ((x[2] as u16 + x[5] as u16) / 2) as u8,
            })
            .collect();

        Ok(Self::from_ycbcr_buffer(pixels, width / 2, height))
    }

    pub fn save_to_ycbcr_444_file(&self, file: impl AsRef<Path>) -> Result<()> {
        let mut image = RgbImage::new(2 * self.width_422, self.height);
        for y in 0..self.height {
            for x in 0..self.width_422 {
                let pixel = self.buffer[(y * self.width_422 + x) as usize];
                image.put_pixel(x * 2, y, image::Rgb([pixel.y1, pixel.cb, pixel.cr]));
                image.put_pixel(x * 2 + 1, y, image::Rgb([pixel.y2, pixel.cb, pixel.cr]));
            }
        }
        Ok(image.save(file)?)
    }

    pub fn save_to_rgb_file(&self, file: impl AsRef<Path> + Debug) -> Result<()> {
        RgbImage::from(self)
            .save(&file)
            .wrap_err_with(|| format!("failed to save image to {file:?}"))
    }

    pub fn width(&self) -> u32 {
        self.width_422 * 2
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    fn coordinates_to_buffer_index(&self, x: u32, y: u32) -> usize {
        let x_422 = x / 2;
        (y * self.width_422 + x_422) as usize
    }

    pub fn at(&self, x: u32, y: u32) -> YCbCr444 {
        let pixel = self.buffer[self.coordinates_to_buffer_index(x, y)];
        let is_left_pixel = x % 2 == 0;
        YCbCr444 {
            y: if is_left_pixel { pixel.y1 } else { pixel.y2 },
            cb: pixel.cb,
            cr: pixel.cr,
        }
    }

    pub fn try_at(&self, x: u32, y: u32) -> Option<YCbCr444> {
        if x >= self.width() || y >= self.height() {
            return None;
        }
        let pixel = self.buffer[self.coordinates_to_buffer_index(x, y)];
        let is_left_pixel = x % 2 == 0;
        let pixel = YCbCr444 {
            y: if is_left_pixel { pixel.y1 } else { pixel.y2 },
            cb: pixel.cb,
            cr: pixel.cr,
        };
        Some(pixel)
    }
}

impl Index<(usize, usize)> for Image {
    type Output = YCbCr422;

    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        &self.buffer[y * self.width_422 as usize + x]
    }
}

impl Index<Point2<usize>> for Image {
    type Output = YCbCr422;

    fn index(&self, position: Point2<usize>) -> &Self::Output {
        &self.buffer[position.y * self.width_422 as usize + position.x]
    }
}

impl From<&Image> for RgbImage {
    fn from(image: &Image) -> Self {
        let mut rgb_image = RgbImage::new(2 * image.width_422, image.height);
        for y in 0..image.height {
            for x in 0..image.width_422 {
                let pixel = image.buffer[(y * image.width_422 + x) as usize];
                let left_color: Rgb = YCbCr444 {
                    y: pixel.y1,
                    cb: pixel.cb,
                    cr: pixel.cr,
                }
                .into();
                let right_color: Rgb = YCbCr444 {
                    y: pixel.y2,
                    cb: pixel.cb,
                    cr: pixel.cr,
                }
                .into();
                rgb_image.put_pixel(
                    x * 2,
                    y,
                    image::Rgb([left_color.r, left_color.g, left_color.b]),
                );
                rgb_image.put_pixel(
                    x * 2 + 1,
                    y,
                    image::Rgb([right_color.r, right_color.g, right_color.b]),
                );
            }
        }

        rgb_image
    }
}

impl Serialize for Image {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let is_human_readable = serializer.is_human_readable();
        let mut state = serializer.serialize_struct("Image", 3)?;
        if is_human_readable {
            state.serialize_field("buffer", &self.buffer)?;
        } else {
            let rgb_image: RgbImage = self.into();
            let mut rgb_image_buffer = vec![];
            {
                let mut encoder = JpegEncoder::new_with_quality(
                    &mut rgb_image_buffer,
                    SERIALIZATION_JPEG_QUALITY,
                );
                encoder
                    .encode_image(&rgb_image)
                    .expect("failed to encode image");
            }
            state.serialize_field("buffer", Bytes::new(rgb_image_buffer.as_slice()))?;
        }
        state.serialize_field("width_422", &self.width_422)?;
        state.serialize_field("height", &self.height)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use std::mem::transmute;

    use serde_test::{assert_ser_tokens, Configure, Token};

    use super::*;

    #[test]
    fn image_has_zero_constructor() {
        let image = Image::zero(10, 12);
        assert!(image.buffer.iter().all(|&x| x == YCbCr422::default()));
    }

    #[test]
    fn image_has_width_and_height() {
        let image = Image::zero(10, 12);
        assert_eq!(image.width(), 10);
        assert_eq!(image.height(), 12);
    }

    #[test]
    fn image_can_be_indexed() {
        let image = Image::from_ycbcr_buffer(
            vec![
                Default::default(),
                Default::default(),
                Default::default(),
                YCbCr422 {
                    y1: 1,
                    cb: 2,
                    y2: 3,
                    cr: 4,
                },
            ],
            2,
            2,
        );
        assert_eq!(
            image[(1, 1)],
            YCbCr422 {
                y1: 1,
                cb: 2,
                y2: 3,
                cr: 4
            }
        );
    }

    #[test]
    fn image_serializes_with_compact_serializer() {
        let image = Image {
            buffer: Arc::new(vec![YCbCr422 {
                y1: 0,
                cb: 1,
                y2: 2,
                cr: 3,
            }]),
            width_422: 1,
            height: 1,
        };
        let rgb_image: RgbImage = (&image).into();
        let mut rgb_image_buffer = vec![];
        {
            let mut encoder =
                JpegEncoder::new_with_quality(&mut rgb_image_buffer, SERIALIZATION_JPEG_QUALITY);
            encoder
                .encode_image(&rgb_image)
                .expect("failed to encode image");
        }
        // serde_test::Token requires static lifetime and since the byte slice is not used anymore once leaving the test, it should be safe (tm)
        let static_rgb_image_buffer: &'static [u8] =
            unsafe { transmute(rgb_image_buffer.as_slice()) };

        assert_ser_tokens(
            &image.compact(),
            &[
                Token::Struct {
                    name: "Image",
                    len: 3,
                },
                Token::Str("buffer"),
                Token::Bytes(static_rgb_image_buffer),
                Token::Str("width_422"),
                Token::U32(1),
                Token::Str("height"),
                Token::U32(1),
                Token::StructEnd,
            ],
        );
    }

    #[test]
    fn image_serializes_with_readable_serializer() {
        let image = Image {
            buffer: Arc::new(vec![YCbCr422 {
                y1: 0,
                cb: 1,
                y2: 2,
                cr: 3,
            }]),
            width_422: 1,
            height: 1,
        };

        assert_ser_tokens(
            &image.readable(),
            &[
                Token::Struct {
                    name: "Image",
                    len: 3,
                },
                Token::Str("buffer"),
                Token::Seq { len: Some(1) },
                Token::Struct {
                    name: "YCbCr422",
                    len: 4,
                },
                Token::Str("y1"),
                Token::U8(0),
                Token::Str("cb"),
                Token::U8(1),
                Token::Str("y2"),
                Token::U8(2),
                Token::Str("cr"),
                Token::U8(3),
                Token::StructEnd,
                Token::SeqEnd,
                Token::Str("width_422"),
                Token::U32(1),
                Token::Str("height"),
                Token::U32(1),
                Token::StructEnd,
            ],
        );
    }
}
