//! App entry. `topside` runs the game; `topside --test` runs the
//! boot test headless and exits 0/1.

use std::time::{Duration, Instant};

use topside::{SimTick, build_app, bunker::FoodStore, entity_map::GpuSlotPool};

fn main() {
    let test_mode = std::env::args().any(|a| a == "--test");
    if test_mode {
        std::process::exit(run_boot_test());
    }
    build_app(false).run();
}

/// Phased boot test, Endless-style: each phase checks one layer and
/// fails fast with the observed value.
fn run_boot_test() -> i32 {
    let mut app = build_app(true);
    println!("phase 1 pass: headless app built");

    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        app.update();
    }
    let ticks = app.world().resource::<SimTick>().0;
    if ticks < 10 {
        println!("phase 2 FAIL: {ticks} simulation ticks in 500ms (expected >= 10 at 60 UPS)");
        return 1;
    }
    println!("phase 2 pass: {ticks} simulation ticks in 500ms");

    let mut pool = GpuSlotPool::default();
    let first = pool.alloc().expect("empty pool must allocate");
    pool.free(first);
    let reused = pool.alloc().expect("pool with a freed slot must allocate");
    if first != reused {
        println!("phase 3 FAIL: freed slot {first} not reused (got {reused})");
        return 1;
    }
    println!("phase 3 pass: slot pool allocates, frees, reuses");

    let food = app.world().resource::<FoodStore>().days;
    if !(food < 3.0 && food > 0.0) {
        println!("phase 4 FAIL: food at {food} days after ticking (expected between 0 and 3)");
        return 1;
    }
    println!("phase 4 pass: food clock ticking ({food:.6} days left)");

    println!("boot test PASS");
    0
}
