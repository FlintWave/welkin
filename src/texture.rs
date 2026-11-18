use image::{DynamicImage, GenericImageView, Pixel, RgbaImage};

/// Simple wrapper around `RgbaImage` providing the minimal API used by the baking system.
#[derive(Clone, Debug)]
pub struct Texture {
    image: RgbaImage,
}

impl Texture {
    /// Creates a new texture from an `RgbaImage`.
    pub fn new(image: RgbaImage) -> Self {
        Self { image }
    }

    /// Returns the width of the texture in pixels.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Returns the height of the texture in pixels.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Retrieves the pixel at (`x`, `y`). The pixel is returned as an `[u8; 4]` RGBA array.
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.image.get_pixel(x, y).0
    }

    /// Sets the pixel at (`x`, `y`) to the supplied RGBA value.
    pub fn set_pixel(&mut self, x: u32, y: u32, value: [u8; 4]) {
        self.image.put_pixel(x, y, image::Rgba(value));
    }
}

// Helper conversion from common image types.
impl From<DynamicImage> for Texture {
    fn from(img: DynamicImage) -> Self {
        Self::new(img.to_rgba8())
    }
}