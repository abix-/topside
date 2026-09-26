//! People drawn in 3D (authority.md "Presentation"; design.md: people are
//! built from pieces). Every body is one model, `person <body>`
//! (scripts/models.py), holding the rig, the idle and walk, and every piece
//! that fits it, each mesh named after its layer. A person shows their own
//! pieces (their rolled `Look` and what they wear), skin and hair tinted,
//! and hides the rest; the rig plays the idle standing and the walk moving.
//! The player is seen through their own eyes, so their body is not drawn.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use modforge::item::EquipSlot;

use crate::actor::Look;
use crate::combat::Equipment;
use crate::content::ItemRegistryRes;
use crate::model::Model;
use crate::player::{BODY_CENTRE, Player};

/// The animations a person plays, as scripts/models.py keeps them
/// (KEPT_ANIMATIONS); change both together.
const IDLE: &str = "Idle_Loop";
const WALK: &str = "Walk_Loop";
/// How long one animation takes to blend into the next.
const BLEND: Duration = Duration::from_millis(200);

/// Where a person was last frame, and how long they have not moved, to
/// tell standing from walking: people move in the fixed step, so a frame
/// between two steps shows no movement.
#[derive(Component, Default)]
pub struct Stride {
    last: Vec3,
    still: f32,
}

/// Seconds without moving before a person counts as standing (the 2D
/// game's).
const STILL: f32 = 0.1;

/// What a person's model shows now, for the control plane: the layers
/// shown, what is in their hand (its models), and whether the walk plays.
#[derive(Component, Default, Debug)]
pub struct Drawn {
    pub layers: Vec<String>,
    pub held: Option<String>,
    pub walking: bool,
}

/// Every NPC's body: their body's model under them, feet on the ground,
/// facing where they face (the model faces its +z; a person faces +x).
pub fn draw_people(mut commands: Commands, people: Query<(Entity, &Look), (Added<Look>, Without<Player>)>) {
    for (person, look) in &people {
        commands.entity(person).insert((Stride::default(), Drawn::default())).with_children(|c| {
            c.spawn((
                Model(format!("person {}", look.0.body)),
                Transform::from_xyz(0.0, -BODY_CENTRE, 0.0).with_rotation(Quat::from_rotation_y(FRAC_PI_2)),
            ));
        });
    }
}

