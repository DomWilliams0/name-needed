#ifndef BULLETC_COMMON_HPP
#define BULLETC_COMMON_HPP

// z axis
const btVector3 UP(0.0, 0.0, 1.0);
const size_t UP_DIM = 2;

// y axis
const btVector3 FWD(0.0, 1.0, 0.0);
const size_t FWD_DIM = 1;

// x axis
const size_t SIDE_DIM = 0;

// collision groups
constexpr uint32_t COL_NONE               = 0;
constexpr uint32_t COL_WORLD              = 1u << 10u;
constexpr uint32_t COL_ENTITIES           = 1u << 11u;
constexpr uint32_t COL_ENTITY_JUMP_SENSOR = 1u << 12u;

// what each collides with
const uint32_t COLMASK_WORLD              = COL_ENTITIES | COL_ENTITY_JUMP_SENSOR;
const uint32_t COLMASK_ENTITY             = COL_WORLD;
const uint32_t COLMASK_ENTITY_JUMP_SENSOR = COL_WORLD;


#endif
