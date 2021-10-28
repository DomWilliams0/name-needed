#version 330 core

in vec3 v_pos;
in vec4 v_color;
out vec4 rgb;

uniform mat4 proj_view;

void main() {
    gl_Position = proj_view * vec4(v_pos, 1.0);
    rgb = v_color;
}
