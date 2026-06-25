#version 450

layout (set = 0, binding = 0) uniform MVP 
{
	mat3 projection;
	mat3 view;
} mvp;

layout(push_constant) uniform PushConsts {
	mat3 model;
} pushConsts;

layout(location = 0) in vec2 position;
layout(location = 1) in vec4 color;

layout(location = 0) out vec4 frag_color;

void main() {

    vec3 pos = mvp.projection * mvp.view * pushConsts.model * vec3(position, 0);

    gl_Position = vec4(pos, 1.0);
    frag_color = color;
}
