# topside authority map

**Authoritative for:** the one implementation path for each game system, the
ownership boundary between modforge and topside, the naming contract, and the
anti-bypass rules for those paths.

**Not authoritative for:** product vision and story (design.md, not yet
written), how fighting works (combat.md), how people find their way
(pathing.md), capability progress (status.md, not yet written), or the task
backlog (todo.md, not yet written).

Evidence date: 2026-08-19. Nothing is built. Every score below reflects that
honestly. This doc exists BEFORE the code so the code never gets a chance to
grow a second path.

## How topside works: every system (2026-09-26)

Every system in the game, documented once, so that whoever builds something
uses the path written here and never makes a second one (the shape of
endless/docs/k8s.md):

- **Def**: what the system's data is, as a type: a definition from content
  (`ActorDef`, `ItemDef`) or the system's own data (`memory::Memory`,
  `WalkMap`).
- **Registry**: the one place all of it is stored: a content registry
  (`ActorRegistryRes`), a component on each person (`Memory`), or one
  resource (`nav::Walk`).
- **How**: the functions everyone goes through to make, change, or read it.
- **Copies**: the control plane's published numbers, the HUD, and the
  drawn models read the owners and are never read back as the truth.

Rules:

1. Before building anything, find its system below and do it that way.
   Never a second path, a second record, or a second answer, even when every
   test passes. One authoritative system for every thing (operator,
   2026-10-02): no path only for loads, only for tests, only for the
   player, or only while paused. When one case misbehaves, the one path is
   fixed for every case.
2. Something with no system here: ask the operator, then add it here.
3. A change that breaks a system here is undone, not worked around.
4. Modforge holds every rule that needs no engine, with its test; topside
   senses, acts, and holds content (topside skill: modforge first).
5. Every tick is a pure result of the game's state and the ops given at
   ticks (operator, 2026-10-02; lockstep games and Factorio): the same
   save played the same ticks ends the same, in play, in steps, and after
   a load. Work done off the main loop (a background task, another thread)
   lands at a fixed tick, and the loop waits for it if it is late; it never
   lands whenever it happens to finish. Nothing that decides anything is
   ordered by a hash map's iteration, by the order Bevy's queries walk
   entities (a load spawns them in another order), or counted per frame:
   who goes first, and who is issued an id first, is by ActorId or ThingId. A failure lands
   too, as its own plain result, never a hang. The Pathing row is the
   first system held to this.

