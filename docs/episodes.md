# topside episodes

**Authoritative for:** what an episode is, what every episode must have,
and how an episode is written. Each episode lives in its own file in
docs/episodes/.

## What an episode is (operator, 2026-09-27)

Procedural content and people acting on their own are not enough; a
world like that gets boring. What the player wants is moments: to be
part of a story they wrote. So topside borrows the TV show episode (the
Simpsons, Rick and Morty): one story that starts from something and
ends in many different endings. The difference from TV is that the
player decides how it goes.

- Each storm starts a new episode. The episode ends at the next storm.
- The storm destroys everything the episode was fought over. What the
  player did to people is kept: the people remember it. The choices
  about things stop mattering; the choices about people carry on.
- The next episode is cast from what people remember, so the same
  people come back as friends, enemies, or people who owe you.
- An episode is a moment the player remembers because it is not
  normal: it is different, and the player is the one making the
  choices. Both are needed. Different without choice is TV, someone
  else's story. Choice without different is an ordinary day.
- The storm changes the whole theme, physics, and logic of the world
  (design.md, "The storm"). The reality is its own thing, separate from
  the episodes (realities.md). An episode runs inside a reality, and
  each episode says which realities it fits: the same episode in
  another reality is a different episode.
- The player finds out what is different by doing something, never by
  being told (realities.md says how, for each rule). Finding it out is
  one of the first pivot points, so every choice after it is made
  knowing the rules changed.
- We will make a lot of episodes. Each one is documented in its own
  file, and every one follows the rules below.

## What makes an episode great

Taken from Rick and Morty season 1 (Wikipedia's episode summaries,
read 2026-09-27). Not every episode there has every one of these; they
are what the writers reach for.

1. An ordinary human problem made enormous. The show is about a family;
   the science fiction only makes a crush, bad grades, or a failing
   marriage enormous. A topside episode is about hunger, thirst, trust,
   jealousy, or loyalty, never about the storm or the loot.
2. The idea is pushed all the way. A love potion ends humanity; a
   Meeseeks takes hostages. Follow the one idea further than is sensible.
3. Under it, the cost is real. Morty buries his own body; Summer learns
   she was unwanted. Someone the player can care about pays.
4. The expected thing does not happen. The giant dies from a stupid
   fall; the Titanic does not sink; Evil Morty was behind it all.
5. People never change to suit the plot. Their flaws cause the mess.
6. The writing: lines, jokes, timing. Only a writer gives this; the
   systems do not.

A second story at home, running alongside the first, is the most common
shape in the season (8 of 11 episodes). In topside the bunker keeps
living while the player is out, so an episode ties it to the same thing.

## Player choice that matters (operator, 2026-09-27)

Every episode must give the player choices that matter, all the way
through, not only at the end.

- An episode has pivot points from the opening to the storm. A pivot
  point is a moment where the player chooses, and the choice changes
  what happens later.
- Every choice at a pivot point is remembered by someone, and the
  episode file names who.
- Every choice changes something later: a later pivot point, what the
  player can find out, who is still alive or still home, or which
  ending plays out and how.
- Doing nothing is always a choice, and it has its own consequence.
  The people in the episode keep going after what they want without
  the player.
- What the player can find out depends on what they chose before. The
  reveal is never handed out; it is found by going, talking, or
  catching someone.
- The same last choice plays out differently depending on the pivot
  points before it.
- No ending is clean. Every ending costs someone something.

## Boss episodes and foreboding (operator, 2026-09-27)

Some episodes are boss, attack, and defense episodes, and by the
reality's danger (realities.md, "Danger") they can be crazy hard.

- Attack: the player goes to the boss. It holds a place, and the best
  loot in the reality is behind it. The player chooses whether to try,
  when, and with whom.
- Defense: it comes to the player's bunker, and the bunker holds or
  falls. The bunker's deeper levels, the guard tower's warning, and what
  was stored all matter (design.md, "The bunker" and "Territory").
- One episode leads into the next. The episode before plants the signs:
  something huge on the horizon at night, tracks bigger than a truck, a
  bunker found emptied, animals all running one way, strangers fleeing
  past the door, the radio going quiet. People talk about it. The
  player has that episode to get ready: store ammo, dig deeper, make
  allies. Then the storm lands and it is there.
- Who fights beside the player is what people remember. Mara's bunker
  comes to help if the player saved Mara's child in The Tap, and watches
  the bunker fall, or comes with the boss, if the player shot first.
- In a deadly reality, holding the bunker may be impossible. Then the
  choice is what to save and where to run.

## How an episode is written

Episodes are not vanilla (operator, 2026-09-27). Rick and Morty does
whatever it wants, and so do we.

Episodes have no set structure, and each episode is written with
parameters so it plays out as many different episodes, never the same
one twice (operator, 2026-09-27). A parameter is a part of the episode
the game fills when it starts: what the wish is, who pays, what the
reveal is. Where it can, a parameter is filled from what the player did
in the episodes before.

Each file in docs/episodes/ has these parts, in this order:

- The parameters.
- The ordinary problem.
- The people: each one a part filled from people who already exist in
  the world (a person in the player's bunker, the leader of a nearby
  bunker), with the flaw that drives them.
- How it opens.
- The wish: the thing that enters and gives someone what they want.
- Pushed all the way: how it gets worse on its own.
- The second story at home.
- The reveal.
- The real cost.
- The pivot points, in order, each choice with what it changes and who
  remembers it.
- The endings.
- What carries across the storm.
- What the game does not do yet that the episode needs.

## Where it lives

- The storyteller in modforge (modforge/src/storyteller.rs) holds how
  an episode runs: picking it, filling its people, checking its pivot
  points, deciding its ending. Nothing there depends on Bevy or on
  topside.
- The episodes themselves are content, in topside's content crate.
- Topside answers what the storyteller asks about its world, starts an
  episode when the storm lands, brings the opening to the player, and
  shows what happened when the storm ends it.

None of this is built yet. Today the storyteller's `Director` only fires
a weighted random rule on a timer.

## The episodes

- [The Tap](episodes/the-tap.md): the first episode.
- [The Fever](episodes/the-fever.md): the second episode.
- [The Other You](episodes/the-other-you.md): the third episode.
- [Nobody Dies](episodes/nobody-dies.md): the fourth episode.
- [The Question](episodes/the-question.md): the fifth episode, a boss
  defense episode.
