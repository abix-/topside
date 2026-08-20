# topside design

**Authoritative for:** the game's story, core loop, and player-facing design.
In progress: decisions land here as the operator makes them. The ownership
boundaries live in authority.md; the games we mine live in references.md.

## The story

You wake up in a bunker after a post-apocalyptic something. You know nothing.
Go.

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

## Faction war

Faction war is part of this. Each faction has a bunker of its own, and the
faction bunkers are THE persistent, consistent things as the world
regenerates, exactly like the player's.

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

Each building type has parameters that vary on every roll (operator-locked
2026-08-20: "the buildings should have parameters that change"):
- Room count within the type's range.
- Door and window placement.
- Color palette (within the type's feel).
- Furniture and interior layout.
- Condition (intact, damaged, partially collapsed).

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

Hunger: you eat food items from your inventory or your bunker's stores.
No food, you starve. The three-day starting clock is the first taste.
Food spoils over time (perishability forces you to keep scavenging,
never stockpile forever). Different foods restore different amounts.
Cooking at a station makes food last longer and restore more.

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

## Still open

The core-loop questions from the todo are all answered above. Open items
live in docs/todo.md.