| System | Def | Registry | How | Status |
|---|---|---|---|---|
| People | modforge `ActorDef` (player, bunker person, scavenger, raider, lookout, wanderer) | Kinds: `ActorRegistryRes` (content `register_actors`). People: each an entity, found by ActorId through `ActorMapRes` | Made by `actor::spawn_person`, where each is named (Bevy's `Name`, modforge `roll_name` from content's `PERSON_NAMES`); the bunker people by `actor::populate_bunkers`, the player's bunker holding the player and Roxanne (operator, 2026-09-27); changed only through their action queue (Actions) and the systems below; dies through `combat::die` and stays, to query; back through `player::respawn` | Dead people stay: tests/the_dead.rs passes on the running game (2026-09-26) |
| Memory | modforge `memory::Memory` (`Known`, `Did`, threats, `learn`) | The `Memory` component on each person | Each person's one memory, written by the path that did the thing (`ai::think`, `combat::hit_actor`, `hud`, `nav::doorways`, `player::respawn`); read by modforge `brain::choose` and the control plane | Memory is the one record: the death log and `LifeStats` removed; the world op totals from memories; tests/the_dead.rs and tests/deaths.rs read deaths from memory on the running game (2026-09-26) |
| The brain | modforge `Activity`, `CombatState`, `Condition`; `Personality`; the `ActorDef`'s behaviour and sight | `ActorActivity`, `ActorCombat`, `ActorCondition`, `Personality` on each person | `ai::think` builds the `Perception` (sight in the cone, memory, worth by modforge `memory::food_worth`); modforge `brain::choose` decides | Not audited |
| Actions | modforge `actions::Action`, `ActionQueue` | `Actions` on each person; the keys' bindings in `input::BindingsRes` | Pushed by the keys (`input::read_input`, the one raw input reader) and the brain (`ai::think`), the same actions; handed out by `input::drain_actions` | Not audited |
| Needs and the day | modforge `SurvivalStats` (drain, `eat`, `is_night`) | `Survival` on each person; the clock `SimTick`; the day's length `survival::Day` | Drained by `survival::survival_tick`; eating and drinking only through modforge (`SurvivalStats::eat`, `use_slot`, `hotbar_key`), then `hud::used` heals and remembers; empty needs hit through `survival::waste` | One eat path: every eat, drink, and heal ends in `hud::used` after modforge decided it; tests/sources.rs passes on the running game (2026-09-26) |
| Health, hits, death | modforge `DamageDef`; `HealthStats` | Kinds of damage: `DamageRegistryRes`. Health: `Health` on each person | Every hit through `combat::hit_actor` (modforge `resolve_hit`); every death through `combat::die` | The respawn rule is modforge `survival::respawn`, with its test; tests/respawn.rs passes on the running game (2026-09-26) |
| Items and things in the world | modforge `ItemDef` (food, material, tool, weapon, deployable, note, medicine, source) | Kinds: `ItemRegistryRes` (content `register_items`). Held: the holder's `Inventory`, `Hotbar`, `Equipment`, or a box's `Container`. In the world: each a `WorldItem` entity | Into the world only through `interact::spawn_item` (a dropped can, a death box, a loot box, a bench, a well, a berry bush; loot rolled by modforge `loot`); moved between holders through `hud` | One way in: wells and berry bushes are `WorldItem`s through `spawn_item` (2026-09-26) |
| Layers (every item and every person) | modforge: each layer registered with its numbers; the thing's type decides its layers (a person: body, head and hair, what they wear; a melee weapon: handle and head; a gun: body, barrel, grip, stock, sight; a simple item: one layer) | Kinds: registered in content. A thing's own layers: an item's on its stack, carried wherever the stack goes; a person's their rolled look (modforge `actor::Look`) and what they wear | Layers are set when the thing enters existence (an item through modforge `item::create`, a person through `spawn_actor`); its numbers only from the one modforge function that works them out from its layers, read by every system that uses them; each layer is its own model from scripts/models.py: a person's are the meshes of their body's model (`people::dress` shows theirs), an item's are models stacked in one frame (`held::hold`) | Item layers carried, and an item's numbers (its fight, and worn armor) worked out from them by modforge `item::numbers_of` (2026-09-26); melee handles and heads and the pipe shotgun's parts registered, a gun part adding to reach, spread and delay; which models a stack is drawn with only from `model::stacked` (the hand and the ground); a person's worn layers redrawn when what they wear changes (`people::redress`) |
| Using things | modforge `hud::InteractKind` | `Interactable` on each thing; doors indexed by chunk in `nav::DoorIndex` | `interact::interact_on_use` (Use reaches the thing in reach nearest the aim), decided by modforge `hud::interact` | Not audited |
| Crafting | modforge `RecipeDef` | `RecipeRegistryRes` | Stations made by `interact::spawn_station`; items made only by the Craft action through modforge `crafting::craft` | Not audited |
| The world and its ground | modforge `GroundDef` (content `small_ground_def`, `ground_def`), `BiomeDef` with its scatter | Kinds: `BiomeRegistryRes`. The world: `terrain::WorldRes` and the made chunks in `terrain::Chunks` | Rolled by `terrain::roll` (content `roll_world2d`), made by `terrain::make_chunks`; a tile's ground only from the made chunk, modforge `World2d::chunk` | One answer: drinking reads the made chunks' shores (land and water tile); tests/sources.rs passes on the running game with river water drunk (2026-09-26) |
| Pathing | Decided 2026-10-01 (pathing.md "A navigation mesh"): a Recast navigation mesh, `bevy_rerecast` built from the Avian colliders by `avian_rerecast`, walked with `bevy_landmass` (joined by `landmass_rerecast`); where to go is modforge `Action::Go`, a point in 3D | The mesh: bevy_rerecast's; each walker's way: landmass's agent (`AgentTarget3d`, `AgentState`). The walk grid `nav::Walk` stays only as the wall map for sight maps and the minimap | The mesh built from the colliders, one per chunk joined into one world (src/navmesh.rs). The mesh at every tick is a pure result of the game's state (decided 2026-10-02, the lockstep and Factorio rule: work off the main loop lands at a fixed tick, and the loop waits for it if it is late): chunks are taken in a fixed order, a set number a tick; a chunk's mesh asked at tick T lands at tick T + SETTLE exactly, with its landmass island; if it is not done by then, the fixed step waits for it before running that tick (logged with how long), paused or not, in play, in steps and after a load alike; a tile that fails to build or to join landmass lands as no mesh for that chunk, logged, never a hang. The brain, commands, following and leading push Go; `navmesh::walk` makes the walker a landmass agent and turns its desired velocity into Aim and Move | Moving: people walk the mesh (2026-10-02: tests/command.rs walked the player from 6 m down up the stairwell to the ground); RED until people step around each other (no one is a landmass character yet, so a walker pushes into someone standing in the way) |
| Monuments and buildings | modforge `MonumentTypeDef`, `BuildingTypeDef`, `StructureDef`; content `SMALL_SITES` | Kinds: `MonumentRegistryRes`. Standing: each building an entity; the monuments standing in `structure::MonumentsRes` | Every building through `structure::spawn_structure` (the bunkers too); sites through `structure::spawn_sites` | Not audited |
| Factions | modforge `FactionDef` | `FactionRegistryRes` (content `register_factions`); a person's faction in their `Npc` | Hostility: modforge `relationship::stands` (own feeling from memory, else the faction's standing from its members, else the registry's table), read in `ai::think` | tests/relationship.rs (2026-09-27) |
| The storm | modforge `StormDef` (content `STORM`) | `storm::StormRes` | `storm::storm` hits the open through `hit_actor` (a `Shelter` is safe); `storm::shift_world` rolls the world again through `terrain::roll`, bunkers and bunker people kept; topside people die with the world and stay, dead, the storm in their memory | tests/storm.rs passed on the running game |
| Talking | modforge `actions::Said` (tell, ask, offer, threaten, line) on `Action::Talk`; `talk::Answer`; every line, the player's choices included, one rule: modforge storyteller `LineDef` (name, part, `When` conditions, ways, what it tells, felt; episodes.md "One way for every conversation") | Each person's `Memory`: `Did::Talked` and `Did::Heard`, each a `memory::Line` (the line's name and its words); a told thing is `Known` with `told_by`. The rules: an episode's on its `EpisodeDef`, plain talk in content (`plain_lines`), both in `talk::Lines`. The player's open conversation: `Conversation` on the player (with whom, since when, whether they started it); the panel's copy of their choices: `talk::Choices` | Pushed like any action (the keys, the brain, `talk::speak_up`); carried out only by `talk::talk`, in `TALK_REACH`, both memories written by modforge `talk::talk`, the answer only from the listener's memory; `TALK_REACH` (3 m) is the one reach for speaking and for a conversation staying open (operator, 2026-10-02): two people farther apart than it are not talking, and their conversation ends; a knock can carry one called line (The Tap's "You awake in there?"), heard like the knock itself, through the shut door within 6 m on that level, and remembered as heard; the conversation starts only once the two are within `TALK_REACH` (operator, 2026-10-02); what anyone says and the player's choices picked only by modforge `talk::pick` and `talk::choices` against the speaker's memory, the words worked out when said (`talk::words_of`); pivot points said reached only by `episode::pivots_said` reading memory. Left: an NPC speaks up unasked from `talk::speak_up`, not yet from its brain's tree | modforge talk tests (2026-09-30): an episode conversation and plain talk on the rules alone; the running game not tested since the rules (2026-09-30) |
| Realities | modforge `RealityDef` (its name, danger, night, whether death sticks; realities.md) | Kinds: `RealityRegistry`, held as `RealityRegistryRes` (content `register_realities`). The reality now: the `Reality` resource | Changed only by `storm::shift_world`: modforge `RealityRegistry::pick`, or the one the `storm` op names; its rules read from `Reality` by the system they change (`survival::night`, `combat::die`) | tests/reality.rs passes on the running game (2026-09-27) |
| Episodes | modforge `EpisodeDef` (its name and the realities it fits; episodes.md) | Kinds: `EpisodeRegistry`, held as `EpisodeRegistryRes` (content `register_episodes`). The episode running now: the `Episode` resource | Started only by `storm::shift_world` after the reality, through modforge `EpisodeRegistry::pick`; the player learns only what they see or are told | tests/reality.rs: the running game's episode fits its reality (2026-09-27); nothing in the game acts on it yet |
| Bunkers and their stores | a building (Monuments and buildings) | The bunker building's entity; its store a box, known through `Store` on each bunker person | As Monuments and buildings; the store is a box (Items) | Not audited |
| Sight | the walls' colliders | `BlocksSight` on each thing that hides what is past it | `sight` (`Sight`), for `ai::think`'s cone and `sight::player_sight` | Not audited |
| Copies | the JSON each op returns | The control plane's statics in `control_plane.rs` | Published by `control_plane.rs` for tests and watching, never read back by the game | Not audited |
| Tests | a test function | `tests/`, one binary `all`; the queue of tests to run and their results: the game's test queue | Every test reads the running game through the control plane; no headless tests; started one, a list, or a group at a time through the test queue ("Running tests" below) | Built; the test queue not yet |
| Commands and replays | modforge `command::Command` (move to, follow, talk to, attack, attack the nearest enemy), `command::Replay` (the world seed and each command with the tick it was given) | The commands a person has left: `command::Commanded` on them; every command given this run, in order: `command::GivenThisRun`; a replay playing: `command::Playing`; saved replays in `replays/` | Given only by `command::give`, from the control plane's `command` op or a replay played by the `replay` op; carried out only by `command::carry_out` through modforge `command::step`, which pushes the same actions a person's brain pushes (modforge `brain::walk_toward`, `turn_toward`), walked by `nav::steer`; follow hands over to `Following`, talk to ends in the same Use as E on the one looked at, attack in the same Attack; an enemy is whoever the person stands hostile to by the brain's one rule, modforge `relationship::stands` | tests/command.rs passes on the running game (2026-10-01): move to walked 52 m and stopped there, talk to walked 72 m and the talk box opened with them, a saved replay played gave its first command again. Follow, attack, and attack the nearest enemy not tested yet |

