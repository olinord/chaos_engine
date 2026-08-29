#version 460

layout(location = 0) in vec2 v_local;
layout(location = 1) in flat vec2 v_size;
layout(location = 2) in flat vec4 v_bg;
layout(location = 3) in flat vec4 v_border_color;
layout(location = 4) in flat float v_border_width;
layout(location = 5) in flat float v_corner_radius;
layout(location = 6) in flat vec4 v_clip_rect;
layout(location = 7) in vec2 v_world_pos;

layout(location = 0) out vec4 f_color;

// Signed distance to a rounded rectangle centred on the origin, with
// half-extents `half_size` and corner radius `r`.
float sd_rounded_rect(vec2 p, vec2 half_size, float r) {
    vec2 d = abs(p) - half_size + vec2(r);
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

void main() {
    if (v_world_pos.x < v_clip_rect.x || v_world_pos.x > v_clip_rect.z ||
        v_world_pos.y < v_clip_rect.y || v_world_pos.y > v_clip_rect.w) {
        discard;
    }

    vec2 half_size = v_size * 0.5;
    vec2 p = v_local - half_size;
    float r = clamp(v_corner_radius, 0.0, min(half_size.x, half_size.y));

    float dist = sd_rounded_rect(p, half_size, r);
    float aa = max(fwidth(dist), 1e-4);

    if (dist > aa) {
        discard;
    }

    float outer_alpha = clamp(0.5 - dist / aa, 0.0, 1.0);

    vec4 base;
    if (v_border_width > 0.0) {
        float inner_dist = dist + v_border_width;
        float bg_frac = clamp(0.5 - inner_dist / aa, 0.0, 1.0);
        base = mix(v_border_color, v_bg, bg_frac);
    } else {
        base = v_bg;
    }

    base.a *= outer_alpha;
    f_color = base;
}
