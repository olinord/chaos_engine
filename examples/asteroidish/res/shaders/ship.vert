
#version 460

layout (set = 0, binding = 0) uniform Camera 
{
	mat4 projection;
	mat4 view;
} camera;

layout(set = 0, binding = 1) uniform World {
	mat4 model;
} world;

layout(location = 0) in vec2 position;

void main() {
	gl_Position = camera.projection * camera.view * world.model * vec4(position, 0, 1);
}