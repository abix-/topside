//! Low poly models: ONE file per item def (`ItemDef.model`), the
//! same on the ground, in the hand, and as the icon. Files live in
//! assets/models and are built by scripts/models (Blender headless)
//! or dropped in from a generator; Bevy loads the glTF natively.
//! Every mesh in a loaded model is repainted with the one
//! `CelMaterial` from its glTF base colour, so a model never brings
//! its own shading.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

use crate::actor::Hotbar;
use crate::cel::CelMaterial;
use crate::combat::Firing;
use modforge::combat::Swing;
use crate::content::ItemRegistryRes;
use crate::player::Player;

/// The model file this entity shows: `pipe.glb` under assets/models.
/// `attach` turns it into the loaded scene as a child; headless apps
/// carry the marker and draw nothing.
#[derive(Component, Debug, Clone)]
pub struct Model(pub String);

/// Load the scene for every entity that just got a `Model`.
pub fn attach(
    mut commands: Commands,
    assets: Res<AssetServer>,
    added: Query<(Entity, &Model), Added<Model>>,
) {
    for (entity, model) in &added {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(format!("models/{}", model.0)));
        commands
            .entity(entity)
            .insert(bevy::world_serialization::prelude::WorldAssetRoot(scene));
    }
}

/// The thing in the player's hand: a child of the player (the
/// camera) showing the held hotbar slot's model, named so a change
/// of slot swaps it.
#[derive(Component, Debug)]
pub struct Held(pub String);

/// Where the fist sits in front of the eye: lower right, the way
/// every first-person game holds things. The held model's grip is
/// pinned here and everything rotates about it.
const HAND: Vec3 = Vec3::new(0.3, -0.35, -0.45);
/// Models lie along their own X with the grip at the negative end;
/// this slides the grip onto the fist.
const GRIP: f32 = 0.4;
/// At rest a melee weapon's tip points up, a little forward and a
/// little inward, the way a bat is carried; a gun points forward.
const MELEE_REST: Vec3 = Vec3::new(-0.3, 0.8, -0.5);
const GUN_REST: Vec3 = Vec3::new(0.0, 0.0, -1.0);
/// A melee swing arcs this far down and across (degrees); a gun
/// kicks this far back (metres).
const SWING_DEGREES: f32 = 110.0;
const KICK: f32 = 0.08;

/// Show the held slot's item in the hand and move it with the fire
/// timer from `combat` (todo 10b): the pose is a function of how far
/// through the weapon's delay we are, no second timer. Melee swings
/// down and back; a gun kicks back and returns.
pub fn held(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CelMaterial>>,
    registry: Res<ItemRegistryRes>,
    players: Query<(Entity, &Hotbar, &Firing, Option<&Children>), With<Player>>,
    mut helds: Query<(Entity, &Held, &mut Transform)>,
) {
    let Ok((player, hotbar, firing, children)) = players.single() else {
        return;
    };
    let def = hotbar
        .0
        .held
        .and_then(|i| hotbar.0.slots.slots.get(i))
        .and_then(|s| s.as_ref())
        .and_then(|s| registry.0.def(&s.item));
    let shown = children
        .into_iter()
        .flat_map(|c| c.iter())
        .find_map(|c| helds.get(c).ok().map(|(e, h, _)| (e, h.0.clone())));
    // Swap when the held item changed.
    let wanted = def.map(|d| d.name.clone());
    if shown.as_ref().map(|(_, name)| name) != wanted.as_ref() {
        if let Some((old, _)) = shown {
            commands.entity(old).despawn();
        }
        if let Some(def) = def {
            // The fist: rotates. Under it the model, slid so its
            // grip sits on the fist.
            let fist = commands
                .spawn((
                    Held(def.name.clone()),
                    ChildOf(player),
                    Transform::from_translation(HAND),
                    Visibility::default(),
                ))
                .id();
            let mut shape = commands.spawn((
                ChildOf(fist),
                Transform::from_xyz(GRIP, 0.0, 0.0),
                Visibility::default(),
            ));
            match &def.model {
                Some(file) => {
                    shape.insert(Model(file.clone()));
                }
                None => {
                    shape.insert((
                        Mesh3d(meshes.add(Cuboid::new(0.5, 0.08, 0.08))),
                        MeshMaterial3d(materials.add(CelMaterial::from_srgb(0.5, 0.5, 0.5))),
                    ));
                }
            }
        }
        return;
    }
    // Pose it from the fire timer.
    let (Some(def), Some((entity, _))) = (def, shown) else {
        return;
    };
    let Ok((_, _, mut transform)) = helds.get_mut(entity) else {
        return;
    };
    let is_gun = def.combat.as_ref().is_some_and(|c| c.ammo.is_some());
    // 0 at rest, 1 just after firing; the timer counts down over the
    // delay it was set with.
    let through = if firing.0.delay > 0.0 {
        (firing.0.ready_in / firing.0.delay).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let arc = (through * std::f32::consts::PI).sin();
    if is_gun {
        let rest = Quat::from_rotation_arc(Vec3::X, GUN_REST.normalize());
        transform.translation = HAND + Vec3::Z * arc * KICK;
        transform.rotation = rest;
        return;
    }
    // Carried tip up. Gothic's four moves turn the weapon about the
    // fist, which never moves: the overhead comes forward and down
    // the line; a side swing winds the tip out to that side, sweeps
    // it level across the front to the other side, and returns; a
    // block holds the weapon flat across the view.
    let rest = Quat::from_rotation_arc(Vec3::X, MELEE_REST.normalize());
    transform.translation = HAND;
    // Progress through the swing, 0 at the press, 1 back at rest.
    let progress = 1.0 - through;
    transform.rotation = match firing.0.swing {
        Swing::Overhead => Quat::from_rotation_x(-arc * SWING_DEGREES.to_radians()) * rest,
        Swing::Left => side_swing(rest, -1.0, progress),
        Swing::Right => side_swing(rest, 1.0, progress),
        Swing::Block => Quat::from_rotation_z(-1.2 * arc) * rest,
    };
}

/// A horizontal swing from `side` (-1 left, 1 right): the tip winds
/// out level on that side, sweeps across the front to the other
/// side, and comes home. Three phases of the swing's progress, each
/// a smooth turn between two tip directions.
fn side_swing(rest: Quat, side: f32, progress: f32) -> Quat {
    let toward = |tip: Vec3| Quat::from_rotation_arc(Vec3::X, tip.normalize());
    let wound = toward(Vec3::new(side * 0.9, 0.15, -0.4));
    let through = toward(Vec3::new(-side * 0.9, 0.0, -0.4));
    let smooth = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    if progress < 0.2 {
        rest.slerp(wound, smooth(progress / 0.2))
    } else if progress < 0.6 {
        wound.slerp(through, smooth((progress - 0.2) / 0.4))
    } else {
        through.slerp(rest, smooth((progress - 0.6) / 0.4))
    }
}

/// The glTF loader hands every primitive a `StandardMaterial`; swap
/// it for the cel material of the same base colour the tick it
/// appears. The one material rule (design.md "Cel shading").
pub fn cel_shade(
    mut commands: Commands,
    standard: Res<Assets<StandardMaterial>>,
    mut cel: ResMut<Assets<CelMaterial>>,
    loaded: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
) {
    for (entity, material) in &loaded {
        let color = standard
            .get(&material.0)
            .map_or(LinearRgba::WHITE, |m| m.base_color.into());
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(cel.add(CelMaterial { color })));
    }
}
