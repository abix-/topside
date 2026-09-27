# topside realities

**Authoritative for:** what a reality is, the rules a reality can
change, and the list of realities. How each world's ground and
buildings are made (its generator) lives in design.md, "The world" and
"The generators we want". Episodes live in episodes.md.

## What a reality is (operator, 2026-09-27)

The storm shifts everyone into a different reality (design.md, "The
storm"). A reality is its own thing, separate from the episodes:

- A reality is a world (one generator, from design.md) plus the rules
  that work differently in it: the whole theme, physics, and logic.
  Death, night, falling, what water does, what people believe: any of
  it can be different.
- A reality exists whether an episode runs or not. The people live by
  its rules either way.
- An episode runs inside a reality. Each episode says which realities
  it fits. One reality can hold many episodes, and one episode can play
  in many realities.
- The storm picks the reality, then an episode that fits it, cast from
  what people remember.
- A rule is written once, here, never copied into the episodes that
  use it.
- The player finds out what is different by doing something, never by
  being told. Each rule below says how.
- Each episode everything topside dies, and only bunker people come
  back (operator, 2026-09-27): the rule of the base reality for now. The
  storm takes every topside person with their world for good, in every
  reality; a rule like Loop's changes only deaths inside the reality.

## Danger (operator, 2026-09-27)

Just as there are normal realities, there are realities of extreme
danger and combat, where the player defends the bunker or is destroyed.
Elden Ring is the model: a world full of giant, hard to fight bosses
and great loot.

- Every reality has a danger: calm (a reality to breathe in), hard
  (fights won with care), or deadly (giant bosses guard the best loot,
  and sometimes the only answer is to run or lose everything, as with
  the assassin in design.md, "The unknown").
- Loot follows danger (design.md, "Loot tables scale with danger").
- A boss is an actor through `spawn_actor` like everyone else, bigger
  and harder. Everyone is equal still holds.
- The same danger plays differently by the reality's rules: in Loop the
  player dies and comes back, so a boss can be learned; in Sky Islands a
  boss can knock the player off the edge; in Maze the maze sends it and
  moves the walls to help it.

## The rules a reality can change

- Water remembers. Water carries the memories of the last one who drank
  it; whoever drinks sees what the one before them saw. The player finds
  out with their first drink from a shared source. Used by The Tap.
- Nobody dies. Anyone who dies wakes the next morning where they last
  slept, and remembers who killed them. The people know they are stuck
  in the loop and never chose it. The player finds out by seeing someone
  they know to be dead, or by their own first death. Used by Nobody
  Dies.
- Everyone has a double. The reality already holds another copy of each
  person who crossed, who made the opposite choices. The player finds
  out by seeing their double. Used by The Other You.
- The sick share minds. Everyone with the same sickness hears what the
  others think. The player finds out by catching it. Used by The Fever.
- Falling is death, there is no night, the dead are kept working, water
  takes away power, the walls move: the Death Gate worlds' own rules,
  below.

## The realities

Reality names are our own, never a name from a book, show, or game
(operator, 2026-09-27). Where an idea came from is said once, in the
reality's file.

Each reality has its own file in docs/realities/ (operator, 2026-09-27),
with these parts, in this order:

- Where it came from.
- The world: the ground and buildings, and its generator (design.md
  where one is described).
- The theme.
- The physics: gravity, motion, time, and anything else that works
  differently. "Normal" when nothing does.
- The logic: the rules from the list above, or its own.
- Danger: calm, hard, or deadly, and what the danger is.
- How the player finds out.
- The episodes that fit it.

A reality can be any world with any rules from the list above. Not built
yet: today the game has one world and no rule changes.

- [Mixed world](realities/mixed-world.md)
- [Backrooms](realities/backrooms.md)
- [Sky Islands](realities/sky-islands.md)
- [Endless Day](realities/endless-day.md)
- [Deep Stone](realities/deep-stone.md)
- [Frozen Sea](realities/frozen-sea.md)
- [Maze](realities/maze.md)
- [Ruined](realities/ruined.md)
- [Glitch](realities/glitch.md)
- [Mirror](realities/mirror.md)
- [Body](realities/body.md)
- [Rot](realities/rot.md)
- [Great Tree](realities/great-tree.md)
- [False Stars](realities/false-stars.md)
- [Still Storm](realities/still-storm.md)
- [Loop](realities/loop.md)
