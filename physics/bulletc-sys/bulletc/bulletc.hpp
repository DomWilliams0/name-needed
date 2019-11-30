#ifndef BULLETC_HPP
#define BULLETC_HPP

#include <cstddef>
#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

struct dynworld;
struct dynworld *dynworld_create(float gravity);
void dynworld_destroy(struct dynworld *world);
void dynworld_step(struct dynworld *world, float elapsed_seconds, float fixed_rate);
void dynworld_step_render_only(struct dynworld *world, float elapsed_seconds);

typedef void (*fn_draw_line)(void *blob, const float *from, const float *to, const float *color);
void dynworld_set_debug_drawer(struct dynworld *world, fn_draw_line draw_line);
/// frame_blob will be passed back to rust draw_* functions as first argument
void dynworld_debug_draw(struct dynworld *world, void *frame_blob);

struct slab_collider;
/// if prev is not null, it is deleted from the world and freed first
slab_collider *
slab_collider_update(dynworld *world, slab_collider *prev, const float slab_pos[3], const float *vertices,
                     size_t vertices_count, const uint32_t *indices, size_t indices_count);

struct entity_collider;
struct entity_collider *entity_collider_create(struct dynworld *world, const float center[3], const float half_extents[3]);

/// returns 0 on success
int entity_collider_get(struct entity_collider *collider, float pos[3], float rot[3]);

/// returns 0 on success
int entity_collider_get_pos(struct entity_collider *collider, float pos[3]);

/// returns 0 on success
int entity_collider_set(struct entity_collider *collider, const float pos[3], const float rot[3], const float vel[3]);

/// hello world example from bullet
void hello_world_example();

#ifdef __cplusplus
}
#endif

#endif
