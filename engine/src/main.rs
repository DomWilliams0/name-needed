use world;

mod engine;

fn main() {
    let mut world = world::World::default();
    let eng = engine::Engine::new(&mut world);
    eng.run();
}
