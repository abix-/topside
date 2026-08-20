# topside authority map

**Authoritative for:** the one implementation path for each game system, the
ownership boundary between modforge and topside, the naming contract, and the
anti-bypass rules for those paths.

**Not authoritative for:** product vision and story (design.md, not yet
written), capability progress (status.md, not yet written), or the task
backlog (todo.md, not yet written).

Evidence date: 2026-08-19. Nothing is built. Every score below reflects that
honestly. This doc exists BEFORE the code so the code never gets a chance to
grow a second path.

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
| **Quality** | The one item-value system: every item rolls a tier, rarer is better, the whole ecosystem (crafters, thieves, traders, warlords) recognizes it. |
| **Named unique** | A one-of-a-kind item that enters only with a matching incursion and is tracked by the chronicle. |
| **The work board** | Offers generated from what factions are actually doing (bounties, escorts, ransoms), paid from real stores. Never scripted. |
| **Upgrades** | Per-structure and settlement-wide upgrade tracks: unbounded levels, diminishing benefits, rising real costs. The player's resource sink. |
| **Chronicle** | The read-only narration of world events to the player. Presentation only; it decides nothing and stores nothing the world does not. |
| **Storm** | The recurring world-changing event (design.md; modeled on MISERY's emission, but that name stays MISERY's and STALKER's). The storyteller schedules it; worldgen executes the regeneration; one hide-or-die check applies to the player and every NPC alike. |
| **EntityMap** | The one slot-to-entity identity index, taken from Endless. Authoritative for identity, ownership checks, and live counts. |
| **StructureDef** | One building as data (rooms, openings, stairs, furniture, lights), the Def in Def -> Registry -> Instance. Lives in MODFORGE (decided 2026-08-19: the data and relation rules are engine-agnostic, on glam math and plain rgb, so mods can roll structures too, e.g. survivalist town growth). Hand-authored and generated buildings use the same shape; code never composes geometry outside one. |
| **spawn_structure** | The one path from a StructureDef to world geometry and colliders (the Controller). Lives in the CONSUMER: it is the binder that makes a def real in one world (topside spawns Bevy entities; a mod would call its host game's build functions). |
| **MonumentDef** | One monument as data (design.md: a destination worth traveling to, Rust's monuments as the reference): member structures with placements, loot spots, npc spots, gates (a card room's locked access), special features, and the name. Hand-authored or generated, same shape. |
| **spawn_monument** | The one path from a MonumentDef to a placed monument. It places member structures ONLY through spawn_structure and decides nothing that belongs elsewhere: loot value is Quality's, NPC identity is Genome's and the edge's, placement in the world and regeneration are Worldgen's and the storm's. |
| **HUD** | The ONE player-facing information layer. Made of panels: inventory, skills, health, food, prompts, work board, chronicle, map, and any future panel. Lives in MODFORGE as engine-agnostic data and decisions (what to show, when, which panel is open, moving stacks, skill trees). The consumer implements one binder to read its world and one renderer to paint the panels. No second way to show the player information. |
| **GpuSlotPool** | The one slot allocator, taken from Endless. Authoritative for buffer sizing and dispatch bounds. |

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

## Frame structure (from Endless)

All simulation runs in FixedUpdate at a locked 60 UPS, gated on the app
state; Update handles presentation (camera, UI, audio) at display rate.
Gameplay never varies with frame rate. The GPU tick order lives in the GPU
offload section below. The 1000 persistent people (the design.md scale
target) are simulated in full near the player and as cheap abstract faction
facts when far, the A-Life trick; both representations feed the same
systems through the same binders, and identity persists across the switch.

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
| CPU-authoritative | Health; movement goals and waypoints; faction identity; speeds; slot identity (GpuSlotPool + EntityMap) | Written by ECS systems, uploaded to the GPU each tick. The GPU's copy is advisory. |
| CPU-only | Every decision: all nine game systems, behavior states, timers, the off-map abstract world | Never touches the GPU. |
| Render-only | Instance data, animation indices, cel-shade material parameters | Uploaded to render buffers, never seen by compute. |

Hard rules, carried from Endless verbatim:

- A GPU target is a candidate only. The CPU resolves the slot in EntityMap
  and re-validates faction and identity from ECS before any gameplay effect.
  If GPU and ECS disagree on identity or ownership, ECS wins, always.
- Throttled readbacks feed heuristics only, never identity, ownership, or
  validity gates.
- No system reads a GPU field and writes the same field back in the same
  tick (the one-frame delay compounds into oscillation).
- One count source each: GpuSlotPool for buffer sizing and dispatch bounds,
  EntityMap for live gameplay counts. Never mixed.
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
not writing it from scratch.

| Priority | System | Lives in | Score | Authoritative for | Status |
|---|---|---|---:|---|---|
| 1 | World | topside | - | The one simulation state. Every fact about what exists now. Scale target: 1000 persistent simulated NPCs (design.md). | UNBUILT. Decided 2026-08-19: 3D first person on Bevy (see Presentation below). |
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
| 12 | HUD | modforge | - | The one player-facing information layer, made of panels. Inventory, skills, health, food, prompts, work board, chronicle, map. Engine-agnostic data and decisions; the consumer provides a binder to read its world and a renderer to paint it. | UNBUILT. Operator-locked 2026-08-20: "our whole game hud structs and systems and all engine agnostic should be in modforge. so we have ONE way we do all that stuff." |
| 13 | Content registry | topside | - | Checked-in definitions the systems consume (items, structures, factions, incursion kinds, upgrade tracks, the story). Def -> registry -> use. | STARTED 2026-08-19: StructureDef + spawn_structure landed (src/structure.rs); the starting bunker is the first composition expressed as data. The registry itself and the other content kinds remain. |
| 14 | Control plane | modforge | - | The one runtime inspection and op surface (HTTP server, op registry). | EXISTS in modforge; rated 9/10 in survivalist-mod. The first thing wired into topside per the runtime-control-http doctrine. |
| 15 | GPU simulation | topside | - | The compute-shader half of the simulation: crowd movement with separation, the spatial grid, target candidates, projectiles, under the GPU offload contract above. | UNBUILT. The contract is proven in Endless; the WGSL is new (3D). |
| 16 | Worldgen | topside | - | Procedural generation of the topside world, and the storm's regeneration: terrain and monuments (layouts AND names) re-roll; bunkers and the persistent people never regenerate. | UNBUILT. Endless worldgen.rs is the in-house prior art. |
| 17 | Spawn path | topside | - | The one function through which every actor enters the world (edge arrivals, saves, tests). | UNBUILT. Endless materialize_npc is the model. |
| 18 | Save | topside | - | Persistence of the 1000 people, bunkers, factions, and world seed across restarts, with version migration. | UNBUILT. Endless save-load.md is the model. |
| 19 | Test framework | topside | - | In-app integration tests selectable from a debug menu, CLI-runnable headless (--test), phased assertions. Every system row's proof runs here. | UNBUILT. Endless tests/ is the model; the piece factoriobot lacked early and paid for. |
| 20 | MonumentDef | topside | - | Monuments as data: member structures, loot spots, npc spots, gates, special features, names. spawn_monument is the one path; members spawn only through spawn_structure; loot value, NPC identity, and world placement belong to Quality, Genome/the edge, and Worldgen respectively. | UNBUILT. Rust's monuments are the reference (design.md, references.md). |

## Presentation (decided 2026-08-19)

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
