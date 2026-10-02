# How people find their way

**Authoritative for:** how every NPC gets from where they are to where
the brain sends them (the player walks where the keys say): the
walk grid, the way round, what a plan may cost, and doors on the way.
The brain decides WHERE to go (life.md); this doc decides HOW they get
there. Decisions land here in plain words as they are made.

## Why this doc exists (2026-09-25)

The first way-round search (topside `nav.rs`) asked the physics world
about every cell during every plan and remembered nothing, walked a
straight line by a physics cast every tick, and had no limit on how many
people planned at once. In the real game (tests/game.rs) its cost climbed
from 2.7 ms a step at the start to 7.8 ms after two minutes, still
rising, as more people went out on long trips. The fix is the design
Endless already proved at scale, not a new one.

## A navigation mesh: rerecast and landmass (operator, 2026-10-01)

People find their way on a navigation mesh, the way Unity and Unreal do,
replacing the walk grid and its search below. The world now has levels
(bunkers going down, buildings going up, stairs between them), which a
flat tile grid cannot see.

- The mesh is built by Recast, the generator Unity, Unreal and Godot use,
  in its Rust port: `bevy_rerecast`, from our Avian colliders through
  `avian_rerecast` (walls, floors, stairs, doors are already colliders).
  Stairs come out as walkable slopes and every level as its own surface,
  so people go up and down without anything more.
- People walk it with `bevy_landmass`: finding the way, straightening it,
  steering, and (once people are its characters) keeping out of each
  other's way; `landmass_rerecast` joins the two.
- What stays: the brain decides where to go (life.md); commands, following
  and leading push the same aim and move. Only how the way is found and
  walked changes.
- One mesh per chunk, joined at the edges into one connected world
  (operator, 2026-10-01): the world is already made in chunks (32 by 32
  tiles, 32 m), so the mesh is too. Each chunk's mesh is built when the
  chunk is made (bevy_rerecast's `aabb`, the chunk's column from its
  deepest level to its highest) and is its own island in landmass, which
  links islands along their shared edges by itself (landmass
  `create_boundary_links`), so a way runs across the whole world. A chunk
  that changes (a building stands in it, the storm shifts the world) is
  built again alone. Prior art: Factorio (Friday Facts #317: each 32 by
  32 chunk's connected areas, joined only at the chunk's edge tiles,
  redone only where the world changed) and Unreal's tiled Recast mesh.
- Built (src/navmesh.rs) and proved on the running game by
  tests/navmesh.rs (2026-10-01): every chunk made has its mesh (1,156,
  none waiting); a way runs from the player's bunker to the bunker 300 m
  east across the chunk edges; a way runs from the bunker's entrance down
  the stairwell to the player's room, every piece joined, through a
  building that stands across chunk edges. Each chunk is a tile of
  Recast's tiled mesh (its box padded by Recast's border), its colliders
  only those reaching into it; cells 0.133 m (at 0.2 m a stair's
  neighbouring cells can be two steps apart, over the climb, and the
  flights were lost). The ground is a slab per chunk (terrain), every
  floor below the ground has its slab, and doors are left out of the mesh
  (people open them).
- People walk it (2026-10-02): the brain, commands, following and leading
  push one action, Go to a point in 3D, its height included (modforge
  `Action::Go`); `navmesh::walk` makes the person a landmass agent with
  that target and turns landmass's desired velocity into the Aim and Move
  every walker already used, opening a closed door ahead with Use. The
  walk grid and `nav::steer` no longer walk anyone; the grid stays only as
  the wall map for sight maps and the minimap. Everything that keeps a
  place keeps it in 3D: memory, home, asked, leading, the episode's
  things, where a person respawns. Proved on the running game by
  tests/command.rs (2026-10-02): a command walked the player from their
  room 6 m down up the stairwell to the ground (feet -6.00 to 0.00).
