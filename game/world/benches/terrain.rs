use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use common::*;
use unit::dim::CHUNK_SIZE;
use unit::world::WorldPositionRange;
use world::block::BlockType;
use world::helpers::{apply_updates, loader_from_chunks_blocking, world_from_chunks_blocking};
use world::loader::WorldTerrainUpdate;
use world::{ChunkBuilder, ChunkDescriptor, DeepClone};

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

fn deep_clone(chunks: &[ChunkDescriptor]) -> Vec<ChunkDescriptor> {
    chunks.iter().map(DeepClone::deep_clone).collect()
}

pub fn small_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("world initialization");
    group.sample_size(10);

    for i in 1..=4 {
        let chunks = small_world_chunks(i);
        group.bench_with_input(BenchmarkId::new("chunk radius", i), &i, |b, _| {
            let chunks = &chunks;
            b.iter(move || {
                let _ = world_from_chunks_blocking(deep_clone(chunks));
            })
        });
    }
}

pub fn tall_world(c: &mut Criterion) {
    let mut group = c.benchmark_group("tall world");
    group.sample_size(10);

    for z in &[100, 1000, 10_000] {
        let chunks = tall_world_chunks(*z);
        // group.throughput(Throughput::Elements((z * CHUNK_SIZE.as_i32()) as u64));

        // generate only
        group.bench_with_input(BenchmarkId::new("creation only", z), &z, |b, _| {
            let chunks = &chunks;
            b.iter(move || {
                let _ = world_from_chunks_blocking(deep_clone(chunks));
            })
        });

        // generate and apply a tiny 1 block change
        let updates = vec![WorldTerrainUpdate::new(
            WorldPositionRange::with_single((1, 1, 1)),
            BlockType::Grass,
        )];
        group.bench_with_input(BenchmarkId::new("tiny 1 block change", z), &z, |b, _| {
            let chunks = &chunks;
            let mut loader = loader_from_chunks_blocking(deep_clone(chunks));
            let updates = &updates;
            b.iter(move || {
                let updates = updates.as_slice();
                apply_updates(&mut loader, updates).expect("updates failed");
            })
        });
    }
}

pub fn access_block(c: &mut Criterion) {
    const CHUNKS: i32 = 20;
    let world = world_from_chunks_blocking(small_world_chunks(CHUNKS));
    let w = world.borrow();

    let mut rng = thread_rng();
    let blocks: Vec<(i32, i32, i32)> = (0..1000)
        .map(|_| {
            (
                rng.gen_range(-CHUNKS * CHUNK_SIZE.as_i32(), CHUNKS * CHUNK_SIZE.as_i32()),
                rng.gen_range(-CHUNKS * CHUNK_SIZE.as_i32(), CHUNKS * CHUNK_SIZE.as_i32()),
                rng.gen_range(-2, 50),
            )
        })
        .collect();

    c.bench_function("block access", |b| {
        let mut positions = blocks.iter().cycle();
        b.iter(|| {
            let pos = *positions.next().unwrap();
            let block = w.block(pos);
            black_box(block);
        });
    });
}

criterion_group!(benches, small_world, tall_world, access_block);
criterion_main!(benches);
