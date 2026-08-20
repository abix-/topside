//! The ONE material for every mesh in the game: a base color with
//! hard light bands. No textures, no smooth gradients.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CelMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for CelMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/cel.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

impl CelMaterial {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self {
            color: LinearRgba::new(r, g, b, 1.0),
        }
    }
}
