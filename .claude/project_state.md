# topside project state

Git-tracked, shared across agent clones. No secrets.

## Current focus

The 2D game (decided 2026-09-25, the Factorio way, Bevy 0.19 and
Avian 2D 0.7) on the small world by default: 32 by 32 chunks of flat
grass, the player's bunker, 5 other bunkers with 5 people each, 25
neutral and 25 hostile topside people. The people live in it: needs,
sight, memory of everything they do, stores, trips topside, and
learning to stay alive (docs/life.md). Next is the first unchecked
todo row. The 3D game in `3d/` is frozen and not built.

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

## Last session (2026-09-25, docs caught up 2026-09-26)

Built on 2026-09-25 (docs/changelog.md): the 2D game from the top
down, walls and doors as Avian colliders, melee, guns, the roll,
loot, the hotbar, the player's death, hunger and thirst on screen,
starvation damage, sight (cone and clear view, walls and closed doors
block it), memory of what was seen and who hit them, reaction time
per person, the day clock with night tiredness, the crafting bench
and bandage, healing, the storm and the world shift, every bunker its
own faction, bunker people respawning at home, topside people dying
with each world, real stores and trips topside, pathing on one tile
grid, valid worlds, chunks made as needed, the small world, the
control plane ops (status, perf, people, npc, ways, world, speed),
and learning to stay alive.

On 2026-09-26 the todo and changelog were caught up to the code:
rows 14, 14b, 15, 10c, 11f, 11g, 11p, 11y, 11z, 11aa, 12, 10, 10ca
closed, and 11e4 cut to what is left. Each was checked in the code
only; their tests were not run (the operator does not like the state
of the tests: the last run went on forever).

## Next steps

The first unchecked row of docs/todo.md, unless the operator says.

## Open questions

- The tests: the operator does not like their state and the last run
  went on forever. Nothing is decided about them yet.
- Todo row 1 (one content file for both games) asks the 3D game to
  build, but the 3D game is frozen and not built.
- Seen in logs/game.txt (2026-09-25 22:11, two minutes at 1x), not
  looked into: every person's hunger, thirst, and rest read 90, 80,
  90 after two minutes; the alive seconds column reads 4081 and 2785
  for two people in a 123 s run; one person has 126 health; "walk
  grid: chunks painted" is 547 a step; every wanderer stays Idle.
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
