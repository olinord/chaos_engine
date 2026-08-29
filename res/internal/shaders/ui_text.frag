#version 460

// UI text fragment shader.
//
// Samples the R8 glyph atlas stored as a packed SSBO (4 bytes per uint,
// little-endian). The sampled value is the glyph alpha; final colour is
// the per-glyph text colour tinted by that alpha.

layout(location = 0) in      vec2  v_uv;
layout(location = 1) in flat vec4  v_color;
layout(location = 2) in flat vec4  v_clip_rect;
layout(location = 3) in      vec2  v_world_pos;
layout(location = 4) in flat uvec2 v_atlas_size;

layout(set = 0, binding = 2) readonly buffer AtlasData {
    uint data[];
} atlas;

layout(location = 0) out vec4 f_color;

float sample_atlas(vec2 uv) {
    uint x = uint(clamp(uv.x * float(v_atlas_size.x), 0.0, float(v_atlas_size.x) - 1.0));
    uint y = uint(clamp(uv.y * float(v_atlas_size.y), 0.0, float(v_atlas_size.y) - 1.0));
    uint idx  = y * v_atlas_size.x + x;
    uint word = atlas.data[idx / 4u];
    uint shift = (idx % 4u) * 8u;
    return float((word >> shift) & 0xFFu) / 255.0;
}

void main() {
    if (v_world_pos.x < v_clip_rect.x || v_world_pos.x > v_clip_rect.z ||
        v_world_pos.y < v_clip_rect.y || v_world_pos.y > v_clip_rect.w) {
        discard;
    }

    float alpha = sample_atlas(v_uv);
    if (alpha < 0.01) {
        discard;
    }

    f_color = vec4(v_color.rgb, v_color.a * alpha);
}
