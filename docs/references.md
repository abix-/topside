# topside references

The games we mine for ideas, from the design discussion of 2026-08-19.
Each row says what topside takes from it and what it deliberately avoids.
design.md will draw from this list.

| Game | Mine | Avoid |
|---|---|---|
| MISERY | Base expansion in a post-apocalyptic world, fighting for survival. The feel we loved first. | Hand-authored content that runs dry: 4 generators, identical repeating patterns; the loop is felt out and there is nothing else. |
| Survivalist: Invisible Strain | The substrate our own survivalist-mod proved out: settlements, recruits, zombie apocalypse survival. | Too static without the mod; content instead of simulation. |
| Survivalist (2013) | The original, and amazing: leading a growing community after the collapse, recruiting survivors with their own personalities, dealing with factions through trade, diplomacy, or force. Solo-dev proof of the community-leadership fantasy. | Top-down isometric distance from the world; scripted story beats that end; a world that waits for the player. |
| Fallout Shelter | The pull of building up a shelter and its people; the bunker as home base. | Shallow loop, no world to be embodied in. |
| The Bloodline | The shell: solo-dev, first person, your town that you build and recruit for, a world to explore. Proof the shape works at indie scale. Also the reference for first-person melee combat feel: weighty, directional, satisfying even with crude weapons (operator-locked 2026-08-20: "think about how bloodlines did combat too. its a lot of fun if done properly"). | Too static: the world only does what the developer scripted. Topside puts a living simulation behind the same shell. |
| Valheim | You FEEL survival because you are first person in 3D. Low poly carrying atmosphere through lighting and sound. Indie-scale proof of embodied survival plus base building. | World does not act on its own; content gates (biome bosses) are finite. |
| STALKER (A-Life) | A living faction simulation experienced from first person: squads travel, fight, and take territory whether or not you are there. The closest prior art to topside's heart. | The world state resets and forgets too much; factions do not build. |
| RimWorld | The storyteller model (already operator-locked in the survivalist design): a base simulation plus a director pacing drama. Randy Random by name. | Top-down spectating; we go embodied instead. |
| Dwarf Fortress | Systems that interact instead of content that repeats; stories the developer never authored. | Presentation and accessibility. |
| Mount & Blade | A faction war layer that runs without the player, experienced from inside one body. | Nothing major; scale is the difference. |
| Kenshi | A brutal open world that does not care about you; squad survival between living factions. | Clunky control layer between the player and the world. |
| Project Zomboid | Zombie apocalypse survival depth (needs, injuries, seasons); "this is how you died" honesty. | Isometric distance from the world; largely static world state. |
| Diablo 2 / Path of Exile | Loot randomization: affix rolls, quality tiers, the chase for a specific drop. PoE's lesson: systems multiplying against each other beat a bigger random list. | A memorized slot machine: once you know the pool, randomization alone stops surprising. |
| Tarkov (extraction shooters) | Risk of losing what you carry makes every trip out tense; extraction as the rhythm of leaving safety and coming back. | Multiplayer-dependent tension; topside's danger comes from the simulation instead. |
| Rust | Raids. Monuments as loot destinations with tiered danger (card rooms, Oil Rig with heavies and crates, underwater lab, Dome). Special timed events like Cargo. We do not have to be unique, just take good ideas and implement them well. Topside goes more extreme on randomization: monument layouts AND names are random. | Fixed monument identities you memorize; multiplayer-dependent danger. |
| Shadow of Mordor (nemesis system) | Enemies that remember you, survive, and come back promoted: content generated from the player's own history. | Nothing major; the idea, not the implementation. |
| Incremental games | The always-more-to-buy pull: unbounded upgrade tracks, diminishing returns, rising costs. Already designed in the survivalist mod's settlement upgrades. | Numbers replacing the world; upgrades must stay physical and visible. |
| State of Decay | The facility/upgrade model the survivalist-mod settlement upgrades were patterned on (operator: more complex in types and levels). | Timer-driven busywork. |
| Soulmask | The model for NPC control: recruited people with their own skills and traits, assigned to work stations and duties they run autonomously, guard and follow roles, companions in the field. First-person-compatible crew management without micromanagement. | Nothing named yet; mined for the control scheme. |
| Atlas | The survival pressure model: thirst, hunger, temperature, vitamins, weather effects all stacking, never letting up. 2000 hours of proof that layered survival pressure keeps a game dangerous long after the early game. The reference for how topside's survival systems should feel. | Vitamin system complexity; server instability; shallow endgame beyond survival. |
| Oregon Trail | The operator's frame for the whole game: Oregon Trail meets post-apocalyptic 3D. Attrition IS the game: rationing, sickness, weather, hazards that just hit you, named companions dying along the way. | Nothing major; the frame, not the format. |
| Tales of Maj'Eyal (operator, 2026-08-22: "does a great job of procedural generation without being samey") | Six things, read from its zone files (te4.org wiki, T4 Modules Howto Guide/Zones): (1) a different generator per zone, never one generator tuned many ways (Roomer, Cavern, Forest, Maze, Building, Town, Static), our "generators VERY different" rule; (2) random layout, authored pieces: the `rooms` list names hand-made room files and vaults the generator drops into the rolled layout, so the player meets things a person designed in places nobody designed; (3) each zone carries its own monster and object lists with level ranges; (4) fixed and rolled mixed per level (`levels` overrides one floor with a static map); (5) randomized uniques: bosses that steal class talents, random artifacts with rolled affixes; (6) per-level random events rolled on entry. | Turn-based tiles, the class and talent tree as the replay hook; we replay through the world, not the build. |

