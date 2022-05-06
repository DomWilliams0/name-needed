# Active tasks

* [X] add sheep entity
* [X] species component
* [X] herding component
*  herdable entities should start/join a herd with nearby members of same species
    * [X] herd allocation
    * [X] debug renderer to show herds
    * [X] leave herd if alone
    * [X] add cow species to ensure they form their own herds
* herd formation
    * [X] dont leave immediately after leaving the radius, but rather slowly decay over a few ticks
    * [X] if parts of a herd become disconnected, split into 2
    * [X] the herd that wins during merging should be the biggest, not just the first one found
* [X] wander behaviour should stay near herd
    * use herd avg position and avg position of nearby members too
    * [X] wander target should be found locally instead of searching globally
* [ ] skip ahead a few nodes when path finding if visible
* [ ] startling of sheep based on senses
* [ ] propagation of startling through herd
    * could propagate the original startle source, or just be startled at the startlement of another
* [ ] fleeing from startle
    * use another kind of navigation that doesn't use path finding? navigate locally but just away.
        or rather search outward for a flee destination instead of choosing top-down
    * could use this for wandering too
* [X] use type name for debug renderer idenfier
* [ ] add chained modify_x|y|z helper to worldpoint
* [X] frame allocator helpers for debug/display/vec
* [ ] reuse some allocaions in herd joining system
* simple fauna for sheep to eat
    * [O] generation of plants in procgen
        * [X] species definition based on abstract plant
            * ensure no display text on hover component is added
        * [X] random position offset
        * [ ] random rotation
    * [ ] growth/death of existing plants
    * [ ] growth of new plants from seeds/corpses/poo
    * [ ] approximate scattering of non quantitive growth like grass
    * [X] sheep have hunger and find nearby plants to eat
    * [ ] subfeatures can overlap because theyre generated in parallel, e.g. plants and tree trunks.
        prune collisions across all (sub)features during rasterization?
    * [X] pass in a source of random to subfeature rasterization
    * [X] bug: higher concentration of plants in initial chunks only
* [ ] fix config before merging to develop
* [ ] remove catchup frames between game start and waiting for initial terrain
* [X] remove hunger from ai blackboard, the input will be cached anyway
* [ ] bug: can't select treetop blocks?
* [ ] entity selection - choose better in z direction e.g. from top down
* [ ] grazing animals often get stuck between 2 nearby plants and keep switching between them

## Edible plants
* [X] food interests and flavours
* [X] plants are edible
* [X] use flavours and interests when choosing food to eat
* [X] eating of plants without needing to pick them up
    * [X] humans shouldn't consider eating these
* [ ] instead of consuming a plant entirely in one sitting, grazing animals should nibble and
    wander more
    * [X] different foods are eaten at different speeds

## Herd leader
* [X] identify leader of herd for others to follow
* [o] event for a herd member becoming the leader of its herd
    * [X] and being demoted
    * [ ] add associated herd leader AI DSEs
* [ ] dev way to kill an entity to test dead herd leader
* new sheep dses
    * [ ] herd leader specific: lead herd to a new location
    * [X] stay near herd: if too far from leader, run towards it until in range
* [ ] herd_centre() should return a transformcomponent ref instead of position, because its repeated too often
