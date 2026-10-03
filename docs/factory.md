# topside factory

**Authoritative for:** how food, water and resources get from the world
into the bunker and through its stations: where they are found, who
carries them, the orders the player gives, power and wiring, storage,
and defenses. design.md holds the game; this doc holds the factory.

**Not authoritative for:** water as a fluid (design.md, "Water is a
fluid"), territory and captured buildings (design.md, "Territory:
factions fight for buildings"), what a person does on their own when no
order applies (life.md), the tech tree and crafting stations (design.md,
"Technology and crafting").

## The goal (operator, 2026-10-03)

Topside is a proper visual programming factory, the way Factorio is.
In Factorio the factory is a program you can see: items move, machines
work, and when something stops you follow it back to where it ran out.
Topside captures the same thing in the bunker.

## The bunker is the factory (operator, 2026-08-20)

We do not have a factory, but we do have a bunker the player expands,
crafts in, and fills with benches, power and boxes. Not conveyor belts,
but the same idea: you start with one bench and end up with a powered,
multi-room operation with specialized stations and automated defenses.
Rust's base building is the model for what you can place and build;
Factorio's depth is the model for how far the tech goes.

## People are the belts (operator, 2026-10-03)

There are no conveyor belts. People carry everything. The flow is
visible because it is real: a person walking a path with something in
their hands.

## Not just crafting (operator, 2026-10-03)

A bunker needs food, water and resources. They have to be found in the
world and mined, collected, or gathered, then carried home. The factory
starts at the source out in the world, not at the bench.

## Orders (operator, 2026-10-03)

The order given is what the person does. The player is a dictator.
This may change later with NPC behaviour.

Prior art: RimWorld's work priorities, where a pawn does the work it is
given.

## The home systems (moved from design.md, 2026-10-03)

- Power: generators that burn fuel, wiring, powered lights, powered
  defenses, powered stations. The grid is a real system: no fuel, no
  power, no powered anything.
- Storage: boxes with capacity, organization, maybe sorting automation.
- Defenses: turrets, reinforced walls, blast doors, all scaling with
  tech level.

## Not decided yet

Proposed in the 2026-10-03 conversation. None of this is decided; the
operator decides each one.

- Links between boxes and benches: this bench takes only from that box,
  its output goes to this box, keep 10 on hand. Each link is one line of
  the program, placed in the bunker, not in a menu. Prior art: Dwarf
  Fortress stockpile links to workshops, RimWorld bills ("make until you
  have 10").
- Wiring as logic: switches, splitters, AND and OR switches, timers,
  and sensors wired to doors, lights and turrets. Prior art: Rust's
  electricity, Factorio's circuit network.
- Reading the factory: the bunker's days of water and days of food show
  whether it works. When the barrel drops, you follow it back: too few
  carriers, the source too far, the bottles too small, or a carrier who
  did not come home.
- How the parts line up with Factorio:

  | Factorio | topside |
  |---|---|
  | Ore patch | A river, a well, food, a monument to scavenge, scrap |
  | Mining drill | A person ordered to gather there, or a captured building that produces |
  | Belt or train | People carrying it home; the container sets how much one trip moves |
  | How much a line moves | What one carrier brings per day: distance, container, walking speed |
  | Biters attacking an outpost | Raiders ambush a carrier on the road and take what they carry |
  | The ore patch running out | The storm: every source and every road home is gone |

- Prior art for people carrying from the world: Kenshi (squads mine at
  outposts and haul home), RimWorld caravans, Timberborn's haulers.
