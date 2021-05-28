# Active tasks

* fix scaling
	* [ ] each voxel = 1 world unit i.e. WorldPoint, WorldPosition
	* [ ] 3 or 4 voxels = 1 metre i.e. ViewPoint. used for physics/collisions (later) and rendering
* [ ] new unit type for metres/rename ViewPoint
* [ ] new unit type for distances between worldpoints, not just float
* [ ] render entities at the proper scale for their PhysicalComponent size
* [X] enforce no NaN or infinite in worldpoint and viewpoint
* [X] replace asserts with returning Options in units
