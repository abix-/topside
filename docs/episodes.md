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
- The first episode of the game is The Tap, in the Mixed world. Every
  episode and reality after it comes from the player's decisions and
  randomness (operator, 2026-09-27): choice through consequences, a
  living world. What the player did, as people remember it, makes some
  episodes and realities more likely than others; the storm's pick stays
  random, weighted by those memories. It changes what can come next, never
  how hard it is (authority.md: Randy Random never scales to the player's
  safety). The player never picks the next episode or reality directly.
- The player is never told anything they did not see or hear
  (operator, 2026-09-27): telling them would be cheating. No narration
  of the episode, before, during, or after. The player learns how an
  episode went the way anyone does: by seeing it, or by a person who
  saw it telling them (life.md, "What a person knows").
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

## The lines (operator, 2026-09-27)

The dialogue has to make sense, and it is part of the episode: the
writing, the one thing only a writer gives.

- Lines belong to parts and pivot points, never to a person: whoever is
  cast as Dell speaks Dell's lines.
- The player's choices at a pivot point are things the player says
  ("Show me."), and choosing one reaches the pivot point.
- Lines are parameterized like everything else: anything always the same
  becomes static, repeating, and boring. Each line has several ways of
  saying it, one picked each time, and slots filled from the world.
- A line that points somewhere gives directions a person would give from
  where the player stands, by landmarks they can see: which way out of
  the door, a landmark and which side to keep it on, the place by what it
  is (design.md "The unknown": the player navigates by landmarks, memory,
  and what people say). Never "halfway to the other bunker".
- A person says only what they know: every slot is filled from the
  speaker's memory (memory is the one record).
- Outside an episode, plain talk is built the same way from what the
  speaker knows.

## Conversations (operator, 2026-09-27)

Talking is a conversation: it starts, the player makes choices, and it
ends. Nothing is always on offer.

- The player looks at a person and presses E: that is talking, to that
  person, each NPC on their own. While the player looks at someone in
  talking distance, the screen says "[E] Talk to" and their name.
- A conversation starts when someone speaks (an episode's moment) or when
  the player presses E on someone. The other person says their lines; the
  player's choices are the moment's only.
- Every choice gets the other person's reply, several ways of saying it
  like every line.
- After the last reply the conversation ends and the panel closes by
  itself, once the player has had time to read it.
- E on someone with nothing to talk about gets a short line of theirs
  ("Not now." / "What do you want?"), several ways, and it ends.

### How a conversation looks (operator, 2026-10-01)

Fallout: New Vegas's conversation box, read from its screenshots,
changed for first person. The mockup: docs/mockups/conversation.png.

- One see-through box low in the middle of the screen, its words amber,
  as New Vegas.
- On top, the name of the one talked to and the last line said; under a
  line across the box, the player's choices.
- The choices are numbered, and the number keys say them: 1 says the
  first. That is the only way to choose.
- A choice that does more than words shows its kind in angle brackets,
  in red, before its words: <Threaten>, <Offer> (New Vegas's <Lie> and
  <Attack.>).
- The last line stays on screen while the player chooses. New Vegas
  swaps the line out for the choices; here nobody speaks aloud, so the
  words stay to be read.
- No zoom on the face and no blur: the world goes on, the player keeps
  walking and looking, and the crosshair stays.
- Walking away, past twice talking distance, ends the conversation, as
  Escape does.
- Under the choices, what the keys do and that walking away leaves.
- Each of their lines stays up as long as it takes to read before what
  comes after it, their next line or the player's choices (operator,
  2026-10-02; subtitle practice: about 15 characters a second, never under
  1.5 s; modforge `talk::reading_secs`, the one rule). The player's turn
  waits as long as they like. With nothing left to say, the box closes
  once the last line and the choices still shown have been read; "Let's
  trade." stays offered until then.

### One way for every conversation (operator, 2026-09-30)

Every conversation, an episode's or plain talk, works one way. Prior
art: Valve's response rules (Left 4 Dead, Elan Ruskin, GDC 2012): every
line has conditions on what the speaker knows and feels, and the line
whose conditions match best is the one said. Not a written tree of
moments (Yarn Spinner, Ink): a tree is scripted, and people here react
from what they know (design.md "Your own story").

- Lines and the player's choices are rules: who may say it, what must
  be true of the speaker (what they know, how they feel, what they need,
  the part they are cast as, what was just said to them), the ways of
  saying it, what it tells, and how hearing it feels. When anyone is to
  speak, the best matching line is said; with several equally good, one
  is picked.
- The player's choices are the lines the player could say now: the
  rules for the player that match, shown in the panel.
- A reply is the listener's best matching line to what they just heard.
  A conversation goes on as long as a line matches, and ends when none
  does: as many steps as the rules give, never a fixed tree.
- An episode's lines are rules that need its part (Dell, Mara): while
  the episode runs they win over plain talk. Plain talk is the same rules
  with no part: asked about water and knowing a well, a person gives the
  way; nothing matching, "Not now.".
- What was said is the one record: both memories (life.md "What a person
  knows"). The conversation keeps no list of its own; how far it has gone
  is read from memory.
- A pivot point is reached when the storyteller reads it from memory,
  whatever reached it: a choice said, a shot fired, walking away, doing
  nothing. The talk path never decides a pivot point.
- Anyone starts a conversation the same way: the player with E, an NPC
  when its brain chooses to talk (Dell bringing the news, Mara's child
  asking for water).
- What a choice does beyond words goes through the system that owns it:
  telling and asking through talking, an offer through trading, asking
  someone to come through going with someone (life.md), taking someone
  in through recruiting, an order to one of your people through your
  people (design.md "Your people").
- Each line's words, slots, and ways go through the storyteller's `say`,
  filled from the speaker's memory; a person says only what they know
  and nothing they keep quiet.

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
- The lines: what each part says, and the player's choices at each pivot
  point as things the player says.
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
  episode when the storm lands, and brings the opening to the player
  through the world (a person comes to them, a thing lies on their road).

None of this is built yet. Today the storyteller's `Director` only fires
a weighted random rule on a timer.

## The episodes

- [The Tap](episodes/the-tap.md): the first episode.
- [The Fever](episodes/the-fever.md): the second episode.
- [The Other You](episodes/the-other-you.md): the third episode.
- [Nobody Dies](episodes/nobody-dies.md): the fourth episode.
- [The Question](episodes/the-question.md): the fifth episode, a boss
  defense episode.