/// The layer a mesh of a person's model belongs to: the name of the node
/// it hangs off, before any `.n` (scripts/models.py names them so).
fn layer_of(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

/// Whether `worn` has on the item whose model layer is `layer` (the road
/// sign vest), in any slot but the hands.
fn wears(items: &ItemRegistryRes, worn: &Equipment, layer: &str) -> bool {
    EquipSlot::ALL
        .into_iter()
        .filter(|slot| *slot != EquipSlot::Weapon)
        .filter_map(|slot| worn.0.get(slot))
        .filter_map(|stack| items.0.def(&stack.item))
        .any(|def| def.picture.as_deref() == Some(layer))
}

/// How a person shows one layer of their model: hidden (None), or shown
/// with this tint (the body their skin, a piece of their look their
/// hair, what they wear untinted).
fn shows(layer: &str, look: &Look, worn: Option<&Equipment>, items: &ItemRegistryRes) -> Option<Option<[f32; 3]>> {
    if layer == "body" {
        Some(Some(look.0.skin))
    } else if look.0.pieces.iter().any(|p| p == layer) {
        Some(Some(look.0.hair))
    } else if worn.is_some_and(|w| wears(items, w, layer)) {
        Some(None)
    } else {
        None
    }
}

/// The layer of a mesh of a person's model: its node's name (the mesh's
/// parent), before any `.n`.
fn mesh_layer(mesh: Entity, parents: &Query<&ChildOf>, names: &Query<&Name>) -> Option<String> {
    let node = parents.get(mesh).ok()?.parent();
    names.get(node).ok().map(|n| layer_of(n.as_str()).to_string())
}

/// The tick a mesh of a person's model appears: shown if it is one of
/// their own layers (the body, a piece of their look, what they wear) and
/// hidden if it is another, tinted as `shows` says. Every mesh of the
/// model is one of these (the rig's bones have none).
pub fn dress(
    items: Res<ItemRegistryRes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<HashMap<(AssetId<StandardMaterial>, [u8; 4]), Handle<StandardMaterial>>>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    mut people: Query<(&Look, Option<&Equipment>, &mut Drawn)>,
    helds: Query<(), With<crate::held::Held>>,
    mut loaded: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
) {
    for (mesh, mut material, mut visibility) in &mut loaded {
        // What they hold hangs off their hand but is not their body.
        if parents.iter_ancestors(mesh).any(|a| helds.contains(a)) {
            continue;
        }
        let Some(person) = parents.iter_ancestors(mesh).find(|a| people.contains(*a)) else {
            continue;
        };
        let Some(layer) = mesh_layer(mesh, &parents, &names) else {
            continue;
        };
        let Ok((look, worn, mut drawn)) = people.get_mut(person) else {
            continue;
        };
        let Some(tint) = shows(&layer, look, worn, &items) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if !drawn.layers.contains(&layer) {
            drawn.layers.push(layer);
        }
        let Some([r, g, b]) = tint else {
            continue;
        };
        let colour = Color::srgb(r, g, b);
        let key = (material.0.id(), colour.to_srgba().to_u8_array());
        if let Some(handle) = made.get(&key) {
            material.0 = handle.clone();
            continue;
        }
        let Some(mut tinted) = materials.get(&material.0).cloned() else {
            continue;
        };
        let (a, t) = (tinted.base_color.to_linear(), colour.to_linear());
        tinted.base_color = LinearRgba::new(a.red * t.red, a.green * t.green, a.blue * t.blue, a.alpha).into();
        let handle = materials.add(tinted);
        made.insert(key, handle.clone());
        material.0 = handle;
    }
}

/// Whenever what a person wears changes, their model's layers are shown
/// or hidden again by the same rule (`shows`): a vest put on shows, one
/// taken off hides.
pub fn redress(
    items: Res<ItemRegistryRes>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    children: Query<&Children>,
    mut people: Query<(Entity, &Look, &Equipment, &mut Drawn), Changed<Equipment>>,
    helds: Query<(), With<crate::held::Held>>,
    mut meshes: Query<&mut Visibility, With<Mesh3d>>,
) {
    for (person, look, worn, mut drawn) in &mut people {
        let mut shown = Vec::new();
        for mesh in children.iter_descendants(person) {
            // What they hold hangs off their hand but is not their body.
            if parents.iter_ancestors(mesh).any(|a| helds.contains(a)) {
                continue;
            }
            let Ok(mut visibility) = meshes.get_mut(mesh) else {
                continue;
            };
            let Some(layer) = mesh_layer(mesh, &parents, &names) else {
                continue;
            };
            if shows(&layer, look, Some(worn), &items).is_some() {
                visibility.set_if_neq(Visibility::Inherited);
                if !shown.contains(&layer) {
                    shown.push(layer);
                }
            } else {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
        if !shown.is_empty() {
            drawn.layers = shown;
        }
    }
}

/// Each body's model file, and once it has loaded, its animation graph
/// with the idle and the walk.
#[derive(Resource, Default)]
pub struct Animations {
    files: HashMap<String, Handle<Gltf>>,
    graphs: HashMap<String, (Handle<AnimationGraph>, AnimationNodeIndex, AnimationNodeIndex)>,
}

/// Every person's rig plays the idle standing and the walk moving,
/// blending between them.
pub fn animate(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut animations: ResMut<Animations>,
    parents: Query<&ChildOf>,
    mut people: Query<(&Look, &Transform, &mut Stride, &mut Drawn)>,
    mut rigs: Query<(Entity, &mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) {
    for (rig, mut player, transitions) in &mut rigs {
        let Some(person) = parents.iter_ancestors(rig).find(|a| people.contains(*a)) else {
            continue;
        };
        let Ok((look, transform, mut stride, mut drawn)) = people.get_mut(person) else {
            continue;
        };
        let body = look.0.body.clone();
        if transform.translation.distance(stride.last) > 0.001 {
            stride.still = 0.0;
        } else {
            stride.still += time.delta_secs();
        }
        stride.last = transform.translation;
        let moving = stride.still < STILL;
        drawn.walking = moving;
        // The graph for this body, made once its file has loaded.
        if !animations.graphs.contains_key(&body) {
            let file = animations
                .files
                .entry(body.clone())
                .or_insert_with(|| assets.load(format!("models/person {body}.glb")))
                .clone();
            let Some(gltf) = gltfs.get(&file) else {
                continue;
            };
            let (Some(idle), Some(walk)) = (gltf.named_animations.get(IDLE), gltf.named_animations.get(WALK)) else {
                warn!("person {body} has no {IDLE} or {WALK}");
                continue;
            };
            let (graph, nodes) = AnimationGraph::from_clips([idle.clone(), walk.clone()]);
            animations.graphs.insert(body.clone(), (graphs.add(graph), nodes[0], nodes[1]));
        }
        let (graph, idle, walk) = animations.graphs[&body].clone();
        let Some(mut transitions) = transitions else {
            let mut transitions = AnimationTransitions::new();
            transitions.play(&mut player, idle, Duration::ZERO).repeat();
            commands.entity(rig).insert((AnimationGraphHandle(graph), transitions));
            continue;
        };
        let wanted = if moving { walk } else { idle };
        if !player.is_playing_animation(wanted) {
            transitions.play(&mut player, wanted, BLEND).repeat();
        }
    }
}
