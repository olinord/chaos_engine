#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 vs_in_position;
layout(location = 1) in vec3 vs_in_color;

layout(location = 0) out vec3 vs_out_color;

void main() {
    gl_Position = vec4(vs_in_position, 0.0, 1.0);
    vs_out_color = vs_in_color;
}