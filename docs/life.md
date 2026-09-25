# How the people live

How a thousand persistent NPCs live their lives as a simulation,
with combat inside it rather than around it (operator, 2026-08-21:
"our AI are most aligned with the sims in terms of AI needs. rust and
halo and most games have very simple ai that path and aggro and
move. i need my NPCs to live their life persistently. like a
simulation"). This doc is the design; design.md holds the game, and
authority.md holds who owns what. Operator decisions land here
verbatim.

## Prior art, and what each one gives us

- The Sims: needs plus smart objects. A person has needs that drain;
  every thing in the world advertises what it satisfies ("this bed:
  rest 40", "this box: food 20"); the person picks the best
  advertisement for its worst need, discounted by distance. The
  brain never knows what a bed is. The cheat (operator, 2026-08-21:
  "does it make sense to advertise the solution to it and basically
  give it ESP??"): every Sim hears every object on the lot, and a
  RimWorld pawn scans the whole map. It passes there because the
  world is one house. On 3.5 km with a thousand strangers it would
  show. We keep "things say what they are good for" as a field on
  the def, drop the broadcast, and put knowledge in between (below).
- RimWorld: the same needs, plus jobs, a daily schedule (sleep,
  work, free time), mood from memories, a storyteller throwing
  events at the colony. Closest to design.md in spirit.
- STALKER's A-Life and Bethesda's Radiant AI: everyone exists all
  the time, but only people near the player have bodies; the far
  ones are facts (where, doing what, arriving when) and their
  outcomes are resolved on paper. The only known way to run a
  thousand lives on one CPU. Already in authority.md as the A-Life
  split.
- Kenshi, Mount and Blade: factions as persistent actors roaming a
  big map, towns with real stores, wars that redraw the map. What
  the faction layer should feel like.
- Dwarf Fortress: needs, memories, relationships per person; the
  deepest and the slowest. The warning: simulate only what the
  player can ever notice (the bored-player rule).
- Endless (in-house): the two states per NPC, the ordered hard rules
  (arrive, flee, leash, fight), the weighted lottery with
  personality, bucketed thinking, decisions separate from movement.
  Kept.
- Rust and Halo: combat AIs (aggro, path, cover, flee). Their lesson
  is movement and perception (a navmesh, cover, belief instead of
  truth), not the life.

## Where it lives (operator: "as much of this in modforge as possible")

modforge: the needs and their drain rules, the good-for fields on
the defs, `Memory` and its staleness, `Activity`, `CombatState`,
`Perception`, `decide`, the day clock rules, the far-people record
and its paper resolution, faction knowledge sharing, faction stores.
All plain data and pure functions, tested without an engine.

topside only: building a `Perception` from Avian rays and entity
positions, pushing the returned actions onto the queue, dropping and
rebuilding bodies at the radius, and the content (which defs say good
for what).

## The shape: sense, decide, act

Three layers kept apart so each is tested alone (Millington, "AI for
Games"):

- Act: the action queue every actor already has. The brain pushes
  the same actions the keyboard pushes. Nothing else moves a person.
- Sense: what topside gathers from its world each think: the nearest
  hostile in sight, the offers nearby, own health and needs, home,
  the clock. Plain data (`Perception`), no engine types.
- Decide: one pure function in modforge:
  `decide(perception, state, personality, roll) -> (state, actions)`.
  Tests feed perceptions and read actions.

Not per-behaviour Bevy systems: one system per behaviour loses the
order and lets two systems fight over one person (Endless's doc
warns about exactly this).

## The person

Every actor carries the same parts (authority.md Actor), and for the
life these matter:

- Needs: hunger and thirst (built, `Survival`), rest and safety
  (next). Each drains over the day; each is what makes a person
  leave a spot.
- Personality: the seven signed axes from Endless (courage,
  diligence, vitality, power, agility, precision, ferocity), seeded
  from the modforge genome by ActorId, multiplying scores and stats
  (todo 11d; design.md's Diablo 2 enemy traits).
- Home: where it sleeps and returns to. A bunker person's home is its
  bunker, where it respawns. Topside people have no home and die in
  every storm (design.md "Bunker people and topside people").
- Faction or none (built, 11b).
- Memory: what I have seen (a thing, where, when, what it held), who
  hurt me, who fed me, the last seen position of a threat. Small,
  per person; the brain reads it, perception writes it, and it feeds
  relationships and the chronicle.
- Two states: `Activity` (kind, phase, target) and `CombatState`
  (none, fighting from where, fleeing). Kept separate so a fight
  interrupts an errand and the errand resumes.

## The brain

In priority order, the first rule that applies wins:

1. Arrived at the target: Transit becomes Active (sleeping, looting,
   eating).
2. Fighting and health under the flee line (courage moves the line):
   flee toward home.
3. Fighting and chased past the leash from where it started: break
   off, go home.
4. A hostile in sight (by the faction table): fight: look, close,
   attack. The combat we have.
5. Otherwise, the life: pick something I know of.

The pick: for the worst need, every KNOWN thing (memory, below) that
is good for it scores satisfaction minus travel cost, times the
personality multiplier, plus a small seeded random so the same person
in the same spot does not always choose the same; the best wins. The
roll is seeded from the tick and the ActorId, so a replay matches.
When nothing known is good enough, the need drives LOOKING: go to the
nearest known place not yet checked, or walk the road to the next
site; what is found goes into memory. Wander is the floor when no
need presses, so nobody ever stands frozen.

## How often a person thinks (2026-09-25)

A person decides at their reaction time, not every tick. Reaction time
depends on what they are doing:

- Combat: every 250 ms (4 times a second).
- Awake: every 1 second.
- Asleep: every 10 seconds.

Being hit does not earn an extra reaction; a person under fire is at the
combat level like anyone fighting. Being tired slows reaction time, and
personality makes a person quicker or slower. Between decisions a person
keeps doing what they decided; walking and steering still run every
tick. Each person's thinking is spread across ticks by who they are, so
only a slice of people think in any one tick.

## What things are good for

The RimWorld way (decided 2026-08-22, operator: "we're building this
like rimworld needs system", "we need to do this right"; read from
RimWorld's decompiled `JobGiver_GetFood`, `FoodUtility`,
`JobGiver_GetJoy`):

- Each definition carries a field named after the need it fills,
  with that need's numbers, the way an item already has `food`
  (hunger, thirst). A camp, a building type, or a bed gets `shelter`
  (rest, safety). RimWorld's `ingestible` and `building.bed`. There
  is no general "good for" list on any def; `good_for` and `GoodFor`
  go away.
- A box does not feed anyone; the cans inside do. A person remembers
  the box: its kind, where, when, and what they saw inside it. When
  hungry they look through memory for a thing whose definition has
  `food` and find "the box at X had canned food". The box definition
  has no food numbers. RimWorld finds a meal in a stockpile, not a
  stockpile that is good for food.
- The brain has one rule per need, the way RimWorld has one job giver
  per need: hungry, look in memory for food; tired or afraid, look in
  memory for shelter. Memory stores only what kind of thing it saw;
  the registry says what that kind does. No copy of a definition's
  numbers lives in memory.

Nothing is broadcast to anyone (operator: no advertising, "things
describe what they are good for and people remember what they saw";
The Sims advertises, we do not). Where we differ from RimWorld on
purpose: RimWorld scans the whole map for the nearest food; we look
only in memory. A new kind of thing is a new def with the need's
field, never a new rule in the brain.

## What a person knows

The brain only considers what the person knows, and knowledge comes
from three places:

- Home: the things at its home site, known from the start (it lives
  there).
- Seen: perception writes what the rays reach into memory: the
  thing, where, when, and what it held if checked.
- Told: a faction shares knowledge. A scavenger comes home and the
  camp now knows about the wreck on the ridge, with a delay and only
  within the faction. Neutrals share nothing.

Knowledge goes stale: the box remembered with two cans may be empty
now because a camp mate ate them. The person walks there, finds
nothing, updates memory, and moves on. That is a story, not a bug.
Finding things is how a faction gets rich: a camp that knows the map
eats; one that does not starves and raids. The risks to tune: too
forgetful and nobody finds anything; too chatty and everyone knows
everything.

## Home and the day

A day clock (GAME_DAY_SECS exists). Night raises the rest need so
people go home, camps go quiet, roads empty; morning sends them out
again. Guards hold posts, hunters range within a radius of home and
come back. Seen from a hilltop, that is the world looking alive.

## Real stores, no cheating

Survivalist's rule, already in design.md: every item came from
somewhere. A scavenger looting a box carries real items home to a
faction store; a faction eats from its store; a store running dry
raises the faction's need, and that is what later becomes the
franchise vote, the raid, and trade (todo 11i, 11j). The economy is
needs at the faction level on the same scoring.

## Far people

Beyond a radius from the player an actor drops its body and keeps
its record: ActorId, position, activity, target, arrival tick, needs
ticking at a slower rate, inventory. It thinks with the same
`decide` on a perception built from the map instead of from rays,
less often. A far fight or loot resolves on paper (strength against
strength, a roll). When the player comes near, the body returns at
the record's position. Same id, same items, same brain; fewer calls.
This is todo 11h, pulled forward to right after the brain because the
brain must work for far people from the start.

## Perception with belief, not truth

An NPC sees what a ray from its eyes reaches inside a cone, hears a
hit on itself, and remembers where it last saw a threat (Halo 2's
lesson: NPCs that know your exact position through walls feel like
cheats). Todo 11f. Line of sight is the consumer's job; the memory is
the person's.

## Movement

The brain says "go there". Today topside walks a straight line and
slides along walls, which is fine around open sites. Proper is a
navmesh with cover points (Rust on Unity's navmesh). A row for a
navmesh (`vleue_navigator` or `oxidized_navigation` for Bevy) comes
before the acts that cross the map (raids, trade caravans).

## One day in one life

312, a scavenger at a wasteland camp, faction raiders. Morning: her
needs tick; no hostile in sight; hunger is worst. Her memory holds
the camp box (two cans, seen yesterday), a wreck east (unchecked),
and a drop north the camp told her about. The camp box scores best
(close, known food; she is Lazy so distance costs her). She walks
there through the same movement as the player, arrives, eats one can
through the same `eat_from_slot`, finds it was the last (a camp mate
ate the other), and her memory says so. Nothing known feeds her now,
so she goes looking: the wreck east. On the road her rays reach a
body site she never knew: into memory. Then you, 40 m off: the table
says hostile, she fights, you hurt her past her flee line (she is
Cowardly), she runs home; memory keeps "a player, hostile, at the
road". She sleeps at home; by morning the camp knows about you.
Meanwhile 700 people you cannot see do the same without bodies:
records with a target and an arrival tick, the same boxes, the same
brain, fights resolved on paper, boxes left where they fell for you
to find a week later.

## Build order

1. 11d personality (the scores need the multipliers).
2. Needs: rest and safety beside hunger and thirst.
3. What things are good for: the fields on the defs, and memory on
   the person; home knowledge at spawn.
4. 11e the brain as above: the hard rules, the pick from memory,
   looking, wander as the floor.
5. 11f perception with memory: rays in a cone write what is seen.
6. Home and the day clock.
7. 11h far people.
8. Told: faction knowledge with a delay.
9. 11g bucketed thinking.
10. Faction stores and the needs of a faction.
11. The navmesh, then the acts (11i, 11j).
