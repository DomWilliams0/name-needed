# Active tasks

* [X] add sheep entity
* [X] species component
* [o] herding component
*  herdable entities should start/join a herd with nearby members of same species
    * [X] herd allocation
    * [X] debug renderer to show herds
    * [X] leave herd if alone
    * [ ] add cow species to ensure they form their own herds
* [ ] startling of sheep based on senses
* [ ] propagation of startling through herd
    * could propagate the original startle source, or just be startled at the startlement of another
* [ ] fleeing from startle
    * use another kind of navigation that doesn't use path finding? navigate locally but just away.
        or rather search outward for a flee destination instead of choosing top-down
    * could use this for wandering too
* [ ] use type name for debug renderer idenfier
* [ ] add chained modify_x|y|z helper to worldpoint
