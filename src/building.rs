//! Building (todo 9f), Rust's way, on modforge's rules (modforge
//! building.rs: the grid, grades, what holds a piece up, the tool
//! cupboard, upkeep and decay, soft sides). Anyone holding the building
//! plan strikes (Attack) to place a twig piece where they face, snapped
//! to the grid, paid in wood; uses it (E) to pick the next piece. The
//! hammer struck at a piece upgrades it to its next grade, paid in that
//! grade's material. The tool cupboard struck down stands in the cell
//! faced: only its group builds in its reach, and each game hour it pays
//! its pieces' upkeep from what it holds, or they decay. A piece no
//! longer held up falls. The pieces are drawn and made solid from what
//! is built, never saved apart. Everyone is equal: the player and any
//! person build the same way.

use avian3d::prelude::*;
use bevy::prelude::*;
use modforge::actions::Action;
use modforge::building::{Build, CELL, Cupboard, Grade, Kind, LEVEL, Slot};

use crate::actor::{Dead, Hotbar, Inventory, Npc};
use crate::content::{BUILDING_PLAN, HAMMER, ItemRegistryRes, PLAYER_FACTION, TOOL_CUPBOARD, grade_material};
use crate::input::ThisTick;
use crate::interact::{Door, Interactable};
use crate::model::Model;
use crate::structure::{FLOOR_TILE, FURNITURE_HEIGHT, WALL_HEIGHT};
use crate::{Ground, on_ground};
use modforge::hud::InteractKind;
use modforge::structure::DOORWAY_HEIGHT;

/// A foundation's height: everything on it stands this much higher.
pub const FOUNDATION_TOP: f32 = 0.3;
/// How far ahead a piece goes, metres: a cell.
const AHEAD: f32 = CELL;

/// Everything built, the one record of it (saved).
#[derive(Resource, Default, Clone, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Builds(pub Build);

/// The piece a building plan's holder places next (saved).
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Component, Serialize, Deserialize)]
pub struct Planning(pub Kind);

/// A drawn piece: its slot, and the grade it is drawn at.
#[derive(Component)]
pub struct Drawn(pub Slot, pub Grade);

fn group_of(is_player: bool, npc: Option<&Npc>) -> String {
    if is_player { PLAYER_FACTION.to_string() } else { npc.and_then(|n| n.faction.clone()).unwrap_or_default() }
}

/// Take `n` of an item from the bag and the hotbar, only if both hold
/// that many together.
fn pay(inventory: &mut Inventory, hotbar: &mut Hotbar, item: &str, n: u32) -> bool {
    if inventory.0.count_of(item) + hotbar.0.slots.count_of(item) < n {
        return false;
    }
    for _ in 0..n {
        if !inventory.0.take_one(item) {
            hotbar.0.slots.take_one(item);
        }
    }
    true
}

/// The point a piece goes at: a cell ahead of the feet, on the grid's
/// levels (a foundation's top counted as its level's floor).
fn ahead(at: &Transform, facing: &crate::actor::Facing) -> [f32; 3] {
    let feet = crate::interact::feet_of(at);
    let point = on_ground(at.translation.ground() + facing.direction() * AHEAD, feet.y - FOUNDATION_TOP);
    [point.x, point.y.max(0.0), point.z]
}

fn cell_of(at: &Transform) -> (i32, i32) {
    ((at.translation.x / CELL).floor() as i32, (at.translation.z / CELL).floor() as i32)
}