## Combat, read from source (2026-08-20)

Cloned next to the repos and read, not summarised from memory:
`C:\code\Quake-III-Arena` (GPL), `C:\code\halflife` (Valve SDK),
`C:\code\DOOM-3` (GPL). Rust is closed; its rules come from
Facepunch's devblogs 74, 75, 117, 123.

| Source | Mine | Avoid |
|---|---|---|
| Quake 3 `g_weapon.c`, `g_combat.c`, `g_missile.c`, `bg_pmove.c` | One damage function for every hit: `G_Damage(target, inflictor, attacker, dir, point, damage, flags, means_of_death)`: armor absorbs a share, knockback is damage over mass added to velocity, then health, then `die` or `pain`. Fire is a trace from the eye along the view; melee (gauntlet) is a 32 unit trace. Projectiles trace their own path every step from last position to new. Fire rate is a timer on the shooter in the shared movement code (`PM_Weapon`): ready only when the timer is zero, no ammo costs a delay. Shotgun spread is seeded so the client predicts the same pattern. | Damage numbers hardcoded in C per weapon. No damage types, no hit location. |
| Half-Life 1 `combat.cpp`, `weapons.cpp`, `crowbar.cpp` | Melee traces a line, and if it misses, sweeps a fat hull and takes that hit (`Swing`). Pellets accumulate per target through a multi-damage register and land as one `TakeDamage` call. Hitgroups (head, chest, stomach, arms, legs) multiply damage per difficulty. Damage type bits (club, bullet, burn, blast) or'd onto the hit drive reactions. | Global accumulator state; monsters had no armor. |
| Doom 3 `Actor.cpp`, `Player.cpp`, `Weapon.cpp`, `Projectile.cpp` | Damage as data: `Damage(inflictor, attacker, dir, damageDefName, scale, location)` reads `damage`, `push`, `kickDir`, `noArmor`, `noGod`, `selfDamageScale`, `gib` from a def. Weapons and projectiles name their def. Armor saves `ceil(damage * protection)` and always leaves 1. | Script-driven weapon state; not needed. |
| Rust (Facepunch devblogs) | The hybrid bullet: "the bullet immediately travels a set number of meters (this is ammo dependent), and then it carries on as normal", so "close quarters gunplay is effectively hitscan, but long range retains the effects of gravity" (117). Damage falloff by distance per weapon class (123). Protection per body area from worn clothing instead of hitboxes that armor resizes; per-weapon body part multipliers; "best-fit hit detection" (117). Penetration power versus surface resistance, damage scaled by remaining projectile integrity (75). | Client-side projectiles with server verification (no network here). Penetration and ricochet, not decided; the reason to wait, no body parts, is gone: hits land by body part and armor protects the part it is worn on (combat.md, 2026-09-27). |
| Valheim (wiki) | Damage types slash, pierce, blunt, fire, frost, lightning, poison, spirit; per-creature resistances as multipliers; stagger, block, parry; weapon arcs as swept shapes. | Swept arcs until weapons differ by arc; the ray plus fat retry covers it. |
| Source (Valve wiki) | The simulation decides hits, never the renderer. Lag compensation rewinds other players' hitboxes to the shooter's time. | The rewind itself: single player. |

## Input, read from source (2026-08-20)

| Source | Mine | Avoid |
|---|---|---|
| Quake 3 `common.c` (EVENTS AND JOURNALING), `win_main.c`, `cl_keys.c`, `cl_input.c` | Every input is one record, `sysEvent_t`: time, type, two values, optional blob; keys, chars, mouse, console lines, and packets alike. The platform only produces them (`Sys_QueEvent` into a ring); the engine only consumes them in one loop (`Com_EventLoop`), once per frame before simulation. A binding table maps keys to actions (`bind MOUSE1 +attack`), so no game system reads a key. `Com_Milliseconds` takes "now" from the last event's timestamp. Journaling: with `journal 1` every event is written to disk as fetched, with `journal 2` read back instead of the OS, replaying a whole session bit for bit. | Text commands as the action format; the console as the binding target. |

