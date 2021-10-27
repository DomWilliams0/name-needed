#version 330 core

// shared attribs
layout(location = 0) in vec3 v_pos;

// instance attribs
layout(location = 1) in vec4 e_color;
layout(location = 2) in mat4 e_model;

out vec4 rgb;

uniform mat4 proj;
uniform mat4 view;

void main() {
    vec4 v_pos_translated = e_model * vec4(v_pos, 1.0);
    gl_Position = proj * view * v_pos_translated;
    rgb = e_color;
}