/// Every step: each person's building plan, hammer and tool cupboard,
/// struck or used. Then whatever is no longer held up falls.
#[allow(clippy::type_complexity)]
pub fn build(
    mut commands: Commands,
    mut builds: ResMut<Builds>,
    items: Res<ItemRegistryRes>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    tick: Res<crate::SimTick>,
    mut people: Query<
        (Entity, &mut ThisTick, &Transform, &crate::actor::Facing, &mut Inventory, &mut Hotbar, Option<&Npc>, Has<crate::player::Player>, Option<&Planning>),
        Without<Dead>,
    >,
) {
    for (entity, mut this_tick, at, facing, mut inventory, mut hotbar, npc, is_player, planning) in &mut people {
        let Some(held) = hotbar.0.held_stack().map(|s| s.item.clone()) else {
            continue;
        };
        let struck = this_tick.0.iter().any(|s| s.action == Action::Attack);
        let used = this_tick.0.iter().any(|s| s.action == Action::Use);
        let group = group_of(is_player, npc);
        let kind = planning.map_or(Kind::Foundation, |p| p.0);
        let point = ahead(at, facing);
        match held.as_str() {
            BUILDING_PLAN if used => {
                commands.entity(entity).insert(Planning(kind.next()));
                this_tick.0.retain(|s| s.action != Action::Use);
                info!("building plan: {}", kind.next().name());
            }
            BUILDING_PLAN if struck => {
                let slot = Slot::at(kind, point);
                let cost = Grade::Twig.cost();
                let material = grade_material(Grade::Twig);
                if inventory.0.count_of(material) + hotbar.0.slots.count_of(material) < cost {
                    continue;
                }
                match builds.0.place(&group, kind, slot, cell_of(at)) {
                    Ok(()) => {
                        pay(&mut inventory, &mut hotbar, material, cost);
                        info!("built a twig {} at {slot:?}", kind.name());
                    }
                    Err(why) => info!("no {} at {slot:?}: {why:?}", kind.name()),
                }
            }
            HAMMER if struck => {
                // The piece nearest the point faced, within a cell.
                let target = Vec3::from(point);
                let Some(slot) = builds
                    .0
                    .pieces
                    .keys()
                    .map(|s| (*s, Vec3::from(s.centre()).distance(target)))
                    .filter(|(_, d)| *d <= CELL)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s)
                else {
                    continue;
                };
                let Some(next) = builds.0.pieces[&slot].grade.next() else {
                    continue;
                };
                if !builds.0.may_build(&group, slot) || !pay(&mut inventory, &mut hotbar, grade_material(next), next.cost()) {
                    continue;
                }
                builds.0.upgrade(&group, slot);
                info!("upgraded {slot:?} to {next:?}");
            }
            TOOL_CUPBOARD if struck => {
                let slot = Slot::at(Kind::Foundation, point);
                if builds.0.privilege(slot).is_some() {
                    continue;
                }
                let Some(def) = items.0.def(TOOL_CUPBOARD) else {
                    continue;
                };
                let Some(index) = hotbar.0.held else {
                    continue;
                };
                hotbar.0.slots.remove(index, 1);
                let [x, y, z] = slot.centre();
                let stack = modforge::item::create(def, 1, &[], 0.0, tick.0);
                crate::interact::spawn_item(&mut commands, &mut ids, def, stack, Vec3::new(x, y + FOUNDATION_TOP, z));
                builds.0.cupboards.push(Cupboard { owner: group.clone(), at: slot });
                info!("{group} set a tool cupboard at {slot:?}");
            }
            _ => {}
        }
    }
    for (slot, piece) in builds.0.collapse() {
        info!("the {} at {slot:?} fell", piece.kind.name());
    }
}

/// Once a game hour: each cupboard pays its pieces' upkeep from what it
/// holds; every piece no paid cupboard reaches decays.
pub fn upkeep(
    tick: Res<crate::SimTick>,
    day: Res<crate::survival::Day>,
    mut builds: ResMut<Builds>,
    mut cupboards: Query<(&crate::interact::WorldItem, &Transform, &mut crate::interact::Container)>,
) {
    let hour = ((day.0 / 24.0) * 60.0).round().max(1.0) as u64;
    if tick.0 == 0 || tick.0 % hour != 0 || builds.0.pieces.is_empty() {
        return;
    }
    let mut paid: Vec<(Slot, Vec<Grade>)> = Vec::new();
    for cupboard in builds.0.cupboards.clone() {
        let [x, _, z] = cupboard.at.centre();
        let Some((_, _, mut holds)) = cupboards
            .iter_mut()
            .find(|(item, t, _)| item.stack.item == TOOL_CUPBOARD && Vec2::new(t.translation.x - x, t.translation.z - z).length() < CELL / 2.0)
        else {
            continue;
        };
        let mut grades = Vec::new();
        for (grade, n) in builds.0.upkeep(&cupboard) {
            let material = grade_material(grade);
            if holds.0.count_of(material) >= n {
                for _ in 0..n {
                    holds.0.take_one(material);
                }
                grades.push(grade);
            }
        }
        paid.push((cupboard.at, grades));
    }
    let reach = modforge::building::CUPBOARD_REACH;
    let is_paid = |slot: Slot, grade: Grade| {
        paid.iter().any(|(at, grades)| (at.x - slot.x).abs() <= reach && (at.z - slot.z).abs() <= reach && grades.contains(&grade))
    };
    for slot in builds.0.decay(&is_paid) {
        info!("the piece at {slot:?} decayed away");
    }
    for (slot, piece) in builds.0.collapse() {
        info!("the {} at {slot:?} fell", piece.kind.name());
    }
}

