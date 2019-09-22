#version 330 core

// geometry attribs
in vec3 v_pos;

// instance attribs
in vec3 e_pos;
in vec3 e_color;

out vec3 rgb;

uniform mat4 proj;
uniform mat4 view;

// TODO because we can't manually set the number of instances of entities to draw, hackily skip uninitialized instances for now
uniform int instance_count;

void main() {
    if (gl_InstanceID >= instance_count) {
        // skip :(
        gl_Position = vec4(-1.0, -1.0, -1.0, 1.0);
    } else {
        // normal
        gl_Position = proj * view * vec4(v_pos + e_pos, 1.0);
        rgb = e_color;
    }
}
