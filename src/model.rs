//! Models (authority.md "Presentation"): every thing in the world is a
//! low poly model drawn live, exported by scripts/models.py into
//! assets/models and loaded by Bevy as glTF (the 3D game's way). Anything
//! drawn carries `Model`, the name of its model; `attach` loads it under
//! the thing, and `tint` colours a near-white model under a `Tint`.

use std::collections::HashMap;

use bevy::gltf::GltfAssetLabel;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

/// Towards the sun, in the world: the cel shade's light (and the one the
/// `DirectionalLight` in lib.rs shines from).
pub const SUN: Vec3 = Vec3::new(0.5, 1.0, 0.3);

/// The cel shade on top of Bevy's standard material (assets/shaders/
/// cel_shade.wgsl): the model's colours, the sun's light cut into hard
/// bands. The one material every mesh in the game is drawn with. Whether
/// it gets outlines (src/outline.rs) is in its prepass
/// (assets/shaders/cel_prepass.wgsl): not a material cut out by its
/// alpha (leaves), whose every cut edge would be a line.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CelShade {
    #[uniform(100)]
    pub sun: Vec4,
    /// 1 outlined, 0 not.
    #[uniform(100)]
    pub outlined: f32,
}

impl MaterialExtension for CelShade {
    fn fragment_shader() -> ShaderRef {
        "shaders/cel_shade.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/cel_prepass.wgsl".into()
    }
}

/// A standard material, cel shaded.
pub type Cel = ExtendedMaterial<StandardMaterial, CelShade>;

/// The cel shaded twin of every standard material met so far.
#[derive(Resource, Default)]
pub struct CelShaded {
    twins: HashMap<AssetId<StandardMaterial>, Handle<Cel>>,
    /// The twin of each different material, by everything it is.
    alike: HashMap<String, Handle<Cel>>,
}

impl CelShaded {
    /// The cel shaded twin of a standard material, made the first time
    /// it is asked for; None while the material is not loaded.
    pub fn twin(&mut self, material: &Handle<StandardMaterial>, standard: &Assets<StandardMaterial>, cel: &mut Assets<Cel>) -> Option<Handle<Cel>> {
        if let Some(handle) = self.twins.get(&material.id()) {
            return Some(handle.clone());
        }
        let base = standard.get(material)?.clone();
        // Every model file loads its own materials, the same colours over
        // and over; one twin for each different material, as each
        // material is a batch of its own to draw (Bevy issue 26182: on
        // DX12 no standard material shares a bind group).
        let same = format!("{base:?}");
        if let Some(handle) = self.alike.get(&same) {
            let handle = handle.clone();
            self.twins.insert(material.id(), handle.clone());
            return Some(handle);
        }
        // Cut out by its alpha (leaves): no outlines.
        let outlined = if matches!(base.alpha_mode, AlphaMode::Mask(_)) { 0.0 } else { 1.0 };
        let handle = cel.add(Cel {
            base,
            extension: CelShade { sun: SUN.normalize().extend(0.0), outlined },
        });
        self.twins.insert(material.id(), handle.clone());
        self.alike.insert(same, handle.clone());
        Some(handle)
    }
}

/// The tick a mesh appears with a standard material (a model's, the
/// ground's), it is drawn with the cel shaded twin of it instead; after
/// anything that tints it.
pub fn cel_shade(
    mut commands: Commands,
    mut shaded: ResMut<CelShaded>,
    standard: Res<Assets<StandardMaterial>>,
    mut cel: ResMut<Assets<Cel>>,
    loaded: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
) {
    for (entity, material) in &loaded {
        let Some(handle) = shaded.twin(&material.0, &standard, &mut cel) else {
            continue;
        };
        // The mesh may be despawned this same frame (a load, a despawn
        // after this system ran): then there is nothing to shade.
        commands
            .entity(entity)
            .try_remove::<MeshMaterial3d<StandardMaterial>>()
            .try_insert(MeshMaterial3d(handle));
    }
}

/// Draw this thing as the model of this name (assets/models/<name>.glb),
/// its ground point at the thing's own.
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
#[require(Transform, Visibility)]
pub struct Model(pub String);

/// The colour a near-white model is tinted (a building's walls).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Tint(pub Color);

