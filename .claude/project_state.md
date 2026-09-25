# topside project state

Git-tracked, shared across agent clones. No secrets.

## Current focus

The people (docs/life.md): a thousand persistent NPCs living their
lives as a simulation. The brain, needs, memory, factions, and
personality are built; next is perception with belief (todo 11f),
then the day clock (11e4) and far people (11h).

## Design goals (the locked ones)

- docs/authority.md owns who decides what; docs/design.md the game;
  docs/life.md how the people live; docs/todo.md the ordered plan.
  Read them first, every session.
- As much as possible in modforge (engine-free, tested without
  Bevy); topside senses, acts, and holds content.
- One path per thing: spawn_actor, spawn_structure, spawn_monument,
  spawn_item, spawn_world. Player and NPC on the same components and
  systems; the keyboard and the AI push the same actions.
- An actor is anything that moves or shoots (turrets and vehicles
  too); WorldItems are only acted on. ActorId is the one identity;
  the ActorMap issues it and the GPU row.
- No advertising: things say what they are good for on their defs;
  people remember what they saw; the brain reads memory only.

## Last session (2026-08-21)

Built in order: the action queue (all input through one system),
the NPC spawn path, ActorMap with ActorId, combat rest (player body,
pipe shotgun with shells and pellets, body areas, worn armor, Rust
style hotbar), actor death with a loot box, worldgen (3500 m world,
heightmap with plains, ridges to 400 m, plateaus, biomes, sites,
roads, minor sites every 150 m), admin fly on V, factions with the
relation table, personality on the genome, needs (rest, safety),
good-for fields and memory with home knowledge, the brain (sense,
decide, act: eat from known boxes, sleep, wander, fight, flee,
leash), a `people` control plane op with a live test.

## Next steps

1. 11f perception: cone plus Avian ray, seen things into memory, a
   striker becomes the target, remembered threats.
2. 11e4 the day clock: people home at night.
3. 11h far people: drop bodies beyond a radius, same brain on paper.
4. 11o measure the 3500 m world; delete ground.rs.
5. Then 11e5 told, 11g cadence, 11e6 stores, 11e7 navmesh.

## Open questions

- The player's own death (design.md "Death") is not built and has no
  row.
- Water is a blue flat you walk on; rivers and swimming are row 33.
- modforge `input::tests::backend_parse_rejects_garbage` fails from
  commit 6796b7bc (another agent's work); left alone.

## How to run

- `scripts/build.ps1` is the one way to build, test, and run:
  `pwsh -NoProfile -File scripts/build.ps1 run | test [name] | build`.
  It stops the game first, logs everything to logs/, prints the
  summary, and stops a cargo run that stalls.
- modforge: `k3sc cargo-lock test -p modforge --lib` from
  C:\code\modforge.
