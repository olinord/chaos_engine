
#version 460

layout(location = 0) in vec2 position;

layout (set = 0, binding = 0) uniform Camera 
{
	mat4 projection;
	mat4 view;
} camera;

layout(set = 0, binding = 1) readonly buffer Positions {
    mat4 world[];
} locations;

void main() {
    mat4 model = locations.world[gl_BaseInstance];

    vec4 world = model * vec4(position, 0.0, 1.0);
    gl_Position = camera.projection * camera.view * world;
}