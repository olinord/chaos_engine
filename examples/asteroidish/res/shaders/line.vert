#version 450

layout(push_constant) uniform fragmentPushConstants {
    vec2 position;
} u_pushConstants;

layout(location = 0) in vec2 vs_in_position;

layout(location = 0) out vec3 vs_out_color;


void main() {
    gl_Position = vec4(vs_in_position + u_pushConstants.position, 0.0, 1.0);
    vs_out_color =  vec3(u_pushConstants.position, 0.5);
}