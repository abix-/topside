# How people find their way

**Authoritative for:** how every NPC gets from where they are to where
the brain sends them (the player walks where the keys say): the
walk grid, the way round, what a plan may cost, and doors on the way.
The brain decides WHERE to go (life.md); this doc decides HOW they get
there. Decisions land here in plain words as they are made.

## Why this doc exists (2026-09-25)

The first way-round search (topside `nav.rs`) asked the physics world
about every cell during every plan and remembered nothing, walked a
straight line by a physics cast every tick, and had no limit on how many
people planned at once. In the real game (tests/game.rs) its cost climbed
from 2.7 ms a step at the start to 7.8 ms after two minutes, still
rising, as more people went out on long trips. The fix is the design
Endless already proved at scale, not a new one.

## What we take from Endless (agreed 2026-09-25)

Endless (`C:\code\endless\rust\src\systems\pathfinding.rs` and
`movement.rs`) ran pathfinding for its whole population. We port its
design, built once in modforge (the DRY rule), on modforge's one A*
(`path::grid_path`):

1. **A walk grid, built once, not asked of physics per plan.** One number
   per cell: blocked, or what a step there costs. Built from the world
   (water, scatter, every building's walls) and kept until the world
   changes (the storm rolls a new one, a building changes). Planning then
   reads plain numbers.
2. **Doors are walkable.** A person opens a closed door on the way with
   the player's own Use.
3. **Short trips go straight.** A clear straight line on the grid (the
   cells along it, Bresenham, Endless's `line_of_sight`) means no search.
   No physics cast every tick.
4. **One queue of plans with a budget per frame.** Every person who needs
   a way asks the one queue; each frame works through it up to a count
   and a time limit (Endless: 200 a frame, 2 ms), and the rest wait for
   the next frame. A crowd can never blow the frame.
5. **A failed search waits.** A person whose way cannot be found waits
   (Endless: 2 s) before asking again, and counts as blocked.
6. **Long trips search a small graph (HPA*).** The grid is cut into
   chunks; the ways between chunk entrances are worked out once and
   cached; a long trip searches the entrances, not every cell, and the
   cached ways are stitched into the full way. When a building changes,
   only its chunks and their neighbours are rebuilt, and only people
   whose way crosses them plan again.

topside's part is only to paint the grid from its colliders and to ask
the queue for ways; the grid, the search, the queue, and the budget live
in modforge and are tested without the game.

## Open: the grid's size against the world's (2026-09-25)

Endless's grid was 250 by 250 cells. Topside's world is 3500 m across,
walls are 0.2 m thick, and doors 1.2 m wide, so a grid fine enough to
keep doorways open (0.5 m) is 7000 by 7000 cells for the whole world.
Built all at once that is too big. How the grid and the world's size fit
together is the next decision; until it is made, nothing in this doc is
built.
