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
  brain never knows what a bed is. Adding content means adding
  advertisers, not rules. This is our brain's shape.
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
- Home: where it sleeps and returns to: its monument, its faction's
  bunker, or nowhere for a drifter.
- Faction or none (built, 11b).
- Memory: who hurt me, who fed me, last seen position of a threat.
  Small, per person; feeds relationships and the chronicle.
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
5. Otherwise, the life: pick an offer.

The pick (The Sims): for the worst need, every offer in reach scores
satisfaction minus travel cost, times the personality multiplier,
plus a small seeded random so the same person in the same spot does
not always choose the same; the best wins. The roll is seeded from
the tick and the ActorId, so a replay matches. Wander is an offer the
world always makes with a low score, so nobody ever stands frozen.

## The world advertises

modforge keeps an `Offer` list as data: what need it satisfies, how
much, for whom (a faction, anyone), where, and what doing it does
(eat from a box, sleep, loot into the inventory, stand guard).
topside fills the list from its entities: loot boxes (`Container`),
monuments' loot spots, the bunker and faction bunkers (safety,
rest), campsites, beds and benches when they exist. A new kind of
thing in the world is a new advertiser, never a new rule in the
brain.

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

## Build order

1. 11d personality (the scores need the multipliers).
2. Needs: rest and safety beside hunger and thirst.
3. Offers: the list in modforge, topside advertising boxes, loot
   spots, the bunker, campsites.
4. 11e the brain as above, with wander as the floor.
5. Home and the day clock.
6. 11h far people.
7. 11f perception with memory.
8. 11g bucketed thinking.
9. Faction stores and the needs of a faction.
10. The navmesh, then the acts (11i, 11j).
