//! People walk and go through doorways (design.md "The small world
//! first"), read from each person's memory in the running game's
//! `people` op.

use serde_json::{Value as Json, json};

use crate::endpoint::url;

fn people() -> Option<Json> {
    let resp = ureq::post(url()).send_json(json!({"op": "people", "args": {}})).ok()?;
    resp.into_body().read_json().ok()
}

#[test]
fn people_go_through_doorways() {
    crate::opening::you_wake();
    let people = people().unwrap_or_else(|| panic!("no topside game answering on {}; start it with scripts/build.ps1 run", url()));
    let list = people["result"].as_array().cloned().unwrap_or_default();
    let through: Vec<String> = list
        .iter()
        .filter_map(|p| {
            let n = p["did"]["doorway"].as_u64().unwrap_or(0);
            (n > 0).then(|| format!("{} {} ({}): {n}", p["id"], p["def"].as_str().unwrap_or("?"), p["faction"].as_str().unwrap_or("-")))
        })
        .collect();
    eprintln!("people op: {} people, {} went through a doorway\n{}", list.len(), through.len(), through.join("\n"));
    assert!(!through.is_empty(), "nobody in the running game has gone through a doorway ({} people)", list.len());
}
