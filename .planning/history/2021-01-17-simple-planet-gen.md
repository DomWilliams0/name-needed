# Active tasks

* slab generation from planet
	* [X] create regions
	* [X] initialize chunks with default block density
	* [~] position simple large scale features (e.g. forest)
	* [~] generate sub features (e.g. tree placement)
	* [~] rasterize sub features (e.g. tree blocks)
	* [X] rasterize slab and return to game
* [X] fix unevenness of chunk descriptor heightmap ranges at chunk borders
* [X] add async to planet chunk initialization
* [X] refactor terrainsource usage to avoid so much boxing
* [X] refactor the entirety of world loading and modification to process slabs rather than full chunks
* [X] dynamic chunk loading
	* [X] load slabs as camera pans
	* [X] load slabs as camera moves up and down
	* [~] config opt to disable discovery by camera, only by society entities
	* [X] all-air placeholder slabs should not be marked as fully loaded
	* [X] bug: all air placeholders clobber existing terrain!
	* [~] restrict camera at world edge?
* [X] return full slab from procgen
* [X] block modification should be localised to the slabs
* [X] mark slabs as dirty instead of chunks in viewer
* terrain should be generated at the slab level
	* [X] pass slab range along with chunk pos to load
	* [X] load result should only hold slabs, chunk container should be lightweight
* [X] replace threadpool with async
	* required to allow blocking of slab tasks
	* simplfies tests, dont need to use crappy blocking impl
* [X] split up loader/mod.rs into a few separate modules
* [~] investigate flashing shadows when a lot of terrain updates happen
	* probably because occlusion changes are queued for next tick
* [X] world or camera changes between restarts on world with same seed
* [X] fix random entity placement on planet