## Commands and replays (operator, 2026-10-01)

How Claude plays the running game to test it. A replay of every mouse
movement and click is too low; a command is one plain order a person
carries out: move to a point, follow someone, talk to someone, attack
someone, attack the nearest enemy. One command, several, or a whole list
is given, depending on what is being tested.

Prior art: Minecraft's Baritone bot (`goto`, `follow`, `mine`: an order
carried out through the normal movement) and StarCraft's and Age of
Empires' replays (the player's commands are stored, not the mouse, and the
engine plays them again).

- A command is carried out with the same actions a person's brain pushes:
  aim, move, Use, Attack. Nothing new reaches the game; `nav::steer` walks
  the way around walls, as it does for everyone. No player-only rule: any
  person can be given a command, the player too.
- Move to: walk to the point. Follow: the same `Following` the game has.
  Talk to: walk to them, face them, and Use, as E does; the conversation
  then goes as any (the number keys say the choices). Attack: walk to them,
  face them, and attack in reach until they are dead. Attack the nearest
  enemy: the nearest living person they stand hostile to, by the one rule
  the brain uses, then attack them.
- The commands a person is given are carried out one after another. A
  move of their own (the keys) ends them, as it ends following; any
  command but follow ends following too, being their own order.
- Every command given in a run is kept in order with the tick it was
  given. Saved, it is a replay (`replays/<name>.json`, with the world
  seed); played, each command is given again at the same distance in
  ticks from the first. On the same world it does the same things again.

## Running tests (operator, 2026-10-02)

Tests are run from a queue the game holds: one test, a list of them, or a
group (every test of an episode) is queued, the queue is watched while it
runs, and the results are read when all are done. Prior art: Unreal's
automation tab (pick tests, they queue and run, results at the end) and a
CI runner taking jobs from a queue and reporting each back.

- The tests stay in `tests/`. The game holds only the queue and each
  test's result: queued, running, passed or failed, with its output. The
  `tests` op queues tests and reports the queue.
- A runner process takes the next queued test, runs it against the
  running game, and reports its result, one test at a time, as there is
  one world. It exits 0 only if every test it ran passed, so one command
  proves a goal.
- A test that cannot play what it tests fails. It never returns early
  as passed.
- Each test starts from its own saved game (Saving, todo 31a): its
  starting point is reached once by playing and saved, and the test loads
  that save before every run, as speedrunners practise one part of a run
  from a savestate. Saving and loading in place work (src/save.rs: the
  `save` and `load` ops, `saves/<name>.save`, tests/save.rs).
- Save states for everything (operator, 2026-10-02): "we shouldn't be
  waiting 2 hours for something, just load the save." A test never waits
  through a long game span for its event: it loads a save made just before
  the event, and tests the thing. As we go through the game, each save is
  reached once organically, from the one before, and saved
  (`endpoint::start_from`; the opening's in tests/opening.rs, the rest in
  tests/saves.rs); the saves are committed, and every test after loads
  them.
