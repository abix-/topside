//! The silo (todo 30; authority.md "Silo"): the player's bunker dug
//! downward a level at a time. Anyone of the bunker's group holding a
//! shovel strikes the deepest floor, pays what that level costs (deeper
//! costs more, content `dig_cost`), and the bunker is built again
//! through the one path, `spawn_structure`, from content `silo_bunker`
//! at its new depth. The depth is saved; the bunker standing is rebuilt
//! to it whenever the two differ (a dig, or a load). Rooms on a level
//! are the building pieces (building.rs); a station is set down from
//! its item and becomes the station where it stands.

use bevy::prelude::*;
use modforge::actions::Action;
use modforge::hud::InteractKind;

use crate::actor::{Dead, Hotbar, Inventory, Npc};
use crate::content::{BUNKER_LEVEL, PLAYER_ROOM_FLOOR, SHOVEL, dig_cost, silo_bunker, station_of};
use crate::input::ThisTick;
use crate::interact::{Interactable, WorldItem};

/// How many levels the player's bunker is dug below the player's room
/// (saved).
#[derive(Resource, Default, Clone, Copy, serde::Serialize, serde::Deserialize, Reflect)]
#[reflect(opaque, Resource, Serialize, Deserialize)]
pub struct Silo(pub u32);

/// The player's bunker standing, and the depth it was built to (worked
/// out, never saved).
#[derive(Component, Clone, Copy)]
pub struct PlayerBunker(pub u32);

/// Half the bunker's footprint, x and z, metres (the room's interior).
const HALF: Vec2 = Vec2::new(3.0, 4.0);

/// Every step: a shovel struck on the deepest floor digs the next level.
#[allow(clippy::type_complexity)]
pub fn dig(
    mut silo: ResMut<Silo>,
    world: Option<Res<crate::terrain::WorldRes>>,
    mut people: Query<(&ThisTick, &Transform, &mut Inventory, &Hotbar, Option<&Npc>, Has<crate::player::Player>), Without<Dead>>,
) {
    let Some(world) = world else {
        return;
    };
    for (this_tick, at, mut inventory, hotbar, npc, is_player) in &mut people {
        if hotbar.0.held_stack().is_none_or(|s| s.item != SHOVEL) || !this_tick.0.iter().any(|s| s.action == Action::Attack) {
            continue;
        }
        if crate::research::group_of(is_player, npc) != crate::content::PLAYER_FACTION {
            continue;
        }
        let feet = at.translation.y - crate::player::BODY_CENTRE;
        let deepest = PLAYER_ROOM_FLOOR - silo.0 as f32 * BUNKER_LEVEL;
        let off = (Vec2::new(at.translation.x, at.translation.z) - world.bunker).abs();
        if off.x > HALF.x || off.y > HALF.y || (feet - deepest).abs() > 1.0 {
            continue;
        }
        let (item, n) = dig_cost(silo.0);
        if crate::research::pay(&mut inventory.0, &[(item, n)]) {
            silo.0 += 1;
            info!("dug the silo to {} levels", silo.0);
        }
    }
}

/// Every step: the player's bunker standing is rebuilt to the silo's
/// depth when the two differ.
pub fn rebuild(
    mut commands: Commands,
    silo: Res<Silo>,
    world: Option<Res<crate::terrain::WorldRes>>,
    mut ids: ResMut<crate::interact::ThingIssuer>,
    bunkers: Query<(Entity, &PlayerBunker)>,
) {
    let (Some(world), Ok((entity, built))) = (world, bunkers.single()) else {
        return;
    };
    if built.0 == silo.0 {
        return;
    }
    commands.entity(entity).despawn();
    let root = crate::structure::spawn_structure(&mut commands, &mut ids, &silo_bunker(silo.0), world.bunker, false, Some(crate::structure::BUNKER_DOOR));
    commands.entity(root).insert(PlayerBunker(silo.0));
}

/// A station's item set down becomes that station where it stands.
pub fn set_down(mut placed: Query<(&WorldItem, &mut Interactable), Added<WorldItem>>) {
    for (item, mut use_as) in &mut placed {
        if let Some(kind) = station_of(&item.stack.item) {
            use_as.kind = InteractKind::Station(kind);
        }
    }
}
