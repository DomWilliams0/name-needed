#include <btBulletDynamicsCommon.h>
#include <BulletCollision/CollisionDispatch/btGhostObject.h>
#include <iterator>
#include "bulletc.hpp"
#include "common.hpp"
#include "dynworld.hpp"

struct jump_sensor_callback : btCollisionWorld::ContactResultCallback {
    btScalar addSingleResult(btManifoldPoint &cp, const btCollisionObjectWrapper *colObj0Wrap, int partId0, int index0,
                             const btCollisionObjectWrapper *colObj1Wrap, int partId1, int index1) override {
        // if we got here it means the sensor (obj0) is colliding with the world (obj1)
        void *sensor = colObj0Wrap->getCollisionObject()->getUserPointer();
        unsigned int *occluded = static_cast<unsigned int *>(sensor);

        // TODO use contact points to detect what kind of block this is
        //          - half step?
        //          - is the gap too narrow to fit through?

        // set persistently
        *occluded |= 1u;

        return 0; // ??
    }

};

struct entity_jump_sensor {
    unsigned int occluded;
    btGhostObject body;
    btBoxShape shape;

    explicit entity_jump_sensor(const btVector3 &half_dims) : occluded(false), shape(half_dims /* temporary */) {
        btVector3 sensor_dims = half_dims;
        sensor_dims[SIDE_DIM] *= 0.8;
        sensor_dims[FWD_DIM] *= 0.8;
        sensor_dims[UP_DIM] *= 0.25;

        shape = btBoxShape(sensor_dims);
        body.setCollisionShape(&shape);
        body.setCollisionFlags(body.getCollisionFlags()
                               | btCollisionObject::CF_KINEMATIC_OBJECT
                               | btCollisionObject::CF_NO_CONTACT_RESPONSE);

        // user pointer is the jump status of the sensor: if occluded, it will be OR'd with 1
        body.setUserPointer(&occluded);
    }

    /// keep sensor in front of body
    void update_jump_sensor_transform(const btVector3 &half_dims, const btTransform &body_transform) {
        btTransform transform = btTransform::getIdentity();

        btVector3 &translation = transform.getOrigin();
        translation += FWD * half_dims[FWD_DIM] * (2.0 - shape.getHalfExtentsWithoutMargin()[FWD_DIM]);
        translation += UP * half_dims[UP_DIM] * -0.5;

        body.setWorldTransform(body_transform * transform);
    }

    /// sensor should already be in place
    bool poll(dynworld *world) {
        static jump_sensor_callback contact_callback;

        auto len = body.getNumOverlappingObjects();
        auto pairs = body.getOverlappingPairs();

        for (int i = 0; i < len; ++i) {
            btCollisionObject *other = pairs.at(i);
            // TODO check the other is the world
            world->dynamicsWorld->contactPairTest(&body, other, contact_callback);
        }

        bool ret = occluded == 1;

        // reset for next time
        occluded = 0;

        return ret;
    }
};


struct entity_collider {
    btRigidBody body;
    btBoxShape *body_shape;
    btVector3 half_dims;

    entity_jump_sensor *jump_sensor;

    entity_collider(btBoxShape *body_shape, const btRigidBody::btRigidBodyConstructionInfo &body_info,
                    const btVector3 &half_dims, bool add_jump_sensor) : body(
            body_info), body_shape(body_shape), half_dims(half_dims), jump_sensor(nullptr) {

        // only rotate around up axis
        body.setAngularFactor(UP);

        if (add_jump_sensor) {
            jump_sensor = new entity_jump_sensor(half_dims);
            update_jump_sensor_transform();
        }
    }

    /// keep sensor in front of body
    /// only call if `has_jump_sensor() == true`
    void update_jump_sensor_transform() {
                btAssert(has_jump_sensor());
        jump_sensor->update_jump_sensor_transform(half_dims, body.getWorldTransform());
    }

    bool has_jump_sensor() const { return jump_sensor != nullptr; }

    /// sensor should already be in place from previous call to `update_jump_sensor_transform`
    bool poll_jump_sensor(dynworld *world) {
                btAssert(has_jump_sensor());
        return jump_sensor->poll(world);
    }

    virtual ~entity_collider() {
        delete body_shape;
        delete jump_sensor;
    }
};


struct entity_collider *
entity_collider_create(struct dynworld *world, const float center[3], const float half_extents[3],
                       float friction, float linear_damping, bool jump_sensor) {
    btVector3 dims(half_extents[0], half_extents[1], half_extents[2]);
    btVector3 pos(center[0], center[1], center[2]);

    btBoxShape *shape = new btBoxShape(dims);
    btRigidBody::btRigidBodyConstructionInfo desc(2.0, nullptr, shape);
    desc.m_friction = friction;
    desc.m_linearDamping = linear_damping;
    desc.m_startWorldTransform.setOrigin(pos);

    entity_collider *collider = new entity_collider(shape, desc, dims, jump_sensor);

    // add to world
    world->dynamicsWorld->addRigidBody(&collider->body, COL_ENTITIES, COLMASK_ENTITY);
    collider->body.setUserPointer(reinterpret_cast<void *>(600));

    if (jump_sensor) {
        world->dynamicsWorld->addCollisionObject(&collider->jump_sensor->body,
                                                 COL_ENTITY_JUMP_SENSOR, COLMASK_ENTITY_JUMP_SENSOR);
    }

    return collider;
}

int entity_collider_get(struct dynworld *world, struct entity_collider *collider, float pos[3], float rot[2],
                        bool *jump_sensor_occluded) {
    int ret = -1;

    if (collider != nullptr) {
        const btTransform &transform = collider->body.getInterpolationWorldTransform();
        const btVector3 &position = transform.getOrigin();
        const btVector3 rotation = quatRotate(transform.getRotation(), FWD);

        pos[0] = position.x();
        pos[1] = position.y();
        pos[2] = position.z();

        rot[0] = rotation.x();
        rot[1] = rotation.y();

        if (collider->has_jump_sensor()) {
            *jump_sensor_occluded = collider->poll_jump_sensor(world);
        }

        ret = 0;
    }

    return ret;
}

int entity_collider_get_pos(struct entity_collider *collider, float pos[3]) {
    int ret = -1;

    if (collider != nullptr) {
        const btTransform &transform = collider->body.getInterpolationWorldTransform();
        const btVector3 &position = transform.getOrigin();

        pos[0] = position.x();
        pos[1] = position.y();
        pos[2] = position.z();

        ret = 0;
    }

    return ret;
}


int
entity_collider_set(struct entity_collider *collider, const float pos[3], float rot, const float vel[3],
                    float jump_force) {
    int ret = -1;

    if (collider != nullptr) {
        btTransform transform = collider->body.getWorldTransform();

        btVector3 new_pos(pos[0], pos[1], pos[2]);
        transform.setOrigin(new_pos);

        btQuaternion new_rot(UP, rot);
        transform.setRotation(new_rot);

        collider->body.setWorldTransform(transform);
        if (collider->has_jump_sensor()) {
            collider->update_jump_sensor_transform();
        }

        btVector3 new_vel(vel[0], vel[1], vel[2]);
        // TODO jump only if touching the ground
        new_vel[UP_DIM] += jump_force;
        collider->body.applyCentralForce(new_vel);

        ret = 0;
    }

    return ret;
}
