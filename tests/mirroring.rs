//! People's left-side facings are mirrors (RimWorld draws west as east
//! flipped): in the running game no person picture for facing 3, 4 or 5
//! is shown (none is rendered), and people facing the left side show the
//! right side's picture flipped.

use crate::endpoint::people;

/// The facing in a person picture's name: `... <facing>.png` or
/// `... <facing> walk <frame>.png`.
fn facing_of(name: &str) -> Option<usize> {
    let stem = name.strip_suffix(".png")?;
    let stem = stem.split(" walk ").next()?;
    stem.rsplit(' ').next()?.parse().ok()
}

#[test]
fn people_facing_left_show_the_right_side_flipped() {
    let list = people();
    let mut left = 0;
    for p in list.iter().filter(|p| p["dead"] == false) {
        let pictures = p["pictures"].as_array().cloned().unwrap_or_default();
        for picture in &pictures {
            let name = picture["picture"].as_str().unwrap_or("");
            let shown = facing_of(name).unwrap_or_else(|| panic!("{} shows {name}, no facing in it", p["id"]));
            assert!(!(3..=5).contains(&shown), "{} shows {name}: facings 3 to 5 are never rendered", p["id"]);
        }
        let facing = p["facing"].as_u64().expect("the people op reports each facing") as usize;
        let mirrored = (12 - facing) % 8;
        if (3..=5).contains(&facing)
            && !pictures.is_empty()
            && pictures
                .iter()
                .all(|pic| pic["flipped"] == true && facing_of(pic["picture"].as_str().unwrap_or("")) == Some(mirrored))
        {
            left += 1;
        }
    }
    eprintln!("{} people, {left} alive facing the left side, drawn as the right side flipped", list.len());
    assert!(left > 0, "nobody alive faces the left side drawn flipped; read again after");
}
