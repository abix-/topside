# topside project state

## Current focus

Replace duplicated world-state stores with Bevy ECS ownership, starting with
structure collision and player inventory.

## Design goals

- Bevy ECS is the live World.
- State describing one runtime thing lives on that entity as components.
- Resources are limited to true world-wide state, indexes, allocators, and
  transient input.
- Modforge owns generic decisions; Topside binders observe and apply them.
- Presentation reads gameplay state and never owns it.

## Last session

Upgraded Topside to Bevy 0.19.1 and aligned modforge with Bevy's glam 0.32
types. Topside check, tests, strict Clippy, headless boot, and windowed Vulkan
startup pass. The next change can integrate Avian 0.7.0 without mixing an
engine migration into the physics work.

## Next steps

1. Replace `StaticColliders` and `Walkables` with ECS-owned Avian colliders and
   queries.
2. Move player inventory from `HudState` to a player component.
3. Route interaction through the modforge interaction path.

## Open questions

- Benchmark the 1,000-NPC movement and spatial-query slice before settling
  CPU or GPU authority for positions.
