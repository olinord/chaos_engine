#version 450

layout (set = 0, binding = 0) uniform MVP 
{
	mat4 projection;
	mat4 view;
} mvp;

layout(push_constant) uniform PushConsts {
	mat4 model;
} pushConsts;

layout(location = 0) in vec2 position;
layout(location = 1) in vec4 color;
 
layout(location = 0) out vec4 frag_color;

void main() {

	gl_Position = mvp.projection * mvp.view * pushConsts.model * vec4(position.x, position.y, 0, 1);
    frag_color = color;
}
