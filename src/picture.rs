//! Pictures (docs/references.md "Graphics, how Factorio does it"): every
//! model rendered ahead of time by scripts/models.py into assets/pictures,
//! drawn in the world. Anything drawn as a picture carries `Picture`; the
//! one system here draws it (registered with a window only; headless has
//! no pictures) on a child sprite so the thing's own body and collider
//! keep their size.

use bevy::prelude::*;

/// Pixels to a metre in every picture scripts/models.py renders (its
/// PIXELS_PER_METRE): a picture drawn at this scale is its model's size.
pub const PIXELS_PER_METRE: f32 = 64.0;

/// Draw this thing as the picture of this name (assets/pictures/<name>.png),
/// its ground point at the picture's centre.
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct Picture(pub String);

/// Facings a person is rendered in (scripts/models.py FACINGS): 0 east,
/// then a turn of 45 degrees counter-clockwise each, as `Facing` turns.
pub const FACINGS: usize = 8;

/// A person drawn as picture layers, bottom to top (design.md "What 2D
/// changes": built from pieces), each with its tint; each layer's picture
/// is `<layer> <facing>`.
#[derive(Component)]
pub struct Person(pub Vec<(String, Color)>);

impl Person {
    /// The layers of a rolled look: the body in its skin, then each piece
    /// in its hair colour.
    pub fn of(look: &modforge::actor::Look) -> Person {
        let tint = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
        let body = (format!("person {} body", look.body), tint(look.skin));
        let pieces = look.pieces.iter().map(|p| (format!("person {} {p}", look.body), tint(look.hair)));
        Person(std::iter::once(body).chain(pieces).collect())
    }
}

/// One of a person's layers, and the facing its picture shows now.
#[derive(Component)]
pub struct Layer {
    name: String,
    facing: usize,
}

/// Which facing's picture shows a person turned to `angle` (radians,
/// 0 east, counter-clockwise).
pub fn facing_index(angle: f32) -> usize {
    let step = std::f32::consts::TAU / FACINGS as f32;
    ((angle / step).round() as i64).rem_euclid(FACINGS as i64) as usize
}

fn layer_picture(name: &str, facing: usize) -> String {
    format!("pictures/{name} {facing}.png")
}

/// Give every new `Person` a child sprite per layer, and one for what
/// they hold.
pub fn draw_people(mut commands: Commands, assets: Res<AssetServer>, new: Query<(Entity, &Person), Added<Person>>) {
    for (entity, person) in &new {
        commands.entity(entity).with_children(|p| {
            for (i, (name, tint)) in person.0.iter().enumerate() {
                p.spawn((
                    Layer { name: name.clone(), facing: 0 },
                    Sprite {
                        color: *tint,
                        ..Sprite::from_image(assets.load(layer_picture(name, 0)))
                    },
                    // Above the marker under their feet, each layer over the last.
                    Transform::from_xyz(0.0, 0.0, 0.01 * (i + 1) as f32).with_scale(Vec3::splat(1.0 / PIXELS_PER_METRE)),
                ));
            }
            p.spawn((Held(None), Sprite::default(), Visibility::Hidden, Transform::from_scale(Vec3::splat(1.0 / PIXELS_PER_METRE))));
        });
    }
}

/// What a person holds, drawn as its picture at their hand, pointing where
/// they face (design.md: the weapon is one picture turned to the aim); the
/// picture it shows now.
#[derive(Component)]
pub struct Held(Option<String>);

/// How far in front of a person their hand holds an item, in metres.
const HAND_REACH: f32 = 0.35;

/// Every person's hand shows the picture of the item they hold, or nothing.
/// The person is turned to their facing, so the hand is straight ahead of
/// them; it is behind the body when they face away from the camera.
pub fn draw_held(
    assets: Res<AssetServer>,
    registry: Res<crate::content::ItemRegistryRes>,
    people: Query<(&crate::actor::Facing, &crate::actor::Hotbar, &Children), With<Person>>,
    mut hands: Query<(&mut Held, &mut Sprite, &mut Visibility, &mut Transform), Without<Person>>,
) {
    for (facing, hotbar, children) in &people {
        let picture = hotbar
            .0
            .held_stack()
            .and_then(|stack| registry.0.def(&stack.item))
            .and_then(|def| def.picture.clone());
        let away = (1..=3).contains(&facing_index(facing.0));
        for child in children.iter() {
            let Ok((mut held, mut sprite, mut shown, mut at)) = hands.get_mut(child) else {
                continue;
            };
            at.translation = Vec3::new(HAND_REACH, 0.0, if away { 0.005 } else { 0.05 });
            if held.0 != picture {
                match &picture {
                    Some(name) => {
                        sprite.image = assets.load(format!("pictures/{name}.png"));
                        *shown = Visibility::Inherited;
                    }
                    None => *shown = Visibility::Hidden,
                }
                held.0 = picture.clone();
            }
        }
    }
}

/// Every person's layers show the picture for where they face, upright
/// however the person itself is turned.
pub fn turn_people(
    assets: Res<AssetServer>,
    people: Query<(&crate::actor::Facing, &Transform, &Children), With<Person>>,
    mut layers: Query<(&mut Layer, &mut Sprite, &mut Transform), Without<Person>>,
) {
    for (facing, transform, children) in &people {
        let k = facing_index(facing.0);
        for child in children.iter() {
            let Ok((mut layer, mut sprite, mut at)) = layers.get_mut(child) else {
                continue;
            };
            at.rotation = transform.rotation.inverse();
            if layer.facing != k {
                layer.facing = k;
                sprite.image = assets.load(layer_picture(&layer.name, k));
            }
        }
    }
}

/// Give every new `Picture` its sprite.
pub fn draw_pictures(mut commands: Commands, assets: Res<AssetServer>, new: Query<(Entity, &Picture), Added<Picture>>) {
    for (entity, picture) in &new {
        commands.entity(entity).with_child((
            Sprite::from_image(assets.load(format!("pictures/{}.png", picture.0))),
            Transform::from_scale(Vec3::splat(1.0 / PIXELS_PER_METRE)),
        ));
    }
}
