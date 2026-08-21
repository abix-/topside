//! The one path from a rolled `World` (modforge::worldgen) to the
//! scene (design.md "The world"): the heightmap as chunk meshes with
//! Avian trimesh colliders, coloured by biome through CelMaterial
//! (one mesh per colour per chunk, as buildings are baked per
//! colour), roads as a darker strip, scatter per biome, and the
//! rolled monument at every site through `spawn_monument`. Cells
//! under a basement are left out of mesh and collider alike, so the
//! hole is walkable. Everything spawned here carries `WorldPiece`
//! so the storm can take it all down and roll again.

use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use modforge::biome::BiomeRegistry;
use modforge::item::ItemRegistry;
use modforge::monument::{MonumentRegistry, BuildingRegistry, footprint};
use modforge::structure::Rgb;
use modforge::worldgen::World;

use crate::cel::CelMaterial;
use crate::structure::{Aabb, spawn_monument};

/// Cells per chunk side.
pub const CHUNK: usize = 32;
/// Cells on either side of a road's centre line that are road.
const ROAD_HALF_WIDTH: usize = 1;
const ROAD_COLOR: Rgb = [0.22, 0.21, 0.2];
const WATER_COLOR: Rgb = [0.2, 0.3, 0.45];

/// Anything the world roll put in the scene: terrain chunks, scatter,
/// monument roots. Not the bunker.
#[derive(Component)]
pub struct WorldPiece;

/// The rolled world as an app resource, for anything that asks the
/// ground height or the biome at a point.
#[derive(Resource)]
pub struct WorldRes(pub World);

/// Build the whole scene from a rolled world. Returns the monument
/// roots spawned at the sites.
pub fn spawn_world(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CelMaterial>,
    world: &World,
    biomes: &BiomeRegistry,
    monuments: &MonumentRegistry,
    buildings: &BuildingRegistry,
    items: &ItemRegistry,
    now: f32,
) -> Result<Vec<Entity>, String> {
    // Roll and spawn the monuments first: their basements cut the
    // ground, and their loot boxes need the item registry.
    let mut holes: Vec<Aabb> = Vec::new();
    let mut roots = Vec::new();
    for (i, site) in world.sites.iter().enumerate() {
        let rolled = monuments.roll(
            &site.monument,
            buildings,
            world.seed ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9),
        )?;
        let ground = world.height_at(site.position);
        let at = Vec3::new(site.position.x, ground, site.position.y);
        for member in &rolled.members {
            if member.structure.rooms.iter().any(|r| r.origin.y < 0.0) {
                let foot = footprint(&member.structure);
                let by = at + member.offset;
                holes.push(Aabb {
                    min: foot.min + by,
                    max: foot.max + by,
                });
            }
        }
        let root = spawn_monument(commands, meshes, materials, items, &rolled, at, now)?;
        commands.entity(root).insert(WorldPiece);
        roots.push(root);
    }

    // Which cells are road.
    let n = world.cells;
    let mut road = vec![false; n * n];
    for r in &world.roads {
        for p in &r.points {
            let (c, row) = world.cell_of(*p);
            for dr in -(ROAD_HALF_WIDTH as isize)..=(ROAD_HALF_WIDTH as isize) {
                for dc in -(ROAD_HALF_WIDTH as isize)..=(ROAD_HALF_WIDTH as isize) {
                    let (cc, rr) = (c as isize + dc, row as isize + dr);
                    if cc >= 0 && rr >= 0 && (cc as usize) < n && (rr as usize) < n {
                        road[rr as usize * n + cc as usize] = true;
                    }
                }
            }
        }
    }
    // Which cells are under a basement.
    let hole_at = |c: usize, r: usize| {
        let p = world.cell_center(c, r);
        let half = world.def.cell / 2.0;
        holes.iter().any(|h| {
            p.x + half > h.min.x && p.x - half < h.max.x && p.y + half > h.min.z && p.y - half < h.max.z
        })
    };

    let chunks = n.div_ceil(CHUNK);
    let mut palette: Vec<(Rgb, Handle<CelMaterial>)> = Vec::new();
    let mut material_for = |rgb: Rgb, materials: &mut Assets<CelMaterial>| -> Handle<CelMaterial> {
        if let Some((_, h)) = palette.iter().find(|(c, _)| *c == rgb) {
            return h.clone();
        }
        let h = materials.add(CelMaterial::from_srgb(rgb[0], rgb[1], rgb[2]));
        palette.push((rgb, h.clone()));
        h
    };
    for cr in 0..chunks {
        for cc in 0..chunks {
            let (c0, r0) = (cc * CHUNK, cr * CHUNK);
            let (c1, r1) = ((c0 + CHUNK).min(n), (r0 + CHUNK).min(n));
            // Cells of this chunk by colour, and the collider over all.
            let mut groups: Vec<(Rgb, Vec<(usize, usize)>)> = Vec::new();
            let mut all: Vec<(usize, usize)> = Vec::new();
            for r in r0..r1 {
                for c in c0..c1 {
                    if hole_at(c, r) {
                        continue;
                    }
                    let rgb = if road[r * n + c] {
                        ROAD_COLOR
                    } else if world.is_water(c, r) {
                        WATER_COLOR
                    } else {
                        biomes.def(world.biome_at_cell(c, r)).map_or([0.3, 0.3, 0.3], |b| b.ground)
                    };
                    match groups.iter_mut().find(|(g, _)| *g == rgb) {
                        Some((_, cells)) => cells.push((c, r)),
                        None => groups.push((rgb, vec![(c, r)])),
                    }
                    all.push((c, r));
                }
            }
            if all.is_empty() {
                continue;
            }
            let (vertices, indices) = cells_mesh(world, &all);
            let collider = Collider::trimesh(vertices, indices);
            let chunk = commands
                .spawn((
                    WorldPiece,
                    RigidBody::Static,
                    collider,
                    Transform::default(),
                    Visibility::default(),
                ))
                .id();
            for (rgb, cells) in groups {
                let (vertices, indices) = cells_mesh(world, &cells);
                let mesh = meshes.add(build_mesh(vertices, indices));
                commands.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material_for(rgb, materials)),
                    Transform::default(),
                    ChildOf(chunk),
                ));
            }
        }
    }

    // Scatter: per cell, each of the biome's kinds stands here with
    // its density, decided by a hash of the cell so the same seed
    // scatters the same. Never on roads, water, or holes.
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    for r in 0..n {
        for c in 0..n {
            if road[r * n + c] || world.is_water(c, r) || hole_at(c, r) {
                continue;
            }
            let Some(biome) = biomes.def(world.biome_at_cell(c, r)) else {
                continue;
            };
            let p = world.cell_center(c, r);
            if p.distance(world.def.bunker) < world.def.bunker_clear
                || world.sites.iter().any(|s| s.position.distance(p) < s.spacing * 0.3)
            {
                continue;
            }
            for (k, spec) in biome.scatter.iter().enumerate() {
                let h = cell_hash(world.seed, c, r, k);
                let roll = (h & 0xFFFF) as f32 / 65535.0;
                if roll >= spec.density {
                    continue;
                }
                let jitter = Vec2::new(
                    ((h >> 16) & 0xFF) as f32 / 255.0 - 0.5,
                    ((h >> 24) & 0xFF) as f32 / 255.0 - 0.5,
                ) * world.def.cell * 0.6;
                let at = p + jitter;
                let y = world.height_at(at) + spec.size.y / 2.0;
                commands.spawn((
                    WorldPiece,
                    RigidBody::Static,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(material_for(spec.color, materials)),
                    Transform::from_xyz(at.x, y, at.y).with_scale(spec.size),
                ));
            }
        }
    }
    Ok(roots)
}

