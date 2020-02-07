#ifndef BULLETC_DYNWORLD_HPP
#define BULLETC_DYNWORLD_HPP

#include <btBulletDynamicsCommon.h>
#include <BulletCollision/CollisionDispatch/btGhostObject.h>
#include "debug.hpp"

struct dynworld {
    explicit dynworld(float gravity);

    virtual ~dynworld() {
        delete dynamicsWorld;
        delete solver;
        delete overlappingPairCache;
        delete dispatcher;
        delete collisionConfiguration;
        delete ghostCallback;
        delete debugRenderer;
    }


    btDefaultCollisionConfiguration *collisionConfiguration;
    btCollisionDispatcher *dispatcher;
    btBroadphaseInterface *overlappingPairCache;
    btSequentialImpulseConstraintSolver *solver;
    btDiscreteDynamicsWorld *dynamicsWorld;
    btGhostPairCallback *ghostCallback;

    DebugRenderer *debugRenderer;
};

struct slab_collider {
    btRigidBody *slab_body = nullptr;
    btTriangleIndexVertexArray *mesh = nullptr;
    btBvhTriangleMeshShape *shape = nullptr;
    float *vertices = nullptr;
    uint32_t *indices = nullptr;

    virtual ~slab_collider() {
        if (indices != nullptr) delete[](indices);
        if (vertices != nullptr) delete[](vertices);
        delete shape;
        delete mesh;
        delete slab_body;
    }
};

#endif
