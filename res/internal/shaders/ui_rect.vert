#version 460

// UI rect vertex shader.
//
// Instance data is indexed via gl_InstanceIndex out of a storage buffer.
// The per-vertex `corner` attribute carries a unit-quad corner in [0,1]^2
// (provided by the CPU as 6 corners = 2 triangles).

layout(location = 0) in vec2 corner;

struct RectInstance {
    vec4 rect;         // pos.xy, size.xy
    vec4 bg;
    vec4 border_color;
    vec4 clip_rect;    // xmin, ymin, xmax, ymax
    vec4 params;       // border_width, corner_radius, unused, unused
};

layout(set = 0, binding = 0) uniform Projection {
    mat4 view_proj;
} proj;

layout(set = 0, binding = 1) readonly buffer Instances {
    RectInstance data[];
} instances;

layout(location = 0) out vec2 v_local;
layout(location = 1) out flat vec2 v_size;
layout(location = 2) out flat vec4 v_bg;
layout(location = 3) out flat vec4 v_border_color;
layout(location = 4) out flat float v_border_width;
layout(location = 5) out flat float v_corner_radius;
layout(location = 6) out flat vec4 v_clip_rect;
layout(location = 7) out vec2 v_world_pos;

void main() {
    RectInstance inst = instances.data[gl_InstanceIndex];
    vec2 pos = inst.rect.xy;
    vec2 size = inst.rect.zw;
    vec2 world = pos + corner * size;

    gl_Position = proj.view_proj * vec4(world, 0.0, 1.0);

    v_local = corner * size;
    v_size = size;
    v_bg = inst.bg;
    v_border_color = inst.border_color;
    v_border_width = inst.params.x;
    v_corner_radius = inst.params.y;
    v_clip_rect = inst.clip_rect;
    v_world_pos = world;
}
