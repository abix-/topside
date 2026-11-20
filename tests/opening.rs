//! The Tap's opening (docs/episodes/the-tap.md "How it opens" and "The
//! lines", todo 11aj), one behaviour a test, for the operator's /mygoal
//! "The Tap's opening plays". The path tested is the player believing
//! Roxanne ("Show me."); the other answers are later goals.
//!
//! Every test starts from one of the opening's saves (authority.md
//! "Running tests"), keeps the game paused, and moves it only with `step`,
//! so the same test plays the same every time. The first save,
//! `opening_woke`, can only be made on a game just launched
//! (scripts/build.ps1 run); every later save is reached from the one
//! before it by playing, and made once.
//!
//! Roxanne plays the part the-tap.md calls Dell: the game's world op names
//! that part "Dell".

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, pause, start_from, step};

/// One game second, in ticks.
pub(crate) const SECOND: u64 = 60;

/// Play a game second at a time, at most `secs` game seconds, until `done`
/// holds; whether it did.
pub(crate) fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs {
        if done() {
            return true;
        }
        step(SECOND);
    }
    done()
}

/// Roxanne's ActorId: the person playing The Tap's "Dell" part.
pub(crate) fn roxanne() -> u64 {
    let w = op("world");
    w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Dell")).and_then(|p| p["id"].as_u64()).unwrap_or_else(|| panic!("The Tap has no one in Roxanne's part: episode {}", w["episode"]))
}

pub(crate) fn player() -> u64 {
    op("player")["id"].as_u64().expect("the player op reports the player's id")
}

/// Where a person stands: ground x, ground z, and their feet's height.
pub(crate) fn spot_of(id: u64) -> (f64, f64, f64) {
    let p = op_with("npc", json!({"id": id}));
    (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN), p["feet"].as_f64().unwrap_or(f64::NAN))
}

pub(crate) fn player_spot() -> (f64, f64, f64) {
    let s = op("status");
    (s["player"][0].as_f64().unwrap_or(f64::NAN), s["player"][1].as_f64().unwrap_or(f64::NAN), s["player_feet"].as_f64().unwrap_or(f64::NAN))
}