/// Vertices and triangles for a set of cells: four corners per cell
/// at the heightmap's corner heights (the mean of the cells around
/// each corner), two triangles per cell. Corners are shared between
/// neighbouring cells so the surface has no cracks.
fn cells_mesh(world: &World, cells: &[(usize, usize)]) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let n = world.cells;
    let half = world.def.size / 2.0;
    let size = world.def.cell;
    let corner_height = |c: usize, r: usize| -> f32 {
        // Corner (c, r) is the shared point of cells (c-1..c, r-1..r).
        let mut sum = 0.0;
        let mut count = 0.0;
        for (dc, dr) in [(0isize, 0isize), (-1, 0), (0, -1), (-1, -1)] {
            let (cc, rr) = (c as isize + dc, r as isize + dr);
            if cc >= 0 && rr >= 0 && (cc as usize) < n && (rr as usize) < n {
                sum += world.height_at_cell(cc as usize, rr as usize);
                count += 1.0;
            }
        }
        sum / count
    };
    let mut vertices: Vec<Vec3> = Vec::new();
    let mut index_of: std::collections::HashMap<(usize, usize), u32> = Default::default();
    let mut indices = Vec::new();
    let mut corner = |c: usize, r: usize, vertices: &mut Vec<Vec3>| -> u32 {
        *index_of.entry((c, r)).or_insert_with(|| {
            vertices.push(Vec3::new(
                c as f32 * size - half,
                corner_height(c, r),
                r as f32 * size - half,
            ));
            (vertices.len() - 1) as u32
        })
    };
    for &(c, r) in cells {
        let a = corner(c, r, &mut vertices);
        let b = corner(c + 1, r, &mut vertices);
        let d = corner(c, r + 1, &mut vertices);
        let e = corner(c + 1, r + 1, &mut vertices);
        // Counter-clockwise seen from above (+y).
        indices.push([a, d, b]);
        indices.push([b, d, e]);
    }
    (vertices, indices)
}

fn build_mesh(vertices: Vec<Vec3>, indices: Vec<[u32; 3]>) -> Mesh {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.to_array()).collect();
    let flat: Vec<u32> = indices.iter().flatten().copied().collect();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32, 0.0]; vertices.len()]);
    mesh.insert_indices(Indices::U32(flat));
    mesh.compute_smooth_normals();
    mesh
}

fn cell_hash(seed: u64, c: usize, r: usize, k: usize) -> u64 {
    let mut h = seed ^ 0xD6E8_FEB8_6659_FD93;
    for v in [c as u64, r as u64, k as u64] {
        h ^= v.wrapping_add(0x9E37_79B9_7F4A_7C15);
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 31;
    }
    h
}
