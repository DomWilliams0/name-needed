#version 330 core

layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 tex;
layout (location = 2) in vec4 color;

uniform mat4 transform;

out vec2 f_tex_pos;
out vec4 f_color;

void main() {
    gl_Position = transform * vec4(pos, 0.0, 1.0);
    f_tex_pos = tex;
    f_color = color;
}