/// Ground distance between two spots, and whether they are on one level.
pub(crate) fn apart(a: (f64, f64, f64), b: (f64, f64, f64)) -> (f64, bool) {
    (((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt(), (a.2 - b.2).abs() < 1.0)
}

/// The tap: where it stands (ground x, ground z, height 0).
pub(crate) fn the_tap() -> (f64, f64, f64) {
    let w = op("world");
    let tap = w["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).cloned().expect("The Tap has its tap in the world");
    (tap["at"][0].as_f64().unwrap_or(f64::NAN), tap["at"][1].as_f64().unwrap_or(f64::NAN), 0.0)
}

/// The door of the player's room (design.md "The bunker you start in"):
/// the hatch over them, lying in the floor of the room above.
pub(crate) fn the_players_door() -> Json {
    let me = player_spot();
    op("world")["bunker_doors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["hatch"] == true && d["floor"].as_f64().is_some_and(|f| f > me.2 + 0.5 && f < me.2 + 4.5))
        .min_by(|a, b| {
            let far = |d: &Json| apart((d["at"][0].as_f64().unwrap_or(0.0), d["at"][1].as_f64().unwrap_or(0.0), me.2), me).0;
            far(a).total_cmp(&far(b))
        })
        .cloned()
        .expect("a hatch over the player's room")
}

/// A room of the player's bunker as the world op reports it: the one on
/// the "surface" or the one "below".
pub(crate) fn bunker_room(on: &str) -> Json {
    let w = op("world");
    w["starting_bunker"].as_array().and_then(|r| r.iter().find(|r| r["on"] == on).cloned()).unwrap_or_else(|| panic!("no room {on} in the starting bunker: {}", w["starting_bunker"]))
}

/// The middle of a room of the player's bunker on its floor (x, z, height).
pub(crate) fn middle_of(room: &Json) -> (f64, f64, f64) {
    (room["at"][0].as_f64().unwrap_or(f64::NAN), room["at"][1].as_f64().unwrap_or(f64::NAN), room["floor"].as_f64().unwrap_or(f64::NAN))
}

/// Whether a spot (x, z, feet) is inside a room of the player's bunker, on
/// its floor.
pub(crate) fn in_room(room: &Json, p: (f64, f64, f64)) -> bool {
    let (x, z, floor) = middle_of(room);
    let (w, l) = (room["size"][0].as_f64().unwrap_or(0.0), room["size"][1].as_f64().unwrap_or(0.0));
    (p.0 - x).abs() <= w / 2.0 && (p.1 - z).abs() <= l / 2.0 && (p.2 - floor).abs() < 1.0
}

/// How far a ground point is from a hatch's hole (0 over it): its middle,
/// its length along its hinge, and its width across.
pub(crate) fn from_the_hole(hatch: &Json, p: (f64, f64)) -> f64 {
    let f = |v: &Json| v.as_f64().unwrap_or(f64::NAN);
    let (cx, cz, ax, az) = (f(&hatch["at"][0]), f(&hatch["at"][1]), f(&hatch["along"][0]), f(&hatch["along"][1]));
    let (dx, dz) = (p.0 - cx, p.1 - cz);
    let along = (dx * ax + dz * az).abs() - f(&hatch["length"]) / 2.0;
    let across = (dx * -az + dz * ax).abs() - f(&hatch["width"]) / 2.0;
    (along.max(0.0).powi(2) + across.max(0.0).powi(2)).sqrt()
}

/// Every line `by` said to the player, in order: (tick, words).
pub(crate) fn heard_from(by: u64) -> Vec<(u64, String)> {
    op("player")["heard"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|h| h["by"].as_u64() == Some(by))
        .map(|h| (h["tick"].as_u64().unwrap_or(0), h["said"].as_str().unwrap_or_default().to_string()))
        .collect()
}

pub(crate) fn reached(pivot: &str) -> bool {
    op("world")["reached"].as_array().is_some_and(|r| r.iter().any(|p| p["pivot"] == pivot))
}

pub(crate) fn talk() -> Json {
    op("hud")["talk"].clone()
}

pub(crate) fn choices() -> Vec<String> {
    talk()["choices"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(str::to_string)).collect()
}

/// Press the number key of the first choice shown that is one of `words`;
/// the words chosen.
pub(crate) fn choose(words: &[&str]) -> String {
    let shown = choices();
    let i = shown.iter().position(|c| words.contains(&c.as_str())).unwrap_or_else(|| panic!("none of {words:?} among the choices {shown:?}"));
    op_with("key", json!({"key": (i + 1).to_string()}));
    step(SECOND);
    shown[i].clone()
}

/// The player walks to the note in their room and reads it (E), as
/// bunker.rs does: the note lies 1.4 m west and 3.6 m south of where the
/// player wakes.
fn read_the_note() {
    let (x, z, feet) = player_spot();
    let note = (x - 1.4, z + 3.6);
    op_with("command", json!({"do": "move_to", "x": note.0, "y": feet, "z": note.1}));
    // A walk is done anywhere within the brain's REACH (1.5 m, modforge
    // brain.rs), which is also Use's reach (interact.rs REACH): wait for the
    // command to end there.
    let there = within(10, || op("player")["commands"].is_null() && apart(player_spot(), (note.0, note.1, feet)).0 <= 1.5);
    if !there {
        crate::endpoint::why_stopped((note.0, feet, note.1));
    }
    assert!(there, "the player never walked to the note at ({:.1}, {:.1}), feet {feet:.2}: they stand at {:?}", note.0, note.1, player_spot());
    op_with("key", json!({"key": "E"}));
    let read = within(3, || op("hud")["reading"] == true);
    assert!(read, "E by the note did not open it");
    op_with("key", json!({"key": "Escape"}));
    step(SECOND);
}

pub(crate) fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Which hand a point `p` (from where someone stands) is on for someone
/// facing `f`, both on the ground (x, z). The world is y-up: a body facing
/// (fx, fz) has its right hand toward f x up = (-fz, fx), so p is on the
/// right when fx*pz - fz*px > 0.
fn hand(f: (f64, f64), p: (f64, f64)) -> &'static str {
    if f.0 * p.1 - f.1 * p.0 > 0.0 { "right" } else { "left" }
}

/// Roxanne's way to the tap is true in this world (the-tap.md "The lines"),
/// checked against the world op, not against how the game words it: the
/// way out of the bunker's front door, the building the tap stands in, and,
/// when she names one, a landmark of that kind standing along the way on
/// the side she says; when she names none, none she knows stands there.
fn the_way_is_true(line: &str) {
    let w = op("world");
    let xz = |v: &Json| (v[0].as_f64().unwrap_or(f64::NAN), v[1].as_f64().unwrap_or(f64::NAN));
    // The front door: the player's bunker's door on the ground; the bunker:
    // the building below the ground nearest it.
    let door = w["bunker_doors"].as_array().into_iter().flatten().find(|d| d["hatch"] != true && d["floor"].as_f64().is_some_and(|f| f.abs() < 0.5)).map(|d| xz(&d["at"])).expect("the bunker's front door on the ground");
    let bunker = w["below"].as_array().into_iter().flatten().map(|b| xz(&b["at"])).min_by(|a, b| dist(*a, door).total_cmp(&dist(*b, door))).expect("the player's bunker");
    let thing = |name: &str| w["things"].as_array().into_iter().flatten().find(|t| t["name"] == name).map(|t| xz(&t["at"])).unwrap_or_else(|| panic!("no {name} in the world"));
    let (tap, site) = (thing("tap"), thing("tap site"));
    let monuments: Vec<(String, (f64, f64))> = w["monuments"].as_array().into_iter().flatten().map(|m| (m["kind"].as_str().unwrap_or_default().to_string(), xz(&m["at"]))).collect();
    let place = monuments.iter().filter(|(_, at)| dist(*at, site) < 1.0).map(|(kind, _)| kind.clone()).next().expect("a monument standing at the tap's site");

    // The way out: straight out within 35 degrees of the door's outward
    // facing (from the bunker to its door), else the hand the tap is on.
    let out = (door.0 - bunker.0, door.1 - bunker.1);
    let to = (tap.0 - door.0, tap.1 - door.1);
    let cos = (out.0 * to.0 + out.1 * to.1) / ((out.0.powi(2) + out.1.powi(2)).sqrt() * (to.0.powi(2) + to.1.powi(2)).sqrt());
    let way = if cos >= 35f64.to_radians().cos() { "straight out" } else { hand(out, to) };
    eprintln!("the world: the door at {door:?} facing {out:?}, the tap at {tap:?} in a {place}; the way out is {way}");
    let said_way = ["straight out", "left", "right"].iter().filter_map(|w| line.find(w).map(|i| (i, *w))).min().map(|(_, w)| w);
    assert_eq!(said_way, Some(way), "Roxanne's way out of the door is not the world's: {line:?}");
    assert!(line.contains(&place), "Roxanne does not name the {place} the tap stands in: {line:?}");

    // On the way: within 80 m of the walk from the door to the tap, between
    // them (episode.rs ON_THE_WAY, the-tap.md "{landmark}").
    let on_the_way = |at: (f64, f64)| {
        let len2 = to.0.powi(2) + to.1.powi(2);
        let along = ((at.0 - door.0) * to.0 + (at.1 - door.1) * to.1) / len2;
        let near = (door.0 + to.0 * along, door.1 + to.1 * along);
        along > 0.0 && along < 1.0 && dist(at, near) <= 80.0
    };
    match line.find(" on your ") {
        Some(i) => {
            let side = line[i + " on your ".len()..].split(|c: char| !c.is_alphabetic()).next().unwrap_or_default();
            let before = line[..i].trim_end_matches(" is");
            let landmark = before.rsplit("the ").next().unwrap_or_default();
            let there = monuments.iter().any(|(kind, at)| kind == landmark && dist(*at, site) >= 1.0 && on_the_way(*at) && hand(to, (at.0 - door.0, at.1 - door.1)) == side);
            assert!(there, "no {landmark} stands along the way to the tap on the {side}: {line:?}");
        }
        None => {
            let her = roxanne();
            let knows = op_with("npc", json!({"id": her}))["knows"].clone();
            let known_on_the_way: Vec<String> = knows
                .as_array()
                .into_iter()
                .flatten()
                .map(|k| xz(&k["at"]))
                .filter(|at| dist(*at, site) >= 1.0 && on_the_way(*at))
                .filter_map(|at| monuments.iter().find(|(_, m)| dist(*m, at) < 1.0).map(|(kind, _)| kind.clone()))
                .collect();
            assert!(known_on_the_way.is_empty(), "Roxanne names no landmark, yet she knows {known_on_the_way:?} along the way: {line:?}");
        }
    }
}

// The opening: one play from a game just launched to pivot point 1, every
// check along the way (orchestrator, 2026-10-03: what playing to pivot 1
// does is checked in that play, no saves before it).

/// The opening played from a game just launched to Roxanne's answers shown,
/// checked as it goes:
/// 1. The Tap's people already exist: Roxanne of the player's bunker plays
///    Dell, Mara's part is a person of another bunker.
/// 2. The player wakes in their room, 3 m below the surface room.
/// 3. Without the note read, Roxanne stays away from the hatch over the
///    player's room; the note read, she is on her way to it within ten game
///    seconds, nearer than 3 m to its hole.
/// 4. She comes to the shut hatch on its floor within a game minute.
/// 5. She knocks, the player hears it, and 6. her first words are "You
///    awake in there?", called through the shut door, no talk opened.
/// 7. The player opens the hatch walking up the stairs and stands in the
///    surface room; within talking reach the talk opens with her.
/// 8. She says the water is nearly gone and gives the way to the tap, true
///    to the world, and the player then knows the tap, told by her.
/// 9. Each of her lines stays up long enough to read (modforge
///    `talk::reading_secs`) before the next or the answers.
pub(crate) fn play_to_pivot_1() {
    // A game just launched and paused at once has not yet published its
    // world: one game second first.
    step(SECOND);
    let w = op("world");
    let fresh = w["episode"] == "The Tap" && w["reached"].as_array().is_some_and(|r| r.is_empty()) && talk()["open"] != true;
    assert!(fresh, "the opening is played on a game just launched: run scripts/build.ps1 run first (now: episode {}, reached {})", w["episode"], w["reached"]);
    // 1.
    let part = |name: &str| w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == name)).and_then(|p| p["id"].as_u64());
    let faction = |id: u64| crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(id)).map(|p| p["faction"].clone()).unwrap_or(Json::Null);
    let (her, mara) = (part("Dell").expect("someone plays Roxanne's part"), part("Mara").expect("someone plays Mara's part"));
    let name = crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(her)).and_then(|p| p["name"].as_str().map(str::to_string));
    assert_eq!(name.as_deref(), Some("Roxanne"), "Roxanne's part is played by {name:?}");
    assert_eq!(faction(her), "player", "Roxanne is of the player's bunker");
    assert!(faction(mara) != "player" && !faction(mara).is_null(), "Mara is of another bunker, not {}", faction(mara));
    // 2. And the bunker is its two rooms as they wake (bunker.rs).
    let feet = player_spot().2;
    assert!((feet + 3.0).abs() < 0.3, "the player woke with their feet at {feet:.2}, not 3 m down");
    crate::bunker::the_bunker_is_two_rooms_the_surface_room_and_the_players_room_below();
    // 3.
    let hatch = the_players_door();
    let near = || {
        let (x, z, _) = spot_of(her);
        from_the_hole(&hatch, (x, z)) < 3.0
    };
    assert!(!within(10, near), "Roxanne came to the hatch before the note was read: she stands at {:?}", spot_of(her));
    read_the_note();
    assert!(within(10, near), "Roxanne did not go to the hatch after the note was read: she stands at {:?}", spot_of(her));
    // 4.
    let floor = hatch["floor"].as_f64().unwrap_or(f64::NAN);
    let came = within(60, || {
        let (x, z, feet) = spot_of(her);
        // Her knock spot is 0.7 m out from the hatch's free edge, and she
        // arrives within 0.5 m of it (interact.rs knock_spot, KNOCK_ARRIVE).
        from_the_hole(&hatch, (x, z)) < 1.3 && (feet - floor).abs() < 1.0
    });
    assert!(came, "Roxanne never came to the player's hatch {hatch}: she stands at {:?}", spot_of(her));
    // 5. and 6.
    assert!(within(30, || heard_her_knock(her)), "the player never heard Roxanne knock after reading the note");
    assert!(within(5, || !heard_from(her).is_empty()), "Roxanne said nothing with her knock");
    assert_eq!(heard_from(her)[0].1, "You awake in there?", "Roxanne's first words were not the line through the door");
    assert!(talk()["open"] != true, "a conversation opened through the shut door: {}", talk());
    // 7.
    open_the_door();
    let surface = bunker_room("surface");
    assert!(within(10, || in_room(&surface, player_spot())), "the player never came up into the surface room: they stand at {:?}", player_spot());
    crate::talk_ui::shot("bunker_stairs");
    assert!(within(10, || talk()["to"].as_u64() == Some(her)), "the door open, no conversation with Roxanne: {}", talk());
    // 8. and 9.: her lines until the answers show, a quarter second at a
    // time.
    let mut shown_at = None;
    for _ in 0..60 * 4 {
        if !choices().is_empty() {
            shown_at = Some(op("status")["tick"].as_u64().unwrap_or(0));
            break;
        }
        step(SECOND / 4);
    }
    let shown_at = shown_at.expect("no answers shown within a game minute of the talk opening");
    let lines = heard_from(her);
    eprintln!("Roxanne's lines (tick, words): {lines:?}; answers shown at tick {shown_at}");
    let opening_said = op("player")["heard"].as_array().into_iter().flatten().any(|h| h["by"].as_u64() == Some(her) && h["line"] == "the opening");
    assert!(opening_said, "Roxanne never said the opening line, that the water is nearly gone: {lines:?}");
    let way = lines.iter().map(|(_, w)| w).find(|l| l.contains("tap")).cloned().unwrap_or_else(|| panic!("Roxanne never gave the way to the tap"));
    assert!(!way.contains('{'), "a part of the way left unfilled: {way:?}");
    the_way_is_true(&way);
    let tap_key = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap")).and_then(|t| t["key"].as_u64()).expect("the tap's key");
    let told = op("player")["knows"].as_array().and_then(|k| k.iter().find(|k| k["key"].as_u64() == Some(tap_key))).and_then(|k| k["told_by"].as_u64());
    assert_eq!(told, Some(her), "the player does not know the tap, told by Roxanne");
    let reading = |words: &str| ((words.chars().count() as f64 / 15.0).max(1.5) * SECOND as f64) as u64;
    for pair in lines.windows(2) {
        let ((t, words), (next, _)) = (&pair[0], &pair[1]);
        assert!(next - t >= reading(words), "{words:?} was up {} ticks, {} needed to read it", next - t, reading(words));
    }
    let (last, words) = lines.last().cloned().unwrap_or_default();
    // The answers are seen a quarter game second at a time.
    let up = (shown_at + SECOND / 4).saturating_sub(last);
    assert!(up >= reading(&words), "the answers came {up} ticks after {words:?}, {} needed to read it", reading(&words));
}

