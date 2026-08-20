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

## FixedUpdate (60 UPS, simulation)

| System | What it does | What it touches | Cost | Notes |
|---|---|---|---|---|
| tick_system | Increments the simulation tick counter | One u64 | Negligible | |
| food_tick_system | Drains food by elapsed time | One f32 | Negligible | Will become an inventory read when FoodStore is replaced with real items |
| control_plane::publish_system | Writes snapshot atomics for the HTTP control plane | A few atomics | Negligible | |

## Update (every render frame, presentation)

| System | What it does | What it touches | Cost | Notes |
|---|---|---|---|---|
| cursor_grab | Toggles cursor lock on click/escape | Window entity | Negligible | Only acts on input events |
| mouse_look | Rotates camera from mouse delta | One transform, reads mouse delta | Negligible | |
| movement | WASD movement with collision | Player transform, tests blocked() against all nearby colliders | Low: a few dozen AABB tests per frame | Already the most expensive per-frame system; scales with nearby collider count |
| interact::update_looking_at | Raycast from camera to find the nearest interactable | One ray-AABB test per door and interactable entity | Low: comparable to one blocked() call; scales with nearby entity count | Replaces the old distance-based door check |
| interact::interact_on_press | On E press, dispatches to modforge's interact | One entity lookup | Negligible | Only runs on keypress, not every frame |
| hud::write_vitals | Copies food days into HudState | One f32 write | Negligible | |
| hud::write_prompt | Copies LookingAt into HudState prompt | One string build | Negligible | Only rebuilds when looking at something |
| hud::render | Paints HudState to screen text | Reads HudState, writes one Text | Negligible | Reads no game state directly |

## GPU compute (planned, not built)

Reserved for the crowd simulation compute shaders. Entries will be
added when the GPU simulation (authority row 15) is built. The
contract from Endless: dispatch once per frame, read back one frame
stale.

## Known scaling risks

- **Collider count**: movement's blocked() and the interact raycast
  both test against colliders. As structures increase, a spatial
  index (grid or BVH) replaces the flat list. Not needed yet at
  current scale (one bunker, one house).
- **NPC count**: the 1000 NPC target means the GPU does the expensive
  work (separation, spatial grid, target candidates). CPU per-NPC
  systems must be throttled or bucketed, never every-entity-every-frame.
  The Endless bevy skill documents the throttle tiers.
- **HUD inventory**: at large inventory sizes, change detection
  (only copy when dirty) replaces writing every frame.
