#include <LinearMath/btIDebugDraw.h>
#include "debug.hpp"


void DebugRenderer::drawLine(const btVector3 &from, const btVector3 &to, const btVector3 &color) {
    const float *from_ptr = static_cast<const float *>(from.m_floats);
    const float *to_ptr = static_cast<const float *>(to.m_floats);
    const float *color_ptr = static_cast<const float *>(color.m_floats);

    // calls back into rust
    this->draw_line(this->frame_blob, from_ptr, to_ptr, color_ptr);
}

void DebugRenderer::setFrameBlob(void *blob) {
    this->frame_blob = blob;
}

void
DebugRenderer::drawContactPoint(const btVector3 &PointOnB, const btVector3 &normalOnB, btScalar distance, int lifeTime,
                                const btVector3 &color) {

}

void DebugRenderer::reportErrorWarning(const char *warningString) {

}

void DebugRenderer::draw3dText(const btVector3 &location, const char *textString) {

}

void DebugRenderer::setDebugMode(int debugMode) {
    this->mode = debugMode;
}

int DebugRenderer::getDebugMode() const {
    return this->mode;
}