/// The opening plays to pivot point 1 (`play_to_pivot_1`), on a game just
/// launched (scripts/build.ps1 run).
#[test]
fn the_opening_plays_to_roxannes_answers() {
    let _paused = pause();
    play_to_pivot_1();
}

/// Whether the player has heard `her` knock.
fn heard_her_knock(her: u64) -> bool {
    op("player")["heard_knocking"].as_array().is_some_and(|h| h.iter().any(|k| k["by"].as_u64() == Some(her)))
}

/// The player opens the hatch of their room as a player does: walking up
/// the stairs under it to the floor above (a walker opens the closed door
/// ahead of them, navmesh.rs `walk`), to just past one end of its hole.
pub(crate) fn open_the_door() {
    let door = the_players_door();
    let f = |v: &Json| v.as_f64().unwrap_or(f64::NAN);
    let out = f(&door["length"]) / 2.0 + 0.7;
    let to = (f(&door["at"][0]) + f(&door["along"][0]) * out, f(&door["at"][1]) + f(&door["along"][1]) * out);
    op_with("command", json!({"do": "move_to", "x": to.0, "y": f(&door["floor"]), "z": to.1}));
    let opened = within(20, || op("world")["bunker_doors"].as_array().is_some_and(|d| d.iter().any(|d| d["hatch"] == true && d["open"] == true)));
    if !opened {
        crate::endpoint::why_stopped((to.0, f(&door["floor"]), to.1));
    }
    assert!(opened, "the player never opened the hatch walking up the stairs to {to:?}; they stand at {:?}", player_spot());
}

