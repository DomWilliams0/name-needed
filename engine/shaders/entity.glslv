#version 330 core

// geometry attribs
in vec3 v_pos;

// instance attribs
in vec3 e_pos;
in vec3 e_color;
in mat4 e_model;

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
        vec4 v_pos_translated = e_model * vec4(v_pos, 1.0);
        vec4 e_pos_translated = vec4(e_pos, 1.0);
        gl_Position = proj * view * (v_pos_translated + e_pos_translated);
        rgb = e_color;
    }
}
