# Active tasks

* fix scaling
	* [X] each voxel = 1 world unit i.e. WorldPoint, WorldPosition
	* [X] 3 or 4 voxels = 1 metre i.e. ViewPoint. used for physics/collisions (later) and rendering
* [X] new unit type for metres (viewpoint)
* [ ] new unit type for distances between worldpoints, not just float
* [X] render entities at the proper scale for their PhysicalComponent size
* [X] enforce no NaN or infinite in worldpoint and viewpoint
* [X] replace asserts with returning Options in units
