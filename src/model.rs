//! Models (authority.md "Presentation"): every thing in the world is a
//! low poly model drawn live, exported by scripts/models.py into
//! assets/models and loaded by Bevy as glTF (the 3D game's way). Anything
//! drawn carries `Model`, the name of its model; `attach` loads it under
//! the thing, and `tint` colours a near-white model under a `Tint`.

use std::collections::HashMap;

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

/// Draw this thing as the model of this name (assets/models/<name>.glb),
/// its ground point at the thing's own.
#[derive(Component, Clone, Debug, PartialEq)]
#[require(Transform, Visibility)]
pub struct Model(pub String);

/// The colour a near-white model is tinted (a building's walls).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Tint(pub Color);

/// Load the model of every thing that just got a `Model`, as its scene.
pub fn attach(mut commands: Commands, assets: Res<AssetServer>, added: Query<(Entity, &Model), Added<Model>>) {
    for (entity, model) in &added {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{}.glb", model.0)));
        commands
            .entity(entity)
            .insert(bevy::world_serialization::prelude::WorldAssetRoot(scene));
    }
}

/// Every tinted material made so far, by the material it was made from and
/// the tint, so every wall of one colour shares one material.
#[derive(Resource, Default)]
pub struct Tinted(HashMap<(AssetId<StandardMaterial>, [u8; 4]), Handle<StandardMaterial>>);

/// The tick a mesh of a loaded model appears under something with a
/// `Tint`, its material becomes that material times the tint.
pub fn tint(
    mut tinted: ResMut<Tinted>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parents: Query<&ChildOf>,
    tints: Query<&Tint>,
    mut loaded: Query<(Entity, &mut MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
) {
    for (entity, mut material) in &mut loaded {
        let Some(tint) = parents.iter_ancestors(entity).find_map(|a| tints.get(a).ok()) else {
            continue;
        };
        let key = (material.0.id(), tint.0.to_srgba().to_u8_array());
        if let Some(handle) = tinted.0.get(&key) {
            material.0 = handle.clone();
            continue;
        }
        let Some(mut made) = materials.get(&material.0).cloned() else {
            continue;
        };
        let (a, b) = (made.base_color.to_linear(), tint.0.to_linear());
        made.base_color = LinearRgba::new(a.red * b.red, a.green * b.green, a.blue * b.blue, a.alpha).into();
        let handle = materials.add(made);
        tinted.0.insert(key, handle.clone());
        material.0 = handle;
    }
}
