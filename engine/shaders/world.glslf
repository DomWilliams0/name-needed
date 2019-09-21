#version 330 core

in vec3 rgb;
out vec4 f_rgba;

void main() {
    f_rgba = vec4(rgb, 1.0);
}
