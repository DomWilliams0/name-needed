#version 330 core

uniform sampler2D font_tex;

in vec2 f_tex_pos;
in vec4 f_color;

out vec4 out_color;

void main() {
    float alpha = texture(font_tex, f_tex_pos).r;
    out_color = f_color * vec4(1.0, 1.0, 1.0, alpha);
}
