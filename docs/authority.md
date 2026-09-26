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
uses the path written here and never makes a second one. The shape is
Endless's Kubernetes architecture (endless/docs/k8s.md), which factoriobot's
authority map also follows:

- **Def** (the CRD): the shape of a kind of thing. Immutable, no runtime
  state. Content in `content/src/lib.rs`, the struct in modforge.
- **Registry** (etcd): every Def of that kind, stored once and looked up by
  name; the source of truth for base values.
- **Instance** (a CR): one thing in the game, a Bevy entity. All of its
  runtime data lives on its components, each component one concern; no
  parallel list, no second store. An index (the `ActorMap`) holds lookup
  only, never gameplay data.
- **Apply** (`kubectl apply`): the one function that makes an instance from a
  Def. Nothing else makes one.
- **Controllers**: the systems that read the Def and the instance and act.
  They never keep a copy of Def fields on an instance, and they never keep
  their own copy of an instance's data.
- **Copies**: the control plane's published numbers, the HUD, and the
  pictures read the owners and are never read back as the truth.

Rules:

1. Before building anything, find its system below and use its Def,
   Registry, Apply, and Controllers. Never a second path, a second record,
   or a second answer, even when every test passes.
2. Something with no system here: ask the operator, then add it here.
3. A change that breaks a system here is undone, not worked around.
4. Modforge holds every rule that needs no engine, with its test; topside
   senses, acts, and holds content (topside skill: modforge first).

### 1. People

- **Def:** modforge `ActorDef`, one per kind of person (player, bunker
  person, scavenger, raider, lookout, wanderer). **Registry:**
  `ActorRegistryRes` (content `register_actors`).
- **Instance:** one entity per person, the player included (everyone is
  equal). Its data: `Actor` (the ActorId), `Npc` (def, faction), `Transform`,
  `Facing`, `Health`, `Protection`, `Equipment`, `Inventory`, `Hotbar`,
  `Survival`, `Memory`, `Personality`, `Home`, `Store`, `SpawnedAt`,
  `ActorActivity`, `ActorCombat`, `ActorCondition`, `NextThink`, `Actions`,
  `ThisTick`, `NavPath`, `Firing`, `Rolling`, `Hud`, `OpenContainer`,
  `OpenStation`, and `Dead` while dead. **Index:** `ActorMapRes` (modforge
  `ActorMap`): ActorId to entity, and the live count.
- **Apply:** `actor::spawn_person`, reached by `player::spawn_player` and
  `actor::populate_monuments`, `populate_topside_people`,
  `populate_bunkers`, `spawn_door_enemy`.
- **Controllers:** `ai::think` (NPCs), `input::read_input` (the player's
  keys), `input::drain_actions`, `player::roll`, `player::aim`,
  `player::movement`, `nav::steer`, `interact::interact_on_use`,
  `combat::melee`, `combat::fire`, `survival::survival_tick`, `combat::die`,
  `player::respawn`.
- **Never:** a second spawn function; data about a person anywhere but their
  components; a person deleted. A person who dies still exists, `Dead`, to
  query, memory and all (life.md); a topside person never respawns.
- **Status:** RED. `combat::leave` despawns a dead topside person, and the
  storm's `shift_world` erases topside people through it.

### 2. Memory

- **Instance:** the `Memory` component on each person (modforge
  `memory::Memory`): everything they saw (`Known`), who hurt them, their last
  threat, what they did (`Did`: ate, slept, took, put in, hit, was hit,
  killed, died, respawned, went through a doorway), and what they learned
  (modforge `learn`). Each NPC has one memory and all of it goes there.
- **Written by** the path that did the thing: `ai::think` (seeing, checking
  a box, a threat), `combat::hit_actor` (hits and death), `hud` (eating,
  taking, putting in), `nav::doorways`, `player::respawn`.
- **Read by** modforge `brain::choose` (through `Perception`) and the control
  plane.
- **Never:** a log, list, or counter beside it as the truth.
- **Status:** RED. `combat::Deaths` is a second record of deaths;
  `ai::LifeStats` counts what people did beside memory.

### 3. The brain

