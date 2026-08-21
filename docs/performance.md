# topside performance budget

**Authoritative for:** every per-frame and per-tick cost in the game.
If it runs every frame or every tick, it is listed here with its cost
and why it needs to be that frequent. Anything not listed here that
sneaks into the hot loop is a bug.

The target is 60 FPS with 1000 persistent NPCs on mid-range hardware.
Every entry in this doc is a line item against that budget.

## Rules

- Nothing runs every frame without an entry in this doc.
- Each entry names: what it does, how often it runs, what it touches,
  and the measured or estimated cost.
- When adding a new per-frame system, add its row here FIRST. If you
  cannot justify the frequency, reduce it or gate it on change
  detection.
- Review this doc when performance drops. The answer is usually here.
- Measure before blaming. The `diagnostics` op (below) reports where
  a frame goes; a theory without a number from it is a guess.

## How to measure

The running game publishes every diagnostic Bevy records plus
topside's own frame marks through the control plane:

    POST http://127.0.0.1:15703/topside {"op": "diagnostics"}

`tests/control_plane.rs` (`diagnostics_op_reports_frame_time_and_render_passes`,
run with `--nocapture`) prints the table sorted by value. The HUD
shows FPS top left. Sources:

- `FrameTimeDiagnosticsPlugin`: `fps`, `frame_time`.
- `EntityCountDiagnosticsPlugin`: `entity_count`.
- `RenderDiagnosticsPlugin`: `render/<pass>/elapsed_cpu|elapsed_gpu`
  and shader invocation counts per render pass.