/// The game's start, just before its first pivot point ("the note read"):
/// the player waking in their room on a game just launched, as the
/// opening plays it (`play_to_pivot_1` checks the same fresh game). Made
/// once with TOPSIDE_MAKE_SAVE=tap_0_you_wake on a game just launched.
pub(crate) fn you_wake() {
    start_from("tap_0_you_wake", || {
        step(SECOND);
        let w = op("world");
        let fresh = w["episode"] == "The Tap" && w["reached"].as_array().is_some_and(|r| r.is_empty());
        assert!(fresh, "tap_0_you_wake is made on a game just launched: run scripts/build.ps1 run first (now: episode {}, reached {})", w["episode"], w["reached"]);
    });
}

/// Pivot point 1's save (authority.md "Running tests": one save per pivot,
/// just before it): Roxanne's opening said and the player's answers shown.
/// Made on a game just launched by the opening's one play
/// (`play_to_pivot_1`).

pub(crate) fn choosing() {
    start_from("tap_1_dell_tells_you", play_to_pivot_1);
}

/// The way to the tap played from pivot point 1's save to just before pivot
/// point 2, checked as it goes (orchestrator, 2026-10-03: what playing to a
/// pivot does is checked in that play):
/// 1. "Show me." chosen, the player follows Roxanne out of the bunker onto
///    the ground.
/// 2. A way runs on the navigation mesh from where they stand to the tap.
/// 3. Leading, Roxanne waits for the player who falls behind (operator,
///    2026-10-02, todo 11y: just past 8 m, modforge brain.rs LEAD_WAIT) and
///    goes on toward the tap once they catch up.
/// 4. Following her again, the player comes within 8 m of the tap, not yet
///    within the 4 m that reaches who they took.
pub(crate) fn play_to_pivot_2() {
    choosing();
    let her = roxanne();
    let tap = the_tap();
    // 1.
    choose(&["Show me.", "Let's go."]);
    let out = within(120, || player_spot().2 > -0.5 && op("player")["following"].as_u64() == Some(her));
    if !out {
        // Which link of "Show me." to out the door broke.
        let rox = op_with("npc", json!({"id": her}));
        eprintln!("reached: {}", op("world")["reached"]);
        eprintln!("the player following: {}, at {:?}", op("player")["following"], player_spot());
        eprintln!("Roxanne at {:?}, leading {}, way {}, doing {}", spot_of(her), rox["leading"], rox["way"], rox["doing"]);
    }
    assert!(out, "the player was not following Roxanne on the ground within two game minutes of \"Show me.\"");
    // 2.
    let (x, z, feet) = player_spot();
    op_with("nav_path", json!({"from": [x, feet, z], "to": [tap.0, tap.2, tap.1]}));
    // The mesh answers on the next frame, paused or not.
    let mut way = op("nav_path");
    for _ in 0..30 {
        if way["from"].is_array() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        way = op("nav_path");
    }
    let end = way["path"].as_array().and_then(|p| p.last()).cloned().unwrap_or(Json::Null);
    let ends_at = (end[0].as_f64().unwrap_or(f64::NAN), end[2].as_f64().unwrap_or(f64::NAN), 0.0);
    assert!(apart(ends_at, tap).0 < 1.5, "no way on the mesh from the bunker door to the tap {tap:?}: it ends at {ends_at:?}; {way}");
    // 3. She may be waiting already (she can get ahead on the stairs); let
    // her walk on first, so the wait measured is the one the player's
    // stopping causes.
    let waiting = || !op_with("npc", json!({"id": her}))["leading"]["waiting_at"].is_null();
    assert!(within(30, || !waiting()), "Roxanne never walked on with the player following");
    // The player stops: an order of their own ends following (authority.md
    // "Commands and replays").
    let (x, z, feet) = player_spot();
    op_with("command", json!({"do": "move_to", "x": x, "y": feet, "z": z}));
    // The gap on the tick she starts waiting, in 3D (authority.md
    // "Presentation": nothing is flattened to the ground).
    let gap3d = || {
        let (h, p) = (spot_of(her), player_spot());
        (apart(h, p).0.powi(2) + (h.2 - p.2).powi(2)).sqrt()
    };
    let mut gap = 0.0;
    let mut waits = false;
    for _ in 0..30 * SECOND {
        step(1);
        if waiting() {
            waits = true;
            gap = gap3d();
            break;
        }
    }
    assert!(waits, "Roxanne never waited for the player left behind");
    // Past 8 m by at most what she walks while turning to it.
    assert!(gap > 8.0 && gap <= 13.0, "Roxanne waits {gap:.1} m from the player, not just past 8 m");
    let waited_at = spot_of(her);
    op_with("command", json!({"do": "move_to", "x": waited_at.0, "y": waited_at.2, "z": waited_at.1}));
    assert!(within(30, || apart(spot_of(her), tap).0 < apart(waited_at, tap).0 - 3.0), "caught up, Roxanne did not go on toward the tap: she stands at {:?}", spot_of(her));
    // 4.
    crate::saves::follow(None, her);
    let following = || op("player")["following"].as_u64() == Some(her);
    assert!(within(3, following), "told to follow Roxanne again, the player is not following her: {}", op("player")["following"]);
    let near = within(600, || !following() || apart(player_spot(), tap).0 < 8.0);
    assert!(following(), "the player stopped following Roxanne on the way to the tap: at {:?}, Roxanne at {:?}, commands {}", player_spot(), spot_of(her), op("player")["commands"]);
    assert!(near, "following Roxanne, the player never came within 8 m of the tap {tap:?}: at {:?}, Roxanne at {:?}", player_spot(), spot_of(her));
    assert!(!reached("takes Dell"), "who the player took was reached before 4 m of the tap: {}", op("world")["reached"]);
}

