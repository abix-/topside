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
- The emission, taken from MISERY: after X time there is some event and the
  world changes. Keep similar timing to MISERY's at first. In topside this
  is the storyteller's beat; the full mechanic is its own section below.

## The emission

Occasionally the event occurs (like MISERY's emission):

- If the player is out in the world, they can hide in some building or
  similar shelter that MAY be safe. Caught in the open, they die.
- Then the world gets REGENERATED around them, EXCEPT the player's base and
  enemy faction bases, which persist.
- The 1000 persistent NPCs survive regeneration the same way bunkers do; the
  emission re-rolls terrain and monuments, never deletes people. People die
  only by dying.
- NPCs obey the same emission rules as the player: they hide from it just
  like you do, or die in the open. That creates natural tension around the
  'safe' places and leads to more situations: shelter is contested when the
  sky turns, and whoever is near your door when it hits becomes your
  problem, or you become theirs.

This is what keeps the world from ever being memorized: monuments, their
layouts, and their names re-roll with the world, while everything with an
owner (your bunker, the factions' bunkers) carries forward. The map is never
fully known, but the story of who holds what continues.

## Faction war

Faction war is part of this. Each faction has a bunker of its own, and the
faction bunkers are THE persistent, consistent things as the world
regenerates, exactly like the player's.

- An enemy faction can raid you; you can raid them.
- Their bunker stays the same across emissions, but the path there and back
  might be different each time, because the world between the bunkers
  re-rolled.

So a rivalry survives any number of emissions: you know WHO your enemy is
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

## Death and difficulty

Topside has a custom difficulty level, and the player controls how death
works through it.

- Initially two death settings: drop items on death, or keep items on death.
- We build keep-items-on-death first because it is easier.

## Decided elsewhere

Presentation is decided in authority.md: 3D, first person, low poly with cel
shading, AI-generated assets, on Bevy.

## Still open

The core-loop questions from the todo are all answered above. Open items
live in docs/todo.md.
