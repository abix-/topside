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

/// Where a person was last frame, to tell standing from walking.
#[derive(Component, Default)]
pub struct Stride(Vec3);

/// Every NPC's body: their body's model under them, feet on the ground,
/// facing where they face (the model faces its +z; a person faces +x).
pub fn draw_people(mut commands: Commands, people: Query<(Entity, &Look), (Added<Look>, Without<Player>)>) {
    for (person, look) in &people {
        commands.entity(person).insert(Stride::default()).with_children(|c| {
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

/// The tick a mesh of a person's model appears: shown if it is one of
/// their own layers (the body, a piece of their look, what they wear) and
/// hidden if it is another; the body tinted their skin, a piece their
/// hair. Every mesh of the model is one of these (the rig's bones have
/// none).
pub fn dress(
    items: Res<ItemRegistryRes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut made: Local<HashMap<(AssetId<StandardMaterial>, [u8; 4]), Handle<StandardMaterial>>>,
    parents: Query<&ChildOf>,
    names: Query<&Name>,
    people: Query<(&Look, Option<&Equipment>)>,
    mut loaded: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>, &mut Visibility),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
) {
    for (mesh, mut material, mut visibility) in &mut loaded {
        let Some((look, worn)) = parents.iter_ancestors(mesh).find_map(|a| people.get(a).ok()) else {
            continue;
        };
        let Some(layer) = parents
            .get(mesh)
            .ok()
            .and_then(|p| names.get(p.parent()).ok())
            .map(|n| layer_of(n.as_str()).to_string())
        else {
            continue;
        };
        let wearing = worn.is_some_and(|w| wears(&items, w, &layer));
        let tint = if layer == "body" {
            Some(look.0.skin)
        } else if look.0.pieces.contains(&layer) {
            Some(look.0.hair)
        } else if wearing {
            None
        } else {
            *visibility = Visibility::Hidden;
            continue;
        };
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
    assets: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut animations: ResMut<Animations>,
    parents: Query<&ChildOf>,
    mut people: Query<(&Look, &Transform, &mut Stride)>,
    mut rigs: Query<(Entity, &mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) {
    for (rig, mut player, transitions) in &mut rigs {
        let Some(person) = parents.iter_ancestors(rig).find(|a| people.contains(*a)) else {
            continue;
        };
        let Ok((look, transform, mut stride)) = people.get_mut(person) else {
            continue;
        };
        let body = look.0.body.clone();
        let moving = transform.translation.distance(stride.0) > 0.001;
        stride.0 = transform.translation;
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
