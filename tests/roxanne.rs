//! Roxanne, the one person in the player's bunker besides them, in the
//! parts The Tap's opening is made of, each tested alone over the
//! `command` op (design.md "The bunker you start in"): she walks the
//! bunker's levels, she knocks on the player's door, she leads the player.

use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::endpoint::{op, op_with, people};

/// Up to `secs` real seconds for `done`, a quarter second apart.
fn within(secs: u64, done: impl Fn() -> bool) -> bool {
    for _ in 0..secs * 4 {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    done()
}

/// Roxanne as the `people` op reports her.
fn roxanne() -> Json {
    people().into_iter().find(|p| p["name"] == "Roxanne" && p["dead"] != true).expect("Roxanne, alive, in the people op")
}

/// Where a person stands: the ground point and their feet's height.
fn spot(p: &Json) -> (f64, f64, f64) {
    (p["at"][0].as_f64().unwrap_or(f64::NAN), p["at"][1].as_f64().unwrap_or(f64::NAN), p["feet"].as_f64().unwrap_or(f64::NAN))
}

/// Where person `id` stands now, read from the `npc` op, answered live.
fn npc_at(id: u64) -> (f64, f64, f64) {
    spot(&op_with("npc", json!({"id": id})))
}

/// Person `id`'s walk, commands and actions now, one line, to read a stop
/// by.
fn npc_doing(id: u64) -> String {
    let p = op_with("npc", json!({"id": id}));
    format!("way {}, commands {}, acting {}", p["way"], p["commands"], p["this_tick"])
}

/// Where the player stands now (the `status` op, every step).
fn player_at() -> (f64, f64, f64) {
    let s = op("status");
    (s["player"][0].as_f64().unwrap_or(f64::NAN), s["player"][1].as_f64().unwrap_or(f64::NAN), s["player_feet"].as_f64().unwrap_or(f64::NAN))
}

/// Whether someone (where they stand, read live by `at`) comes within
/// 1.5 m of `to` on its level within `secs` real seconds, and the nearest
/// they came: a command ends on arrival and a person's brain walks them on
/// at once, faster than the once-a-second `people` op shows.
fn comes_to(at: impl Fn() -> (f64, f64, f64), doing: impl Fn() -> String, to: (f64, f64, f64), secs: u64) -> (bool, f64) {
    let start = std::time::Instant::now();
    let mut nearest = f64::INFINITY;
    let mut traced = 0;
    while start.elapsed() < Duration::from_secs(secs) {
        let (x, z, feet) = at();
        // Where they are and what they do, every two seconds, to read a
        // stop by.
        if start.elapsed().as_secs() / 2 >= traced {
            traced += 1;
            eprintln!("    {:.1} s: at ({x:.2}, {z:.2}), feet {feet:.2}; {}", start.elapsed().as_secs_f64(), doing());
        }
        if (feet - to.2).abs() < 0.5 {
            nearest = nearest.min(((x - to.0).powi(2) + (z - to.1).powi(2)).sqrt());
        }
        if nearest <= 1.5 {
            return (true, nearest);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    (false, nearest)
}

/// The player in their room, woken there by dying (the respawn), its door
/// shut first: the bunker's door on the deepest level nearest where the
/// player wakes, used (the `use` op) if it stands open. Returns where the
/// player wakes.
fn the_player_shut_in_their_room() -> (f64, f64, f64) {
    crate::endpoint::player_alive();
    // No commands left from an earlier test.
    op_with("replay", json!({"stop": true}));
    let room = shut_in();
    // Roxanne out of the room last, up in the entrance, so the test starts
    // with her there (an earlier test, or her own errands while the player
    // woke, can leave her in the room with them).
    let id = roxanne()["id"].as_u64().expect("Roxanne's id");
    op_with("command", json!({"id": id, "do": "move_to", "x": 1.5, "y": 0.0, "z": 0.0}));
    let (out, _) = comes_to(|| npc_at(id), || npc_doing(id), (1.5, 0.0, 0.0), 20);
    assert!(out, "Roxanne never went up to the entrance before the test");
    room
}

/// The player woken in their room with its door shut.
fn shut_in() -> (f64, f64, f64) {
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let wake = |_: ()| {
        op_with("kill", json!({"id": player}));
        within(3, || op("player")["dead"] == true);
        crate::endpoint::player_alive();
        within(3, || (op("status")["player_feet"].as_f64().unwrap_or(0.0) + 6.0).abs() < 0.3);
        let s = op("status");
        (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0))
    };
    let room = wake(());
    let doors = op("world")["bunker_doors"].as_array().cloned().unwrap_or_default();
    let door = doors
        .iter()
        .filter(|d| d["floor"].as_f64().is_some_and(|f| (f - room.2).abs() < 0.5))
        .min_by(|a, b| {
            let far = |d: &Json| ((d["at"][0].as_f64().unwrap_or(0.0) - room.0).powi(2) + (d["at"][1].as_f64().unwrap_or(0.0) - room.1).powi(2)).sqrt();
            far(a).total_cmp(&far(b))
        })
        .cloned()
        .expect("a door on the player's level");
    eprintln!("the player's room door: {door}");
    if door["open"] == true {
        op_with("use", json!({"id": player, "at": door["at"], "height": door["floor"]}));
        std::thread::sleep(Duration::from_millis(1500));
        let now = op("world")["bunker_doors"].as_array().and_then(|d| d.iter().find(|d| d["at"] == door["at"] && d["floor"] == door["floor"]).cloned());
        eprintln!("used it: now {now:?}");
        return wake(());
    }
    room
}

/// Roxanne knocks on the player's shut door: the player hears her knock
/// and is in a conversation with her, the talk box open, through the door.
#[test]
fn knocks_on_the_players_door_and_the_talk_box_opens() {
    let room = the_player_shut_in_their_room();
    let her = roxanne();
    let id = her["id"].as_u64().expect("Roxanne's id");
    let heard_before = op("player")["heard_knocking"].as_array().map_or(0, |h| h.len());
    eprintln!("Roxanne ({id}) at {:?}; the player in their room at {room:?}, heard {heard_before} knocks before", spot(&her));
    op_with("command", json!({"id": id, "do": "knock", "who": op("player")["id"]}));
    let mut heard = false;
    for i in 0..120 {
        heard = op("player")["heard_knocking"].as_array().is_some_and(|h| h.len() > heard_before && h.last().is_some_and(|k| k["by"].as_u64() == Some(id)));
        if heard {
            break;
        }
        // Where she is and what she does, every two seconds.
        if i % 8 == 0 {
            let her = op_with("npc", json!({"id": id}));
            eprintln!("  {:.0} s: Roxanne at {:?}, way {}, commands {}, acting {}", i as f64 / 4.0, spot(&her), her["way"], her["commands"], her["this_tick"]);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let opened = within(5, || op("hud")["talk"]["to"].as_u64() == Some(id));
    crate::talk_ui::shot("roxanne_knocks");
    let talk = op("hud")["talk"].clone();
    eprintln!("heard: {heard}, Roxanne at {:?}, the talk box: {talk}", spot(&roxanne()));
    if !heard {
        crate::endpoint::why_they_stopped(id, (room.0, room.2, room.1));
        let her = op_with("npc", json!({"id": id}));
        let did: Vec<Json> = her["did"].as_array().map(|d| d.iter().rev().take(5).cloned().collect()).unwrap_or_default();
        eprintln!("her way: {}, acting: {}, last did: {did:?}", her["way"], her["this_tick"]);
    }
    op_with("key", json!({"key": "Escape"}));
    assert!(heard, "the player never heard Roxanne knock");
    assert!(opened, "the talk box never opened with Roxanne after her knock: {talk}");
}

/// Roxanne walks the stairs again and again: from the entrance on the
/// ground down to the player's room 6 m down and back up, three times, each
/// by a command, never stalling on a flight or a landing (people were seen
/// pressed against the stairwell's wall on a landing).
#[test]
fn walks_the_stairs_up_and_down_three_times() {
    let room = the_player_shut_in_their_room();
    let id = roxanne()["id"].as_u64().expect("Roxanne's id");
    let entrance = (1.5, 0.0, 0.0);
    // In the room off to one side of the player, who wakes in its middle.
    let in_the_room = (room.0 - 1.5, room.1 + 1.5, room.2);
    for trip in 1..=3 {
        for (name, to) in [("down to the room", in_the_room), ("up to the entrance", entrance)] {
            op_with("command", json!({"id": id, "do": "move_to", "x": to.0, "y": to.2, "z": to.1}));
            let (got, nearest) = comes_to(|| npc_at(id), || npc_doing(id),to, 20);
            eprintln!("trip {trip}, {name}: {got}, nearest {nearest:.2} m, Roxanne at {:?}", npc_at(id));
            if !got {
                crate::endpoint::why_they_stopped(id, (to.0, to.2, to.1));
            }
            assert!(got, "trip {trip}: Roxanne never got {name}");
        }
    }
}

/// Roxanne leads the player from their room up to the entrance (a `lead`
/// command), and the player follows her (a `follow` command): the lead is
/// done once both are within 2 m of the spot (operator, 2026-10-02), and
/// she is no longer leading.
#[test]
fn leads_the_player_from_their_room_up_to_the_entrance() {
    let room = the_player_shut_in_their_room();
    let id = roxanne()["id"].as_u64().expect("Roxanne's id");
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // She comes down to the player first, to lead them from their room.
    let (came, _) = {
        op_with("command", json!({"id": id, "do": "move_to", "x": room.0 - 1.5, "y": room.2, "z": room.1 + 1.5}));
        comes_to(|| npc_at(id), || npc_doing(id),(room.0 - 1.5, room.1 + 1.5, room.2), 20)
    };
    assert!(came, "Roxanne never came down to the player's room");
    let entrance = (1.5, 0.0, 0.0);
    op_with("command", json!({"id": id, "do": "lead", "who": player, "x": entrance.0, "y": entrance.2, "z": entrance.1}));
    op_with("command", json!({"do": "follow", "who": id}));
    // Done when both are within 2 m of the spot (modforge `LEAD_DONE`).
    let near = |p: (f64, f64, f64)| ((p.0 - entrance.0).powi(2) + (p.1 - entrance.1).powi(2)).sqrt() <= 2.0 && (p.2 - entrance.2).abs() < 0.5;
    // Where both stand read the moment the lead is seen done (her brain
    // walks her on at once).
    let mut ended = None;
    let start = std::time::Instant::now();
    while ended.is_none() && start.elapsed() < Duration::from_secs(40) {
        let her = op_with("npc", json!({"id": id}));
        if her["leading"].is_null() {
            ended = Some((player_at(), spot(&her)));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let done = ended.is_some();
    let (me, her) = ended.unwrap_or_else(|| (player_at(), npc_at(id)));
    eprintln!("the lead done: {done}; the player at {me:?}, Roxanne at {her:?}");
    if !done || !near(me) || !near(her) {
        crate::endpoint::why_stopped((entrance.0, entrance.2, entrance.1));
        crate::endpoint::why_they_stopped(id, (entrance.0, entrance.2, entrance.1));
    }
    assert!(done, "Roxanne never finished leading the player to the entrance");
    assert!(near(me) && near(her), "the lead ended with the player at {me:?} and Roxanne at {her:?}, not both within 2 m of the entrance");
}

/// Roxanne and the player pass each other: she sent up from the player's
/// room to the entrance and the player down from the entrance to the room
/// at once, through the same doorways and stairs; both get there (walkers
/// stuck in each other's way step aside).
#[test]
fn she_and_the_player_pass_each_other_on_the_way() {
    let room = the_player_shut_in_their_room();
    let id = roxanne()["id"].as_u64().expect("Roxanne's id");
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    let entrance = (1.5, 0.0, 0.0);
    let in_the_room = (room.0 - 1.5, room.1 + 1.5, room.2);
    // Her in the room, the player up in the entrance.
    op_with("command", json!({"id": id, "do": "move_to", "x": in_the_room.0, "y": in_the_room.2, "z": in_the_room.1}));
    let (in_room, _) = comes_to(|| npc_at(id), || npc_doing(id), in_the_room, 20);
    op_with("use", json!({"id": player, "at": [entrance.0, entrance.1]}));
    assert!(in_room, "Roxanne never came down to the player's room");
    // Both at once, past each other.
    op_with("command", json!({"id": id, "do": "move_to", "x": entrance.0, "y": entrance.2, "z": entrance.1}));
    op_with("command", json!({"do": "move_to", "x": in_the_room.0, "y": in_the_room.2, "z": in_the_room.1}));
    let start = std::time::Instant::now();
    let (mut she, mut he) = (false, false);
    while start.elapsed() < Duration::from_secs(30) && !(she && he) {
        let near = |p: (f64, f64, f64), to: (f64, f64, f64)| ((p.0 - to.0).powi(2) + (p.1 - to.1).powi(2)).sqrt() <= 1.5 && (p.2 - to.2).abs() < 0.5;
        she |= near(npc_at(id), entrance);
        he |= near(player_at(), in_the_room);
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("Roxanne up: {she}, the player down: {he}; Roxanne at {:?}, the player at {:?}", npc_at(id), player_at());
    if !she {
        crate::endpoint::why_they_stopped(id, (entrance.0, entrance.2, entrance.1));
    }
    if !he {
        crate::endpoint::why_stopped((in_the_room.0, in_the_room.2, in_the_room.1));
    }
    assert!(she && he, "they never got past each other");
}

/// The player walks past Roxanne standing in their way: she has knocked
/// and stands outside their door, talking, and the player, sent up to the
/// entrance by a command, steps around her (every person a landmass agent)
/// and gets there.
#[test]
fn the_player_steps_around_her_standing_at_their_door() {
    let room = the_player_shut_in_their_room();
    let id = roxanne()["id"].as_u64().expect("Roxanne's id");
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    op_with("command", json!({"id": id, "do": "knock", "who": player}));
    let knocked = within(30, || op("hud")["talk"]["to"].as_u64() == Some(id));
    let her = npc_at(id);
    eprintln!("knocked: {knocked}; Roxanne stands at {her:?}, the player at {room:?}");
    assert!(knocked, "Roxanne never knocked, so never stood in the way");
    // The entrance, on the ground: where she stood before going down.
    let entrance = (1.5, 0.0, 0.0);
    let entrance_3d = (entrance.0, entrance.2, entrance.1);
    op_with("command", json!({"do": "move_to", "x": entrance.0, "y": entrance.2, "z": entrance.1}));
    let (there, nearest) = comes_to(player_at, || format!("way {}", op("player")["way"]), entrance, 30);
    crate::talk_ui::shot("stepped_around");
    eprintln!("there: {there}, nearest {nearest:.2} m; the player at {:?}, Roxanne at {:?}", player_at(), npc_at(id));
    if !there {
        crate::endpoint::why_stopped(entrance_3d);
    }
    assert!(there, "the player never got past Roxanne to the entrance");
}

/// Roxanne walks from where she is down to the player's room, 6 m down
/// through its shut door, and back up to where she stood, each by a
/// command on the navigation mesh. The player is stood topside first, so
/// no one is in her way.
#[test]
fn walks_down_to_the_players_room_and_back() {
    crate::endpoint::player_alive();
    // No commands left from an earlier test.
    op_with("replay", json!({"stop": true}));
    let player = op("player")["id"].as_u64().expect("the player op reports the player's id");
    // Where the player wakes: in their room.
    op_with("kill", json!({"id": player}));
    within(3, || op("player")["dead"] == true);
    crate::endpoint::player_alive();
    within(3, || (op("status")["player_feet"].as_f64().unwrap_or(0.0) + 6.0).abs() < 0.3);
    let s = op("status");
    let room = (s["player"][0].as_f64().unwrap_or(0.0), s["player"][1].as_f64().unwrap_or(0.0), s["player_feet"].as_f64().unwrap_or(0.0));
    // Out of her way: topside, south of the front door.
    op_with("use", json!({"id": player, "at": [0.0, 12.0]}));
    within(3, || op("status")["player_feet"].as_f64().is_some_and(|f| f.abs() < 0.1));

    let her = roxanne();
    let id = her["id"].as_u64().expect("Roxanne's id");
    let from = npc_at(id);
    eprintln!("Roxanne ({id}) at {from:?}; the player's room at {room:?}");
    op_with("command", json!({"id": id, "do": "move_to", "x": room.0, "y": room.2, "z": room.1}));
    let (down, nearest) = comes_to(|| npc_at(id), || npc_doing(id),room, 30);
    crate::talk_ui::shot("roxanne_down");
    eprintln!("down: {down}, nearest {nearest:.2} m; Roxanne now at {:?}, doing {}", spot(&roxanne()), roxanne()["doing"]);
    if !down {
        let her = op_with("npc", json!({"id": id}));
        eprintln!("her way: {}, acting: {}", her["way"], her["this_tick"]);
    }
    assert!(down, "Roxanne never reached the player's room at {room:?}");

    op_with("command", json!({"id": id, "do": "move_to", "x": from.0, "y": from.2, "z": from.1}));
    let (back, nearest) = comes_to(|| npc_at(id), || npc_doing(id),from, 30);
    eprintln!("back: {back}, nearest {nearest:.2} m; Roxanne now at {:?}", spot(&roxanne()));
    if !back {
        let her = op_with("npc", json!({"id": id}));
        eprintln!("her way: {}, acting: {}", her["way"], her["this_tick"]);
    }
    assert!(back, "Roxanne never walked back to {from:?}");
}
