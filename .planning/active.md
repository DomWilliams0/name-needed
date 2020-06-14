# Active tasks

* [X] generated terrain source
* [X] generic "seed rng from Option<u64>"
* [X] plain perlin noise within small z range
* [X] output an image for easier debugging
* [X] bug: nothing renders until world viewer first moves
* [X] render slice boundaries better to represent whats above/below
* [X] adjust chunk and viewer range size
	* [X] viewer range should be a config option
	* [X] add bounds and current range to debug ui
	* [X] use modifier key to stretch it up/down
	* [X] use modifier key to jump view range by a bigger amount
* [X] increase area graph index size, its exceeded by 30+ chunk radius
* [X] add some better block types
	* [-] ~vary the colours when relevant e.g. stone and grass~
* [-] ~true max viewer range should be detected through air slices rather than slab count~
* [X] restarting the game should reuse the same window if possible
* [X] world viewer default pos with seed 342878 is terrible
* [X] multichunkwonder viewer default pos is bad
* [X] fix critical bug where blocks are outside of the default slab
* [X] debug renderers have disappeared
