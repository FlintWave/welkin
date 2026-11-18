use crate::texture::Texture;

/// Combines wearable layers (skin, tattoo, undershirt, etc.) into a final baked texture.
/// The layers are applied in order, with later layers composited on top of earlier ones.
///
/// For now this implementation performs a simple overlay blend where non‑transparent
/// pixels from the top layer replace the corresponding pixels of the bottom layer.
/// This is sufficient for the current test suite and can be replaced with a more
/// sophisticated PBR‑aware compositing later.
///
/// # Arguments
///
/// * `layers` - A vector of `Texture` objects ordered from bottom (e.g., skin) to top
///   (e.g., tattoo). Each texture must have the same dimensions and pixel format.
///
/// # Returns
///
/// A new `Texture` representing the composited result.
///
/// # Panics
///
/// Panics if the input vector is empty or if the textures have mismatched dimensions.
pub fn bake_composite(layers: Vec<Texture>) -> Texture {
    assert!(!layers.is_empty(), "At least one layer is required for compositing");

    // Clone the bottom-most layer to use as the base canvas.
    let mut result = layers[0].clone();

    // Ensure all layers share the same size.
    let (width, height) = (result.width(), result.height());

    for layer in layers.iter().skip(1) {
        assert_eq!(
            (layer.width(), layer.height()),
            (width, height),
            "All layers must have identical dimensions"
        );

        // Simple per‑pixel overlay: if the top pixel's alpha is > 0, replace the bottom pixel.
        for y in 0..height {
            for x in 0..width {
                let top_pixel = layer.get_pixel(x, y);
                if top_pixel[3] > 0 {
                    result.set_pixel(x, y, top_pixel);
                }
            }
        }
    }

    result
}