- **Def:** the `ActorDef`'s behaviour and sight; `Personality` from the
  genome. **Rules:** modforge `brain::choose` (the ordered rules and the
  pick), `brain::reaction_ticks`, modforge `learn` (what was learned picks
  among the choices).
- **Instance:** `ActorActivity`, `ActorCombat`, `ActorCondition`,
  `NextThink` on the person.
- **Controller:** `ai::think` builds the `Perception` from the world (what
  is in sight in the cone with a clear view, `sight`), calls `choose`, and
  pushes the actions it returns. What a remembered thing is worth comes from
  modforge `memory::food_worth` and the registries.
- **Never:** a decision made in topside; the brain reading the world instead
  of memory.

### 4. Actions

- **Def:** modforge `actions::Action`; the keys' bindings
  (`input::BindingsRes`).
- **Instance:** `Actions` (modforge `ActionQueue`) and `ThisTick` on each
  person.
- **Apply:** the keys through `input::read_input` (the one raw input
  reader) and the brain through `ai::think` push the same actions.
- **Controller:** `input::drain_actions` hands each tick's actions to the
  systems that carry them out.
- **Never:** moving or changing a person any other way.

### 5. Needs and the day

- **Rules:** modforge `survival` (`SurvivalStats`, the drain, `eat`,
  `is_night`); the day's length `survival::Day`, the clock `SimTick`.
- **Instance:** `Survival` on each person.
- **Controllers:** `survival::survival_tick` (needs drain, night tires the
  awake), `survival::waste` (starvation and dehydration hit through
  `combat::hit_actor`).
- **Eating and drinking:** only modforge `SurvivalStats::eat`, reached
  through `hud::use_slot`, which also heals and goes into memory.
- **Status:** RED. `interact::use_source` repeats eat, heal, and remember.

### 6. Health, hits, and death

- **Def:** modforge `DamageDef` (weapons, hunger, thirst, the storm).
  **Registry:** `DamageRegistryRes`.
- **Instance:** `Health`, `Protection`, `Equipment` on each person.
- **Controllers:** every hit through `combat::hit_actor` (modforge
  `resolve_hit`), from `combat::melee`, `combat::fire` and `combat::fly`,
  `survival::waste`, and `storm::storm`; every death through `combat::die`
  (the death box, `Dead`); `player::respawn` brings a person with a home
  back.
- **Never:** health changed anywhere else; a person deleted at death.
- **Status:** RED. The respawn rule (half health, full food and water) is
  written in topside `player::respawn`, not in modforge.

### 7. Items and things in the world

- **Def:** modforge `ItemDef` (kind: food, material, tool, weapon,
  deployable, note, medicine, source). **Registry:** `ItemRegistryRes`
  (content `register_items`).
- **Instance:** an item held is a stack in `Inventory`, `Hotbar`,
  `Equipment`, or a box's `Container`; an item in the world is an entity with
  `WorldItem` (and `Container` for a box, `Interactable` to be used).
- **Apply:** `interact::spawn_item`, the one way anything from the item
  system enters the world: a dropped can, a death box, a loot box, a bench,
  a well, a berry bush. Loot rolls through modforge `loot`.
- **Never:** a second spawn function or a marker component for one kind of
  item.
- **Status:** RED. Wells and berry bushes spawn through
  `interact::spawn_source` with a `Source` component.

### 8. Using things

- **Instance:** `Interactable` (modforge `hud::InteractKind`) on a thing;
  `Door` on a door (indexed by chunk in `nav::DoorIndex`).
- **Controller:** `interact::interact_on_use`: a person's Use reaches the
  thing in reach nearest their aim, and modforge `hud::interact` decides
  what happens.
- **Never:** a second use path.

### 9. Crafting

- **Def:** modforge `RecipeDef`. **Registry:** `RecipeRegistryRes`.
- **Instance:** a station is an `Interactable` from `interact::spawn_station`.
- **Controller:** the Craft action in `hud`, through modforge
  `crafting::craft`.

### 10. The world and its ground

- **Def:** modforge `GroundDef` (content `small_ground_def`, `ground_def`),
  `BiomeDef` with its scatter. **Registry:** `BiomeRegistryRes`.
