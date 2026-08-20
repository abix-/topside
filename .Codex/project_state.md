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

Documented how Bevy entities, components, systems, resources, messages,
hierarchies, persistent identity, Avian, GPU buffers, modforge binders, and the
HUD fit Topside's World authority.

## Next steps

1. Replace `StaticColliders` and `Walkables` with ECS-owned Avian colliders and
   queries.
2. Move player inventory from `HudState` to a player component.
3. Route interaction through the modforge interaction path.

## Open questions

- Benchmark the 1,000-NPC movement and spatial-query slice before settling
  CPU or GPU authority for positions.
