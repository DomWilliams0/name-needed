# TODOs (101)
 * [.build/build-release.sh](.build/build-release.sh) (1)
   * `# TODO declare sdl version somewhere else`
 * [.build/run-tests.sh](.build/run-tests.sh) (1)
   * `# TODO fix "LNK1189: library limit of 65535 objects exceeded" on windows when building `testing` crate`
 * [ai/src/consideration.rs](ai/src/consideration.rs) (1)
   * `// TODO dont bother running destructors`
 * [ai/src/context.rs](ai/src/context.rs) (1)
   * `// TODO use a separate allocator for ai to avoid fragmentation`
 * [ai/src/decision.rs](ai/src/decision.rs) (1)
   * `// TODO cow type for dse (aibox, framealloc, borrowed)`
 * [ai/src/intelligence.rs](ai/src/intelligence.rs) (6)
   * `// TODO bump allocator should not expose bumpalo specifically`
   * `// TODO pool/arena allocator`
   * `// TODO use an arena-allocator hashmap`
   * `// TODO perfect hash on C::Input`
   * `// TODO add momentum to initial weight to discourage changing mind so often`
   * `// TODO reuse allocation`
 * [grid/src/dynamic.rs](grid/src/dynamic.rs) (3)
   * `// TODO use same CoordType for DynamicGrid`
   * `// TODO profile and improve coord wrapping`
   * `// TODO return <C: GridCoord>`
 * [grid/src/grid_impl.rs](grid/src/grid_impl.rs) (1)
   * `// TODO can still panic`
 * [misc/Cargo.toml](misc/Cargo.toml) (1)
   * `# TODO feature for cgmath`
 * [misc/src/newtype.rs](misc/src/newtype.rs) (1)
   * `// TODO support f64 too`
 * [resources/src/container.rs](resources/src/container.rs) (2)
   * `// TODO depends on feature gate`
   * `// TODO add feature gate info e.g. from disk, from archive`
 * [unit/src/dim.rs](unit/src/dim.rs) (2)
   * `// TODO unsafe unchecked casts with no panicking code`
   * `// TODO helper for this-1`
 * [unit/src/world/slab_position.rs](unit/src/world/slab_position.rs) (1)
   * `// TODO consider using same generic pattern as SliceIndex for all points and positions`
 * [unit/src/world/slice_index.rs](unit/src/world/slice_index.rs) (1)
   * `// TODO ideally handle global slice integer overflow, although unlikely`
 * [world/src/block.rs](world/src/block.rs) (2)
   * `// TODO store sparse block data in the slab instead of inline in the block`
   * `// TODO this should return an Option if area is uninitialized`
 * [world/src/chunk/double_sided_vec.rs](world/src/chunk/double_sided_vec.rs) (1)
   * `// TODO refactor to use a single vec allocation`
 * [world/src/chunk/slab.rs](world/src/chunk/slab.rs) (7)
   * `// TODO detect when slab is all air and avoid expensive processing`
   * `// TODO if exclusive we're in deep water with CoW`
   * `// TODO discover internal area links`
   * `// TODO sucks to do this because we cant mutate the block directly while iterating`
   * `// TODO if leaving alone, ensure default is correct`
   * `// TODO consider resizing/populating changes_out initially with empty events for performance`
   * `// TODO reserve space in changes_out first`
 * [world/src/chunk/slice.rs](world/src/chunk/slice.rs) (2)
   * `// TODO consider generalising Slice{,Mut,Owned} to hold other types than just Block e.g. opacity`
   * `// TODO make not pub`
 * [world/src/chunk/terrain.rs](world/src/chunk/terrain.rs) (6)
   * `// TODO actually add get_{mut_}unchecked to slabs for performance`
   * `// TODO could skip next slice because it cant be walkable if this one was?`
   * `// TODO this is sometimes a false positive, triggering unnecessary copies`
   * `// TODO use an enum for the slice range rather than Options`
   * `// TODO set_block trait to reuse in ChunkBuilder (#46)`
   * `// TODO 1 area at z=0`
 * [world/src/context.rs](world/src/context.rs) (1)
   * `/// TODO very temporary "walkability" for block types`
 * [world/src/loader/finalizer.rs](world/src/loader/finalizer.rs) (10)
   * `let mut entities_to_spawn = vec![]; // TODO reuse`
   * `// TODO mark chunk as "not ready" so its mesh is only rendered when it is finalized`
   * `let mut area_edges = Vec::new(); // TODO reuse buf`
   * `let mut links = Vec::new(); // TODO reuse buf`
   * `let mut ports = Vec::new(); // TODO reuse buf`
   * `// TODO is it worth combining occlusion+nav by doing cross chunk iteration only once?`
   * `// TODO only propagate across chunk boundaries if the changes were near to a boundary?`
   * `// TODO reuse/pool bufs, and initialize with proper expected size`
   * `// TODO is it worth attempting to filter out updates that have no effect during the loop, or keep filtering them during consumption instead`
   * `// TODO prevent mesh being rendered if there are queued occlusion changes?`
 * [world/src/loader/loading.rs](world/src/loader/loading.rs) (4)
   * `// TODO add more efficient version that takes chunk+multiple slabs`
   * `// TODO shared instance of CoW for empty slab`
   * `// TODO reuse vec allocs`
   * `// TODO reuse buf`
 * [world/src/loader/terrain_source/mod.rs](world/src/loader/terrain_source/mod.rs) (1)
   * `// TODO handle wrapping of slabs around planet boundaries`
 * [world/src/loader/update.rs](world/src/loader/update.rs) (1)
   * `// TODO include reason for terrain update? (god magic, explosion, tool, etc)`
 * [world/src/loader/worker_pool.rs](world/src/loader/worker_pool.rs) (1)
   * `// TODO detect this as an error condition?`
 * [world/src/mesh.rs](world/src/mesh.rs) (4)
   * `let mut vertices = Vec::<V>::new(); // TODO reuse/calculate needed capacity first`
   * `// TODO skip if slice knows it is empty`
   * `// TODO ignore occluded face, return maybeuninit array and len of how much is initialised`
   * `// TODO also rotate texture`
 * [world/src/navigation/area_navigation.rs](world/src/navigation/area_navigation.rs) (3)
   * `// TODO use graphmap to just use areas as nodes? but we need parallel edges`
   * `|edge| edge.weight().cost.weight(), // TODO could prefer wider ports`
   * `// TODO avoid calculating path just to throw it away`
 * [world/src/navigation/block_navigation.rs](world/src/navigation/block_navigation.rs) (2)
   * `// TODO use vertical distance differently?`
   * `// TODO improve allocations`
 * [world/src/navigation/cost.rs](world/src/navigation/cost.rs) (1)
   * `// TODO currently arbitrary, should depend on physical attributes`
 * [world/src/navigation/discovery.rs](world/src/navigation/discovery.rs) (3)
   * `/// flood fill queue, pair of (pos, pos this was reached from) TODO share between slabs`
   * `// indices are certainly valid - TODO unchecked unwrap`
   * `// TODO use unchecked unwrap here`
 * [world/src/navigation/path.rs](world/src/navigation/path.rs) (1)
   * `// TODO smallvecs`
 * [world/src/navigation/search.rs](world/src/navigation/search.rs) (1)
   * `// TODO this might be expensive, can we build up the vec in order`
 * [world/src/occlusion.rs](world/src/occlusion.rs) (7)
   * `/// TODO bitset of Opacities will be much smaller, 2 bits each`
   * `// TODO this is different to the actual Default!`
   * `// TODO return a transmuted u16 when bitset is used, much cheaper to create and compare`
   * `// TODO ideally check the slice first before calculating offset but whatever`
   * `// TODO only for debugging`
   * `// TODO pub(crate)`
   * `// TODO comparison by face or against all faces`
 * [world/src/ray.rs](world/src/ray.rs) (3)
   * `// TODO optimise to reuse chunk ref and avoid duplicate block pos checks`
   * `// TODO capture face`
   * `// TODO skip if slab is not visible to the player`
 * [world/src/viewer.rs](world/src/viewer.rs) (9)
   * `assert!(size > 0); // TODO Result`
   * `chunk_range: (initial_chunk, initial_chunk), // TODO is this ok?`
   * `// TODO do mesh generation on a worker thread? or just do this bit in a parallel iter`
   * `// TODO slice-aware chunk mesh caching, moving around shouldn't regen meshes constantly`
   * `// TODO limit to loaded slab bounds if camera is not discovering`
   * `// TODO only request slabs that are newly visible`
   * `// TODO which direction to stretch view range in? automatically determine or player input?`
   * `// TODO submit only the new chunks in range`
   * `// FIXME tepmorary`
 * [world/src/world.rs](world/src/world.rs) (8)
   * `// TODO optimize path with raytracing (#50)`
   * `// TODO only calculate path for each area as needed (#51)`
   * `let src_area = self.area(exiting_block).ok().expect("bad src"); // TODO`
   * `// TODO logging`
   * `// TODO benchmark filter_blocks_in_range, then optimize slab and slice lookups`
   * `// TODO filter_blocks_in_range should pass chunk+slab reference to predicate`
   * `// TODO build area graph in loader`
   * `// TODO make stresser use generated terrain again`
