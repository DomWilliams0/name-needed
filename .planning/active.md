# Active tasks

* [ ] return full slab from procgen
* [ ] block modification should be localised to the slabs
* [ ] dont mark chunk as dirty, but chunk+slab range
* terrain should be generated at the slab level
	* [ ] pass slab range along with chunk pos to load
	* [ ] load result should only hold slabs, chunk container should be lightweight
* actual generation
	* [ ] position trees with poisson disk sampling
	* [ ] derive a seed per slab using world seed + coords
* [ ] replace threadpool with async
	* required to allow blocking of slab tasks
	* simplfies tests, dont need to use crappy blocking impl
* [ ] add test for single slab navigability > load slab below > is navigation as expected between the 2
* [ ] dont require doublesizedvec to have no holes
	* the impl can have no holes and store options instead or something
* [ ] split up loader/mod.rs into a few separate modules