- `profile::ProfilePlugin` (topside): marker systems at known points
  of the frame, as `main/*`:
  `first_to_transform` (First through Update before transform
  propagation), `fixed_loop` (the whole fixed loop this frame),
  `fixed_steps_per_frame`, `fixed_gameplay` (topside's FixedUpdate
  chain, summed over the steps), `fixed_physics` (FixedPostUpdate
  through FixedLast, which is Avian's step, summed), `transform_propagate`,
  `visibility`, `post_to_last`, `extract_and_wait` (Last to the next
  First: render extraction plus waiting on the GPU).

## Measured 2026-08-20: the ruined city

Standing inside the ruined city (50 buildings of 20+ rooms, about
12,000 solids with colliders, 55,642 entities total). Before any fix:

| Span | ms | Reading |
|---|---:|---|
| frame_time | 95.7 | 10.5 FPS |
| main/fixed_loop | 75.9 | 6.2 fixed steps per frame: the catch-up spiral (a slow frame puts the fixed clock behind, the next frame runs more steps, which is slower) |
| main/fixed_physics | 74.5 | Avian's step, about 12 ms per step. THE cost. One collider entity per solid, each a child of the structure's static body, so every step re-derives 12,000 collider transforms and refits the collider tree |
| main/fixed_gameplay | 1.2 | topside's whole FixedUpdate chain for all 6 steps. Not a factor |
| main/extract_and_wait | 25.1 | mostly the GPU below |
| render/bin_unpacking/elapsed_gpu | 19.9 | Bevy's GPU-driven culling and binning over 55,000 mesh entities. The second cost |
| render/main_opaque_pass_3d/elapsed_cpu | 7.3 | command encoding for the opaque pass |
| main/visibility | 2.6 | frustum culling. Not a factor |
| main/transform_propagate | 0.2 | Not a factor |

At the bunker the same build reads 59 FPS with GPU passes under 2 ms:
the cost is the city's solid count, nothing else.

Theories that the numbers killed: transform propagation and visibility
over 55K entities (2.8 ms together), directional shadows (no shadow
pass appears at all), topside's own systems (1.2 ms).

Why Avian cost that much: `update_child_collider_position`
(avian3d `collision/collider/collider_transform/plugin.rs`) runs
every step over every collider that is a child of a body and writes
its position from the parent's without checking whether the parent
moved. That marks all 12,000 as changed, so the AABB update that
would skip unchanged colliders recomputes every one. Child colliders
are for a few shapes on one moving body. Avian's own examples mark
each static piece as its own static body, and build one collider for
a surface made of many boxes (`voxels_3d.rs`). We had one collider
entity per wall because todo row 1 said "attached to the root" and
it was read literally; despawn-takes-all only needs the entity
parent link.

Fixes applied the same day, both inside `spawn_structure`:

1. One compound Avian collider per structure root instead of one
   collider entity per solid. `fixed_physics` 74.5 ms to 9.6 ms (the
   6 catch-up steps still ran because the frame was still slow).
2. One merged mesh per material per building (`Mesh::merge`) instead
   of one mesh entity per solid; furniture shares one material per
   colour. Measured after both, standing in the same spot:

| Span | before | after |
|---|---:|---:|
| frame_time | 95.7 ms | 8.5 ms (118 FPS) |
| main/fixed_physics | 74.5 | 0.9 |
| main/fixed_steps_per_frame | 6.2 | 0.6 |
| main/extract_and_wait | 25.1 | 5.4 |
| render/bin_unpacking/elapsed_gpu | 19.9 | 0.8 |
| render/main_opaque_pass_3d/elapsed_cpu | 7.3 | 0.6 |
| entity_count | 55,642 | 9,796 |

What remains as entities: doors (one kinematic body each), lights,
storage boxes, ground tiles, UI. Not yet done: capping the fixed
loop's catch-up (`Time<Virtual>::max_delta`) so a hitch never
multiplies itself. A guard, not a fix.

Endless hit the same wall at 16K sprite entities and answered with one
entity per batch and a custom instanced pipeline (endless
docs/rendering.md). Merged meshes are the 3D equivalent that keeps
Bevy's lighting and the cel material; a custom pipeline is the step
after, only if merging is not enough.

## FixedUpdate (60 UPS, simulation)

| System | What it does | What it touches | Cost | Notes |
|---|---|---|---|---|
| tick_system | Increments the simulation tick counter | One u64 | Negligible | |
| survival::survival_tick | Drains hunger and thirst on every actor | One component per actor | Negligible at one actor; O(actors) | Bucket when NPCs land |
| structure::lock_gates | Locks the door nearest each pending gate | Pending gates x doors | Negligible once gates resolve; a gate that never finds a door stays pending | Gates despawn on lock |
| player::movement | Move and slide against the Avian world | One shape cast chain, one down cast | Low: Avian spatial queries | Off while the inventory is open |
| interact::update_looking_at | Ray test from the camera to find the nearest interactable | One ray-AABB test per door and interactable | Low; O(doors + items) | Candidate for Avian's ray cast when the AABBs go |
| interact::interact_on_press | On E, dispatches to modforge's interact | One entity lookup | Negligible | Keypress only |
| control_plane::publish_system | Writes snapshot atomics; once a second copies the diagnostics store | A few atomics; one map per second | Negligible | |
| Avian PhysicsPlugins (FixedPostUpdate) | Physics step: transforms, collider tree, broad and narrow phase | Every collider entity: one compound per building, one per door, one per box, one per ground tile | 1.6 ms per step in the city | Never give a building child colliders; see the measured table |

## Update (every render frame, presentation)

| System | What it does | What it touches | Cost | Notes |
|---|---|---|---|---|
| cursor_grab | Toggles cursor lock on click/escape | Window entity | Negligible | Input events only; off while a panel is open |
| mouse_look | Rotates camera from mouse delta | One transform | Negligible | Off while a panel is open |
| capture_movement_input, capture_interact_input | Read keys into input resources | Two resources | Negligible | Off while a panel is open |
| hud::toggle_inventory, hud::transfer_hovered, hud::drop_hovered, hud::hotbar_keys | Key handling for the panels | Resources and the player's holders | Negligible | Keypress only |
| hud::write_prompt | Builds the prompt text from LookingAt | One string | Negligible | |
| hud::render | Paints FPS, hunger, thirst, prompt to one Text | Reads the player's Survival in place | Negligible | No copies of game state |
| hud::render_inventory | Paints every slot label and colour | 30 slots + 12 container slots, one Text each | Low; runs whether or not the panel is open | Gate on change when it shows |
| profile marks | Six clock stamps per frame plus two per fixed step | One resource | Negligible | |

## GPU compute (planned, not built)

Reserved for the crowd simulation compute shaders. Entries will be
added when the GPU simulation (authority row 15) is built. The
contract from Endless: dispatch once per frame, read back one frame
stale.

## Known scaling risks

- **Solid count**: see the measured table. Compound colliders and
  merged meshes per building are the fixes; both are spawner-side,
  nothing about defs changes.
- **Point lights**: one per lit room, a few hundred in the city. Not
  yet measured as a cost (clustering is 0.1 ms GPU); shadows on
  point lights would be.
- **NPC count**: the 1000 NPC target means the GPU does the expensive
  work (separation, spatial grid, target candidates). CPU per-NPC
  systems must be throttled or bucketed, never every-entity-every-frame.
  The Endless bevy skill documents the throttle tiers.
- **Interact ray test**: one AABB test per door and item per fixed
  step. Fine at hundreds; replace with Avian's ray cast before
  thousands.