- Loading (operator, 2026-10-02; every game's loading screen): the game
  shows a loading screen until the world is ready to play, after a launch,
  a storm's new world, and loading a saved game alike, and nobody acts
  while it shows. Ready means everything worked out from the world is
  built, landmass's navigation mesh included. The `status` op reports
  `loading` while it shows; a test waits until loading is done and asks
  nothing before it (`endpoint::start_from`).
- What a save holds is one list, `saved!` in src/save.rs: every component
  and resource that changes what happens next. A new one that changes
  what happens next goes in that list, or a load loses it. What is worked
  out from saved things (bodies, drawing, the walk grid, the mesh's
  stand-ins) is never saved: it is made whenever its saved thing comes
  into the world, spawned or loaded (`Actor`'s required parts,
  `interact::item_body`, `interact::door_body`, the `Added` systems).
- The world is rolled once (operator, 2026-10-02; Factorio's map is made
  once, not on every load). A save holds its world (the seed, the size,
  the storms that shifted it) and which chunks were made. Loading a save
  of the world that is running keeps that world: its ground, buildings and
  navigation mesh stay, only the saved people and things are swapped, and
  chunks made after the save are removed. Only a save of another world
  (after a relaunch or a storm) rolls that world again, once, through the
  one roll (`terrain::roll`).
- While tests run, nobody plays that game: the tests drive the world, the
  player and the camera.

## The one boundary: modforge owns the systems, topside uses them

ALL game systems live in modforge. Topside is a consumer of modforge exactly
like survivalist-mod and misery-mod are, with one difference: those mods bind
modforge systems into someone else's game, and topside binds them into a world
it owns itself.

The rule for where code goes, stated once: anything generic, anything that
could apply to other games or mods, goes in modforge. Only what is
topside-only lives in this repo. In detail:

- A system (the decisions: who attacks whom, what quality an item rolls, what
  crosses the edge, what offer a faction posts) lives in modforge. If it could
  drive a second game or a mod, it is a system. Decided 2026-08-19: the
  systems live as modules IN the modforge foundation crate itself (like
  modforge::rpg does today), not in a separate crate, so one dependency
  serves mods OR the game.
- A binder (the eyes and hands: how a system reads and changes one particular
  world) lives in the consumer. This is the existing ueforge pattern: the
  framework module owns the decisions, the per-game crate implements the
  binder trait. Topside's binders read and write topside's own world instead
  of reflecting into a host game.
- Content (checked-in definitions: items, structures, factions, incursion
  kinds, upgrade tracks, the story) lives in topside. Content is data a system
  consumes, never logic.
- The world (the simulation substrate: map, people, items, structures, time)
  lives in topside. The world substrate and engine are NOT chosen yet; that is
  an open decision recorded below, not something this doc locks.

The payoff of this boundary is the whole point of the project: everything
hardened for topside (tests, robustness, tuning) flows back to every mod
sitting on modforge, and everything the mods proved (the survivalist designs)
flows forward into topside.

## Prior art

Three patterns, all already in use in our repos, are the model:

- The factoriobot authority map (factoriobot/docs/authority.md): one owner per
  decision, one numbered ownership table, a score rubric, and an executable
  proof module so the doc cannot rot.
- The Endless Def -> registry -> use pattern, already the modforge convention
  (OpDef -> OPS): definitions are checked-in data, one registry holds them,
  systems read the registry.
- The ueforge binder pattern (modforge/README.md): a framework module owns a
  mechanic's decisions, the consumer implements a small trait that gives it
  eyes and hands into one world.

The game design itself follows the RimWorld storyteller model as locked in
survivalist-mod/docs/status.md: a solid base simulation plus a director on
top, with two layers that must never be confused or merged.

## Naming contract

One concept, one name. These names come from the survivalist design docs and
the operator's own words. No synonyms, no renamed wrappers.

| Name | One responsibility |
|---|---|
| **World** | Topside's one simulation state: map, people, items, structures, factions, time. The only source of truth about what exists right now. |
| **Binder** | The trait a consumer implements to give one modforge system eyes and hands into its world. Binders observe and act; they never decide. |
| **Storyteller** | The one director. Two locked layers: Randy Random (random events regardless of how the player is doing; never adaptive) and Mario Kart (adaptive pressure on whoever is winning). Nothing else schedules dramatic events. In the story the Storyteller is the Director: the alien entity managing it all, whose show the episodes are (operator, 2026-09-27; design.md "The story"). |
| **The edge** | The border of the unknown beyond the map. The ONE faucet through which new people, items, and factions enter the world. |
| **Incursion** | One thing the storyteller sends across the edge, telegraphed by the dread loop. |
| **Dread loop** | Telegraph, uncertainty, varied payoff, lingering mystery, escalation. The storyteller's pacing mechanism. |
| **Genome** | A faction's traits (aggression, expansionism, defensiveness, guile) and the per-survivor franchise vote that turns traits into decisions. |
| **Act** | One thing a faction does to another or to the player (scavenge, steal, trade, rob, murder, extort, raid). Chosen by the genome vote, executed through the world. |
| **Relationship** | Two levels, both (operator, 2026-09-27; RimWorld's faction goodwill and pawn opinions, Mount & Blade's kingdom and lord relations). A person's feeling toward another person, from their own memory: what was done to them, what they saw, what they were told. A faction's standing toward a person or another faction, worked out from its members' feelings, never stored apart (memory is the one record). A person acts on their own feeling when they have one, else on their faction's; their own history comes first. Ranges from hostile (fight on sight) through neutral to friendly. The rules live in modforge; topside feeds what people did into memory. |
| **Trading** | Exchange of items between the player and any faction or neutral NPC they encounter. Factions trade from real stores (the world is the truth, no infinite shops). Relationship determines willingness and prices. Trading happens at faction territory, their bunker, or in the field. Trade decisions live in modforge; topside provides the UI and world context. |
| **Quality** | The one item-value system: every item rolls a tier, rarer is better, the whole ecosystem (crafters, thieves, traders, warlords) recognizes it. |
| **Named unique** | A one-of-a-kind item that enters only with a matching incursion and is tracked by the chronicle. |
| **The work board** | Offers generated from what factions are actually doing (bounties, escorts, ransoms), paid from real stores. Never scripted. |
| **Upgrades** | Per-structure and settlement-wide upgrade tracks: unbounded levels, diminishing benefits, rising real costs. The player's resource sink. |
| **Storm** | The recurring world-changing event (design.md; modeled on MISERY's emission, but that name stays MISERY's and STALKER's). The storyteller schedules it; worldgen executes the regeneration; one hide-or-die check applies to the player and every NPC alike. |
| **WorldItem** | Anything that exists in the world with a position: dropped items, placed crafting benches, generators, storage boxes, doors, loot crates. The ONE name for "a thing in the world." An item in inventory is an ItemStack; when placed or dropped into the world it becomes a WorldItem. Lives in topside (it is Bevy-specific); the concept is referenced by modforge systems that reason about world placement. Avoids "entity" to prevent confusion with Bevy's ECS Entity handle. |
| **Actor** | Anything that acts: it moves or shoots, meaning it pushes actions onto an action queue that `movement` and `fire` run. The player, every NPC, animals, turrets, and vehicles are actors, all on the same components (inventory, hotbar, equipment, survival, health, protection, facing, an action queue) and all through `spawn_actor`. A WorldItem never acts; it is only acted on (picked up, opened, used, hit). The test for a new thing (operator-locked 2026-08-21): does it push actions? Yes means actor, no means WorldItem. A turret is an actor with speed zero placed from the inventory; a vehicle is an actor whose queue the driver fills instead of their own body's. The only differences between actors are the def and who fills the queue (the keyboard, the AI, or a driver). |
| **ActorId** | The one persistent identity of an actor: a monotonic number issued once at spawn, never reused, surviving save and load. Carried as a component. Bevy `Entity` is a runtime handle and the GPU row is a buffer index; neither is an identity. Replaces the slot-as-identity that Endless used. Decided 2026-08-21. |
| **ThingId** | A thing's persistent identity (an item, a station, a door), as ActorId is a person's: given once where the thing is made (`interact::next_thing_id`, in `spawn_item`, `spawn_station` and a building's doors), surviving save and load. What memory knows a thing by (a box, a source, the tap); never an `Entity`'s bits, which change on every load. `ThingIds` finds the thing by it. Decided 2026-10-02. |
| **ActorMap** | The one actor registry, taken from Endless's EntityMap and GpuSlotPool folded into one resource (decided 2026-08-21: "have it working clean in our actormap"). Issues the ActorId and the GPU row at spawn, frees the row at death, and answers every lookup: by ActorId, by Entity, by row. Two counts, two methods, never mixed: the high-water row count (buffer sizing and dispatch bounds) and the live actor count (gameplay). Only the map and the GPU uploader ever see a row number. Holds every actor and nothing that is not an actor: no WorldItems, no projectiles, no structures (Endless put buildings in its EntityMap; that is what muddied the name). The map, its rules, and its tests are modforge `ActorMap` (modforge src/actor.rs); topside holds it as the `ActorMapRes` resource and carries the id on each entity as the `Actor` component (src/actor_map.rs). |
| **StructureDef** | One building as data (rooms, openings, stairs, furniture, lights), the Def in Def -> Registry -> Instance. Lives in MODFORGE (decided 2026-08-19: the data and relation rules are engine-agnostic, on glam math and plain rgb, so mods can roll structures too, e.g. survivalist town growth). Hand-authored and generated buildings use the same shape; code never composes geometry outside one. |
| **spawn_structure** | The one path from a StructureDef to world geometry and colliders (the Controller). Lives in the CONSUMER: it is the binder that makes a def real in one world (topside spawns Bevy entities; a mod would call its host game's build functions). |
| **BuildingType** | One building type as a StructureDef generator: gas station, warehouse, lab, etc. (full list in design.md). Each type defines a range of parameters (room count, doors, windows, colors, condition) that vary on every roll. The generator produces a different StructureDef each time, but always recognizable as that type. BuildingType is data; it never touches geometry. |
| **Territory** | A building in the world captured and held by a faction (including the player's). Holding a building provides a real bonus tied to its BuildingType (design.md). Capture is through presence and force using the same mechanic path for player and NPCs. The storm wipes all territory when the map regenerates; faction rivalries persist, territory does not. Territory decisions (who can capture, what bonuses apply, garrison behavior) live in modforge as part of the faction war system; topside owns the world-side bookkeeping of which building entity belongs to whom. |
| **MonumentDef** | One monument as data (design.md: a destination worth traveling to, Rust's monuments as the reference). A MonumentDef names a list of possible BuildingTypes to pick from, loot spots, NPC spots, gates, arrangement rules, and the name. Two layers of randomization: the monument picks which buildings from its list, and each picked building generates with varied parameters. No two instances of the same MonumentDef are alike. Hand-authored or generated, same shape. |
| **spawn_monument** | The one path from a MonumentDef to a placed monument. It places member structures ONLY through spawn_structure and decides nothing that belongs elsewhere: loot value is Quality's, NPC identity is Genome's and the edge's, placement in the world and regeneration are Worldgen's and the storm's. |
| **HUD** | The ONE player-facing information layer. Made of panels: inventory, skills, health, food, prompts, work board, chronicle, map, and any future panel. Lives in MODFORGE as engine-agnostic data (HudState) and decisions (toggle_panel, move_stack, interact, prompt_for). No binder trait: the consumer writes HudState fields directly from its own systems and reads HudState to paint. No second way to show the player information. |
| **CelMaterial** | The ONE material for every mesh in the game. A base color quantized into hard light bands: no textures, no smooth gradients. Lives in TOPSIDE (it is Bevy-specific presentation). One struct, one shader, every mesh goes through it. Operator-locked 2026-08-20: "i want all our stuff to be cell shading so i dont waste time with textures." |
| **TechTree** | The one research progression system. Deep and branching, with infinite upgrade topics that never cap (Factorio model). Research consumes components found in monuments. Decisions (what is unlockable, costs, prerequisites, infinite scaling formula) live in modforge; content (the specific topics and recipes) lives in topside. |
| **CraftingStation** | A placed station in the bunker (crafting bench, chemistry table, electronics workbench, armory bench, research station). Each type is unlocked by research, each has upgrade tiers that improve output quality and unlock recipes. Stations are content (topside); the crafting decision logic (recipe matching, quality rolls, time costs) lives in modforge. |
| **Combat** | First-person melee and ranged combat. Melee reference: The Bloodline (weighty, directional). Ranged reference: Rust (simple, lethal, ammo matters). Weapon types are content in topside; damage calculations, hit detection, and weapon behavior decisions live in modforge. Player and NPCs fight through the same mechanic path. |
| **EnemyTraits** | Random traits on hostile NPCs and creatures (Diablo 2 model): extra fast, armored, pack leader, etc. Traits express through the modforge genome system at the individual level, not a separate mechanic. Trait definitions are content in topside; the rolling and application logic lives in modforge. |
| **Affixes** | Random properties on items (Diablo 2 model), layered on top of the quality tier system. A base item plus its layers (the Layers row above) plus a quality tier plus affixes creates the full item identity. Affix definitions are content in topside; the rolling logic lives in modforge alongside the quality system. |
| **LootTable** | Per-location loot tables that scale with danger. The danger of the location (set by the MonumentDef and the specific spot within it) determines the ceiling of quality tiers, affix rarity, and item pools that can roll there. Harder location means harder enemies and better rewards. You cannot get top-tier drops from safe areas. Loot table definitions are content in topside; the rolling logic lives in modforge alongside quality and affixes. |
| **Survival** | The survival pressure systems: hunger, thirst, temperature, radiation. Always active, never fully eliminated by tech or gear. Player and NPCs share one mechanic path for each pressure. Food restores health and hunger (Valheim model: each food has multiple benefits). A craftable bandage restores health faster than food. These are the healing path; player and NPCs heal through the same mechanic. Decisions (drain rates, thresholds, effects of exposure, healing amounts) live in modforge; topside feeds the environmental inputs (biome temperature, weather events, radiation zones). Atlas is the reference for how pressures stack. |
| **Weather** | Per-biome weather events driven by the storyteller: rain, fog, heat wave, cold snap, radiation storm. Affects visibility, temperature, radiation, and NPC behavior. Weather decisions live in modforge (the storyteller schedules them); biome weather tables are content in topside. Distinct from the big storm (which regenerates the world). |
| **RPG** | Player and NPC progression through modforge::rpg. Use-based skills (crafting, combat, medicine, construction, research) that improve with practice. The player and NPCs share one mechanic path. Skills affect output quality, speed, damage, accuracy, and recipe access. Decisions live in modforge (the RPG module); topside feeds the skill-use events from its world. |
| **Silo** | The bunker's vertical expansion. The player starts with the one starting room (the bunker) and digs downward into the silo, adding levels that expand horizontally within their footprint (Fallout Shelter vertical, Rust room-by-room freedom). Deeper is safer but more expensive. The silo is world geometry through StructureDef and spawn_structure, same as every other building. Lives in topside. |
| **PowerGrid** | The bunker's power system: generators burn fuel, wiring connects consumers, no fuel means no power means no powered stations or defenses. A real simulation, not a toggle. Lives in topside (it is world-specific infrastructure). |

## Ownership flow

One line, one direction: the world is the only source of truth; systems read
it through binders, decide, and act back through binders; the chronicle tells
the player what happened.

1. The world holds the current state. Nothing else stores a fact about what
   exists; a system that needs a fact asks its binder now.
2. The storyteller paces events: Randy Random rolls what and when, Mario Kart
   watches who is winning and applies pressure. It never annihilates; brutal
   but survivable is the line (operator-locked in the survivalist design).
3. Factions decide through the genome vote and act through the act repertoire.
   Consequences flow through the world's own mechanics, never through a side
   channel.
4. Everything new enters at the edge. No system conjures people or items in
   place. This is the no-cheating rule: every person walked in or was born
   here, every item came from the edge, crafting, or harvest. Destroying a
   faction's people or stores actually weakens it.
5. Quality is rolled at the moment an item enters existence (at the edge, at
   the crafter's hands) by the one quality system. No second place assigns
   item value.
6. The work board reads faction activity and posts offers. It generates
   nothing itself; the simulation writes the work.
7. The chronicle narrates public events. It reads; it never writes.

Anti-bypass rules, stated once here and enforced by the proof module:

- No system keeps a private copy of world state as decision memory. Observed
  facts are read fresh for each decision.
- No consumer reimplements a system's decision. If topside needs a behavior a
  system lacks, the system grows upstream in modforge; topside never carries a
  local fork of a decision.
- Randy Random never scales to the player's safety (that would be Cassandra,
  rejected in the survivalist design). Mario Kart alone is adaptive, and it
  targets the leader, AI or player alike.
- The bored-player test bounds every feature (operator-locked): simulation
  the player never sees is cut. A change counts as gameplay only if a bored
  player, safe behind their walls, would sit up.
- The player and NPCs go through the same mechanic paths. The storm's
  hide-or-die check is ONE implementation applied to everyone; no separate
  player-only or NPC-only version of a shared rule.
- Every actor enters the world through ONE spawn path (Endless's
  materialize_npc discipline): arrivals at the edge, loads from save, and
  test setups all use the same function. No second way to make a person
  exist.
- Content composes upward in data, never in code: solids and openings make
  a RoomSpec, rooms make a StructureDef, structures make a MonumentDef,
  worldgen places monuments. Every layer is data; each layer spawns only
  through the layer below's one path; a generator WRITES defs and never
  touches geometry. This is what makes the world super generative: rolling
  a new monument means rolling data, and hand-authored and generated
  content are indistinguishable downstream.

## Bevy ECS world model (decided 2026-08-20)

Bevy ECS is Topside's live World, not a presentation wrapper around another
game-state store. Runtime ownership follows these rules:

| Bevy concept | Topside use | Ownership rule |
|---|---|---|
| Entity | One runtime thing: player, NPC, item, door, wall, floor, or structure root | Despawning the entity removes the thing and every fact owned by it. A structure root is one static body carrying the pieces as data (`Pieces`) and one compound collider of every piece; its children are one merged mesh per colour and the doors (kinematic bodies with their own collider). Recursive despawn removes geometry and collision together. |
| Component | One fact about that thing: transform, inventory, health, faction, collider, interactable state, or world item | If a fact describes one thing, it lives on that entity. Gameplay must not mirror it in a resource, HUD model, physics store, or index. |
| System | One operation over the entities with the required components: movement, hunger, interaction, combat, or spawning | Gameplay systems run in `FixedUpdate`. Presentation systems may read World state in `Update`, but do not decide or own gameplay facts. |
| Resource | A value that genuinely applies to the whole World: simulation tick, world seed, registry, identity index, slot allocator, or transient input latch | A resource may index or derive entity data, but it is not a second owner. Per-player, per-NPC, per-item, and per-structure state belongs in components. |
| Message | A transient request or result between systems: spawn, interaction, damage, or GPU upload intent | Messages move work between ordered systems. They are consumed and never become durable World state. |

The modforge boundary remains unchanged. Modforge owns generic definitions and
decisions. A Topside binder reads the required ECS components, calls the
modforge system, and applies the result back to ECS components. The binder
does not keep its own state or repeat the decision. For structures, the flow
is `StructureDef -> spawn_structure -> root entity + child entities`. The
root owns the pieces and the one compound collider baked from them; the
children are the merged meshes (one per colour) and the doors. `bake` in
src/structure.rs rebuilds both once when a piece changes.

The HUD is a projection of the World. Player inventory, health, skills, and
other gameplay state live on the player entity; HUD systems copy only the
values needed to paint the current frame. Player actions return through the
normal interaction or gameplay system rather than mutating the HUD model as
game state.

Bevy `Entity` is a temporary runtime handle. Every persistent actor receives
a monotonic `ActorId` that survives save and load and is never reused.
`ActorMap` links that ID to the current Bevy entity and GPU row, but does
not own the actor's gameplay state. A GPU row is also temporary and may be
reused only after stale mappings are rejected.

Avian follows the same ownership model: bodies, colliders, velocities, and
relationships are components on the entities they describe. Avian's spatial
indexes and contact caches are derived data. GPU buffers are likewise not a
second World. The GPU may own only fields explicitly assigned to it in the GPU
offload contract; ECS identity and CPU gameplay facts remain authoritative,
and every readback is resolved and validated against them before use.

## Frame structure (from Endless)

All simulation runs in FixedUpdate at a locked 60 UPS, gated on the app
state; Update handles presentation (camera, UI, audio) at display rate.
Gameplay never varies with frame rate. The GPU tick order lives in the GPU
offload section below. The 1000 persistent people (the design.md scale
target) are simulated in full near the player and as cheap abstract faction
facts when far, the A-Life trick; both representations feed the same
systems through the same binders, and identity persists across the switch.

## Physics (decided 2026-08-20)

Avian 3D is the ONE physics system (operator, 2026-08-22: "everything in
the game should be part of the same physics system"). Every body, every
collider, every ray, every sweep, every fall, and the one gravity value
(Avian's `Gravity` resource) are Avian's. Nothing in topside keeps its own
box, its own ray test, its own gravity number, or its own idea of what
touches what; if a system needs to know what is where, it asks Avian
(row 2 deleted the last hand-kept boxes, `Door.collider`,
`Interactable.aabb`, and `ray_aabb`). Physics bodies, colliders, positions,
velocities, and relationships are Bevy ECS data; there is no second
authoritative physics world. Avian's acceleration structures and contact
caches are derived implementation details.

Actors (the player and every NPC) are kinematic bodies moved by Avian's
`move_and_slide`, the way Avian's own character controller, Rust, and
Quake move characters: a dynamic capsule gets shoved by doors and slides on
stairs. Kinematic means Avian does not apply gravity to them, so the fall
is driven by the movement system from Avian's `Gravity` and lands through
Avian's casts (todo row 2a). That is still one physics system: one gravity,
one collision world, Avian decides every hit.

Every WorldItem carries an Avian collider; a small item's is a sensor, so
the walk, the swing, the pellets, and the crosshair look through it and
only the E prompt's ray stops on it.

`spawn_structure` creates one static structure root with one compound Avian
collider of every piece and one merged mesh per colour as child entities;
doors are kinematic children with their own collider (performance.md: one
collider per piece cost 12 ms a step in the city). Removing the root removes
both presentation and collision. Player movement, stairs, doors, raycasts, and future dynamic
objects use Avian rather than a Topside collision or physics implementation.

## GPU offload (decided 2026-08-19)

Topside runs the crowd half of the simulation on the GPU from day one. The
model is lifted whole from Endless (endless/docs/authority.md, the authority
contract rated 9/10 there), because the expensive part, the contract and the
hard rules learned from paid-for regressions, is already figured out. We
adopt Endless's names and rules verbatim; the WGSL itself is new (Endless's
compute is 2D, topside is 3D).

The split: the GPU is muscle, the CPU is brain. The GPU does math that is
identical for thousands of actors at once; the CPU makes every decision.

Ownership classes, one owner per datum:

| Class | Data | Rule |
|---|---|---|
| GPU-authoritative | Positions; the spatial grid (internal, never read back); target candidates; crowd/threat counts (throttled) | Written by compute shaders, read back asynchronously about one frame stale. Staleness is expected and budgeted, not exceptional. |
| CPU-authoritative | Health; movement goals and waypoints; faction identity; speeds; actor identity (ActorId, the ActorMap) | Written by ECS systems, uploaded to the GPU each tick. The GPU's copy is advisory. |
| CPU-only | Every decision: all nine game systems, behavior states, timers, the off-map abstract world | Never touches the GPU. |
| Render-only | Instance data, animation indices, cel-shade material parameters | Uploaded to render buffers, never seen by compute. |

Hard rules, carried from Endless verbatim:

- A GPU target is a candidate only. The CPU resolves the row to an ActorId
  in the ActorMap and re-validates faction and identity from ECS before any
  gameplay effect.
  If GPU and ECS disagree on identity or ownership, ECS wins, always.
- Throttled readbacks feed heuristics only, never identity, ownership, or
  validity gates.
- No system reads a GPU field and writes the same field back in the same
  tick (the one-frame delay compounds into oscillation).
- One count source each, both methods on the ActorMap: the high-water row
  count for buffer sizing and dispatch bounds, the live actor count for
  gameplay. Never mixed, never one number.
- Pathfinding stays CPU (A* picks the route); the GPU steers each actor
  toward its current waypoint.

Structure: systems never write GPU buffers directly. They emit intent
messages; one uploader drains them into the buffers each tick. The
FixedUpdate order at a locked 60 UPS is decide, upload, dispatch, then
read back next tick and re-validate.

Placement: this layer is Bevy-and-WGSL-specific, so the modforge placement
rule does not send it to modforge (modforge stays engine-agnostic). It
starts life in the topside crate; if it proves out, it can lift into a
shared Bevy simulation crate later, with Endless as the prior art and a
possible second consumer.

## Score rubric

Same rubric as factoriobot:

| Score | Meaning |
|---|---|
| 10 | The one implementation path is used across audited callers and carries an anti-bypass guard. |
| 9 | The one implementation path and guard exist; a broader caller audit or live proof remains. |
| 7-8 | The one implementation path exists; a named integration or enforcement boundary remains. |
| 5-6 | The one implementation path exists beside a substantial open consolidation. |
| 1-4 | Competing authorities remain. |
| - | Not built yet. |

## Current authority priorities

Row order is build priority. "IN SURVIVALIST-MOD" means a game-specific
implementation exists in modforge's survivalist-mod crate and the work is
lifting its decisions into a game-agnostic modforge system behind a binder,
not writing it from scratch. How the built game works, system by system, is
"How topside works: every system" above; this table holds the systems still
to build.

| Priority | System | Lives in | Score | Authoritative for | Status |
|---|---|---|---:|---|---|
| 1 | World | topside | - | The one simulation state. Every fact about what exists now. Scale target: 1000 persistent simulated NPCs (design.md). | Decided 2026-09-25: 2D (see Presentation below). |
| 2 | Binder traits | modforge | - | The contract between each system and any world. One trait per system; observe and act only. | UNBUILT. The shape exists as ueforge doctrine (ViewportBinder, DamageBinder). |
| 3 | Storyteller | modforge | - | All dramatic pacing. Randy Random and Mario Kart, the dread loop, incursion scheduling. | IN SURVIVALIST-MOD (incursion.rs, stranger.rs; dread loop and signal payoff live-verified there 2026-07-07). |
| 4 | The edge | modforge | - | The one faucet. What enters the world, carrying what, sent by whom. | IN SURVIVALIST-MOD (edge spawn + march, all arrival kinds). |
| 5 | Genome | modforge | - | Faction traits, the franchise vote, learning, heredity on absorption. | IN SURVIVALIST-MOD (live-verified: 200+ voters, emergent personalities, persistence). |
| 6 | Acts | modforge | - | The act repertoire and its consequences flowing through world mechanics. | IN SURVIVALIST-MOD (six act modules, most arcs watched live). |
| 7 | Quality | modforge | - | Tier rolls at item creation, sender and skill odds, ecosystem recognition. | IN SURVIVALIST-MOD (full tier system code-landed, unwatched). |
| 8 | Named uniques | modforge | - | Once-per-save entry with matching incursions, holder tracking. | IN SURVIVALIST-MOD (The Colonel's Rifle slice code-landed). |
| 9 | The work board | modforge | - | Offers read off live faction activity, paid from real stores. | IN SURVIVALIST-MOD (bounty + camp-clearing code-landed). |
| 10 | Upgrades | modforge | - | Upgrade tracks, levels, costs, prerequisites, and their stat effects. | IN SURVIVALIST-MOD (eleven per-structure tracks + settlement-wide class; core loop live-verified 2026-07-11). |
| 11 | Chronicle | modforge | - | Turning public world events into narration. Read-only. | IN SURVIVALIST-MOD (verified posting live). |
| 12 | HUD | modforge | - | The one player-facing information layer, made of panels. Inventory, skills, health, food, prompts, work board, chronicle, map. Engine-agnostic data (HudState) and decisions (toggle_panel, move_stack, interact, prompt_for). No binder trait: the consumer writes HudState fields directly and reads HudState to paint. | STARTED 2026-08-20: HudState, panels, InteractKind, prompt_for, interact, move_stack landed. HudBinder trait being removed (unnecessary layer). |
| 13 | Content registry | topside | - | Checked-in definitions the systems consume (items, structures, factions, incursion kinds, upgrade tracks, the story). Def -> registry -> use. | STARTED 2026-08-19: StructureDef + spawn_structure landed (src/structure.rs); the starting bunker is the first composition expressed as data. The registry itself and the other content kinds remain. |
| 14 | Control plane | modforge | - | The one runtime inspection and op surface (HTTP server, op registry). | EXISTS in modforge; rated 9/10 in survivalist-mod. The first thing wired into topside per the runtime-control-http doctrine. |
| 15 | GPU simulation | topside | - | The compute-shader half of the simulation: crowd movement with separation, the spatial grid, target candidates, projectiles, under the GPU offload contract above. | UNBUILT. The contract is proven in Endless; the WGSL is new (3D). |
| 16 | Worldgen | topside | - | Procedural generation of the topside world, and the storm's regeneration: terrain and monuments (layouts AND names) re-roll; bunkers and the persistent people never regenerate. | UNBUILT. Endless worldgen.rs is the in-house prior art. |
| 17 | Spawn path | topside | - | The one function through which every actor enters the world (edge arrivals, saves, tests). | STARTED 2026-08-21: `spawn_actor` (src/actor.rs) on Endless's materialize_npc model; monument NPC spots and the bunker scavenger come through it; the ActorId and ActorMap are next. |
| 18 | Save | topside | - | Persistence of the 1000 people, bunkers, factions, and world seed across restarts, with version migration. | UNBUILT. Endless save-load.md is the model. |
| 20 | MonumentDef | topside | - | Monuments as data: a list of possible BuildingTypes to pick from, loot spots, NPC spots, gates, arrangement rules, names. Two layers of randomization: the monument picks buildings from its list, each building generates with varied parameters. spawn_monument is the one path; members spawn only through spawn_structure; loot value, NPC identity, and world placement belong to Quality, Genome/the edge, and Worldgen respectively. | UNBUILT. Building types and monument composition rules documented in design.md. Rust's monuments are the reference. |

## Presentation (decided 2026-09-26: 3D first person, with Diablo 2's map)

Topside is one 3D world played in first person (operator, 2026-09-26). The
world is built once in 3D: low poly cel shaded models drawn live, 3D
physics, height; the pictures rendered ahead of time and their drawing
retire. The overhead view is the map, as Diablo 2's automap: a minimap in
the corner, which M makes a full screen overlay (operator, 2026-09-26),
both drawn over the first person view while the player keeps playing, showing only
what the player has seen (their map, life.md "The map each person
carries"), the player at its centre, no live people on it. It is drawn from
the player's map, not by a second camera.

Things stand at every height (operator, 2026-10-01): x, y and z. A thing,
a person, a station, or a dropped item can be on any level, below the
ground or above it, and everything that reaches one (using it, picking it
up, seeing it, walking to it) works by where it really is, at its height,
wherever it is placed.

Everything is 3D (operator, 2026-10-02: "this is a 3d game"): memory, the
brain, and every system carry where a thing really is, its height
included. A person remembers a thing where it stands, at its height; the
brain's targets are points in 3D; a way is walked to a point at its height
on the navigation mesh. Nothing is flattened to the ground to decide
anything.

This replaces the 2026-09-25 decision (2D, pictures rendered ahead of time,
because 3D takes far more work for one person): the 3D models remove the
picture work instead of adding to it, and first person brings back the
embodiment the 2026-08-19 decision was made for.

The 2026-08-19 decision:

Topside is 3D, first person, low poly with cel shading, with AI-generated
assets, built on Bevy.

- First person because embodiment is the point: 2D top-down means selecting
  a guy and clicking him to move, which does not feel like survival. Valheim
  and its kin prove you FEEL survival in first person. Prior art for a living
  faction simulation experienced from first person: STALKER's A-Life,
  Mount & Blade.
- The player IS one survivor. Their settlement's people run on the same
  genome and acts systems as every AI camp; incursions are experienced from
  the ground.
- Low poly with cel shading because both styles are forgiving of
  AI-generated meshes and textures, and Valheim proves low poly carries
  survival atmosphere (lighting and sound do the work, not polygons). Cel
  shading is a custom toon material in WGSL.
- Bevy because topside links the modforge crate directly in one Rust
  workspace with no engine boundary, and the Endless project already proved
  Bevy plus WGSL experience in-house. Godot was the considered alternative,
  rejected for the bridge it would put between the game and modforge.
- AI-generated 3D assets arrive as glTF, which Bevy loads natively.

The simulation layer stays camera-agnostic behind the binder traits; this
decision bets only the presentation layer.

## Open questions

Decisions this doc deliberately does not make yet:

- How much of each survivalist-mod system lifts cleanly versus needs a
  rewrite behind its binder trait. Answered per system when its row is
  started, by reading that code, not by assuming.

## Proof: what tests validate this doc

Planned, not written: a design_authority proof module in each repo, following
factoriobot's tests/all/design_authority.rs. Modforge's module proves each
system is the one decision path and binders only observe and act; topside's
module proves the consumer boundary (no local forks of decisions, no second
world-state store, content is data only). Zero proofs exist today; every
claim above is unproved until they do.
