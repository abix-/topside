# topside combat

**Authoritative for:** how fighting works, for the player and every NPC
alike. design.md holds the game; this doc holds combat. The rules are
engine-free in modforge; topside only moves bodies and casts rays.

## The goal

Combat is simple to use but takes understanding and timing to master
(2026-09-25). Enter the Gungeon is the closest match for the 2D fight.

## Weapons

Post-apocalyptic means everything is a weapon (2026-08-20): melee, guns,
clubs, random crafted things. A pipe, a sharpened piece of scrap, a found
pistol with three rounds, a crafted rifle. Early game you swing junk, late
game you carry crafted firearms with ammunition you made.

- Melee: clubs, pipes, blades, axes, spears. Crafted from scrap and
  materials. Always available, no ammo cost, higher risk.
- Ranged: pistols, shotguns, rifles, bows. Found or crafted. Ammunition is
  a real cost that limits use. Higher tier weapons need higher tier ammo.
- Improvised: anything the crafting system can make. A spiked bat, a nail
  bomb, a sharpened road sign.

Rust is the reference for guns: simple, lethal, ammo matters.

## Simple to use

- Few inputs. WASD moves, the mouse aims, and the person faces the mouse.
  One attack button (plus a direction for the four melee moves) and one
  roll button.
- Every enemy shows its attack before it lands: a wind-up you can see.
- Every hit gives clear feedback: a flash, a knock back, a sound. You
  always know what hit you and why.

## Takes timing to master

- Timing windows: the beat (an attack pressed in the last third of the
  swing chains), the block (about a second), the roll (safe only for its
  first half).
- Commitment: a swing or a roll locks the person in until it ends. A miss
  leaves you open.
- Every threat has a right answer: a wind-up is blocked or rolled, gunfire
  is met with cover or a roll, a crowd is swept or backed away from.
  Understanding is knowing which one.
- Enemies change the answer: the rolled traits (design.md "Enemy
  randomization") make the same bandit a different fight.
- Weapons feel different: speed, reach, and swing width per weapon.

## Melee: the four moves

Gothic's four moves, read relative to where the person faces. One attack
button plus the direction held when the attack lands, read from the same
action queue, so the player and every NPC have the same four.

- Attack plus forward is the overhead: it steps in, hits a narrow strip in
  front, and chains into a combo on the beat.
- Attack plus left or right sweeps an arc across the front.
- Attack plus back blocks for about a second.

The hit is the swing's shape in front of the person. The beat is the fire
timer: the next attack pressed in the last third of the delay continues
without a pause; early or late stalls. A clumsy person (the genome's power
and agility) swings slow and cannot chain. Hits roll against skill later
(Gothic: skill is the chance of a full hit, otherwise a tenth); today every
swing that touches lands.

## Guns

- Every shot is a visible bullet moved in the fixed step, a ray from its
  last position to its new one. No drop.
- Range is damage falloff plus a maximum range per weapon.
- Pellets add up per target: N bullets with a seeded spread, damage summed
  per target, one damage call per target.
- Fire rate is a timer on the shooter, ticked in the simulation: fire only
  when ready and the trigger is held; no ammo costs a delay.

## The roll

A dodge roll like Enter the Gungeon's: about 0.7 s, safe from damage for
the first half, direction locked once started. At first it is only a roll;
upgrades come later.

## Cover

Walls and furniture stop bullets, and a table can be flipped into cover,
as in Enter the Gungeon. Nobody sees behind walls (design.md "What 2D
changes"), so cover also hides.

## How deadly

Similar to Enter the Gungeon, kept on topside's damage numbers: an
ordinary hit takes about a sixth of a person's health.

No body parts for damage, as in Enter the Gungeon and Factorio: a hit lands
on the person, not on a part of them. The body still has parts and armor
pieces worn on them, but their protection is added together into one total
that every hit goes through. Armor reduces damage by type.

## How a hit is resolved

1. One damage function for every hit: fists, bullets, arrows, explosions,
   a wall. It takes the target, what hit it, who fired, the direction, the
   point, and the damage def. Armor first, then health, then die or pain.
   Quake 3's `G_Damage`.
2. Damage is data. A weapon names a damage def in the registry: amount,
   type, knockback, armor rules, falloff by distance. Doom 3.
3. Damage types with resistances: slash, pierce, blunt, fire, and more;
   creatures and worn gear multiply per type. Half-Life's type bits,
   Valheim's resistances. Traits and affixes hang off this.
4. The simulation decides hits, never the renderer: casts run in the fixed
   step against the physics world. Source's rule; single player needs no
   rewind.
5. Buildings are targets: a piece has health and no armor, and the same
   function routes to it.

## Not decided

- A way out in an emergency, like Enter the Gungeon's blank (clears every
  bullet on screen): crafted smoke or a flashbang, now or later.