/// Pivot point 2's save: following Roxanne to the tap, just short of it.
/// Made by `play_to_pivot_2` from pivot point 1's save.
pub(crate) fn who_you_take() {
    start_from("tap_2_who_you_take", play_to_pivot_2);
}

/// The loading screen holds until the navigation mesh is built (todo 9j,
/// authority.md "Running tests": loading): a saved game loaded, and the
/// moment `status` says loading is done, the way from the bunker door out
/// to the tap is asked once, and the first answer to that ask is a way that
/// ends at the tap.
#[test]
fn a_way_runs_from_the_bunker_door_to_the_tap() {
    let _p = pause();
    // `start_from` waits until `status` says loading is done (`loaded`).
    start_from("tap_2_who_you_take", || panic!("the save tap_2_who_you_take is made by the_way_to_the_tap_plays_to_who_you_take"));
    let door = op("world")["bunker_doors"].as_array().and_then(|d| d.iter().find(|d| d["hatch"] == false)).cloned().expect("the bunker's door out");
    let from = [door["at"][0].as_f64().unwrap_or(f64::NAN), door["floor"].as_f64().unwrap_or(f64::NAN), door["at"][1].as_f64().unwrap_or(f64::NAN)];
    let tap = the_tap();
    let to = [tap.0, tap.2, tap.1];
    op_with("nav_path", json!({"from": from, "to": to}));
    // Answered on the next frame; only this ask's own answer counts (the
    // game keeps the last answer until the next is worked out).
    let own = |a: &Json| a["from"].as_array().is_some_and(|f| f.iter().zip(from).all(|(f, x)| f.as_f64().map(|f| f as f32) == Some(x as f32)));
    let mut way = op("nav_path");
    for _ in 0..50 {
        if own(&way) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        way = op("nav_path");
    }
    assert!(own(&way), "no answer of its own to the way from the door {from:?} to the tap {to:?}: {way}");
    let end = way["path"].as_array().and_then(|p| p.last()).cloned().unwrap_or(Json::Null);
    let ends_at = (end[0].as_f64().unwrap_or(f64::NAN), end[2].as_f64().unwrap_or(f64::NAN), 0.0);
    assert!(apart(ends_at, tap).0 < 1.5, "asked once loading was done, no way from the bunker door {from:?} to the tap {tap:?}: it ends at {ends_at:?}; {way}");
}

