// Existing imports…
mod bake;
mod texture;

// Re‑export the compositing function for external use.
pub use bake::compositing::bake_composite;