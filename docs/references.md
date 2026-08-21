# topside references

The games we mine for ideas, from the design discussion of 2026-08-19.
Each row says what topside takes from it and what it deliberately avoids.
design.md will draw from this list.

| Game | Mine | Avoid |
|---|---|---|
| MISERY | Base expansion in a post-apocalyptic world, fighting for survival. The feel we loved first. | Hand-authored content that runs dry: 4 generators, identical repeating patterns; the loop is felt out and there is nothing else. |
| Survivalist: Invisible Strain | The substrate our own survivalist-mod proved out: settlements, recruits, zombie apocalypse survival. | Too static without the mod; content instead of simulation. |
| Survivalist (2013) | The original, and amazing: leading a growing community after the collapse, recruiting survivors with their own personalities, dealing with factions through trade, diplomacy, or force. Solo-dev proof of the community-leadership fantasy. | Top-down isometric distance from the world; scripted story beats that end; a world that waits for the player. |
| Fallout Shelter | The pull of building up a shelter and its people; the bunker as home base. | Shallow loop, no world to be embodied in. |
| The Bloodline | The shell: solo-dev, first person, your town that you build and recruit for, a world to explore. Proof the shape works at indie scale. Also the reference for first-person melee combat feel: weighty, directional, satisfying even with crude weapons (operator-locked 2026-08-20: "think about how bloodlines did combat too. its a lot of fun if done properly"). | Too static: the world only does what the developer scripted. Topside puts a living simulation behind the same shell. |
| Valheim | You FEEL survival because you are first person in 3D. Low poly carrying atmosphere through lighting and sound. Indie-scale proof of embodied survival plus base building. | World does not act on its own; content gates (biome bosses) are finite. |
| STALKER (A-Life) | A living faction simulation experienced from first person: squads travel, fight, and take territory whether or not you are there. The closest prior art to topside's heart. | The world state resets and forgets too much; factions do not build. |
| RimWorld | The storyteller model (already operator-locked in the survivalist design): a base simulation plus a director pacing drama. Randy Random by name. | Top-down spectating; we go embodied instead. |
| Dwarf Fortress | Systems that interact instead of content that repeats; stories the developer never authored. | Presentation and accessibility. |
| Mount & Blade | A faction war layer that runs without the player, experienced from inside one body. | Nothing major; scale is the difference. |
| Kenshi | A brutal open world that does not care about you; squad survival between living factions. | Clunky control layer between the player and the world. |
| Project Zomboid | Zombie apocalypse survival depth (needs, injuries, seasons); "this is how you died" honesty. | Isometric distance from the world; largely static world state. |
| Diablo 2 / Path of Exile | Loot randomization: affix rolls, quality tiers, the chase for a specific drop. PoE's lesson: systems multiplying against each other beat a bigger random list. | A memorized slot machine: once you know the pool, randomization alone stops surprising. |
| Tarkov (extraction shooters) | Risk of losing what you carry makes every trip out tense; extraction as the rhythm of leaving safety and coming back. | Multiplayer-dependent tension; topside's danger comes from the simulation instead. |
| Rust | Raids. Monuments as loot destinations with tiered danger (card rooms, Oil Rig with heavies and crates, underwater lab, Dome). Special timed events like Cargo. We do not have to be unique, just take good ideas and implement them well. Topside goes more extreme on randomization: monument layouts AND names are random. | Fixed monument identities you memorize; multiplayer-dependent danger. |
| Shadow of Mordor (nemesis system) | Enemies that remember you, survive, and come back promoted: content generated from the player's own history. | Nothing major; the idea, not the implementation. |
| Incremental games | The always-more-to-buy pull: unbounded upgrade tracks, diminishing returns, rising costs. Already designed in the survivalist mod's settlement upgrades. | Numbers replacing the world; upgrades must stay physical and visible. |
| State of Decay | The facility/upgrade model the survivalist-mod settlement upgrades were patterned on (operator: more complex in types and levels). | Timer-driven busywork. |
| Soulmask | The model for NPC control: recruited people with their own skills and traits, assigned to work stations and duties they run autonomously, guard and follow roles, companions in the field. First-person-compatible crew management without micromanagement. | Nothing named yet; mined for the control scheme. |
| Atlas | The survival pressure model: thirst, hunger, temperature, vitamins, weather effects all stacking, never letting up. 2000 hours of proof that layered survival pressure keeps a game dangerous long after the early game. The reference for how topside's survival systems should feel. | Vitamin system complexity; server instability; shallow endgame beyond survival. |
| Oregon Trail | The operator's frame for the whole game: Oregon Trail meets post-apocalyptic 3D. Attrition IS the game: rationing, sickness, weather, hazards that just hit you, named companions dying along the way. | Nothing major; the frame, not the format. |

