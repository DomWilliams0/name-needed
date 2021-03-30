use criterion::{criterion_group, criterion_main, Criterion};

use common::*;
use procgen::benchmark_exports::*;
use procgen::{PlanetParams, RegionLocation};
use std::hint::unreachable_unchecked;

pub fn creation(c: &mut Criterion) {
    let params = PlanetParams::dummy();
    let mut rando = thread_rng();
    let continents = ContinentMap::new_with_rng(&params, &mut rando);

    c.bench_function("region chunk creation", |b| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(8)
            .build()
            .expect("failed to create runtime");
        let mut b = b.to_async(runtime);

        let region_locs = (0..1000)
            .map(|_| {
                RegionLocation(
                    rando.gen_range(0, params.planet_size),
                    rando.gen_range(0, params.planet_size),
                )
            })
            .collect_vec();
        let mut region_locs = region_locs.iter().copied().cycle();

        // TODO make region size a const generic and vary
        b.iter(|| {
            let loc = match region_locs.next() {
                Some(r) => r,
                None => unsafe { unreachable_unchecked() },
            };
            Region::create_for_benchmark(loc, &continents, &params)
        });
    });
}

criterion_group!(benches, creation);
criterion_main!(benches);
