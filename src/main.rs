mod core;
mod paths;

use core::*;

fn main() {
    let mut world = World::new();
    world.load_region(0, 0).unwrap();
    world.tick_bots().unwrap();
}
