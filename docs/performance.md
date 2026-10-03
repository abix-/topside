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

Every number comes from the running game through its control plane,
read by tests/perf.rs (run alone through the queue, so no other test
drives the game while it reads):

    pwsh -NoProfile -File scripts/build.ps1 queue perf::where_the_time_goes_in_play
    pwsh -NoProfile -File scripts/build.ps1 queue perf::what_a_stepped_tick_costs

Each adds a reading to logs/perf.txt. A second game (another clone) runs
on its own port: `build.ps1 run -Port 15704`, and the tests with
`-Port 15704` (TOPSIDE_PORT).

- The `perf` op (src/profile.rs, todo 9m done 2026-10-02): the whole
  fixed step, from the start of FixedFirst to the end of FixedLast
  (`step_ms`, and `step_worst_ms`, its worst since launch), and its
  parts, laps one after another so they add up to the step ("land
  meshes", "landmass", topside's FixedUpdate chain, "rest of fixed
  update", "physics"), each with its average and its worst single step
  since launch (`worst_ms`; a running average hides a one-tick stall:
  474 ms reads as about 16 and fades). Parts with `in_step` false are
  pieces inside a lap ("steer: ...") or work outside the step ("map
  drawing"). `steps_per_frame` counts fixed steps a frame. `frame` is
  every span Bevy ran (each system and schedule, the drawing too), ms a
  frame; each `Query::par_iter` ("par_for_each") is named by its query
  and summed over the worker threads that ran it.
- `what_a_stepped_tick_costs`: loads `opening_woke` (opening.rs
  `woke`), pauses, steps 60 ticks ten times, times each by the real
  clock, and reports the median of the last five (the first steps after
  a load settle) and the step's laps.
- The `diagnostics` op: Bevy's own (`fps`, `frame_time`), copied once
  every 60 ticks, so up to a second older than `status`'s fps (todo 9o).

## Measured 2026-10-02: a stepped tick, and what it costs

Why it matters: the tests move a paused game only with the `step` op,
so how fast a tick steps decides whether a test fits the 2 minutes a
test may take. The target for the first pass is 4 ms a tick (7200 ticks
in under 30 s). All numbers from tests/perf.rs on a second game (port
15704), about 105 people, the small world.

A stepped tick, by the real clock (60 ticks from `opening_woke`, settled
median) and by the perf op's whole step:

| After | real clock a tick | whole step | what changed |
|---|---:|---:|---|
| start | 11.59 ms | (undercounted) | |
| 9p | about 10.2 | | people and items make no contact pairs: narrow phase 3.376 to 0.007 ms a frame |
| 9n | 7.45 | | landmass places points through nearby islands: update_archipelagos 10.891 to 1.020 ms a frame |
| 9m | 8.18 | 8.087 | the step measured whole (no cost change; run-to-run spread) |
| bodies placed | 8.19 to 8.65 | 7.17 to 7.35 | physics lap 2.769 to about 1.8 ms; the real clock did not drop (other builds running; a step call's own cost not yet split, todo 9w) |

The step's laps at the last reading, ms a step: physics 1.8, landmass
1.4 to 1.6, movement 0.7 to 1.0, chunks 0.6, walk 0.5 to 0.8, think 0.5,
interact 0.3 to 0.4, the rest under 0.25 each.

What each finding was, and where it lives:

- The step's total missed most of the step (9m, done): the stopwatch
  started in FixedUpdate, so FixedFirst and FixedPreUpdate (all of
  landmass) were in neither the total nor the parts: 7.3 ms reported
  against about 19 real at startup. Now FixedFirst to FixedLast, laps
  adding up to it within 5% (7.686 against 7.710), FixedMain per step
  within 5% (8.018).
- Avian made contact pairs nobody read (9p, done): every moving
  kinematic person paired with every static compound and person it
  overlapped, the narrow phase redoing every pair every step. People and
  items now carry `player::makes_no_contacts` (a member of every layer,
  filtering none): spatial queries still find them, the broad phase
  pairs them with nothing. Rapier's ActiveCollisionTypes leaves these
  contacts out by default for the same reason. About 8% of a tick.
- Landmass walked every island for every point it placed (9n, open
  until landmass is under 1 ms a step): vendor/landmass nav_data.rs
  `sample_point` looped over all 1156 islands for each agent, its
  target and each character, every step. It now keeps one island box
  hierarchy (`island_bbh`), made when islands change, and looks only at
  the islands near the point, ties in IslandId order (rule 5). Detour
  finds a point's tiles by tile x and y the same way. Landmass main
  still walks every island, so the patch is worth offering upstream.
- Avian propagated every transform tree before each step (done, no
  todo row): `propagate_before_physics` worked out every moving
  person's whole drawn body (rig bones, worn pieces, what they hold)
  every tick. Avian reads only the bodies' own GlobalTransform
  (transform_to_position), so `bodies_placed` (src/lib.rs) works out
  only theirs, and Bevy propagates the drawn trees once a frame for
  drawing. Landmass reads positions through TransformHelper and no
  fixed system reads GlobalTransform, so play and steps see the same
  positions (rule 5). Physics lap 2.769 to about 1.8 ms.
- Chunk meshes were built on 2 of 8 cores (9s, open, bar 5 s): Bevy's
  default async compute pool is a quarter of the cores, at most 4. Both
  it and the compute pool now get every core (TaskPoolPlugin in
  src/lib.rs). The 1156 meshes land in 5368 ms at launch (17061 before)
  and 5459 after a load that rolled its world (20420 before).
- "par_for_each" was not one system: Bevy names every `Query::par_iter`
  span so, and the span timer summed them all and timed overlapping
  threads wrongly. Named by query now: animation targets 4.5 ms a frame
  (9v), CPU visibility culling 3.0 (9q's meshes), a plain Entity query
  1.7 (owner not found), transform propagation 0.4. All per frame, so a
  step call pays them once, not per tick.

Still open, in todo order: 9w (split what a step call pays besides its
ticks: waiting for a frame, publishing, the Moment), 9n (landmass to
under 1 ms a step), 9r (think asks Avian what is near: today each
thinker walks everyone, N squared), 9q (a chunk's things merged per
material: 81,190 meshes, 13,693 in view), 9v (rigs animate only in view),
9t and 9u (chunk mesh builds capped a tick; a load's steps timed), 9o
(one frame time). Watch: sight::look_around (linear in walkers),
share_maps (grows with what people have seen), and 9x (three command
and roxanne tests failing before any of this).

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

### The three collider layouts, measured in the same spot

A third layout was tried the same day on the claim that Rust (the
game) keeps one collider per building piece: every solid its own
standalone static body (not a child of the root), meshes still
merged. It was the worst per step.

| Layout | physics per step | frame |
|---|---:|---:|
| One collider entity per solid, child of the root body | 12 ms | 96 ms |
| One static body per solid, standalone | 15 ms | 24 ms (42 FPS, merged meshes holding the frame) |
| One compound collider per building | 1.6 ms | 8.5 ms (118 FPS) |

The claim was also wrong about Rust. Facepunch's devblogs: "I've been
working on combining building colliders on the server side" and
there is "a single point where I have to do the batched collider to
individual collider lookup" (devblog 79); "Enabled server side
collider batching by default" and a fix for "batched colliders not
refreshing after building part rotation" (devblog 82); later a
`collider_capacity` convar capping "the maximum vertex count of
pooled mesh colliders", and mesh batching moved to a thread so
"large meshes like buildings" refresh without stutter. Rust batches
colliders per building with a lookup back to the piece and refreshes
the batch when a piece changes. That is the compound layout plus two
things this repo does not have yet: the hit-to-piece lookup and
rebuild-on-change, both needed by Rust-style building (design.md).

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