## Combat, read from source (2026-08-20)

Cloned next to the repos and read, not summarised from memory:
`C:\code\Quake-III-Arena` (GPL), `C:\code\halflife` (Valve SDK),
`C:\code\DOOM-3` (GPL). Rust is closed; its rules come from
Facepunch's devblogs 74, 75, 117, 123.

| Source | Mine | Avoid |
|---|---|---|
| Quake 3 `g_weapon.c`, `g_combat.c`, `g_missile.c`, `bg_pmove.c` | One damage function for every hit: `G_Damage(target, inflictor, attacker, dir, point, damage, flags, means_of_death)`: armor absorbs a share, knockback is damage over mass added to velocity, then health, then `die` or `pain`. Fire is a trace from the eye along the view; melee (gauntlet) is a 32 unit trace. Projectiles trace their own path every step from last position to new. Fire rate is a timer on the shooter in the shared movement code (`PM_Weapon`): ready only when the timer is zero, no ammo costs a delay. Shotgun spread is seeded so the client predicts the same pattern. | Damage numbers hardcoded in C per weapon. No damage types, no hit location. |
| Half-Life 1 `combat.cpp`, `weapons.cpp`, `crowbar.cpp` | Melee traces a line, and if it misses, sweeps a fat hull and takes that hit (`Swing`). Pellets accumulate per target through a multi-damage register and land as one `TakeDamage` call. Hitgroups (head, chest, stomach, arms, legs) multiply damage per difficulty. Damage type bits (club, bullet, burn, blast) or'd onto the hit drive reactions. | Global accumulator state; monsters had no armor. |
| Doom 3 `Actor.cpp`, `Player.cpp`, `Weapon.cpp`, `Projectile.cpp` | Damage as data: `Damage(inflictor, attacker, dir, damageDefName, scale, location)` reads `damage`, `push`, `kickDir`, `noArmor`, `noGod`, `selfDamageScale`, `gib` from a def. Weapons and projectiles name their def. Armor saves `ceil(damage * protection)` and always leaves 1. | Script-driven weapon state; not needed. |
| Rust (Facepunch devblogs) | The hybrid bullet: "the bullet immediately travels a set number of meters (this is ammo dependent), and then it carries on as normal", so "close quarters gunplay is effectively hitscan, but long range retains the effects of gravity" (117). Damage falloff by distance per weapon class (123). Protection per body area from worn clothing instead of hitboxes that armor resizes; per-weapon body part multipliers; "best-fit hit detection" (117). Penetration power versus surface resistance, damage scaled by remaining projectile integrity (75). | Client-side projectiles with server verification (no network here). Penetration and ricochet until actors have body parts. |
| Valheim (wiki) | Damage types slash, pierce, blunt, fire, frost, lightning, poison, spirit; per-creature resistances as multipliers; stagger, block, parry; weapon arcs as swept shapes. | Swept arcs until weapons differ by arc; the ray plus fat retry covers it. |
| Source (Valve wiki) | The simulation decides hits, never the renderer. Lag compensation rewinds other players' hitboxes to the shooter's time. | The rewind itself: single player. |
