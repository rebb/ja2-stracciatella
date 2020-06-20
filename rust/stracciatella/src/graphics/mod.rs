use crate::file_formats::stci::{
    etrle::INDEXED_ALPHA_VALUE, StciRgb565, StciRgb888, Stci, StciPalette, StciSubImage,
};
use image::{Delay, DynamicImage, Frame, RgbaImage};
use std::io::{BufRead, Error, ErrorKind, Result, Seek};

const BITS_PER_PIXEL: usize = 4;

#[derive(Debug, Clone)]
pub struct Texture {
    dimensions: (u32, u32),
    offset: (i32, i32),
    data: Vec<u8>,
}

impl Texture {
    pub fn read<R>(r: &mut R) -> Result<Self>
    where
        R: BufRead + Seek,
    {
        if Stci::peek_is_stci(r)? {
            let stci = Stci::from_input(r)?;
            match stci {
                Stci::Indexed {
                    palette,
                    sub_images,
                    ..
                } => {
                    if sub_images.len() != 1 {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "can only use indexed stci with one image as texture, found multiple",
                        ));
                    }
                    let sub_image = &sub_images[0];
                    if let Some(app_data) = &sub_image.app_data {
                        if app_data.number_of_frames != 0 {
                            return Err(Error::new(
                                ErrorKind::InvalidData,
                                "number_of_frames in app_data needs to be zero for texture",
                            ));
                        }
                    }
                    Self::from_sub_image(&palette, sub_image)
                }
                Stci::Rgb {
                    width,
                    height,
                    data,
                    ..
                } => Self::from_rgb565(width.into(), height.into(), data),
            }
        } else {
            let mut buffer: Vec<u8> = vec![];
            r.read_to_end(&mut buffer)?;
            let format = image::guess_format(&buffer).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("could not guess image format: {}", e),
                )
            })?;
            let img = image::load(r, format).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("could not load image: {}", e),
                )
            })?;
            Self::from_image(img)
        }
    }

    fn new(dimensions: (u32, u32), offset: (i32, i32), data: Vec<u8>) -> Result<Self> {
        let expected_data_len = dimensions.0 as usize * dimensions.1 as usize * BITS_PER_PIXEL;
        if data.len() != expected_data_len {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "expected {} bytes of rgba data, got {} bytes",
                    expected_data_len,
                    data.len()
                ),
            ));
        }

        Ok(Texture {
            dimensions,
            offset,
            data,
        })
    }

    fn from_image(img: image::DynamicImage) -> Result<Self> {
        let img = img.to_rgba();
        let width = img.width();
        let height = img.height();
        Texture::new((width, height), (0, 0), img.into_raw())
    }

    fn from_rgb565(width: u32, height: u32, data: Vec<StciRgb565>) -> Result<Self> {
        let data: Vec<u8> = data
            .iter()
            .flat_map(|color565| {
                let color888: StciRgb888 = (*color565).into();
                vec![color888.0, color888.1, color888.2, 255]
            })
            .collect();
        Texture::new((width, height), (0, 0), data)
    }

    fn from_sub_image(palette: &StciPalette, sub_image: &StciSubImage) -> Result<Self> {
        let palette: Vec<[u8; 4]> = palette
            .colors
            .iter()
            .enumerate()
            .map(|(idx, color)| {
                let alpha: u8 = if idx == usize::from(INDEXED_ALPHA_VALUE) {
                    0
                } else {
                    255
                };
                [color.0, color.1, color.2, alpha]
            })
            .collect();

        let data: Vec<_> = sub_image
            .data
            .iter()
            .flat_map(|idx| &palette[usize::from(*idx)])
            .copied()
            .collect();

        Texture::new(
            (sub_image.dimensions.0.into(), sub_image.dimensions.1.into()),
            (sub_image.offset.0.into(), sub_image.offset.1.into()),
            data,
        )
    }

    pub fn into_image(self) -> Result<DynamicImage> {
        Ok(DynamicImage::ImageRgba8(
            RgbaImage::from_raw(self.dimensions.0, self.dimensions.1, self.data).ok_or_else(
                || {
                    Error::new(
                        ErrorKind::InvalidData,
                        "could get rgba image from rgba texture",
                    )
                },
            )?,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct TextureSet {
    textures: Vec<Texture>,
}

impl TextureSet {
    pub fn read<R>(r: &mut R) -> Result<Self>
    where
        R: BufRead + Seek,
    {
        if Stci::peek_is_stci(r)? {
            let stci = Stci::from_input(r)?;
            match stci {
                Stci::Indexed {
                    palette,
                    sub_images,
                    ..
                } => Self::from_sub_images(&palette, &sub_images),
                Stci::Rgb { .. } => Err(Error::new(
                    ErrorKind::InvalidData,
                    "can only use indexed STCI images as texture set",
                )),
            }
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "can only use STCI images as texture set",
            ))
        }
    }

    fn from_sub_images(palette: &StciPalette, sub_images: &[StciSubImage]) -> Result<Self> {
        let mut textures: Vec<Texture> = Vec::with_capacity(sub_images.len());
        if sub_images.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "expected non empty sub_images",
            ));
        }
        for sub_image in sub_images {
            if sub_image.app_data.is_some() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "there should not be app data for texture in texture set",
                ));
            }
        }
        for sub_image in sub_images {
            let texture = Texture::from_sub_image(palette, sub_image)?;
            textures.push(texture);
        }
        Ok(TextureSet { textures })
    }

    pub fn into_images(self) -> Result<Vec<DynamicImage>> {
        self.textures.into_iter().map(|a| a.into_image()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Animation {
    key_frames: Vec<Texture>,
}

impl Animation {
    pub fn read<R>(r: &mut R) -> Result<Self>
    where
        R: BufRead + Seek,
    {
        if Stci::peek_is_stci(r)? {
            let stci = Stci::from_input(r)?;
            match stci {
                Stci::Indexed {
                    palette,
                    sub_images,
                    ..
                } => Self::from_sub_images(&palette, &sub_images),
                Stci::Rgb { .. } => Err(Error::new(
                    ErrorKind::InvalidData,
                    "can only use indexed STCI images as animation",
                )),
            }
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "can only use STCI images as animation",
            ))
        }
    }

    fn from_sub_images(palette: &StciPalette, sub_images: &[StciSubImage]) -> Result<Self> {
        let mut key_frames: Vec<Texture> = Vec::with_capacity(sub_images.len());
        if sub_images.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "expected non empty sub_images",
            ));
        }
        for sub_image in sub_images {
            if sub_image.app_data.is_none() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "all sub images in animation need to have app data",
                ));
            }
        }
        let number_of_frames = sub_images[0].app_data.as_ref().unwrap().number_of_frames;
        if number_of_frames as usize != sub_images.len() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "expected app data to match number of frames in image",
            ));
        }
        for sub_image in sub_images {
            let texture = Texture::from_sub_image(palette, sub_image)?;
            key_frames.push(texture);
        }
        Ok(Animation { key_frames })
    }

    pub fn into_frames(self) -> Result<Vec<Frame>> {
        let min_offset_x = self
            .key_frames
            .iter()
            .map(|k| k.offset.0)
            .min()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    "into_frames has to be called with at least one image",
                )
            })?;
        let min_offset_y = self
            .key_frames
            .iter()
            .map(|k| k.offset.1)
            .min()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    "into_frames has to be called with at least one image",
                )
            })?;
        let max_render_x = self
            .key_frames
            .iter()
            .map(|k| i64::from(k.dimensions.0) + i64::from(k.offset.0))
            .max()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    "into_frames has to be called with at least one image",
                )
            })?;
        let max_render_y = self
            .key_frames
            .iter()
            .map(|k| i64::from(k.dimensions.1) + i64::from(k.offset.1))
            .max()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    "into_frames has to be called with at least one image",
                )
            })?;
        let max_render_x = max_render_x - i64::from(min_offset_x);
        let max_render_y = max_render_y - i64::from(min_offset_y);

        self.key_frames
            .into_iter()
            .map(|key_frame| {
                let offset = key_frame.offset;
                RgbaImage::from_raw(
                    key_frame.dimensions.0,
                    key_frame.dimensions.1,
                    key_frame.data,
                )
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::InvalidData,
                        "could get rgba image from rgba texture",
                    )
                })
                .map(|image| {
                    let mut frame = RgbaImage::new(max_render_x as u32, max_render_y as u32);
                    image::imageops::replace(
                        &mut frame,
                        &image,
                        (offset.0 - min_offset_x) as u32,
                        (offset.1 - min_offset_y) as u32,
                    );
                    Frame::from_parts(frame, 0, 0, Delay::from_numer_denom_ms(1, 60))
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct AnimationSet {
    animations: Vec<Animation>,
}

impl AnimationSet {
    pub fn read<R>(r: &mut R) -> Result<Self>
    where
        R: BufRead + Seek,
    {
        if Stci::peek_is_stci(r)? {
            let stci = Stci::from_input(r)?;
            match stci {
                Stci::Indexed {
                    palette,
                    sub_images,
                    ..
                } => Self::from_sub_images(&palette, &sub_images),
                Stci::Rgb { .. } => Err(Error::new(
                    ErrorKind::InvalidData,
                    "can only use indexed STCI images as animation",
                )),
            }
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "can only use STCI images as animation",
            ))
        }
    }

    fn next_animation<'a, 'b>(
        iter: &'a mut impl Iterator<Item = &'b StciSubImage>,
    ) -> Result<Option<Vec<StciSubImage>>> {
        let first = iter.next();
        if let Some(first) = first {
            let mut keyframes = vec![first.clone()];
            let number_of_frames = first.app_data.as_ref().unwrap().number_of_frames;
            let sub_images = iter.take((number_of_frames - 1).into()).cloned();

            keyframes.extend(sub_images);

            if keyframes.len() != usize::from(number_of_frames) {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "expected number of frames in STCI header to match number of frames in animation",
                ));
            }

            Ok(Some(keyframes))
        } else {
            Ok(None)
        }
    }

    fn from_sub_images(palette: &StciPalette, sub_images: &[StciSubImage]) -> Result<Self> {
        if sub_images.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "expected non empty sub_images",
            ));
        }
        for sub_image in sub_images {
            if sub_image.app_data.is_none() {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "all sub images in animation need to have app data",
                ));
            }
        }
        let mut iter = sub_images.iter();
        let mut animations = vec![];

        while let Some(key_frames) = Self::next_animation(&mut iter)? {
            animations.push(Animation::from_sub_images(palette, &key_frames)?);
        }

        if animations.len() < 2 {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "expected multiple animations for animation set",
            ));
        }

        Ok(AnimationSet { animations })
    }

    pub fn into_frames(self) -> Result<Vec<Vec<Frame>>> {
        self.animations
            .into_iter()
            .map(|a| a.into_frames())
            .collect()
    }
}
