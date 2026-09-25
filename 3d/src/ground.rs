//! The flat ground until terrain lands: tiles of solid slab with
//! colliders, leaving holes under every building footprint that
//! goes below ground so basements are walkable. The tiles around a
//! hole are split to the footprint's edge, so no gap shows.

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::cel::CelMaterial;
use crate::structure::Aabb;

pub const HALF_EXTENT: f32 = 600.0;
const TILE: f32 = 40.0;
const THICKNESS: f32 = 0.2;

/// One ground tile piece.
#[derive(Component)]
pub struct Ground;

/// Spawn the ground as tiles, cut around `holes` (ground-plane
/// footprints, y ignored). One shared mesh and material.
pub fn spawn_ground(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    holes: &[Aabb],
) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = materials.add(CelMaterial::from_srgb(0.35, 0.40, 0.30));
    let tiles = (2.0 * HALF_EXTENT / TILE).ceil() as i32;
    for i in 0..tiles {
        for j in 0..tiles {
            let min = Vec2::new(-HALF_EXTENT + i as f32 * TILE, -HALF_EXTENT + j as f32 * TILE);
            let tile = Rect::from_corners(min, min + Vec2::splat(TILE));
            for piece in cut(tile, holes) {
                let size = piece.size();
                if size.x < 1e-3 || size.y < 1e-3 {
                    continue;
                }
                let centre = piece.center();
                commands.spawn((
                    Ground,
                    RigidBody::Static,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(centre.x, -THICKNESS / 2.0, centre.y)
                        .with_scale(Vec3::new(size.x, THICKNESS, size.y)),
                ));
            }
        }
    }
}

/// `tile` minus every hole, as rectangles. Each hole that crosses a
/// piece splits it into the strips west, east, south, and north of
/// the hole.
pub fn cut(tile: Rect, holes: &[Aabb]) -> Vec<Rect> {
    let mut pieces = vec![tile];
    for hole in holes {
        let h = Rect::from_corners(
            Vec2::new(hole.min.x, hole.min.z),
            Vec2::new(hole.max.x, hole.max.z),
        );
        let mut next = Vec::new();
        for piece in pieces {
            let overlap = piece.intersect(h);
            if overlap.is_empty() {
                next.push(piece);
                continue;
            }
            // West and east strips span the piece's full depth; the
            // south and north strips fill between them.
            if overlap.min.x > piece.min.x {
                next.push(Rect::new(piece.min.x, piece.min.y, overlap.min.x, piece.max.y));
            }
            if overlap.max.x < piece.max.x {
                next.push(Rect::new(overlap.max.x, piece.min.y, piece.max.x, piece.max.y));
            }
            if overlap.min.y > piece.min.y {
                next.push(Rect::new(overlap.min.x, piece.min.y, overlap.max.x, overlap.min.y));
            }
            if overlap.max.y < piece.max.y {
                next.push(Rect::new(overlap.min.x, overlap.max.y, overlap.max.x, piece.max.y));
            }
        }
        pieces = next;
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hole_inside_a_tile_leaves_four_strips_that_cover_the_rest() {
        let tile = Rect::new(0.0, 0.0, 40.0, 40.0);
        let hole = Aabb {
            min: Vec3::new(10.0, -30.0, 12.0),
            max: Vec3::new(20.0, 0.0, 22.0),
        };
        let pieces = cut(tile, &[hole]);
        assert_eq!(pieces.len(), 4);
        let area: f32 = pieces.iter().map(|p| p.size().x * p.size().y).sum();
        assert!((area - (1600.0 - 100.0)).abs() < 1e-3);
        for piece in &pieces {
            assert!(piece
                .intersect(Rect::new(10.0, 12.0, 20.0, 22.0))
                .is_empty());
        }
    }

    #[test]
    fn a_hole_outside_leaves_the_tile_whole() {
        let tile = Rect::new(0.0, 0.0, 40.0, 40.0);
        let hole = Aabb {
            min: Vec3::new(50.0, -3.0, 0.0),
            max: Vec3::new(60.0, 0.0, 10.0),
        };
        assert_eq!(cut(tile, &[hole]).len(), 1);
    }
}
