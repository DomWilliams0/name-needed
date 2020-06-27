# Active tasks

* [X] add durability to blocks
* [X] add "break block" activity
* [X] divine intervention DSE: do whatever dev mode demands
	* [X] go here
	* [X] break this block
* [X] bug: move world down, then scroll over - other chunks are still rendering the higher chunks
* [~] ~move regen_dirty_chunks to render() for quicker rtt of updates~
* [X] adjacent search goal doesnt work with follow path
* [X] bug: VertexOcclusion::combine only works for *new* terrain, not updating
	* need to differentiate between "not occluded because the other chunk doesnt exist" and "not occluded because the block in the other chunk is transparent
* [X] bug: panic on zero-len path with 5 entities and seed 67853852415419
* [X] bug: chunk corner AO not updated properly on terrain change
* [X] test to randomly change blocks in a world to ensure to panics due to attempted mutability on non-exclusive cow slabs 
