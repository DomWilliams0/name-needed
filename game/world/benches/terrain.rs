use common::*;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use unit::dim::CHUNK_SIZE;
use world::block::BlockType;

use unit::world::ChunkPosition;
use world::helpers::{world_from_chunks, world_from_chunks_with_updates};
use world::loader::ChunkTerrainUpdate;
use world::{ChunkBuilder, ChunkDescriptor};

fn small_world_chunks(sz: i32) -> Vec<ChunkDescriptor> {
    let mut rand = thread_rng();
    (-sz..sz)
        .cartesian_product(-sz..sz)
        .map(|(x, y)| {
            ChunkBuilder::new()
                .fill_range(
                    (0, 0, 0),
                    (CHUNK_SIZE.as_i32() - 1, CHUNK_SIZE.as_i32() - 1, 49),
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

fn tall_world_chunks(height_radius: i32) -> Vec<ChunkDescriptor> {
    let mut rng = common::seeded_rng(Some(1238273873));
    vec![ChunkBuilder::new()
        .fill_range(
            (0, 0, -height_radius),
            (
                CHUNK_SIZE.as_i32() - 1,
                CHUNK_SIZE.as_i32() - 1,
                height_radius,
            ),
            |_| {
                *[BlockType::Air, BlockType::Stone, BlockType::Grass]
                    .iter()
                    .choose(&mut rng)
                    .unwrap()
            },
        )
        .build((0, 0))]
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

pub fn tall_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("tall world");

    for z in &[100, 1000, 10_000] {
        let chunks = tall_world_chunks(*z);
        // group.throughput(Throughput::Elements((z * CHUNK_SIZE.as_i32()) as u64));

        // generate only
        group.bench_with_input(BenchmarkId::new("creation only", z), &z, |b, _| {
            let chunks = &chunks;
            b.iter(move || {
                let _ = world_from_chunks(chunks.clone());
            })
        });

        // generate and apply a tiny 1 block change
        let chunk_updates = vec![ChunkTerrainUpdate::Block(
            (1, 1, 1).into(),
            BlockType::Grass,
        )];
        group.bench_with_input(BenchmarkId::new("tiny 1 block change", z), &z, |b, _| {
            let chunks = &chunks;
            let all_updates = vec![(ChunkPosition(0, 0), chunk_updates.clone())];
            b.iter(move || {
                let updates = all_updates.as_slice();
                let _ = world_from_chunks_with_updates(chunks.clone(), updates);
            })
        });
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = small_world, tall_world
);
criterion_main!(benches);
