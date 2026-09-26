//! The map (authority.md "Presentation"): Diablo 2's automap, drawn over
//! the first person view while the player keeps playing. A minimap in the
//! corner, which the Map action (M) makes a full screen overlay and back.
//! It shows only what the player has seen (their `Map`, life.md "The map
//! each person carries"), north up, the player at its centre, no live
//! people: what blocks the eyes (walls, cliffs, trees) as bright lines,
//! the ground seen as a faint wash of its colour.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use modforge::walk::cell_of;

use crate::Ground as _;
use crate::input::{Action, ActionQueue, ThisTick};
use crate::player::Player;

/// Tiles either side of the player the corner minimap shows, and the
/// full screen one. Tuning.
const CORNER_SPAN: i32 = 48;
const FULL_SPAN: i32 = 160;
/// The corner minimap's size on the screen, in pixels.
const CORNER_PIXELS: f32 = 240.0;
/// Redrawn this often, in seconds.
const REDRAW: f32 = 0.25;

/// The map's picture, whether it is full screen, and what it last drew
/// (for the `view` op).
#[derive(Resource)]
pub struct MapView {
    image: Handle<Image>,
    pub full: bool,
    since: f32,
    pub drawn: MapDrawn,
}

/// What the map last drew: the tile at its centre, how many tiles either
/// side, and how many tiles it showed.
#[derive(Clone, Copy, Debug, Default)]
pub struct MapDrawn {
    pub centre: (i32, i32),
    pub span: i32,
    pub shown: usize,
}

/// The screen the map sits on, and the map itself.
#[derive(Component)]
pub struct MapScreen;
#[derive(Component)]
pub struct MapPicture;

fn blank(span: i32) -> Image {
    let side = (2 * span + 1) as u32;
    Image::new_fill(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    )
}

/// The map in the corner, over everything, taking no clicks.
pub fn setup_map(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(blank(CORNER_SPAN));
    commands
        .spawn((
            MapScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::End,
                align_items: AlignItems::Start,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|screen| {
            screen.spawn((
                MapPicture,
                ImageNode::new(image.clone()),
                Node {
                    width: Val::Px(CORNER_PIXELS),
                    height: Val::Px(CORNER_PIXELS),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                Pickable::IGNORE,
            ));
        });
    commands.insert_resource(MapView {
        image,
        full: false,
        since: REDRAW,
        drawn: MapDrawn::default(),
    });
}

/// The player's Map action this step turns the map full screen, or back
/// to the corner.
pub fn toggle_map(mut view: ResMut<MapView>, players: Query<&ThisTick, With<Player>>) {
    if players.iter().any(|t| ActionQueue::has(&t.0, Action::Map)) {
        view.full = !view.full;
        view.since = REDRAW;
    }
}

/// Every `REDRAW`, the map drawn from the player's map around them.
pub fn draw_map(
    time: Res<Time>,
    mut view: ResMut<MapView>,
    mut images: ResMut<Assets<Image>>,
    mut walk: ResMut<crate::nav::Walk>,
    chunks: Option<Res<crate::terrain::Chunks>>,
    mut laps: ResMut<crate::profile::Laps>,
    players: Query<(&Transform, &crate::actor::Map), With<Player>>,
    mut screens: Query<&mut Node, (With<MapScreen>, Without<MapPicture>)>,
    mut pictures: Query<&mut Node, With<MapPicture>>,
) {
    view.since += time.delta_secs();
    let (Some(chunks), Ok((at, map))) = (chunks, players.single()) else {
        return;
    };
    if view.since < REDRAW {
        return;
    }
    view.since = 0.0;
    let started = std::time::Instant::now();
    let span = if view.full { FULL_SPAN } else { CORNER_SPAN };
    let centre = cell_of(at.translation.ground());
    let mut image = blank(span);
    let side = 2 * span + 1;
    let mut shown = 0;
    if let Some(pixels) = image.data.as_mut() {
        for row in 0..side {
            for col in 0..side {
                // North up: the top row is the farthest along +y.
                let tile = (centre.0 + col - span, centre.1 + span - row);
                if !map.map.seen(tile) {
                    continue;
                }
                shown += 1;
                let ground = chunks.ground_at(tile);
                let wall = walk.0.blocked(tile) && ground != Some(modforge::ground::Ground::Water);
                let rgba: [u8; 4] = if wall {
                    [220, 215, 200, 240]
                } else {
                    let [r, g, b] = ground.map_or([0.3, 0.3, 0.3], crate::content::ground_color);
                    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 110]
                };
                let i = ((row * side + col) * 4) as usize;
                pixels[i..i + 4].copy_from_slice(&rgba);
            }
        }
        // The player, at the centre.
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let i = (((span + dy) * side + span + dx) * 4) as usize;
            pixels[i..i + 4].copy_from_slice(&[230, 40, 30, 255]);
        }
    }
    if let Some(mut old) = images.get_mut(&view.image) {
        *old = image;
    }
    let size = if view.full { Val::Vh(90.0) } else { Val::Px(CORNER_PIXELS) };
    for mut node in &mut pictures {
        node.width = size;
        node.height = size;
    }
    for mut screen in &mut screens {
        let (along, across) = if view.full {
            (JustifyContent::Center, AlignItems::Center)
        } else {
            (JustifyContent::End, AlignItems::Start)
        };
        screen.justify_content = along;
        screen.align_items = across;
    }
    view.drawn = MapDrawn { centre, span, shown };
    laps.record("map drawing", started.elapsed().as_secs_f64() * 1000.0);
}
