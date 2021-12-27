#version 330 core

layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 tex_coords;
layout (location = 2) in vec4 color;

out vec2 v_tex_coords;
out vec4 v_color;

void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    v_tex_coords = tex_coords;
    v_color = color;
}
