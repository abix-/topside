//! What the running game shows: the `screenshot` op saves the window to
//! logs/screenshot.png the next frame, to be looked at.

use crate::endpoint::op;

#[test]
fn the_game_saves_what_it_shows() {
    crate::opening::you_wake();
    let answer = op("screenshot");
    eprintln!("screenshot: {answer}");
    assert!(answer["path"].as_str().is_some_and(|p| p.ends_with("screenshot.png")), "no screenshot path: {answer}");
}