- People in the way (2026-10-02): every living person has a landmass
  stand-in at their feet (`navmesh::Walker`, its own entity, because
  landmass reads a place from its entity and every point asked of the mesh
  is at someone's feet): an agent while they walk, a character while they
  stand, so walkers steer round everyone. Only walkers are agents: two
  people standing on one spot, as agents, gave landmass's steering no way
  between them and the game stopped (`dodgy_2d`, a NaN). landmass's
  steering does not part two people already touching, nor get anyone past
  someone in a 1.2 m doorway, so (Bethesda's people step aside, RimWorld's
  make way) people standing step out of a doorway they stand in the way
  of, and sideways out of the way of anyone walking into them; a walker
  stuck half a second with someone right ahead steps aside for a second
  and goes on. landmass's own steering round people and the mesh's edges
  (dodgy_2d) is off (`neighbourhood: 0.0`): it stopped the game now and
  then with a NaN in its solving. Open (todo 13a3): two people meeting
  head-on on one flight get stuck without it.
- Checked first (2026-10-01, GitHub and crates.io): bevy_rerecast 0.5
  (Bevy 0.19), avian_rerecast 0.6 (Avian 0.7), bevy_landmass 0.12.1
  (Bevy 0.19), landmass_rerecast 0.3. Set aside: vleue_navigator (floors
  as layers stitched by hand, not Recast), oxidized_navigation (last
  pushed before Bevy 0.19), modforge `route.rs` (follows a host game's
  paths, builds none).

The sections below are the grid design this replaces, kept until the mesh
has replaced it everywhere.

## What we take from Endless (agreed 2026-09-25)

Endless (`C:\code\endless\rust\src\systems\pathfinding.rs` and
`movement.rs`) ran pathfinding for its whole population. We port its
design, built once in modforge (the DRY rule), on modforge's one A*
(`path::grid_path`):

1. **A walk grid, built once, not asked of physics per plan.** One number
   per cell: blocked, or what a step there costs. Built from the world
   (water, scatter, every building's walls) and kept until the world
   changes (the storm rolls a new one, a building changes). Planning then
   reads plain numbers.
2. **Doors are walkable.** A person opens a closed door on the way with
   the player's own Use.
3. **Short trips go straight.** A clear straight line on the grid (the
   cells along it, Bresenham, Endless's `line_of_sight`) means no search.
   No physics cast every tick.
4. **One queue of plans with a budget per frame.** Every person who needs
   a way asks the one queue; each frame works through it up to a count
   and a time limit (Endless: 200 a frame, 2 ms), and the rest wait for
   the next frame. A crowd can never blow the frame.
5. **A failed search waits.** A person whose way cannot be found waits
   (Endless: 2 s) before asking again, and counts as blocked.
6. **Long trips search a small graph (HPA*).** The grid is cut into
   chunks; the ways between chunk entrances are worked out once and
   cached; a long trip searches the entrances, not every cell, and the
   cached ways are stitched into the full way. When a building changes,
   only its chunks and their neighbours are rebuilt, and only people
   whose way crosses them plan again.

topside's part is only to paint the grid from its colliders and to ask
the queue for ways; the grid, the search, the queue, and the budget live
in modforge and are tested without the game.

## One grid: tiles and chunks (operator, 2026-09-25)

There is one grid, the Factorio way: a tile is 1 by 1 m, a chunk is 32
by 32 tiles, and it is the same grid for everything: the ground, the
buildings, and pathing. All 1:1.

- Buildings sit on the tiles: every wall fills whole tiles, every door
  is a whole number of tiles wide, every building snaps to the tile
  grid. A wall is to us what a belt is to Factorio: 1 by 1 tile.
- The walk grid is the tiles: a tile is blocked or open, and chunks are
  painted as they are needed.
- No finer grid for pathing. The 0.5 m walk grid built first was a
  workaround for walls and doors that were not on the tiles; it goes.

## Never head for a place nobody can stand on (operator, 2026-09-25)

The tiles say what is standable. Whoever picks a place to go (a stroll,
heading out to look, a box to open) picks a tile a person can stand on,
or a thing standing on one. A stroll into a tree, a lake, or a wall is a
bug, never something the search finds out the hard way. (The real game
showed it: every failed search in two minutes was a stroll, half of
them to a blocked tile.)

## Search by chunk, then across chunks (operator, 2026-09-25)

A search is never bigger than one chunk at a time (Factorio: "the first
stage is chunk-based"; Endless's HPA*):

- Start and goal in one chunk: search that chunk only, at most 32 by 32
  tiles.
- Otherwise search across chunks first. Each chunk knows its doorways:
  the open tiles along each edge that meet open tiles in the neighbour,
  and the ways between its own doorways inside it, worked out once when
  the chunk is painted and again when it changes. The search runs over
  those doorways, a small search; the ways inside each chunk are already
  known and are joined into the whole way.
- No way across the chunks means the place cannot be reached, known at
  once, with no search of every tile around.
