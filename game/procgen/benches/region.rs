use std::hint::unreachable_unchecked;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use common::*;
use procgen::benchmark_exports::*;
use procgen::PlanetParams;

macro_rules! mk_bench {
    ($name:ident, $region_size:expr) => {
        pub fn $name(c: &mut Criterion) {
            let params = PlanetParams::dummy();
            let mut rando = thread_rng();
            let continents = ContinentMap::new_with_rng(&params, &mut rando);

            c.bench_with_input(BenchmarkId::new("region chunk creation", $region_size), &(), |b, _| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(8)
                    .build()
                    .expect("failed to create runtime");
                let mut b = b.to_async(runtime);

                let region_locs = (0..1000)
                    .map(|_| {
                        RegionLocationUnspecialized::new(
                            rando.gen_range(0, params.planet_size),
                            rando.gen_range(0, params.planet_size),
                        )
                    })
                    .collect_vec();
                let mut region_locs = region_locs.iter().copied().cycle();

                b.iter(|| {
                    let loc = match region_locs.next() {
                        Some(r) => r,
                        None => unsafe { unreachable_unchecked() },
                    };
                    RegionUnspecialized::<$region_size, {$region_size * $region_size}>::create_for_benchmark(
                        loc,
                        &continents,
                        &params,
                    )
                });
            });
        }
    };
}

mk_bench!(creation_4, 4);
mk_bench!(creation_8, 8);
mk_bench!(creation_16, 16);
mk_bench!(creation_32, 32);

criterion_group!(benches, creation_4, creation_8, creation_16, creation_32);
criterion_main!(benches);
