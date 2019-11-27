#include <btBulletDynamicsCommon.h>
#include <iterator>
#include "bulletc.hpp"
#include "debug.hpp"

//#ifdef __cplusplus
//extern "C" {
//#endif


#pragma clang diagnostic push
#pragma ide diagnostic ignored "OCUnusedGlobalDeclarationInspection"
struct dynworld {
    explicit dynworld(float gravity) {
        collisionConfiguration = new btDefaultCollisionConfiguration();
        dispatcher = new btCollisionDispatcher(collisionConfiguration);
        overlappingPairCache = new btDbvtBroadphase();
        solver = new btSequentialImpulseConstraintSolver;
        dynamicsWorld = new btDiscreteDynamicsWorld(dispatcher, overlappingPairCache, solver, collisionConfiguration);
        debugRenderer = nullptr;

        // gravity
        dynamicsWorld->setGravity(btVector3(0, 0, gravity));
    }

    virtual ~dynworld() {
        delete dynamicsWorld;
        delete solver;
        delete overlappingPairCache;
        delete dispatcher;
        delete collisionConfiguration;
        delete debugRenderer;
    }


    btDefaultCollisionConfiguration *collisionConfiguration;
    btCollisionDispatcher *dispatcher;
    btBroadphaseInterface *overlappingPairCache;
    btSequentialImpulseConstraintSolver *solver;
    btDiscreteDynamicsWorld *dynamicsWorld;

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

struct entity_collider {
    btRigidBody *body;

    explicit entity_collider(btRigidBody *body) : body(body) {}

    virtual ~entity_collider() {
        delete body;
    }
};

#define COL_WORLD (1u << 0u)
#define COL_ENTITIES (1u << 1u)

// -------

struct dynworld *dynworld_create(float gravity) {
    return new(std::nothrow) dynworld(gravity);
}

void dynworld_destroy(struct dynworld *world) {
    delete world;
}

void dynworld_step(struct dynworld *world, float elapsed_seconds, float fixed_rate) {
    world->dynamicsWorld->stepSimulation(elapsed_seconds, 2, fixed_rate);
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
    mesh.m_triangleIndexBase = (const unsigned char *)indices_copy;
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
    desc.m_startWorldTransform.setOrigin(btVector3(slab_pos[0], slab_pos[1], slab_pos[2]-0.5));
    collider->slab_body = new btRigidBody(desc);

    // add to world
    world->dynamicsWorld->addRigidBody(collider->slab_body, COL_WORLD, COL_ENTITIES);

    return collider;
}

struct entity_collider *entity_collider_create(struct dynworld *world, const float center[3], const float half_extents[3]) {
    btVector3 dims(half_extents[0], half_extents[1], half_extents[2]);
    btVector3 pos(center[0], center[1], center[2]);

    btBoxShape *shape = new btBoxShape(dims); // TODO add to struct
    btRigidBody::btRigidBodyConstructionInfo desc(2.0, nullptr, shape);
    desc.m_friction = 0.2;
    desc.m_linearDamping = 0.8;
    desc.m_startWorldTransform.setOrigin(pos);
    btRigidBody *rb = new btRigidBody(desc);

    // only rotate in z axis
    rb->setAngularFactor(btVector3(0.0, 0.0, 1.0));

    // add to world
    world->dynamicsWorld->addRigidBody(rb, COL_ENTITIES, COL_WORLD);