/// The way to the tap plays from pivot point 1 to just before pivot point 2
/// (`play_to_pivot_2`).
#[test]
fn the_way_to_the_tap_plays_to_who_you_take() {
    let _paused = pause();
    play_to_pivot_2();
}

/// The player and Roxanne at the tap.
pub(crate) fn at_the_tap() {
    start_from("opening_at_the_tap", || {
        who_you_take();
        let tap = the_tap();
        let there = within(20, || apart(player_spot(), tap).0 < 4.0 && apart(spot_of(roxanne()), tap).0 < 4.0);
        assert!(there, "reaching opening_at_the_tap: the player and Roxanne were not both at the tap within ten game minutes");
    });
}

// The tests, in the order the opening plays.

/// The player woke three game minutes ago and has not read the note (the
/// operator's playtest of 2026-10-03 read the note only after time had
/// passed): a game just launched, played on three game minutes. It waits,
/// and fits no pivot's save: with the orchestrator to decide.
fn woke_minutes_ago() {
    step(SECOND);
    let w = op("world");
    assert!(w["reached"].as_array().is_some_and(|r| r.is_empty()), "played on a game just launched: run scripts/build.ps1 run first (reached {})", w["reached"]);
    step(3 * 60 * SECOND);
}

/// 6c. Read three game minutes after waking, the note still sends Roxanne
/// down to knock, and nobody in the player's bunker has fought: the
/// operator's playtest of 2026-10-03 found her doing "fight" at tick 9128
/// and no knock.
#[test]
fn read_minutes_after_waking_the_note_still_brings_roxanne_to_knock() {
    let _paused = pause();
    woke_minutes_ago();
    let her = roxanne();
    let fought = |id: u64| -> Vec<String> {
        op_with("npc", json!({"id": id}))["did"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| d["did"].as_str())
            .filter(|d| d.starts_with("hit ") || d.starts_with("was hit by "))
            .map(str::to_string)
            .collect()
    };
    let bunker: Vec<u64> = crate::endpoint::people().into_iter().filter(|p| p["faction"] == "player").filter_map(|p| p["id"].as_u64()).collect();
    // Three game minutes on, she is still home in the surface room: nothing
    // she knows has sent her out before the opening.
    let home = in_room(&bunker_room("surface"), spot_of(her));
    if !home {
        let rox = op_with("npc", json!({"id": her}));
        let tap = op("world")["things"].as_array().and_then(|t| t.iter().find(|t| t["name"] == "tap").cloned()).unwrap_or_default();
        // Only her memory of the tap: all she knows is too long to read.
        let of_tap: Vec<Json> = rox["knows"].as_array().into_iter().flatten().filter(|k| k["key"] == tap["key"]).cloned().collect();
        eprintln!("Roxanne at {} feet {} doing {} about {}", rox["at"], rox["feet"], rox["doing"], rox["about"]);
        eprintln!("she did {}", rox["did"]);
        eprintln!("the tap {tap}; her memory of it {of_tap:?}; she knows {} things", rox["knows"].as_array().map_or(0, |k| k.len()));
    }
    assert!(home, "three game minutes after waking, Roxanne is not home in the surface room: she stands at {:?}", spot_of(her));
    read_the_note();
    let seen_fighting = std::cell::RefCell::new(None);
    let knocked = within(90, || {
        let rox = op_with("npc", json!({"id": her}));
        if rox["doing"] == "fight" && seen_fighting.borrow().is_none() {
            *seen_fighting.borrow_mut() = Some(format!("doing fight about {} at {:?}, last threat {}", rox["about"], spot_of(her), rox["last_threat"]));
        }
        heard_her_knock(her)
    });
    let fighting = seen_fighting.into_inner();
    if let Some(threat) = &fighting
        && let Some(by) = op_with("npc", json!({"id": her}))["last_threat"]["who"].as_u64()
    {
        let who = op_with("npc", json!({"id": by}));
        eprintln!("Roxanne {threat}; the threat: {} of {} at {} feet {}", who["def"], who["faction"], who["at"], who["feet"]);
    }
    assert!(knocked, "the note read three minutes after waking, the player never heard Roxanne knock; Roxanne at {:?}, {}", spot_of(her), fighting.clone().unwrap_or_default());
    assert!(fighting.is_none(), "Roxanne fought on her way to knock: {}", fighting.unwrap_or_default());
    for id in bunker {
        let hits = fought(id);
        assert!(hits.is_empty(), "person {id} of the player's bunker fought: {hits:?}");
    }
}

