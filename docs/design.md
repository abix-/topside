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

It starts with exactly three things:

- a bed
- food for three days
- a crafting bench

The three days of food is the game's first clock: it forces you topside
before you are ready. The crafting bench is the seed everything grows from;
crafting is a core mechanic from the first minute.

The long game at home:

- Building the bunker out.
- Defending it from raiders (the storyteller's incursions come for YOU).
- Infrastructure.

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