    return new entity_collider(rb);
}

int entity_collider_get(struct entity_collider *collider, float pos[3], float rot[3]) {
    int ret = -1;

    if (collider != nullptr && collider->body != nullptr) {
        const btTransform &transform = collider->body->getInterpolationWorldTransform();
        const btVector3 &position = transform.getOrigin();

        btVector3 fwd(0.0, 1.0, 0.0); // y = forwards
        btVector3 rotation = fwd * transform.getBasis();

        pos[0] = position.x();
        pos[1] = position.y();
        pos[2] = position.z();

        rot[0] = rotation.x();
        rot[1] = rotation.y();
        rot[2] = rotation.z();
        ret = 0;
    }

    return ret;
}

int entity_collider_set(struct entity_collider *collider, const float pos[3], const float rot[3], const float vel[3]) {
    int ret = -1;

    if (collider != nullptr && collider->body != nullptr) {
        btTransform transform = collider->body->getCenterOfMassTransform();

        btVector3 new_pos(pos[0], pos[1], pos[2]);
        transform.setOrigin(new_pos);

        // TODO global constant if this actually works
        // TODO rotation AFTER debug rendering of velocity and movement target
        btVector3 new_rot_vec(rot[0], rot[1], rot[2]);
        btVector3 fwd(0.0, 1.0, 0.0); // y = forwards
        btScalar angle = fwd.angle(new_rot_vec);
        btQuaternion new_rot(btVector3(0.0, 0.0, 1.0), angle);
        //transform.setRotation(new_rot);
        collider->body->setCenterOfMassTransform(transform);

        btVector3 new_vel(vel[0], vel[1], vel[2]);
        collider->body->applyCentralForce(new_vel);

        ret = 0;
    }

    return ret;
}

void hello_world_example() {
///-----includes_end-----

    int i;
    ///-----initialization_start-----

    ///collision configuration contains default setup for memory, collision setup. Advanced users can create their own configuration.
    btDefaultCollisionConfiguration* collisionConfiguration = new btDefaultCollisionConfiguration();

    ///use the default collision dispatcher. For parallel processing you can use a diffent dispatcher (see Extras/BulletMultiThreaded)
    btCollisionDispatcher* dispatcher = new btCollisionDispatcher(collisionConfiguration);

    ///btDbvtBroadphase is a good general purpose broadphase. You can also try out btAxis3Sweep.
    btBroadphaseInterface* overlappingPairCache = new btDbvtBroadphase();

    ///the default constraint solver. For parallel processing you can use a different solver (see Extras/BulletMultiThreaded)
    btSequentialImpulseConstraintSolver* solver = new btSequentialImpulseConstraintSolver;

    btDiscreteDynamicsWorld* dynamicsWorld = new btDiscreteDynamicsWorld(dispatcher, overlappingPairCache, solver, collisionConfiguration);

    dynamicsWorld->setGravity(btVector3(0, -10, 0));

    ///-----initialization_end-----

    //keep track of the shapes, we release memory at exit.
    //make sure to re-use collision shapes among rigid bodies whenever possible!
    btAlignedObjectArray<btCollisionShape*> collisionShapes;

    ///create a few basic rigid bodies

    //the ground is a cube of side 100 at position y = -56.
    //the sphere will hit it at y = -6, with center at -5
    {
        btCollisionShape* groundShape = new btBoxShape(btVector3(btScalar(50.), btScalar(50.), btScalar(50.)));

        collisionShapes.push_back(groundShape);

        btTransform groundTransform;
        groundTransform.setIdentity();
        groundTransform.setOrigin(btVector3(0, -56, 0));

        btScalar mass(0.);

        //rigidbody is dynamic if and only if mass is non zero, otherwise static
        bool isDynamic = (mass != 0.f);

        btVector3 localInertia(0, 0, 0);
        if (isDynamic)
            groundShape->calculateLocalInertia(mass, localInertia);

        //using motionstate is optional, it provides interpolation capabilities, and only synchronizes 'active' objects
        btDefaultMotionState* myMotionState = new btDefaultMotionState(groundTransform);
        btRigidBody::btRigidBodyConstructionInfo rbInfo(mass, myMotionState, groundShape, localInertia);
        btRigidBody* body = new btRigidBody(rbInfo);

        //add the body to the dynamics world
        dynamicsWorld->addRigidBody(body);
    }

    {
        //create a dynamic rigidbody

        //btCollisionShape* colShape = new btBoxShape(btVector3(1,1,1));
        btCollisionShape* colShape = new btSphereShape(btScalar(1.));
        collisionShapes.push_back(colShape);

        /// Create Dynamic Objects
        btTransform startTransform;
        startTransform.setIdentity();

        btScalar mass(1.f);

        //rigidbody is dynamic if and only if mass is non zero, otherwise static
        bool isDynamic = (mass != 0.f);

        btVector3 localInertia(0, 0, 0);
        if (isDynamic)
            colShape->calculateLocalInertia(mass, localInertia);

        startTransform.setOrigin(btVector3(2, 10, 0));

        //using motionstate is recommended, it provides interpolation capabilities, and only synchronizes 'active' objects
        btDefaultMotionState* myMotionState = new btDefaultMotionState(startTransform);
        btRigidBody::btRigidBodyConstructionInfo rbInfo(mass, myMotionState, colShape, localInertia);
        btRigidBody* body = new btRigidBody(rbInfo);

        dynamicsWorld->addRigidBody(body);
    }

    /// Do some simulation

    ///-----stepsimulation_start-----
    for (i = 0; i < 150; i++) {
        dynamicsWorld->stepSimulation(1.f / 60.f, 10);

        //print positions of all objects
        for (int j = dynamicsWorld->getNumCollisionObjects() - 1; j >= 0; j--) {
            btCollisionObject* obj = dynamicsWorld->getCollisionObjectArray()[j];
            btRigidBody* body = btRigidBody::upcast(obj);
            btTransform trans;
            if (body && body->getMotionState()) {
                body->getMotionState()->getWorldTransform(trans);
            } else {
                trans = obj->getWorldTransform();
            }
            // printf("world pos object %d = %f,%f,%f\n", j, float(trans.getOrigin().getX()), float(trans.getOrigin().getY()), float(trans.getOrigin().getZ()));
        }
    }

    ///-----stepsimulation_end-----

    //cleanup in the reverse order of creation/initialization

    ///-----cleanup_start-----

    //remove the rigidbodies from the dynamics world and delete them
    for (i = dynamicsWorld->getNumCollisionObjects() - 1; i >= 0; i--) {
        btCollisionObject* obj = dynamicsWorld->getCollisionObjectArray()[i];
        btRigidBody* body = btRigidBody::upcast(obj);
        if (body && body->getMotionState()) {
            delete body->getMotionState();
        }
        dynamicsWorld->removeCollisionObject(obj);
        delete obj;
    }

    //delete collision shapes
    for (int j = 0; j < collisionShapes.size(); j++) {
        btCollisionShape* shape = collisionShapes[j];
        collisionShapes[j] = 0;
        delete shape;
    }

    //delete dynamics world
    delete dynamicsWorld;

    //delete solver
    delete solver;

    //delete broadphase
    delete overlappingPairCache;

    //delete dispatcher
    delete dispatcher;

    delete collisionConfiguration;

    //next line is optional: it will be cleared by the destructor when the array goes out of scope
    collisionShapes.clear();
}

//#ifdef __cplusplus
//}
//#endif


#pragma clang diagnostic pop