/// 9. The four answers show, each said one of its ways, and a number key
/// picks one ("Where exactly?" here, which leaves the opening open).
#[test]
fn the_four_answers_show_and_a_number_key_picks_one() {
    let _paused = pause();
    choosing();
    let shown = choices();
    eprintln!("the answers: {shown:?}");
    let ways: [&[&str]; 4] = [
        &["Show me.", "Let's go."],
        &["Water doesn't just run out there. You're wrong.", "I don't believe it."],
        &["Go and look again. I'll wait.", "Check it again first."],
        &["Where exactly?", "Tell me the way again."],
    ];
    for answer in ways {
        assert!(shown.iter().any(|c| answer.contains(&c.as_str())), "none of {answer:?} among the answers {shown:?}");
    }
    // The box (episodes.md "How a conversation looks"): the answers drawn,
    // and the line on top with no slot left unfilled.
    let box_now = talk();
    assert_eq!(box_now["drawn"], true, "the answers are not drawn: {box_now}");
    let line = box_now["line"]["words"].as_str().unwrap_or_default();
    assert!(!line.contains('{'), "a slot left unfilled in the line on top: {line:?}");
    let said = |p: &Json| p["said"].as_array().map_or(0, |s| s.iter().filter(|s| s["to"].as_u64() == Some(roxanne())).count());
    let before = said(&op("player"));
    let words = choose(&["Where exactly?", "Tell me the way again."]);
    let after = within(3, || said(&op("player")) > before);
    assert!(after, "the number key for {words:?} said nothing");
}

