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

/// Give every new `Picture` its sprite.
pub fn draw_pictures(mut commands: Commands, assets: Res<AssetServer>, new: Query<(Entity, &Picture), Added<Picture>>) {
    for (entity, picture) in &new {
        commands.entity(entity).with_child((
            Sprite::from_image(assets.load(format!("pictures/{}.png", picture.0))),
            Transform::from_scale(Vec3::splat(1.0 / PIXELS_PER_METRE)),
        ));
    }
}