- **Instance:** `terrain::WorldRes` (the rolled modforge `World2d`: ground,
  sites, bunkers), `terrain::Chunks` (the chunks made, their sites, their
  shores), and each chunk's entities (`WorldPiece`: its ground, its water
  and solids, its things).
- **Apply:** `terrain::roll` (from the seed, content `roll_world2d`), then
  `terrain::make_chunks`.
- **The ground of a tile:** only the made chunk, modforge `World2d::chunk`
  (a site's and a bunker's ground cleared).
- **Never:** asking `GroundGen::ground_at` outside world generation.
- **Status:** RED. `interact::nearest_water` asks `GroundGen::ground_at`.

### 11. Pathing

- **Rules and data:** modforge `walk`: the walk grid (`WalkMap`, held in
  `nav::Walk`), the search (`find_way`, by chunk then across chunks by
  doorways), `standable`, and (by pathing.md) the one queue of plans with
  its budget and the wait after a way not found.
- **Instance:** `NavPath` on each walker (its way and goal).
- **Controllers:** `nav::record_solids` paints the grid from every static
  collider; `nav::steer` asks the queue for a way and steps each walker
  along it.
- **Never:** a search, a queue, a budget, or a reach check anywhere else.
- **Status:** RED. The queue, budget, retry, and replanning live in topside
  `nav.rs`, not modforge; `ai::think`'s `can_go` calls `find_way` outside
  the queue.

### 12. Monuments and buildings

- **Def:** modforge `MonumentTypeDef`, `BuildingTypeDef`, `StructureDef`.
  **Registry:** `MonumentRegistryRes`. The small world's sites: content
  `SMALL_SITES`.
- **Instance:** each building a `StructureRoot` entity with its pieces and
  one compound collider, its doors, its `Shelter`; the monuments standing in
  `structure::MonumentsRes`.
- **Apply:** `structure::spawn_structure` (every building, the bunkers
  included), `structure::spawn_sites` (each site world generation placed,
  its boxes through `spawn_item`), `structure::spawn_bunker`,
  `spawn_other_bunkers`.

### 13. Factions

- **Def:** modforge `FactionDef`. **Registry:** `FactionRegistryRes`
  (content `register_factions`: the player's, the topside hostiles, one per
  bunker).
- **Instance:** a person's faction in `Npc`. Hostility is read from the
  registry's table.

### 14. The storm

- **Def:** modforge `StormDef` (content `STORM`), its phase from the day.
- **Instance:** `storm::StormRes` (how many shifts).
- **Controllers:** `storm::storm` (in the open, hit through `hit_actor`;
  under a `Shelter`, safe), `storm::shift_world` (the world goes and rolls
  again from a new seed through `terrain::roll`; bunkers and bunker people
  stay).
- **Status:** RED through row 1: topside people are erased.

### 15. Bunkers and their stores

- **Instance:** a bunker is a building (row 12) with a `Shelter`; its store
  is a box (row 7) that bunker people know through `Store`.
- **Controllers:** the brain's supply rule (row 3) and the hud's take and
  put in (row 7).

### 16. Sight

- **Controllers:** `sight` (`Sight`: what blocks a view, `BlocksSight`),
  used by `ai::think` for the cone and by `sight::player_sight` for what the
  player's person sees.

### 17. Copies: the control plane, the HUD, pictures, timing

- `control_plane.rs` publishes copies of the owners above (`status`,
  `world`, `people`, `npc`, `ways`, `perf`) for tests and watching; the game
  never reads them back. `speed` sets the game's speed.
- The HUD (`hud.rs`) paints the player's own data; pictures (`picture.rs`)
  draw; `profile::Laps` times each step. All copies, never the truth.

### 18. Tests

