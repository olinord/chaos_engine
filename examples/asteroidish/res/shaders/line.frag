#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 ps_in_color;
layout(location = 0) out vec4 ps_out_color;

void main() {
    ps_out_color = vec4(ps_in_color, 1.0);
}