/// A piece's boxes, (centre, size), in its own frame: x along a wall,
/// y up from its level's floor, z across.
fn boxes(kind: Kind) -> Vec<(Vec3, Vec3)> {
    let (w, h, t) = (CELL, LEVEL, 0.2);
    match kind {
        Kind::Foundation => vec![(Vec3::new(0.0, FOUNDATION_TOP / 2.0, 0.0), Vec3::new(w, FOUNDATION_TOP, w))],
        Kind::Floor | Kind::Roof => vec![(Vec3::new(0.0, -t / 2.0, 0.0), Vec3::new(w, t, w))],
        Kind::Wall => vec![(Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(w, h, t))],
        // Two posts and a head round a door-wide gap, the head from a
        // house's doorway height up (modforge `DOORWAY_HEIGHT`), a door
        // hung in it (`door_of`).
        Kind::Doorway => vec![
            (Vec3::new(-1.0, h / 2.0, 0.0), Vec3::new(1.0, h, t)),
            (Vec3::new(1.0, h / 2.0, 0.0), Vec3::new(1.0, h, t)),
            (Vec3::new(0.0, (h + DOORWAY_HEIGHT) / 2.0, 0.0), Vec3::new(1.0, h - DOORWAY_HEIGHT, t)),
        ],
        // A wall with a hole at chest height.
        Kind::Window => vec![
            (Vec3::new(0.0, 0.5, 0.0), Vec3::new(w, 1.0, t)),
            (Vec3::new(0.0, h - 0.5, 0.0), Vec3::new(w, 1.0, t)),
            (Vec3::new(-1.0, 1.5, 0.0), Vec3::new(1.0, 1.0, t)),
            (Vec3::new(1.0, 1.5, 0.0), Vec3::new(1.0, 1.0, t)),
        ],
        // A waist-high wall (Atlas's railing).
        Kind::Railing => vec![(Vec3::new(0.0, 0.5, 0.0), Vec3::new(w, 1.0, t))],
        // A post up the middle of the cell (Atlas's pillar).
        Kind::Pillar => vec![(Vec3::new(0.0, h / 2.0, 0.0), Vec3::new(0.4, h, 0.4))],
        // Steps up to the next level, one cell long.
        Kind::Stairs => (0..6).map(|i| {
            let step = h / 6.0;
            (Vec3::new(0.0, step * (i as f32 + 0.5), -w / 2.0 + w / 12.0 + i as f32 * w / 6.0), Vec3::new(w * 0.6, step, w / 6.0))
        }).collect(),
    }
}

/// The house wall model a grade is drawn with, by what it is made of (the
/// one rule for a wall's model, content `wall_model`, keys on colour; a
/// grade's material is its own): twig and wood the wood wall, stone the
/// stone wall, metal and armored the concrete wall (Claude's).
fn wall_of(grade: Grade) -> &'static str {
    match grade {
        Grade::Twig | Grade::Wood => "wall wood",
        Grade::Stone => "wall stone",
        Grade::Metal | Grade::Armored => "wall concrete",
    }
}

/// How one box of a piece is drawn, as a house's same box is
/// (`structure::spawn_structure`): a wall, post or head as the house's
/// wall model, a foundation, floor or roof as its floor tile, a step as its
/// step block, each stretched to the box.
fn drawn_as(kind: Kind, grade: Grade, centre: Vec3, size: Vec3) -> (Model, Transform) {
    let bottom = centre - Vec3::Y * (size.y / 2.0);
    match kind {
        Kind::Foundation | Kind::Floor | Kind::Roof => {
            (Model("floor".to_string()), Transform::from_translation(centre).with_scale(Vec3::new(size.x, size.y / FLOOR_TILE, size.z)))
        }
        Kind::Stairs => (
            Model("furniture".to_string()),
            Transform::from_translation(bottom).with_scale(Vec3::new(size.x / 0.9, size.y / FURNITURE_HEIGHT, size.z / 0.9)),
        ),
        Kind::Wall | Kind::Doorway | Kind::Window | Kind::Railing | Kind::Pillar => {
            (Model(wall_of(grade).to_string()), Transform::from_translation(bottom).with_scale(Vec3::new(size.x, size.y / WALL_HEIGHT, size.z)))
        }
    }
}

