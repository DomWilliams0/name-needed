#ifndef BULLETC_DEBUG_H
#define BULLETC_DEBUG_H

#include <LinearMath/btVector3.h>
#include <LinearMath/btIDebugDraw.h>
#include "bulletc.hpp"

class DebugRenderer : public btIDebugDraw {
    void drawLine(const btVector3 &from, const btVector3 &to, const btVector3 &color) override;

public:
    explicit DebugRenderer(fn_draw_line draw_line) : frame_blob(nullptr), draw_line(draw_line), mode(0) {}

    void setDebugMode(int debugMode) override;

    int getDebugMode() const override;

    void setFrameBlob(void *blob);

    // --- not implemented

    void drawContactPoint(const btVector3 &PointOnB, const btVector3 &normalOnB, btScalar distance, int lifeTime,
                          const btVector3 &color) override;

    void reportErrorWarning(const char *warningString) override;

    void draw3dText(const btVector3 &location, const char *textString) override;


private:
    void *frame_blob;
    fn_draw_line draw_line;
    int mode;
};

#endif
