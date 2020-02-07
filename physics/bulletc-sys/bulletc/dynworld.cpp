#include "dynworld.hpp"
#include "bulletc.hpp"
#include "common.hpp"
#include "debug.hpp"


struct dynworld *dynworld_create(float gravity) {
    return new(std::nothrow) dynworld(gravity);
}

void dynworld_destroy(struct dynworld *world) {
    delete world;
}

void dynworld_step(struct dynworld *world, float elapsed_seconds, float fixed_rate) {
    world->dynamicsWorld->stepSimulation(elapsed_seconds, 1, fixed_rate);
}

void dynworld_step_render_only(struct dynworld *world, float elapsed_seconds) {
    world->dynamicsWorld->stepSimulation(elapsed_seconds);
}

void dynworld_set_debug_drawer(struct dynworld *world, fn_draw_line draw_line) {
    // delete existing if necessary
    delete world->debugRenderer;

    if (draw_line != nullptr) {
        DebugRenderer *renderer = new DebugRenderer(draw_line);
        renderer->setDebugMode(btIDebugDraw::DBG_DrawWireframe);
        world->debugRenderer = renderer;
        world->dynamicsWorld->setDebugDrawer(renderer);
    }
}

void dynworld_debug_draw(struct dynworld *world, void *frame_blob) {
    world->debugRenderer->setFrameBlob(frame_blob);
    world->dynamicsWorld->debugDrawWorld();
    world->debugRenderer->setFrameBlob(nullptr);
}


slab_collider *
slab_collider_update(dynworld *world, slab_collider *prev, const float slab_pos[3], const float *vertices,
                     size_t vertices_count, const uint32_t *indices, size_t indices_count) {

    // copy vertices and indices
    float *vertices_copy = new float[vertices_count * 3];
    uint32_t *indices_copy = new uint32_t[indices_count];
    std::copy(vertices, vertices + (vertices_count * 3), vertices_copy);
    std::copy(indices, indices + indices_count, indices_copy);

    // clean up previous if necessary and allocate new
    // TODO reuse heap allocation?
    if (prev != nullptr) {
        world->dynamicsWorld->removeRigidBody(prev->slab_body);
    }
    delete prev;

    slab_collider *collider = new slab_collider();
    collider->vertices = vertices_copy;
    collider->indices = indices_copy;

    // create new mesh
    btIndexedMesh mesh;
    mesh.m_numTriangles = (int) indices_count / 3;
    mesh.m_triangleIndexBase = (const unsigned char *) indices_copy;
    mesh.m_triangleIndexStride = sizeof(uint32_t) * 3;
    mesh.m_numVertices = vertices_count;
    mesh.m_vertexBase = (const unsigned char *) vertices_copy;
    mesh.m_vertexStride = sizeof(float) * 3;
    mesh.m_indexType = PHY_ScalarType::PHY_INTEGER;
    mesh.m_vertexType = PHY_ScalarType::PHY_FLOAT;
    collider->mesh = new btTriangleIndexVertexArray();
    collider->mesh->addIndexedMesh(mesh);

    // create rigid body
    collider->shape = new btBvhTriangleMeshShape(collider->mesh, true);

    btRigidBody::btRigidBodyConstructionInfo desc(0.0, nullptr, collider->shape);
    desc.m_startWorldTransform.setOrigin(btVector3(slab_pos[0], slab_pos[1], slab_pos[2] - 0.5));
    desc.m_friction = 0.5;
    collider->slab_body = new btRigidBody(desc);

    // add to world
    world->dynamicsWorld->addRigidBody(collider->slab_body, COL_WORLD, COLMASK_WORLD);
    collider->slab_body->setUserPointer(reinterpret_cast<void *>(500));

    return collider;
}

dynworld::dynworld(float gravity) {
    collisionConfiguration = new btDefaultCollisionConfiguration();
    dispatcher = new btCollisionDispatcher(collisionConfiguration);
    overlappingPairCache = new btDbvtBroadphase();
    solver = new btSequentialImpulseConstraintSolver;
    dynamicsWorld = new btDiscreteDynamicsWorld(dispatcher, overlappingPairCache, solver, collisionConfiguration);
    debugRenderer = nullptr;

    // gravity
    dynamicsWorld->setGravity(btVector3(0, 0, gravity));

    dynamicsWorld->getPairCache()->setInternalGhostPairCallback(ghostCallback = new btGhostPairCallback());
}
