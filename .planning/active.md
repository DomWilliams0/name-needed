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
