#version 460

// UI text vertex shader.
//
// Each instance represents one glyph quad. The per-vertex `corner` attribute
// carries a unit-quad corner in [0,1]^2. Glyph position, size, UV range,
// colour, and clip rect are read from a per-instance storage buffer.

layout(location = 0) in vec2 corner;

struct GlyphInstance {
    vec4 pos_size;   // x, y, width, height in logical pixels
    vec4 uv;         // uv_min.x, uv_min.y, uv_max.x, uv_max.y
    vec4 color;
    vec4 clip_rect;  // xmin, ymin, xmax, ymax
};

layout(set = 0, binding = 0) uniform TextProjection {
    mat4 view_proj;
    uint atlas_width;
    uint atlas_height;
    uint _pad0;
    uint _pad1;
} proj;

layout(set = 0, binding = 1) readonly buffer Glyphs {
    GlyphInstance data[];
} glyphs;

layout(location = 0) out vec2  v_uv;
layout(location = 1) out flat vec4  v_color;
layout(location = 2) out flat vec4  v_clip_rect;
layout(location = 3) out      vec2  v_world_pos;
layout(location = 4) out flat uvec2 v_atlas_size;

void main() {
    GlyphInstance g = glyphs.data[gl_InstanceIndex];
    vec2 pos  = g.pos_size.xy;
    vec2 size = g.pos_size.zw;
    vec2 world = pos + corner * size;

    gl_Position = proj.view_proj * vec4(world, 0.0, 1.0);

    v_uv        = g.uv.xy + corner * (g.uv.zw - g.uv.xy);
    v_color     = g.color;
    v_clip_rect = g.clip_rect;
    v_world_pos = world;
    v_atlas_size = uvec2(proj.atlas_width, proj.atlas_height);
}
