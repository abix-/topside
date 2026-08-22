# topside changelog

## Changelog rules

These rules govern `docs/changelog.md`. Follow them exactly.

- Each day has exactly one `## YYYY-MM-DD` heading and exactly one table below
  it.
- Every daily table has exactly three columns: `System`, `Item`, and
  `Done when`.
- Start every Item with `[x]`. No preceding hyphen. Changelog rows are
  completed work only.
- Never combine completed items into one changelog row. Never replace them with
  a summary paragraph, grouped bullet, status paragraph, or count.
- When recording a completed todo row, copy its System into `System`, its
  completed instruction into `Item`, and its completion proof into `Done when`.
- Verify the exact result before adding the changelog row.
- Leave unfinished work in the todo. Do not add partial work to the changelog.
- Put new days above older days. Add rows to the existing table when that date
  already exists. Never create a second heading or second table for the same
  day.
- Use plain terms. Do not use vague descriptions, invented language, or design
  essays.

## 2026-08-21

| System | Item | Done when |
|---|---|---|
| modforge worldgen | [x] `WorldDef` gets a `generator` field: `Generator::Relief` carrying the existing shaping fields (flatness, mountain height, plateau step) and `roll_world` reads it; nothing else changes | The mixed world rolls identical for the same seed before and after (`the_mixed_world_rolls_as_it_always_has`, fingerprint recorded before the change); topside's `world_def()` builds `Relief`; modforge worldgen 7 tests and every topside test pass |
| Spawner | [x] Build the one NPC spawn path: hostile NPCs at monuments enter through one function with position, faction, and combat stats. modforge `ActorDef` + `ActorRegistry` + `decide` (the AI: look, move, attack from what it sees); topside `spawn_actor` gives an NPC exactly the player's parts (`Facing` is now shared, `Player` is a marker) plus a body and a GPU slot; `populate_monuments` fills every NPC spot; `ai::think` pushes actions onto the NPC's queue so `look`, `movement`, `fire` run unchanged | tests/actor.rs spawns through the one path and checks identity, faction, position, shared parts, and a hunter walking at the player; tests/monument.rs checks one NPC per spot under the root |

## 2026-08-20

| System | Item | Done when |
|---|---|---|
| Engine | [x] Upgrade Topside from Bevy 0.18 to Bevy 0.19.1 and align modforge's shared glam dependency | Topside check, 10 tests, strict Clippy, four-phase boot test, and windowed Vulkan startup pass; modforge builds and its structure tests pass with glam 0.32 |
| FixedUpdate | [x] Move player movement, looking-at selection, door mutation, and item pickup out of `Update`; capture input in `Update` and apply gameplay changes in an ordered `FixedUpdate` chain | `gameplay_result_is_independent_of_update_count` produces the same player position, selected target, door state, and inventory at 60 Hz and 240 Hz render sampling; the boot test, full test suite, and strict Clippy pass |
| Physics | [x] Choose Avian 3D as Topside's physics engine | `docs/authority.md` records Avian's ECS ownership boundary and `docs/todo.md` names Avian for the structure-collision migration |

## 2026-08-19

| System | Item | Done when |
|---|---|---|
| World | [x] Build the topside crate: Bevy 0.18 app, FixedUpdate simulation at a locked 60 UPS gated on AppState, first-person camera walking a ground plane (click grabs, WASD walks, mouse looks, Escape releases) | Launched windowed on Vulkan and walked by the operator in-game |
| Control plane | [x] Wire the modforge control plane at startup: server on 127.0.0.1:15703/topside, status op through OP_REGISTRY, snapshot published from FixedUpdate | tests/control_plane.rs passes against the running game (tick advances between queries; unknown op rejected) |
| GpuSlotPool / EntityMap | [x] Build the slot identity spine from the GPU offload contract: LIFO slot pool (high-water count) and slot-to-entity map (live counts) | 3 unit tests pass (reuse, high-water mark, live count) via k3sc cargo-lock test |
| Test framework | [x] Ship the phased boot test: topside --test runs headless and exits 0/1 | Boot test passes: app built, 29 ticks in 500ms at 60 UPS, slot pool reuse |
| World | [x] Build the first bunker: room with bed, bench, food crate, hinged door, per-axis collision, food clock HUD, restart.ps1 lifecycle script | Walked in-game by the operator; boot test phase 4 (food clock) passes |
| StructureDef | [x] Extract the structure vocabulary: StructureDef as data, spawn_structure as the one path, doors as components; the starting bunker re-expressed as pure data | All tests pass including the wall-segment collider test; the game runs identically through the new path |
| modforge structure | [x] Move the structure vocabulary data and rules into a modforge structure module on glam math and plain rgb colors; topside keeps spawn_structure as its Bevy binder | modforge builds with the module (2 tests pass), topside consumes it with no local copy of the defs, all topside tests pass (5 lib + 2 control plane), game relaunched |
| StructureDef | [x] Extend the structure vocabulary to multi-room buildings: rooms, hallways, stairs, and the rules for how they relate so structures read as logical places to explore | The two-story house (ground room, stairwell, upper room, stairs with landing) spawns through spawn_structure and was walked in-game by the operator; 6 lib tests pass including stair-climb and room-overlap validation |
| World | [x] Decide the world substrate and engine for topside, naming prior art for each candidate considered | The open question in docs/authority.md is replaced by the recorded decision and its reasoning (3D first person, low poly with cel shading, AI-generated assets, Bevy) |
| modforge | [x] Decide where the game systems live in the modforge workspace (modules in the foundation crate versus a new workspace crate) | Recorded in docs/authority.md and the modforge readme: modules in the modforge foundation crate itself, so one dependency serves mods OR the game |
| docs/design.md | [x] Decide the core loop and write docs/design.md: what the player sees and does minute to minute (view, controls, session shape), how the bunker start works, and how the extraction, incremental, and loot-randomization ideas fit around the base simulation | docs/design.md exists; every section was decided by the operator in conversation (goal, bunker, session rhythm, emission, faction war, monuments, death, your people) |