/// A doorway's door, as a house's (`structure::spawn_structure`): hung on
/// its hinge at the gap's side, closed across the gap, on the piece's floor.
fn door_of(slot: Slot) -> (Door, Transform) {
    let place = placed(slot, Kind::Doorway);
    let along = (place.rotation * Vec3::X).ground();
    let centre = place.translation.ground();
    let hinge = centre - along * 0.5;
    let door = Door {
        open: false,
        closed_center: centre,
        hinge,
        closed: Door::facing(hinge, centre),
        width: 1.0,
        height: DOORWAY_HEIGHT,
        lying: false,
        model: "door".to_string(),
    };
    let at = Transform::from_translation(on_ground(hinge, place.translation.y)).with_rotation(door.closed);
    (door, at)
}

fn colour(grade: Grade) -> Color {
    match grade {
        Grade::Twig => Color::srgb(0.75, 0.62, 0.38),
        Grade::Wood => Color::srgb(0.52, 0.36, 0.2),
        Grade::Stone => Color::srgb(0.55, 0.55, 0.52),
        Grade::Metal => Color::srgb(0.42, 0.44, 0.48),
        Grade::Armored => Color::srgb(0.25, 0.27, 0.3),
    }
}

/// Where a piece stands and how it is turned.
fn placed(slot: Slot, kind: Kind) -> Transform {
    let [x, y, z] = slot.centre();
    let floor = if kind == Kind::Foundation { y } else { y + FOUNDATION_TOP };
    let turn = if slot.side == Some(modforge::building::Side::East) { Quat::from_rotation_y(std::f32::consts::FRAC_PI_2) } else { Quat::IDENTITY };
    Transform::from_translation(Vec3::new(x, floor, z)).with_rotation(turn)
}

/// Every fixed step: each built piece is in the world, solid and drawn at
/// its grade with the pieces a house is made of (todo Build 1; operator,
/// 2026-11-23: "the same building system that we're using to build these
/// is what the player can use to build and expand his world"), a doorway
/// with its door; one gone or regraded is made again. A doorway's door is
/// a thing of its own, saved as a house's is, so one already hung (from a
/// save) is kept, and it goes with its doorway.
pub fn make(
    mut commands: Commands,
    builds: Res<Builds>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    drawn: Query<(Entity, &Drawn)>,
    doors: Query<(Entity, &Door)>,
) {
    if !builds.is_changed() {
        return;
    }
    let hung = |slot: Slot| {
        let centre = door_of(slot).0.closed_center;
        doors.iter().find(|(_, d)| d.closed_center.distance(centre) < 0.05).map(|(e, _)| e)
    };
    let mut have = std::collections::BTreeMap::new();
    for (entity, d) in &drawn {
        match builds.0.pieces.get(&d.0) {
            Some(piece) if piece.grade == d.1 => {
                have.insert(d.0, entity);
            }
            Some(piece) if piece.kind == Kind::Doorway => commands.entity(entity).despawn(),
            _ => {
                commands.entity(entity).despawn();
                if let Some(door) = hung(d.0) {
                    commands.entity(door).despawn();
                }
            }
        }
    }
    for (slot, piece) in &builds.0.pieces {
        if have.contains_key(slot) {
            continue;
        }
        let parts = boxes(piece.kind);
        let collider = Collider::compound(parts.iter().map(|(c, s)| (*c, Rotation::IDENTITY, Collider::cuboid(s.x, s.y, s.z))).collect());
        let tint = crate::model::Tint(colour(piece.grade));
        commands
            .spawn((Drawn(*slot, piece.grade), placed(*slot, piece.kind), RigidBody::Static, collider, crate::terrain::BlocksSight, Visibility::default()))
            .with_children(|parent| {
                for (centre, size) in &parts {
                    let (model, at) = drawn_as(piece.kind, piece.grade, *centre, *size);
                    parent.spawn((model, tint, at));
                }
            });
        if piece.kind == Kind::Doorway && hung(*slot).is_none() {
            let (door, at) = door_of(*slot);
            commands.spawn((door, Interactable { kind: InteractKind::Door { open: false } }, ids.issue(), at));
        }
    }
}
