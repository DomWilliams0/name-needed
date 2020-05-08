use common::{thread_rng, Itertools, Rng};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use unit::dim::CHUNK_SIZE;
use world::block::BlockType;

#[cfg(any(test, feature = "benchmarking"))]
use world::{world_from_chunks, ChunkBuilder, ChunkDescriptor};

fn small_world_chunks(sz: i32) -> Vec<ChunkDescriptor> {
    let mut rand = thread_rng();
    (-sz..sz)
        .cartesian_product(-sz..sz)
        .map(|(x, y)| {
            ChunkBuilder::new()
                .fill_range(
                    (0, 0, 0),
                    (CHUNK_SIZE.as_i32(), CHUNK_SIZE.as_i32(), 50),
                    |_| match rand.gen_range(0i32, 3) {
                        0 => BlockType::Grass,
                        1 => BlockType::Stone,
                        _ => BlockType::Air,
                    },
                )
                .build((x, y))
        })
        .collect_vec()
}

pub fn small_world(c: &mut Criterion) {
    for i in 1..=4 {
        let chunks = small_world_chunks(i);
        c.bench_with_input(BenchmarkId::new("small world", i), &i, |b, _| {
            let chunks = &chunks;
            b.iter(move || {
                let _ = world_from_chunks(chunks.clone());
            })
        });
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = small_world
);
criterion_main!(benches);

#[test]
fn test_small_world() {
    let huge = small_world_chunks(3);
    let _ = world_from_chunks(huge);
}