/// 10. "Show me." is believing Roxanne, and she answers "Follow me. Keep
/// up." or "This way. Stay close."
#[test]
fn show_me_believes_roxanne_and_she_says_follow_me() {
    let _paused = pause();
    choosing();
    let her = roxanne();
    assert!(!reached("believes Dell"), "believing Roxanne was reached before the player chose: {}", op("world")["reached"]);
    let shown = talk();
    let words = choose(&["Show me.", "Let's go."]);
    let believed = within(5, || reached("believes Dell"));
    if !believed {
        // Why the choice reached nothing: was it said (the player's memory),
        // was the box still offering it, and how far apart were the two.
        let p = op("player");
        eprintln!("the talk box when chosen: {shown}");
        eprintln!("the talk box now: {}", talk());
        eprintln!("the player said: {}", p["said"]);
        eprintln!("the player heard: {}", p["heard"]);
        eprintln!("the player and Roxanne {:.2} m apart", apart(player_spot(), spot_of(her)).0);
    }
    assert!(believed, "{words:?} reached nothing: reached {}", op("world")["reached"]);
    let replies = ["Follow me. Keep up.", "This way. Stay close."];
    let replied = within(5, || heard_from(her).iter().any(|(_, w)| replies.contains(&w.as_str())));
    assert!(replied, "Roxanne never answered {words:?}: she said {:?}", heard_from(her));
    // Her reply said, the conversation ends by itself.
    assert!(within(8, || talk()["open"] != true), "the conversation never ended after her reply: {}", talk());
}

/// 12. Roxanne leads the player to the tap, and both get there: from
/// pivot point 2's save, within seconds.
#[test]
fn roxanne_leads_the_player_to_the_tap() {
    let _paused = pause();
    who_you_take();
    let her = roxanne();
    let tap = the_tap();
    // Where she is and what she does each second, printed if they do not
    // both get there.
    let track = std::cell::RefCell::new(Vec::new());
    let there = within(20, || {
        let rox = op_with("npc", json!({"id": her}));
        let me = op("player");
        track.borrow_mut().push(format!(
            "tick {} at {} doing {} about {} leading {} way {} commands {}; the player at {} following {} commands {} acting {}",
            rox["tick"], rox["at"], rox["doing"], rox["about"], rox["leading"], rox["way"], rox["commands"], me["at"], me["following"], me["commands"], me["this_tick"]
        ));
        apart(player_spot(), tap).0 < 4.0 && apart(spot_of(her), tap).0 < 4.0
    });
    if !there {
        eprintln!("Roxanne, the first seconds from the load:\n{}", track.borrow().iter().take(3).cloned().collect::<Vec<_>>().join("\n"));
        eprintln!("the player following {}, reached {}", op("player")["following"], op("world")["reached"]);
        // Whom she fought, if she did.
        if let Some(by) = op_with("npc", json!({"id": her}))["last_threat"]["who"].as_u64() {
            let who = op_with("npc", json!({"id": by}));
            eprintln!("her last threat {by}: {} of {} at {} feet {} doing {} about {}", who["def"], who["faction"], who["at"], who["feet"], who["doing"], who["about"]);
        }
    }
    assert!(there, "not both at the tap {tap:?} within twenty game seconds: the player at {:?}, Roxanne at {:?}", player_spot(), spot_of(her));
}

/// 13. The player's first drink at the tap takes one water from Mara's
/// bunker's store and is "the first drink".
#[test]
fn the_first_drink_takes_one_water_from_maras_store() {
    let _paused = pause();
    at_the_tap();
    let w = op("world");
    let mara = w["cast"].as_array().and_then(|c| c.iter().find(|p| p["part"] == "Mara")).and_then(|p| p["id"].as_u64()).expect("Mara's part");
    let bunker = crate::endpoint::people().into_iter().find(|p| p["id"].as_u64() == Some(mara)).map(|p| p["faction"].as_str().unwrap_or_default().to_string()).expect("Mara's bunker");
    let water = || op("world")["bunkers"][bunker.as_str()]["water"].as_f64().unwrap_or(0.0);
    let before = water();
    assert!(!reached("the first drink"), "the first drink was reached before the player drank: {}", op("world")["reached"]);
    let tap = the_tap();
    op_with("use", json!({"id": player(), "at": [tap.0, tap.1]}));
    let drank = within(5, || water() < before);
    if !drank {
        // What the Use reached instead: the talk box, what the player did,
        // and who stands near the tap.
        eprintln!("the talk box {}; the player at {:?}, acting {}, looking at {}", talk(), player_spot(), op("player")["this_tick"], op("hud")["prompt"]);
        for p in crate::endpoint::people() {
            let at = (p["at"][0].as_f64().unwrap_or(1e9), p["at"][1].as_f64().unwrap_or(1e9), p["feet"].as_f64().unwrap_or(0.0));
            if apart(at, tap).0 < 5.0 {
                eprintln!("  near the tap: {} ({}) at {at:?}, doing {}", p["name"], p["id"], p["doing"]);
            }
        }
        eprintln!("the tap at {tap:?}; the world's things {}", op("world")["things"]);
    }
    assert!(drank, "a drink at the tap took no water from Mara's bunker's store ({before})");
    assert!(within(5, || reached("the first drink")), "the first drink at the tap is not reached");
}
