# topside design

**Authoritative for:** the game's story, core loop, and player-facing design.
In progress: decisions land here as the operator makes them. The ownership
boundaries live in authority.md; the games we mine live in references.md.

## The founding idea (operator, 2026-08-22)

The world is a simulation that lives. The player is one actor in it,
not the center of it. The world keeps running if the player hides and
starves in the bunker: people eat, sleep, fight, trade, and die whether
anyone is watching or not.

Who it is for (operator, 2026-08-22): "they split gamers into killers,
achievers, socializers, and explorers" (Bartle, 1996). "Outer Wilds was
made for explorers. I want this game to be made for explorers and
killers." Every system earns its place by giving one of those two
something: a place to find out, or a fight to win. Achievers and
socializers are not the audience.

"A struggle to survive and maybe thrive... and kill" (operator,
2026-08-22). Survivalists are the explorer with stakes: the world is
there to find out, and hunger, thirst, and the people out there make
finding it out cost something. Bartle has no survivalist type; the
struggle itself is the draw (Quantic Foundry's Challenge motivation),
so survival is how the explorer half and the killer half meet.

In one line (operator, 2026-08-22): "think of it like Diablo 2 but with
survival and exploration and incremental and building."

## The story

You wake up in a bunker after a post-apocalyptic something. You know nothing.
Go.

The backstory (operator, 2026-08-21): alternate realities. The storm does
not destroy anything. It shifts the actors, computer and human alike,
between realities. The world you walk out into after a storm is a
different reality, not a rebuilt one. The bunkers and the people cross
with you because they are actors; the ground, the buildings, and the
things in them belong to whichever reality you are in now. The player
is never told this outright; it is something to wonder about (see "The
unknown").

## The goal

The goal of this game is to make the player FEEL like they are in a struggle
to survive. Food. Water. Resources. Enemies. Scarcity. Heat. Radiation. It
all comes for you.

Think about this like Oregon Trail meets post-apocalyptic 3D: attrition is
the game, supplies run down, hazards just hit you, and the people with you
can die along the way.

The game is chaos versus order (operator-locked 2026-08-20: "this is a
game of all chaos where the player is trying to impose order. thats the
game really. a neverending struggle against chaos"). The world's default
state is chaos: storms regenerate the map, enemies roll with random
traits, loot rolls with random affixes, factions act on their own,
weather shifts, nothing stays the same. The player's entire effort is
imposing order: a bunker that persists, gear that's reliable, territory
that's held, people who are loyal. Order carved out of chaos, never
permanent, always under pressure.

Every design decision serves this feeling. (The survivalist mod's line still
bounds it: brutal but survivable; there must always be a way out.)

## The people and animals of the world

- Faction NPCs: enemies as well as friendly.
- Neutrals: people who belong to no side yet. They make the world feel alive,
  their intent is not known on sight, and they are where factions (including
  yours) recruit from.
- Animals: food, threat, and scarcity at once.

The scale target (operator, 2026-08-19): a world population of 1000
simulated NPCs that are PERSISTENT. Not spawned scenery: a thousand people
who exist, belong to factions or to no one, and carry forward. That is the
bar; hit it and the operator is happy.

## The first hour (operator-agreed 2026-08-21)

"Finding out is the game", so the game starts with a question, not
with a door. The opening today (wake, three things in a room, walk
out, a road, a monument at the end of it) is a road as a marker and a
monument as a list item. This is what it becomes. Every rule in "The
unknown" is used once in this hour, so the hour is also the test of
those rules.

1. The bunker asks the first question. You wake with the room and
   nothing else, and the room is wrong in one way: a door bolted from
   the inside with scratches on it, a note with a name you do not
   know, a radio that clicks twice an hour. One question, planted
   before daylight. Told knowledge starts here. The three things in
   the room (bed, bench, food) stay; the question is a fourth.
   Operator, 2026-08-21: "even better, the note tells you not to go
   outside." The first thing the game says to the player is do not
   do the one thing the game is. Three days of food says you will
   have to anyway. Whoever wrote it knew something; finding out what
   is the first question, and the door is the first answer to "do I?".
2. The door opens on one big triangle. Not a road. One thing on the
   horizon you cannot explain (a tower, a light, smoke), and a medium
   triangle between you and it so it vanishes as you walk. The way is
   readable (a gap, a track, a gate) but not paved.
3. The first site is behind the ridge, not beside the path. A trace,
   not a stamp: a body, and the body is someone history made; it
   carries the name from the note, or the note's handwriting. It
   answers the bunker's question and asks the next one.
4. Something happens that the roll did not place. A Fallout 2
   encounter: a stranger crosses your path and leaves, a band passes
   in the distance, the radio's clicks become a voice. The first proof
   that the world can surprise you.
5. The first teeth. An anomaly with a tell near the second site, and
   the bolt in your hand. The first time the world answers what you
   do, and the first time it could have killed you.
6. By the end of the hour: three questions open, one answered, the
   big thing closer once and lost once, nothing told by the HUD, no
   map filled in. The storm has not come but the sky has changed
   once, so the player knows it will.

What this changes outside the hour:

- Roads cannot go to every monument; a road to everything is the
  list. Roads go where history put them (between places people used),
  some broken, and the places that matter are off them. This is a
  change to the topside generator, not only the Labyrinth.
- The bunker's starting room carries one rolled question from a
  registry of questions (the bolted door, the note, the radio, and
  more), each with its answer placed by history somewhere within the
  first hour's walk.
- The first site in any direction is a trace with a link back to the
  question.

In the Labyrinth the hour is the same shape with the clearing as the
room: the first question is a mark on the clearing's wall, the big
triangle is a gate's tower standing over the walls, the first trace
is in the first dead end, the first teeth are an anomaly in the
second.

## The bunker

The bunker is the permanent home base, built up over the whole game.
It grows DOWNWARD, not outward (operator-locked 2026-08-20: "this is a
bunker. the topside isnt safe. so i want this bunker to go down
vertically. like fallout shelter. but also each level has some
expandability. like a silo").

The bunker is a vertical silo. Each level is a floor the player digs
deeper, and each floor can expand horizontally within its footprint.
The deeper you go, the safer (raiders have to fight through every level
above), but also the more expensive (excavation, reinforcement,
ventilation, longer power runs). The entrance is the most vulnerable
point; the bottom is the vault.

You start with just the bunker (the one starting room). The silo
expansion is the progression: you dig down, carve out new levels, and
build your underground base over time. Vertical progression gives the
bunker its own curve:
- The bunker: the starting room. Bed, food, bench. One door to topside.
- First expansion: dig down into the silo. Living quarters, basic
  storage, the first generator.
- Mid levels: specialized stations, expanded storage, the armory.
- Deep levels: the research lab, the vault, the most protected rooms.

Each level is a floor plan the player designs: carving rooms, placing
doors, running power, setting up stations. Fallout Shelter is the
reference for the vertical feel; Rust is the reference for the
room-by-room building freedom.

It starts with exactly three things:

- a bed
- food for three days
- a crafting bench

The three days of food is the game's first clock: it forces you topside
before you are ready. The crafting bench is the seed everything grows from;
crafting is a core mechanic from the first minute.

The long game at home:

- Building the bunker out: new rooms, better walls, blast doors.
- Placing stations: crafting benches, chemistry tables, electronics
  workbenches, armory benches, each unlocked by research.
- Power: generators that burn fuel, wiring, powered lights, powered
  defenses, powered stations. The grid is a real system: no fuel, no
  power, no powered anything.
- Storage: boxes with capacity, organization, maybe sorting automation.
- Defenses: turrets, reinforced walls, blast doors, all scaling with
  tech level.
- Defending it from raiders (the storyteller's incursions come for YOU).

The bunker is the player's factory (operator-locked 2026-08-20: "we dont
have a factory but we do have a bunker that i want the player to be able
to expand and craft and place benches and power and boxes"). Not conveyor
belts, but the same idea: you start with one bench and end up with a
powered, multi-room operation with specialized stations and automated
defenses. Rust's base building is the model for what you can place and
build; Factorio's depth is the model for how far the tech goes.

Bunker defense, and this is deliberately unlike MISERY: sometimes the BUNKER
itself is attacked and has to be defended. We are surviving a post-apocalypse;
home is usually calm but never guaranteed safe.

## The session rhythm (MISERY-shaped)

- In the bunker you have TIME: gear up, craft, do base stuff at your own
  pace. Home is where the pressure usually pauses, but see bunker defense
  above: it can come to your door.
- When you want to, you leave the bunker and go into the topside world. No
  forced trips, no hard session boundary; going out is the player's choice.
- The storm, modeled on MISERY's emission: after X time there is some event
  and the world changes. Keep similar timing to MISERY's at first. In
  topside this is the storyteller's beat; the full mechanic is its own
  section below. (The name emission stays MISERY's and STALKER's; ours is
  the storm.)

## The storm

Occasionally the storm hits (modeled on MISERY's emission):

- If the player is out in the world, they can hide in some building or
  similar shelter that MAY be safe. Caught in the open, they die.
- Then the world gets REGENERATED around them, EXCEPT the player's base and
  enemy faction bases, which persist.
- The 1000 persistent NPCs survive regeneration the same way bunkers do; the
  storm re-rolls terrain and monuments, never deletes people. People die
  only by dying.
- NPCs obey the same storm rules as the player: they hide from it just
  like you do, or die in the open. That creates natural tension around the
  'safe' places and leads to more situations: shelter is contested when the
  sky turns, and whoever is near your door when it hits becomes your
  problem, or you become theirs.

This is what keeps the world from ever being memorized: monuments, their
layouts, and their names re-roll with the world, while everything with an
owner (your bunker, the factions' bunkers) carries forward. The map is never
fully known, but the story of who holds what continues.

What the storm is (operator, 2026-08-21): a shift between alternate
realities, not destruction. "Regenerate" above means the actors arrive
in a different reality. That is why the people and bunkers persist and
nothing else does, and why the next world may be made by a completely
different generator (see "The world").

## Building, the Rust way (operator, 2026-08-20: "think about how
## Rust lets you build buildings. i want that")

Rust's system, from its guides and Facepunch's devblogs: a building
plan places pieces (foundation, wall, doorway, window, floor, stairs,
roof) that snap to a grid anchored on foundations; a hammer upgrades,
repairs, rotates, demolishes. Every piece has its own grade (twig,
wood, stone, sheet metal, armoured: 10, 250, 500, 1000, 2000 health),
its own health, and a stability percentage that falls with distance
from support; zero collapses it. A tool cupboard makes the base
yours, sets who may build, and eats upkeep; an empty cupboard means
decay, faster for weaker grades. Doorways have a soft side that must
face in. At runtime Rust batches the pieces' colliders per building
with a lookup back to the piece and rebuilds the batch when a piece
changes; meshes are batched the same way.

Topside's model: a building is a list of pieces (modforge `Piece`:
kind, box, colour, health), the single source of truth. Everything
the player sees or bumps into is baked from the list (one collider
and one mesh per colour per building) and rebaked once when the list
changes, never per frame. A hit on the building resolves to the
piece under the hit point; damage edits the piece; a dead piece is
removed and the building rebaked. Monuments roll rooms and flatten
them into the same pieces, so a monument building and a player base
are the same thing: capturable, damageable, kept or swapped by the
storm by owner. Grades, stability, the snap grid, the cupboard, and
upkeep are the next layers on the same list.

## Seamless, no loading screens (operator-locked 2026-08-19)

One continuous world, zero loading screens. The bunker is geometry in the
same scene; the door is a door, not a level transition (the Valheim / Rust /
STALKER interior model). At our scale (1000 people, dozens of monuments)
the whole world fits in memory at once.

The storm hides the real work: world regeneration runs while the player is
hiding inside a shelter, unable to see the world being remade; the whiteout
IS the work window (the elevator trick, made diegetic). Regeneration
streams in slices across frames, nearest terrain first, so there is no
hitch when the player steps out into the new world.

## Factions and relationships

Factions are not just war. They are relationships (operator-locked
2026-08-20: "our faction system should be more than just war. its
relationships"). Your behavior toward a faction determines the
relationship:

- Kill their people and they become hostile: no trade, kill on sight,
  raids on your bunker.
- Leave them alone and they stay neutral: cautious but willing to trade.
- Help them (trade, defend their territory, return their people) and
  they become friendly: better trade prices, shared intel, mutual
  defense.

There are different kinds of factions (operator-locked 2026-08-20:
"there should be hostile factions and builder factions. like we have in
endless"):
- Hostile factions: raiders, warlords, bandits. They want your stuff
  and your territory. Trade is unlikely, war is default.
- Builder factions: settlers, traders, craftspeople. They want to grow
  and trade. Violence is a last resort for them.
- The player's faction: your bunker, your people, your territory.

Neutral NPCs are NOT part of any faction (operator-locked 2026-08-20:
"neutral npcs shouldnt be part of factions. they're neutral"). They
wander the world on their own. You can trade with them for what they
carry, rob them, recruit them into your faction, or leave them alone.
They are the wild card between faction territories.

### Trading

You can trade with any faction you find, not just one safe spot
(operator-locked 2026-08-20: "i dont like how misery lets you go to
one safe trader spot and thats it. its boring. i think you should be
able to trade with any of the factions that you find. and then there's
also wandering NPCs that you could fight or trade with").

Trading happens where factions are: at their territory, at their
bunker, or when you encounter their people in the world. Wandering
neutral NPCs also trade what they carry. Your relationship with a
faction determines whether they trade and at what prices. Hostile
factions refuse. Neutral factions trade cautiously. Friendly factions
give better deals.

Factions have real stores (the world is the truth): they trade from
what they actually have, not from an infinite shop. A builder faction
that controls a workshop has crafted goods to sell. A faction that
lost their warehouse to a raid has less to offer. The economy is the
simulation, not a menu.

### Faction war

Faction war is part of the relationship system. Each faction has a
bunker of its own, and the faction bunkers are THE persistent, consistent
things as the world regenerates, exactly like the player's.

- An enemy faction can raid you; you can raid them.
- Their bunker stays the same across storms, but the path there and back
  might be different each time, because the world between the bunkers
  re-rolled.

So a rivalry survives any number of storms: you know WHO your enemy is
and what their bunker is like, but never what lies between you and them.
The faction behavior itself (who raids whom, grudges, trade, war and peace)
is the genome and acts systems from the authority map, proven in the
survivalist mod.

### Territory: factions fight for buildings

Buildings in the world (the ones monuments are made of) can be captured
and held by factions, including the player's. A captured building belongs
to whoever holds it until someone else takes it or the storm re-rolls
the map.

Holding a building provides real bonuses:
- A warehouse feeds your people (food production or storage).
- A workshop lets you craft better gear (unlocks recipes or improves
  quality odds).
- A guard tower gives early warning of incoming raids.
- A lab unlocks research or rare crafting.
- A supermarket provides trade goods.
The bonus depends on the building type, and losing a building means
losing the bonus. This is what makes the war personal: they took your
workshop, now you cannot make ammo.

Capture works through presence and force. Move your people in, clear
out whoever is there, hold the ground. NPCs from your faction garrison
the building and defend it. Enemy factions do the same thing with the
same rules (the player and NPCs share one mechanic path, as always).

The storm resets the board. When the world regenerates, all captured
buildings vanish with the old map. New monuments roll with new buildings,
and the territory war starts fresh. But the faction rivalries carry
forward: you remember who raided your last workshop, and they remember
you. The grudges are permanent even when the ground is not.

This is the 2000-hour loop. Survival pressure gets solved, but the
faction war never does. There is always a building worth taking, always
someone trying to take yours, always a reason to go topside. The bunker
is home, the territory is the game.

## Monuments and timed events (from Rust)

We do not have to be unique; we take good ideas and implement them well.
Rust is the model here:

- MONUMENTS: destinations in the topside world worth traveling to, with
  extra phat loot in their dangerous spots (Rust's card rooms, Oil Rig where
  heavies come and you get crates, underwater lab, Dome).
- TIMED EVENTS: special events on a clock, like Rust's Cargo, that pull you
  out of the bunker toward risk and reward. These ride the storyteller.
- RAIDS exist (see bunker defense).

Where topside goes MORE EXTREME than Rust: in Rust the monument names stay
the same while the layout varies; here the randomization goes further, and
even the monument NAME is random. You never walk into a monument already
knowing it.

## The unknown (operator, 2026-08-21: "i wonder what's out there")

The feeling the game is built for is "I wonder what's out there", never
"I wonder what's left on the list". The player should not know what is
going to happen. Operator-locked 2026-08-21: "finding out is the
game." Not the fight, not the loot, not the base: finding out. Every
system below is judged by whether it gives the player something to
find out and makes them work to find it. Operator-locked 2026-08-21: "size doesn't make
memories. moments do." A bigger map, more sites, more walls are not
the goal; the moment the player tells someone about afterward is.
Every generator and every site is judged by the moments it can make,
not by its size. Reference games, and what each one does that we keep:

- Fallout 2's random encounters: walking the map rolls things you could
  not predict. The Mysterious Stranger who shows up in a fight, the
  crashed whale, the Cafe of Broken Dreams, the bridge keeper. Some
  help, some kill you, some are a joke, and you never know which roll
  you got until you are in it.
- Morrowind: no quest markers, directions in people's words, and
  one-offs that never repeat (the wizard falling out of the sky). The
  one-offs prove the world can surprise you, so you keep believing it
  can.
- Secret rooms (Doom, Zelda, Spelunky): a place exists that nothing
  announces. You find it by noticing something off.
- Breath of the Wild: you see the hint (smoke, a light, an odd shape
  behind a ridge), not the thing. You walk toward a question.
- STALKER's A-Life: a place you explored is unknown again when you come
  back, because the people moved.

What we do NOT do: the Assassin's Creed watchtower. No map reveal, no
climb-and-see-everything, no minimap, no compass, no heading indicator,
no system-placed waypoints or markers of any kind. The map is a
fog-of-war panel the player opens deliberately, showing the bunker,
where the player has walked, and what people told them (see "What a
person knows" in life.md). Nothing the player has not found appears on
it. The player can place their own marks on the map (operator,
2026-08-22: lower priority). No other orientation aid exists; the
player navigates by landmarks, memory, and what people say.

The rules that force the unknown into what we build, so it is data and
not hope:

1. Random encounters ride the storyteller. While the player walks,
   Randy Random rolls encounters from a registered table: a stranger
   who joins a fight, a merchant in the middle of nowhere, a body with
   a note, a band crossing the road, something that fell from the sky.
   Good, bad, and strange all roll from the same table. The player
   never sees the roll coming.
2. One-offs roll at most once per world. A registry of things that can
   exist only once (a building type that exists once, a person with an
   impossible item, a crashed thing). The storm re-rolls which ones
   this world has, so two worlds have different oddities.
3. Every site type can hide a part. A monument or building roll may
   carry a secret: a basement not on the stairs, a walled-off room, a
   box under a floor piece. The generator never advertises it. The
   player finds it by looking: a window count that does not match the
   rooms, a sound, a faint light.
4. Show the hint, hide the thing. Worldgen puts minor sites where the
   road does not see them; what the road sees is smoke, a light at
   night, birds, a shape.
5. Coming back is not safe. The storm and the people mean a visited
   site can be different next visit: a faction moved in, the loot is
   gone, the door is now barred from inside.
6. Not every question gets an answer. Some bodies just lie there with a
   note. The dread loop's "lingering mystery" governs sites and
   encounters, not only incursions.

7. Every map rolls 0 to 5 secrets (operator, 2026-08-21: "every map
   should have the possibility to have 0-5 secrets"). A secret is a
   place the roll hides on purpose: off every road and site, and in a
   maze behind a wall. Zero is a legal roll; the player never knows
   how many this world has.
8. The assassin (operator, 2026-08-21: "there should always be the
   chance that an assassin comes after the player character. we'd
   need a cooldown of course. but the assassin should always be
   (10-200) levels higher than the player. when the assassin comes
   sometimes the only possibility is death"). At any time, on a
   cooldown, an assassin may come for the player. It is always 10 to
   200 levels above the player, rolled, so sometimes it can be fought
   and sometimes the only way out is to die. It rides the
   storyteller's Randy Random side (never adaptive), so it can come
   when the player is weakest as easily as strongest. The assassin is
   an actor through `spawn_actor` like everyone else; levels come
   from the RPG row.

The triangle rule (operator, 2026-08-21, Breath of the Wild's rule
from its GDC 2017 talk): "big triangles as landmarks. medium
triangles blocking your view. a hill you can't see over is a
question. small triangle, the ground rhythm. every triangle has two
purposes. it gives you a goal and hides a surprise." What pulls a
person toward something far away is a question they cannot answer
from where they stand, and a triangle is a question: you see the
top, you cannot see behind it. So the generators place in three
sizes. Big: a few things visible from almost anywhere, never
labelled (a tower, a tall ruin, a light over the maze). Medium: the
hills, ridges, and wall runs that hide what is past them, placed so
the road bends around them and the far thing disappears and comes
back. Small: the bumps, rocks, and scatter that make the ground a
rhythm instead of a plane. Behind every one of them, something: that
is the surprise half. A site is placed behind a triangle, never in
the open beside the road. In the Labyrinth the walls are all medium
triangles; its big ones must rise over them (a gate's tower, smoke,
a light at night) or it has no far things at all.

History, not decoration (operator-agreed 2026-08-21). "You're not an
explorer, you're a man walking through a wallpaper pattern" is the
failure every generator here risks: once the player can predict the
next wreck from the last one they are pattern-matching, not
exploring. The answer the great ones found (Dwarf Fortress, Caves of
Qud, STALKER) is that things are where they are because something
happened. So before the player wakes, the world is run: the people
(the brain, needs, memory, factions) live for some days in the rolled
world. A camp is where squatters settled, a wreck is where a band
fought, a body is someone who died, the loot is what someone left,
and a note says what they knew. The minor sites stop being stamps
and become traces. Then the world keeps running, so coming back
finds what the people did since. Every site should ask a question
(whose body, why is the door barred from inside) whose answer is
another place or a person; not every question is answered.

Minor sites are the danger here: hundreds of "a box and some scatter"
is exactly the samey feeling this section exists to kill. Every minor
site type must carry at least one of rules 3, 4, or 6.

### Building types

Buildings are the pieces monuments are made of. Each building type is a
StructureDef generator: same type, different result every time. The types,
drawn from Rust's monument buildings and adapted for topside:

Small (one room, one floor):
- Gas station: pumps outside, small shop inside.
- Convenience store: shelves, counter, back room.
- Water well: open structure, cistern.
- Guard tower: tall narrow room, ladder or stairs, lookout top.
- Shack: one room, minimal, boarded windows.
- Lighthouse: tall cylinder, spiral stair, lamp room at top.

Medium (two to four rooms, one or two floors):
- Warehouse: large open floor, loading dock, office corner.
- Office building: hallways, small rooms, stairs between floors.
- Workshop: garage bay, tool storage, parts shelves.
- Barracks: bunk room, mess hall, armory closet.
- Lab: clean room, sample storage, monitoring station.
- Supermarket: wide aisles, checkout area, back storage.

Large (five or more rooms, two or more floors):
- Hangar: massive open bay, catwalks, control room.
- Industrial plant: machinery floor, control room, pipe runs, storage.
- Silo: deep vertical shaft, access rooms at each level.
- Dock building: long structure along water, crane platforms, cargo holds.
- Apartment block: three to ten floors of small rooms, many windows.
- Hospital: three to six floors, wards and corridors, a basement or two,
  the clean palette.
- Bunker complex: one floor above ground and five to ten below, no
  windows, the deepest thing in the world.

The last three arrived with the ruined city (2026-08-20, from the
operator's "a monument with 50 buildings, each with 20+ rooms"): the
city needed tall and deep buildings the first list did not have.

Each building type has parameters that vary on every roll (operator-locked
2026-08-20: "the buildings should have parameters that change"):
- Room count within the type's range.
- Door and window placement.
- Color palette (within the type's feel).
- Furniture and interior layout.
- Condition (intact, damaged, partially collapsed).
- Floors: up to ten up, up to ten down like a bunker, or both
  (operator, 2026-08-20: "they should be up to 10 floors up OR down
  like a bunker", "BOTH up and down"). Levels connect through one
  switchback stair tower.
- Scale: a monument can be fifty buildings of twenty or more rooms
  (operator, 2026-08-20: "a monument with 50 buildings, each with 20+
  rooms"). Footprints are carved into L and step shapes so buildings
  stop repeating (operator: "some of the buildings are boring and
  repeating").

### Monument composition

A monument is a cluster of buildings picked from a list, never the same
twice (operator-locked 2026-08-20: "it shouldnt ever be consistent because
consistent is boring"). Two layers of randomization:

1. The monument picks which building types to include from its possible
   list (not all, not always the same ones).
2. Each picked building generates with randomized parameters.

So a "roadside stop" monument might roll a gas station and a shack one time,
a convenience store and a guard tower the next. And each of those buildings
has different room counts, door placements, and colors every time.

A third layer, from Tales of Maj'Eyal (operator, 2026-08-22, references.md):
random layout, authored pieces. A library of hand-made rooms (a barred
storeroom, a collapsed stairwell with a body, a radio room, a workshop with
the tools still out) that the building roll drops into its rolled rooms the
way ToME drops vaults into a dungeon. Rolled boxes give the shape; authored
rooms give the moments. This is how the minor sites and the monuments stop
being "a box and some scatter" (rule 3 and 4 above) without hand-placing
anything in the world: a person designs the room, the roll decides where
it stands. The roll also mixes fixed and rolled per building (ToME's
per-level override): a monument type may name one authored building that
always stands (the launch site's gantry), the rest roll.

Monument types (from Rust's monuments, adapted):
- Roadside stop: 1 to 2 small buildings along a road.
- Airfield: 1 hangar, 1 to 2 small or medium buildings, open tarmac.
- Power plant: 1 industrial plant, 1 to 3 medium support buildings.
- Train yard: 1 to 2 warehouses, 1 to 2 small buildings, rail lines.
- Military outpost: 1 barracks, 1 to 2 guard towers, walls.
- Research station: 1 lab, 1 to 2 medium buildings, fenced perimeter.
- Harbor: 1 dock building, 1 to 2 warehouses, waterfront.
- Junkyard: 2 to 3 shacks or workshops, scrap piles, fencing.
- Sewer: underground access, 1 silo, maintenance rooms.
- Launch site: 1 hangar, 1 industrial plant, 1 to 2 medium buildings,
  the most dangerous and rewarding monument.
- Ruined city: 50 large buildings (apartment blocks, hospitals,
  industrial plants, silos, dock buildings, bunker complexes, hangars)
  on city blocks, gated, danger 4; the "monument with 50 buildings,
  each with 20+ rooms" (operator, 2026-08-20) and the performance test
  for everything (performance.md).

Each monument type also defines:
- Loot spots: where crates and items appear, scaled by danger.
- NPC spots: where guards or hostiles spawn.
- Gates: locked areas requiring progression (Rust's card system or
  topside's equivalent) to access the best loot.
- Arrangement rules: how the buildings sit relative to each other
  (clustered, along a road, around a yard).

## Biomes

The world is painted in biomes (Rust the game's model: regions with
their own look, resources, and hazards). Operator-locked 2026-08-20:
"bios will control weather, possible monuments, possible npcs,
wildlife", and probably more: "the point is that biome should be the
definition of the generic type of area we want. then our generator
can build that biome." A biome decides its weather, which monuments
can roll there, which NPCs can appear there, and its wildlife, plus
its ground look, what nature scatters there, and what can be
harvested there. The def is the generic description; worldgen builds
concrete areas from it, and the storm re-rolls the built areas,
never the defs.

## The world (operator, 2026-08-21: "lets do worldgen")

The world is rolled from a seed as data, then built, the same way
a monument is rolled then spawned. What a roll decides:

- Terrain: a heightmap from layered noise (Red Blob Games' "making
  maps with noise", the recipe Endless's continents and world map
  styles already used). Hills and valleys, not a flat slab.
- Biomes: each cell of the map gets a biome from the registered
  biome defs by its height and moisture, so the defs drive the
  generator as decided above.
- Monuments: sites picked with spacing between them, on land, each
  in a biome that allows that monument type; the monument at each
  site rolls as before.
- Roads: Rust's model. Roads are laid by the generator between the
  monuments by pathing over the heightmap, following the easy slopes,
  so a road is how you find the next monument and the roadside stops
  and gas stations sit along it.
- No road leads to the bunker (operator, 2026-08-22): "this is a game
  of alternate realities. In the first reality this bunker was hidden
  somewhere to survive the apocalypse. It worked. But the fact that it
  was hidden means no roads. In our generated worlds there may or may
  not be roads depending on the history of that reality." The first
  world has none; a generator whose history says otherwise may lay
  one.
- The bunker stays where it is; the world rolls around it.

Rivers, rain and wildlife per biome, and the people who live at the
sites come after the first world stands. The storm re-rolls the
world by rolling a new seed through the same path and rebuilding
everything except the bunkers and the people.

Many generators, very different (operator, 2026-08-21: "i want my
generators to be VERY different... pretty extreme"). The mixed world
above is the first generator, not the only one. Each storm may hand
the actors to a different reality made by a different generator, and
the generators should be extreme in their differences, not variations
on one heightmap: a drowned world where the monuments are on the
ridges and everything else is under water; a city that never ends; a
desert of one road; a world of islands; a world that is mostly
underground; a world where the buildings stand but the ground is
gone. Prior art for a game whose worlds differ this much between
rolls: Minecraft's world types (amplified, superflat, large biomes),
Terraria's seeds (drunk world, not the bees, for the worthy), No Man's
Sky's planet kinds, Noita's biome stack. The rule for us: a generator
is a `WorldDef` plus the code that rolls it, every generator ends in
the same `World` data so terrain, sites, roads, and `spawn_monument`
do not care which one rolled it, and the set of generators is part of
"The unknown": the player does not know which reality the storm will
drop them in.

### The generators we want (operator, 2026-08-21)

The mixed world above is the first. The next two:

The Backrooms. The 2019 creepypasta: "noclip out of reality in the
wrong areas" and you are in "six hundred million square miles of
randomly segmented empty rooms", yellow wallpaper, damp carpet,
fluorescent hum, and something wandering that has already heard you.
The fandom grew it into numbered levels, each with its own look and
its own entities, and exits that drop you to the next level. For us:
a reality with no sky and no terrain, only rooms. The generator rolls
rooms, halls, and openings forever around the actors, the way the
mixed world rolls chunks; monuments are the places that break the
pattern (a flooded hall, an office, a pool, a parking deck); minor
sites are an odd door or a hum that changes; the people are whatever
crossed with you plus the things that live here. This is the reality
that proves the rule that a generator need not be a heightmap: the
`World` data needs to carry rooms, not only cells. It is also the
purest form of "The unknown": the map is useless, every hall looks
the same until it does not.

The Death Gate worlds (Weis and Hickman, 1990 to 1994). The Sundering
split one world into four elemental ones plus a prison, and each is a
complete way to live that is nothing like the others. We take them as
generators, one each:

- Arianus, the world of air: porous floating islands of coralite in
  three bands of altitude, the Maelstrom, an eternal storm, filling
  the bottom third. Water is scarce and made by one continent-sized
  machine (the Kicksey-winsey) in the low realm, and the people who
  hold it hold everyone. For us: a heightmap with void cells, islands
  at three heights, falling is death, the storm lives below you
  instead of coming over you, water is the scarce pressure, the
  machine is the one monument everyone fights over, and crossing
  between islands needs a vehicle.
- Pryan, the world of fire: a hollow sphere lit by small suns at its
  centre, so there is no night, covered in jungle miles high; people
  live in the canopy and on country-sized moss mats, and almost nobody
  has seen the ground. Giants roam, asking a question and killing
  whoever cannot answer. For us: the day clock has no night, the
  world is a canopy with holes to a ground you never reach, citadels
  are the monuments, and one wandering actor is bigger than anything
  else and not hostile until you fail it.
- Abarrach, the world of stone: solid rock honeycombed with tunnels,
  volcanic at the core and frozen at the edge, livable only in the
  band between; a lava sea, poison air, and a people who keep their
  dead working. For us: the underground generator, heat and
  radiation as the pressures, the livable band as the map, the dead
  as a faction.
- Chelestra, the world of water: an egg of ice with a seasun moving
  through it, thawing and refreezing the floating islands people live
  in as it passes; the water itself undoes the old magic. For us: a
  world that changes while you are in it (the sun's path thaws and
  freezes the map on a clock), boats, and water that ruins gear.
- The Labyrinth: a prison world that hates you, a maze split by gates,
  where the people are runners (keep moving) or squatters (hold a
  place). For us: a maze generator whose walls move on the storm's
  smaller cousin, gates as monuments, and the two ways of living as
  the factions. This one is the game's own theme, chaos against
  order, made into a map.

Each of these is one `WorldDef` plus its roll, each ends in the same
`World` data, and none of them is built until the mixed world is
done. Order after the mixed world: the Backrooms first, then the
Death Gate worlds, the Labyrinth before the others because it is the
closest to what the game is about.

### The Labyrinth, mined (operator, 2026-08-21: "i like the idea of
### stalker anomalies and traps. this is a labyrinth after all. and
### honestly lets mine the death gate cycle books for ideas")

The Labyrinth is built as data: 10 m corridors (operator: "wide
enough for 4 people"), walls as high as the corridor is wide, paths
and dead ends, the bunker in a clearing the maze grows out of. That
is the stage. What makes it a place is that the maze is the
antagonist: in the books it is sentient, it hates its prisoners, and
it moves its own terrain and creatures to kill them. Everything
below follows from that one fact.

From STALKER, anomalies and the bolt:

- An anomaly is a patch of corridor that kills: a vortex that pulls
  and crushes, a burner that flares when something enters, an
  electro that arcs, a chemical pool. Each has a tell (a shimmer,
  scorched ground, a hum, bones) that a careful player reads; a
  careless one walks in. Anomalies live in dead ends and narrow
  spots where the maze wants you to go.
- The bolt: the player throws something cheap ahead and watches. This
  is the whole of STALKER's exploration feel in one action: the world
  answers what you do. Throwing a bolt is an action on the queue like
  any other.
- Artefacts: the reward is inside the anomaly. Some of the best loot
  in the maze sits where going to get it can kill you. That is the
  "it can kill you and you go anyway" rule made concrete.

Traps, because it is a labyrinth: a floor piece that gives way (a
pit), a corridor that closes behind you (the walls move), a dead end
that was not a dead end a minute ago, a gate that shuts when the
guard sees you. Traps are pieces and walls, the same data as the
rest, so the player can break them, and the maze can re-lay them.

From the books (Weis and Hickman, Into the Labyrinth and The Seventh
Gate; the fandom wiki and TV Tropes read 2026-08-21, the rest from
memory of the books and marked so):

- The walls move. The Labyrinth manipulates its terrain against you.
  For us: on the storm's smaller cousin, runs of wall vanish and new
  ones stand, a corridor you walked is not there next time, and the
  map you drew yourself is wrong. This is the maze's own version of
  "coming back is not safe".
- Gates. The prison is crossed by numbered gates toward the centre;
  each gate is a milestone a runner counts. For us: gate monuments
  across corridors, locked, guarded, stocked, and knowing how many
  gates you have passed is the only measure of progress the maze
  gives you. The way out is through the last one.
- Runners and squatters. The two ways to live there: keep moving, or
  hold a place and raise children in it. For us: two factions. Runner
  bands roam and fight anything; squatters hold a gate or a clearing
  and trade if you are not a threat. A runner who stops becomes a
  squatter; a squatter camp that is overrun scatters into runners.
- The creatures, sent by the maze: wolfen, man-sized wolves hunting
  in packs of thirty to forty; chaodyn, insect things (from memory:
  killed in one blow or they split into two); snogs; tiger-men;
  dragons. For us: the Labyrinth's NPC pools are these, not raiders,
  and each is a rule, not a reskin. A wolfen pack is a crowd that
  flanks in corridors; a chaodyn punishes a weak hit; a dragon is the
  thing you hear and never want to meet.
- Runes. Patryns carry their magic as tattoos and read runes on
  things; runners mark the way. For us: marks on walls left by people
  who passed (a rune, a scratch, an arrow, a warning), part of what a
  person knows and tells. Reading the maze's marks is the Outer Wilds
  move: knowledge is the progression.
- The dog. Haplo's dog goes where he cannot and comes back. For us,
  later: an animal companion is an actor whose queue the player can
  fill with one order (go, come, stay), and the first thing the
  player can send into a dead end instead of themselves.

The order to build the teeth, after the stage stands: anomalies with
the bolt, then the gates, then the runners and squatters, then the
walls that move, then the creatures.

Sources read 2026-08-21: [The Backrooms (Wikipedia)](https://en.wikipedia.org/wiki/The_Backrooms),
[The Death Gate Cycle (Wikipedia)](https://en.wikipedia.org/wiki/The_Death_Gate_Cycle),
[Dragon Wing (Wikipedia)](https://en.wikipedia.org/wiki/Dragon_Wing).

## Your people

Your bunker's people run themselves by default, on the same genome and acts
systems as every AI faction. The player can order them around as needed:
orders are an override on a self-running crew, not a required management
layer. Nothing falls apart because you did not micromanage; the Oregon Trail
frame applies to them too: named companions who can come topside with you
and die along the way.

Soulmask is the model for NPC control (operator: it does a good job, we want
similar): recruited people with their own skills and traits, assigned to
work stations and duties they then run autonomously, guard and follow roles,
companions at your side in the world.

## Player and NPC progression

The player character and every NPC grow through the same RPG system:
modforge::rpg (operator-locked 2026-08-20: "modforge already has rpg.
we're definitely using that here too"). Skills, leveling, specialization,
all running on the module that survivalist-mod already uses and proved.

The player and NPCs share one mechanic path (as always). A player who
crafts a lot gets better at crafting. An NPC who fights gets better at
fighting. This is use-based progression: you improve at what you do,
not what you choose from a menu.

What skills affect:
- Crafting: higher skill means better quality output, faster crafting,
  access to harder recipes.
- Combat: higher skill means more damage, better accuracy, faster
  reloads.
- Medicine: higher skill means better healing, less waste on medical
  supplies.
- Construction: higher skill means faster building, stronger results.
- Research: higher skill means faster research, reduced component costs.

Skills matter alongside gear and tech. A high-skill crafter at a basic
bench outperforms a low-skill crafter at an upgraded bench. Gear raises
the floor, skill raises the ceiling.

Your people's skills matter too: assigning a skilled crafter to the
armory bench versus a rookie changes what you get. Recruiting NPCs
with high skills in what you need becomes part of the faction game.

## The HUD

ONE HUD, made of panels. The HUD is the whole player-facing information
layer: inventory, skills, health, food, prompts, the work board, the
chronicle, the map, and anything else the player looks at that is not the
3D world. All of it lives in modforge as engine-agnostic data and
decisions (operator-locked 2026-08-20: "our whole game hud structs and
systems and all engine agnostic should be in modforge. so we have ONE
way we do all that stuff"). No binder trait: the consumer writes HudState
fields directly from its game systems and reads HudState to paint the
panels (operator-locked 2026-08-20: no extra layers, no complexity, keep
it clean). No second way to show the player information.

## Items and inventory

Inventory is slots and stacks (the Rust / Valheim-without-weight model).
No inventory weight, operator-locked 2026-08-20: "i hate inventory weight.
i think its a dumb mechanic and not fun gameplay."

The starting bunker's three days of food are real items in the bunker,
on this same item system (operator, 2026-08-20: "the food in our starting
bunker should be items in our bunk aligned with this syte"). The food
clock reads what is actually in the bunker's stores; no separate counter.

## Survival pressures

The world is always trying to kill you (operator-locked 2026-08-20:
"we need thirst. hunger. weather including radiation storm and heat
wave. i played 2000 hours of atlas so kinda built like that from a
survival perspective"). Atlas is the reference for how these pressures
stack and never let up. The goal section says it: Food. Water.
Resources. Enemies. Scarcity. Heat. Radiation. All of these are real
systems, not flavor.

Operator, 2026-08-20: "the food item should be a modforge item. that
item should be picked up and go into the player's inventory. then we
need a system for survival stats with hunger and thirst. that should be
in modforge. the survival system belongs to each npc entity AND the
player entity". So: survival stats are a modforge module, one per
actor (every NPC and the player carry the same stats and the same
rules); food is a registered item that restores hunger when eaten.
Eating is done through the inventory HUD or the hotbar, like Rust
(operator, 2026-08-20: "eating will be through the inventory hud that
we'll build. or the hotbar. like rust"). No eat key.

Item interaction mirrors Atlas (operator, 2026-08-20: "i like atlas
interaction", "mirror on atlas controls"): I or Tab opens the
inventory (operator, 2026-08-20: "i think i like tab better. its more
natural. can we have both for inventory?"),
double click uses an item (eat food, equip gear), drag and drop moves
stacks, hover plus T transfers the stack to the other open inventory
(Shift+T half), hover plus O drops it on the ground, 0-9 uses the
hotbar slot, Shift+0-9 equips to it, E picks up in the world, F opens
another's inventory. E on a crate opens it; E again, Escape, Tab, or I
closes it (operator, 2026-08-20: "make esc and e again close it").

An actor carries three separate item holders, each its own thing
(operator, 2026-08-20: "a player has inventory. they have hotbar. and
they have equipment slots. all of those take items and should be their
own thing in the struct"): the inventory, the hotbar, and the
equipment slots. Not one list with index ranges.

Everything placed in the world from the item system is a WorldItem:
the food on the bunker floor, the storage box, a crafting bench
(operator, 2026-08-20: "we need worlditem to handle ALL the
worlditems"; "it's only the storage box in the world that's a world
item or a crafting bench"). Items inside a box are stacks in its
inventory, not WorldItems. One kind of thing in the world, one spawn
path; a storage box is a WorldItem that also holds an inventory.

The player and every NPC are the same kind of actor on the same
structs; the only difference is who drives them (operator,
2026-08-20: "npc action and human action need to be built on top of
the same structs. the only difference is the human actor is human
controlled while the npc is ai controlled. this keeps it fair and
makes a lot of possibilities"). Every action an NPC or the player
takes (eat, equip, move a stack, pick up, drop, attack) is one
function on those structs; input and AI only choose which to call.

Every NPC is one person with one name for life (operator, 2026-08-21,
on the identity review): an ActorId issued once at spawn and never
reused, kept across save and load and across the storm. That is what
makes the 1000 people persistent instead of scenery: the same id is
what a faction, a grudge, a trade, the chronicle, and the save all
point at. The Bevy entity and the GPU row are bookkeeping and can
change; the ActorId cannot. One registry, the ActorMap, issues it and
answers who is who ("have it working clean in our actormap").

What counts as an actor (operator, 2026-08-21): anything that MOVES or
SHOOTS. People, animals, turrets, vehicles. A storage box, a bench, a
dropped can, a wall only sit there and get used, so they are
WorldItems, not actors ("the worlditems dont MOVE or SHOOT"). A
turret is an actor that happens to be placed from your inventory: it
aims and fires through the same combat path as a raider, takes hits
and dies the same way, and the only difference from an enemy is its
inventory slots and that it does not walk. A vehicle is an actor too,
the Rust and Quake way: while you drive, your controls go to the
truck instead of your legs, and an NPC drives it the same way you do.
An empty parked truck is an actor doing nothing, like a sleeping
raider. This is why there is one spawn path and one AI: a raider in a
truck is the raider's brain on the truck's body.

Healing: food restores health and hunger (Valheim model,
operator-locked 2026-08-22: each food can have multiple benefits,
different foods restore different amounts of health, hunger, or
both). This is the primary healing mechanic. A craftable bandage
from the crafting bench restores health faster than food, giving
the player a reason to craft before going outside and a way to
recover quickly after combat. Healing applies to every actor
through the same path (player and NPC alike).

Hunger: you eat food items from your inventory or your bunker's stores.
No food, you starve. The three-day starting clock is the first taste.
Food spoils over time (perishability forces you to keep scavenging,
never stockpile forever). Different foods restore different amounts
of hunger and health. Cooking at a station makes food last longer
and restore more.

Thirst: you drink water. Clean water is crafted (purification at a
chemistry table) or found. Dirty water works but makes you sick.
Dehydration kills faster than hunger. Water sources exist in the world
(wells, rivers, rain collection) but depend on biome and weather.

Temperature: biomes have base temperatures. Heat waves and cold snaps
are weather events the storyteller rolls. Extreme heat drains water
faster and causes heatstroke. Extreme cold drains food faster and
causes hypothermia. Clothing and shelter counter temperature. The
bunker is always temperate (deep underground, insulated).

Radiation: radiation zones exist near certain monuments (the most
dangerous, most rewarding ones). Radiation storms are weather events
that irradiate the open world temporarily (different from the big
storm that regenerates everything). Exposure builds over time.
Protection comes from gear (hazmat suit, meds) and shelter. High
radiation forces hard choices: is the loot worth the exposure?

Weather: weather is per-biome, driven by the storyteller. Types
include rain, fog, heat wave, cold snap, radiation storm, and the
big storm (the regeneration event, its own system). Weather affects
visibility, temperature, radiation, and NPC behavior. NPCs react
to weather the same way the player does (same mechanic path):
they seek shelter from radiation storms, slow down in heat, and
so on.

All survival pressures apply equally to the player and NPCs (one
mechanic path, as always). An NPC caught in a radiation storm takes
the same damage you would. A faction that controls a clean water
source has a real advantage. Survival pressure is the baseline that
never goes away, even at 2000 hours: tech and gear push back against
it but never eliminate it.

## Technology and crafting

The technology tree is DEEP (operator-locked 2026-08-20: "the factorio
technology tree is deep. i want to model on that. with DEEP levels of
research and infinite upgrades. crafting too"). Factorio is the model
for tech depth; Rust is the model for what you build with it.

Research is how you unlock everything beyond the starting crafting bench.
Early research is cheap and fast (better tools, basic building materials).
Mid-game research costs rare components found in monuments (this is what
makes monument runs matter beyond loot). Late-game research costs
components from the most dangerous monuments (the launch site, the
military outpost). The tree branches: you cannot research everything at
once, so you specialize and trade with factions for what you lack.

Crafting stations, each unlocked by research:
- Crafting bench: the starter. Basic tools, simple repairs.
- Chemistry table: medicine, fuel, explosives, chemicals.
- Electronics workbench: wiring, circuits, powered components.
- Armory bench: weapons, armor, ammunition.
- Research station: where research happens, consuming components.

Each station tier can be upgraded (better bench, better output). Higher
tier stations produce higher quality results and unlock recipes the
lower tier cannot see.

Infinite upgrades (the Factorio model): certain research topics have no
cap. Each level costs more than the last and gives a smaller boost.
Examples: crafting speed, turret damage, armor durability, resource
efficiency, power generation output. These are the 2000-hour grind:
there is always one more level to push, and each level makes a real
(if small) difference.

The crafting loop connects to everything:
- Materials come from topside (scavenging monuments, harvesting the
  world, trading with factions, raiding enemy territory).
- Research components come from dangerous monuments (the progression
  gate that keeps you going topside).
- Crafted output goes into your bunker (stations, power, defenses,
  storage), your gear (weapons, armor, tools), and your people's
  equipment.
- The territory system feeds back: a captured workshop gives a crafting
  bonus, losing it hurts your output.

## Combat

Post-apocalyptic means everything is a weapon (operator-locked 2026-08-20:
"melee. guns. clubs. random crafted things. its all possible"). A pipe, a
sharpened piece of scrap, a makeshift club, a found pistol with three
rounds, a crafted rifle. The scarcity of the world shows in the combat:
early game you swing junk, late game you carry crafted firearms with
ammunition you manufactured.

The Bloodline is the reference for first-person melee feel: weighty,
directional, satisfying even with crude weapons. Rust is the reference
for gunplay: simple, lethal, ammo matters.

Weapon categories:
- Melee: clubs, pipes, blades, axes, spears. Crafted from scrap and
  materials. Always available, no ammo cost, higher risk.
- Ranged: pistols, shotguns, rifles, bows. Found or crafted. Ammunition
  is a real cost that limits use. Higher tier weapons need higher tier
  ammo.
- Improvised: anything the crafting system can make. A spiked bat, a
  nail bomb, a sharpened road sign. The crafting system's creativity
  shows here.

### How a hit works (operator-agreed 2026-08-20, from reading Quake 3,
### Half-Life 1, Doom 3, and Rust's devblogs; references.md)

The rules, engine-agnostic in modforge; topside only casts rays and
moves bodies:

1. One damage function for every hit: fists, bullets, arrows,
   explosions, a wall. It takes the target, what hit it, who fired,
   the direction, the point, and the damage def. Armor first, then
   health, then die or pain. Quake 3's `G_Damage`.
2. Damage is data. A weapon names a damage def in the registry:
   amount, type, knockback, armor rules, falloff by distance. Doom 3.
3. Damage types with resistances: slash, pierce, blunt, fire, and
   more; creatures and worn gear multiply per type. Half-Life's type
   bits, Valheim's resistances. Traits and affixes hang off this.
4. Melee is a short ray from the eye; if the line misses, a fat
   sweep takes its hit. Half-Life's crowbar.
5. Guns are Rust's hybrid: the first N metres (per ammo) are an
   instant ray, then the bullet is a projectile with gravity and
   drag. Close range feels like Quake, long range keeps drop. One
   code path, because a projectile step is a ray from last position
   to new (Quake 3's missiles).
6. Pellets add up per target: N rays with a seeded spread, damage
   summed per target, one damage call per target. Half-Life's
   multi-damage, Quake's seeded spread.
7. Fire rate is a timer on the shooter, ticked in the simulation:
   fire only when ready and the trigger is held; no ammo costs a
   delay. Quake 3's `PM_Weapon`. NPCs use it unchanged.
8. Protection per body area from worn gear (the equipment slots:
   head, chest, hands, legs, feet), not one armor number. Rust after
   it dropped hitboxes.
9. Hit location multipliers come when actors have parts; today an
   actor is one capsule. Half-Life's hitgroups.
10. The simulation decides hits, never the renderer: casts run in
    the fixed step against the physics world. Source's rule; single
    player needs no rewind.
11. Buildings are targets: a piece has health and no armor, and the
    same function routes to it.

Build order: the function and the defs, the timer, melee against the
pipe, the NPC spawn path so there is something to hit back, then guns
and pellets, then types and body-area protection as content grows.
Left out for now: lag compensation, Rust's penetration and ricochet,
Valheim's swept arcs.

### Enemy randomization

Enemies are randomized the way Diablo 2 randomizes its monsters
(operator-locked 2026-08-20: "the enemies just like everything need to
be randomized. think diablo 2 enemy randomization"). Every hostile NPC
or creature rolls random traits on top of its base type: extra fast,
extra tough, pack leader (brings friends), armored, aggressive, cautious,
nocturnal, territorial. A bandit with "extra fast" and "pack leader" is
a completely different fight than one with "armored" and "cautious."

Trait combinations create emergent difficulty. The player never knows
what a fight will be before it starts. The same monument run with
different enemy rolls plays differently every time.

Enemy traits use the same modforge genome system that drives faction
personality. Traits are not a separate mechanic bolted on; they are
the genome expressing itself at the individual level.

### Loot randomization

Loot follows the Diablo 2 affix model (operator-locked 2026-08-20:
"and diablo 2 loot randomization"). Items roll random affixes on top
of their base type and quality tier:
- A "sturdy iron pipe" and a "lightweight iron pipe" are the same base
  item with different affixes affecting durability and swing speed.
- A "precise hunting rifle" and a "rugged hunting rifle" differ in
  accuracy and condition.
- Affixes stack: a "sturdy, sharpened iron pipe" is possible and rare.

The quality system in modforge handles the tier (how good the base item
is). Affixes layer on top, multiplying the variety. The chase for the
right combination of quality tier and affixes is what keeps loot
interesting at 2000 hours, because Diablo 2 proved that systems
multiplying against each other beat a bigger random list (references.md).

### Loot tables scale with danger

The randomness is shaped, never flat (operator-locked 2026-08-20:
"there should always be logic to the randomness. high level loot box in
hard monument has phat possible loot. while box out in the open in low
level area has more common loot table"). The danger of the location
determines the ceiling of what can drop there:

- A crate in a roadside stop shack: common materials, low tier gear,
  basic supplies. A lucky find is possible but rare.
- A crate in a warehouse behind a locked door: mid-tier gear, useful
  components, better quality rolls.
- A crate in the gated room of a launch site: rare affixes, high
  quality tiers, unique research components, the best the game offers.

The same scaling applies to enemies: guards inside a gated monument
room roll from tougher trait pools than wanderers near a roadside stop.
Harder location means harder enemies and better rewards.

This is icarus difficulty (operator-locked 2026-08-20: "icarus
difficulty. it gets harder as you fly closer to the sun"). The further
you push from safety, the harder everything gets, but the rewards
scale with it. The bunker is the ground. The deepest room of the
hardest monument is the sun. Every step closer burns hotter: tougher
enemies, worse weather, higher radiation, but better loot, rarer
affixes, unique components. You cannot skip to the best drops by
getting lucky in a safe area. The better rolls are earned by pushing
closer to the sun.

## Death and difficulty

Topside has a custom difficulty level, and the player controls how death
works through it.

- Initially two death settings: drop items on death, or keep items on death.
- We build keep-items-on-death first because it is easier.

## The v1 session model (operator, 2026-08-22)

v1 is a roguelike session loop. Each session starts fresh: the player
wakes in the bunker with the starting items, the world rolls from a
new seed, and the 1000 people spawn at their sites. The storm provides
the within-session repeat (the world shifts to a different reality,
the player goes out again into a changed world with the same people).
Different seeds provide between-session variety: different questions,
different worlds, different NPCs, different loot rolls. Save and load
(v2) adds persistence across sessions. Until then, "worth repeating
once" means the player closes the game and starts a new session
because the last one was fun, not because they have progress to
continue.

## Cel shading, no textures

Everything in the game uses one cel shade material: a base color quantized
into hard light bands, no smooth gradients, no textures (operator-locked
2026-08-20: "i want all our stuff to be cell shading so i dont waste time
with textures until i want to. and maybe thats never tbh"). One material
struct, one shader, every mesh in the game goes through it. Colors only.
The material lives in topside (it is Bevy-specific presentation).

## Decided elsewhere

Presentation is decided in authority.md: 3D, first person, low poly with cel
shading, AI-generated assets, on Bevy.

## Future goals (not v1)

These are good ideas that are too hard or too expensive for the first
playable version. They stay here as documented goals, not promises.

- **Persistent NPC memory**: every NPC remembers what the player did to
  them personally (Shadow of Mordor's nemesis system, but both hostile
  and friendly). A bandit you beat comes back harder. A trader you
  saved gives better deals. A neutral you robbed warns others. With
  1000 persistent NPCs, that is 1000 individual relationships evolving
  over time. Hard to build, huge payoff when it works.
- **Vehicles**: traversal across the world (cars, bikes, mounts).
  Fuel, maintenance, breakdown risk. Added when the world is big
  enough to need them.

## Still open

The core-loop questions from the todo are all answered above. Open items
live in docs/todo.md.