/// The models a stack is drawn with, bottom to top: its item's `base`
/// model, then each of its layers' on it, `<base> <layer>`
/// (scripts/models.py exports them in one frame, so they stack). The one
/// answer for the hand and the ground.
pub fn stacked(base: &str, stack: &modforge::item::ItemStack, layers: &modforge::item::LayerRegistry) -> Vec<String> {
    std::iter::once(base.to_string())
        .chain(
            stack
                .layers
                .iter()
                .filter_map(|l| layers.def(l))
                .filter_map(|l| l.picture.as_ref())
                .map(|p| format!("{base} {p}")),
        )
        .collect()
}

/// A thing lying in the world with layers (a nailed pipe on the floor)
/// shows each layer's model on its own, as the hand does.
pub fn draw_item_layers(
    mut commands: Commands,
    layers: Res<crate::content::LayerRegistryRes>,
    items: Query<(Entity, &Model, &crate::interact::WorldItem), Added<crate::interact::WorldItem>>,
) {
    for (entity, model, item) in &items {
        let models = stacked(&model.0, &item.stack, &layers.0);
        commands.entity(entity).with_children(|c| {
            for layer in models.into_iter().skip(1) {
                c.spawn(Model(layer));
            }
        });
    }
}

/// Load the model of every thing that just got a `Model`, as its scene.
pub fn attach(mut commands: Commands, assets: Res<AssetServer>, added: Query<(Entity, &Model), Added<Model>>) {
    for (entity, model) in &added {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{}.glb", model.0)));
        // The thing may be despawned this same frame (a load, a despawn
        // after this system ran): then there is nothing to draw.
        commands
            .entity(entity)
            .try_insert(bevy::world_serialization::prelude::WorldAssetRoot(scene));
    }
}

/// Every tinted mesh made so far, by the mesh it was made from and the
/// tint, so every wall of one colour shares one mesh. A tint is carried in
/// the mesh's vertex colours, which the standard material multiplies its
/// colour by, never in a material of its own: each material is a batch of
/// its own to draw (Bevy issue 26182: on DX12 no standard material shares
/// a bind group), and a material per colour made 415 of them (Perf 3).
#[derive(Resource, Default)]
pub struct Tinted(HashMap<(AssetId<Mesh>, [u8; 4]), Handle<Mesh>>);

impl Tinted {
    /// The mesh with the tint in its colours, made the first time it is
    /// asked for; None while the mesh is not loaded.
    pub fn mesh(&mut self, mesh: &Handle<Mesh>, tint: Color, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
        let key = (mesh.id(), tint.to_srgba().to_u8_array());
        if let Some(handle) = self.0.get(&key) {
            return Some(handle.clone());
        }
        let mut made = meshes.get(mesh)?.clone();
        tint_colours(&mut made, tint.to_linear());
        let handle = meshes.add(made);
        self.0.insert(key, handle.clone());
        Some(handle)
    }
}

/// A mesh's vertex colours times a tint; a mesh with none gets the tint as
/// its colour (Bevy's colour attribute is always four floats).
pub fn tint_colours(mesh: &mut Mesh, tint: LinearRgba) {
    if let Some(bevy::mesh::VertexAttributeValues::Float32x4(colours)) = mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR) {
        for c in colours.iter_mut() {
            *c = [c[0] * tint.red, c[1] * tint.green, c[2] * tint.blue, c[3]];
        }
        return;
    }
    let count = mesh.count_vertices();
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[tint.red, tint.green, tint.blue, 1.0]; count]);
}

/// The tick a mesh of a loaded model appears under something with a
/// `Tint`, it is drawn as its mesh with the tint in its colours.
pub fn tint(
    mut tinted: ResMut<Tinted>,
    mut meshes: ResMut<Assets<Mesh>>,
    parents: Query<&ChildOf>,
    tints: Query<&Tint>,
    mut loaded: Query<(Entity, &mut Mesh3d), Added<MeshMaterial3d<StandardMaterial>>>,
) {
    for (entity, mut mesh) in &mut loaded {
        let Some(tint) = parents.iter_ancestors(entity).find_map(|a| tints.get(a).ok()) else {
            continue;
        };
        if let Some(handle) = tinted.mesh(&mesh.0, tint.0, &mut meshes) {
            mesh.0 = handle;
        }
    }
}