## Graphics, how Factorio does it (2026-09-26)

Checked against Factorio's own posts and modding docs. Each point names
its source; "forum" means players, not Wube, said it.

Factorio never draws anything 3D in the game. Every object is a 3D model
rendered ahead of time into flat pictures; the game only stacks pictures.

The pictures:

- Modelled in Blender, rendered to pictures. "All objects in the game
  (including the terrain) have been rendered from 3D models" (FFF 227);
  "Blender is a great software which allows us to have a very flexible
  workflow" (FFF 146). Every animation frame and every facing is its own
  picture on a sprite sheet.
- One camera angle: 45 degrees down, but the tiles stay square. "Our
  'camera angle' is 45 degrees, which in 'real projection' would result in
  rectangular tiles, but in Factorio this is contradicted by our tiles
  being square" (FFF 269). The render camera is orthographic (forum).
  Things are drawn to look right from that angle, not to be true models.
- One light for everything, from the left at 45 degrees, so shadows fall
  to the right (forum; FFF 269 shows the shadows but does not state the
  angle).
- Shadows are separate pictures. The shadow is worked out from the 3D
  model when rendering, so the game cannot make shadows live; trees used
  to carry their shadow in the same picture and were split (FFF 227). A
  picture marked `draw_as_shadow` is drawn as a shadow (lua-api
  SpriteParameters).
- Resolution: 32 pixels per tile is "the usual pixel height/width of 1
  tile in normal resolution" (lua-api SpriteParameters). High resolution
  pictures are 64 pixels per tile, drawn at half scale (forum).
- One entity is many layers put back together in the game. "A single
  entity in Factorio is pretty complex, rendered in multiple layers, and
  assembled again in the engine" (FFF 146), with hand-drawn masks and
  additive blending. Each layer can be marked `draw_as_shadow`,
  `draw_as_glow` ("draws first as a normal sprite, then again as a light
  layer") or `draw_as_light`, and `apply_runtime_tint` tints it with the
  player's colour (lua-api SpriteParameters).

The ground:

- Tiles with variations. Each ground type has several pictures, and
  pictures covering 1x1 up to bigger squares of tiles (powers of 2 up to
  128), picked by probability, so no pattern repeats visibly (lua-api
  TileMainPictures).
- Transition pictures at edges. Each tile has `transitions` to named
  other tiles and `transitions_between_transitions`, and a `layer` number
  deciding which tile draws its edge over the other (lua-api
  TilePrototype).
- Decoratives: small rocks, grass and cracks drawn on the ground, their
  own render layer (lua-api RenderLayer "decorative").

At run time:

- Everything draws in a fixed list of render layers, bottom to top:
  ground and its transitions, decals, decoratives, remnants, floor,
  belts, corpses, items, objects, higher objects, wires, explosions,
  projectiles, smoke, things in the air, the light effect, then the
  cursor (lua-api RenderLayer, 72 layers).
- NOT CONFIRMED: that objects on the same layer are sorted by y (lower
  on screen drawn in front). The RenderLayer docs say nothing about
  sorting inside a layer.
- Night: layers marked `draw_as_light` go to a light layer; a "light-effect"
  render layer exists (lua-api). How the darkening is done was not
  checked.

People, built from pieces (2026-09-26, not read from source, from what
the games are known to do):

- Factorio's character is not built from pieces: each armour tier swaps
  the whole picture set, and one mask is tinted with the player's colour.
- RimWorld draws a pawn per facing (south, east, north; west is east
  mirrored) as layers: body by body type, head, hair, then apparel in
  layer order (on skin, middle, shell, headgear). Each apparel def names
  its pictures; skin and hair come from grey pictures tinted in the game.
  The weapon is one picture drawn at the pawn, turned to the aim angle.
- Diablo 2 built its units from pieces (head, torso, legs, arms, weapon,
  shield), each rendered per direction and per animation frame, with a
  per-direction table of which piece draws over which.

What topside has today: the ground as `TilemapChunk`, actors and
buildings as sprites. Missing: the rendered pictures, one camera angle and
light, ground variations and edge pictures, the render layers, the night
light layer. Topside takes no shadows (design.md, 2026-09-26).

Sources: factorio.com/blog/post/fff-146, fff-227, fff-269;
lua-api.factorio.com types SpriteParameters, TileMainPictures,
RenderLayer and prototypes TilePrototype; forums.factorio.com t=80136
(camera and light), t=87188 (64 pixels per tile).
