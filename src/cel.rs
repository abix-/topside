//! The ONE material for every mesh in the game: a base color with
//! hard light bands. No textures, no smooth gradients.

use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::{Shader, ShaderRef};

pub const CEL_SHADER_HANDLE: Handle<Shader> =
    bevy::asset::uuid_handle!("ce150000-0000-0000-0000-000000000001");

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct CelMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Material for CelMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(CEL_SHADER_HANDLE)
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

pub fn load_shader(mut shaders: ResMut<Assets<Shader>>) {
    let _ = shaders.insert(
        &CEL_SHADER_HANDLE,
        Shader::from_wgsl(include_str!("../assets/shaders/cel.wgsl"), "cel.wgsl"),
    );
}