- `topside-tests` (`tests/`): every test tests the running game through the
  control plane (operator, 2026-09-26); no headless tests.

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
| **Storyteller** | The one director. Two locked layers: Randy Random (random events regardless of how the player is doing; never adaptive) and Mario Kart (adaptive pressure on whoever is winning). Nothing else schedules dramatic events. |
| **The edge** | The border of the unknown beyond the map. The ONE faucet through which new people, items, and factions enter the world. |
| **Incursion** | One thing the storyteller sends across the edge, telegraphed by the dread loop. |
| **Dread loop** | Telegraph, uncertainty, varied payoff, lingering mystery, escalation. The storyteller's pacing mechanism. |
| **Genome** | A faction's traits (aggression, expansionism, defensiveness, guile) and the per-survivor franchise vote that turns traits into decisions. |
| **Act** | One thing a faction does to another or to the player (scavenge, steal, trade, rob, murder, extort, raid). Chosen by the genome vote, executed through the world. |
| **Relationship** | The player's standing with each faction, determined by behavior (killing their people, trading, helping, raiding). Ranges from hostile (kill on sight, raids your bunker) through neutral (cautious trade) to friendly (better prices, mutual defense). Relationship decisions live in modforge (the faction system already tracks inter-faction standing); topside feeds the player's actions as inputs. |
| **Trading** | Exchange of items between the player and any faction or neutral NPC they encounter. Factions trade from real stores (the world is the truth, no infinite shops). Relationship determines willingness and prices. Trading happens at faction territory, their bunker, or in the field. Trade decisions live in modforge; topside provides the UI and world context. |
| **Quality** | The one item-value system: every item rolls a tier, rarer is better, the whole ecosystem (crafters, thieves, traders, warlords) recognizes it. |
| **Named unique** | A one-of-a-kind item that enters only with a matching incursion and is tracked by the chronicle. |
| **The work board** | Offers generated from what factions are actually doing (bounties, escorts, ransoms), paid from real stores. Never scripted. |
| **Upgrades** | Per-structure and settlement-wide upgrade tracks: unbounded levels, diminishing benefits, rising real costs. The player's resource sink. |
| **Chronicle** | The read-only narration of world events to the player. Presentation only; it decides nothing and stores nothing the world does not. |
| **Storm** | The recurring world-changing event (design.md; modeled on MISERY's emission, but that name stays MISERY's and STALKER's). The storyteller schedules it; worldgen executes the regeneration; one hide-or-die check applies to the player and every NPC alike. |
| **WorldItem** | Anything that exists in the world with a position: dropped items, placed crafting benches, generators, storage boxes, doors, loot crates. The ONE name for "a thing in the world." An item in inventory is an ItemStack; when placed or dropped into the world it becomes a WorldItem. Lives in topside (it is Bevy-specific); the concept is referenced by modforge systems that reason about world placement. Avoids "entity" to prevent confusion with Bevy's ECS Entity handle. |
| **Actor** | Anything that acts: it moves or shoots, meaning it pushes actions onto an action queue that `movement` and `fire` run. The player, every NPC, animals, turrets, and vehicles are actors, all on the same components (inventory, hotbar, equipment, survival, health, protection, facing, an action queue) and all through `spawn_actor`. A WorldItem never acts; it is only acted on (picked up, opened, used, hit). The test for a new thing (operator-locked 2026-08-21): does it push actions? Yes means actor, no means WorldItem. A turret is an actor with speed zero placed from the inventory; a vehicle is an actor whose queue the driver fills instead of their own body's. The only differences between actors are the def and who fills the queue (the keyboard, the AI, or a driver). |
| **ActorId** | The one persistent identity of an actor: a monotonic number issued once at spawn, never reused, surviving save and load. Carried as a component. Bevy `Entity` is a runtime handle and the GPU row is a buffer index; neither is an identity. Replaces the slot-as-identity that Endless used. Decided 2026-08-21. |
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
| **Affixes** | Random properties on items (Diablo 2 model), layered on top of the quality tier system. A base item plus a quality tier plus affixes creates the full item identity. Affix definitions are content in topside; the rolling logic lives in modforge alongside the quality system. |
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

## Presentation (decided 2026-09-25: 2D)

Topside is 2D, because 3D takes far more work and this is a one person
project; Factorio and the other big scope games stayed 2D for the same
reason. Everything is done the Factorio way: the low poly cel shaded models
are rendered to pictures ahead of time, and the game draws only pictures,
from Factorio's view. Only 3D first person changes; low poly, cel shading,
AI-generated assets, and Bevy stay.

The 2026-08-19 decision, with only its 3D first person part replaced:

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
