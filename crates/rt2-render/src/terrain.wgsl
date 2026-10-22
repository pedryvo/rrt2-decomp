struct Uniforms {
    view_proj: mat4x4<f32>,
    // Direction towards the light (xyz, normalised).
    light: vec4<f32>,
    viewport: vec2<f32>,
    // Screen pixels per native sprite pixel.
    sprite_scale: f32,
    // Ocean animation frame, 0..16.
    water_frame: u32,
    // Free-running animation counter.
    anim: u32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var ground_tex: texture_2d_array<f32>;
@group(1) @binding(1) var ocean_tex: texture_2d_array<f32>;
@group(1) @binding(2) var track_tex: texture_2d_array<f32>;
@group(1) @binding(3) var samp: sampler;
// river.imb (90 layers) followed by road.imb
@group(1) @binding(4) var extra_tex: texture_2d_array<f32>;
@group(1) @binding(5) var beach_tex: texture_2d_array<f32>;

const NO_LAYER: u32 = 0xffffffffu;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // [ground, ocean, track, quarter turns]
    @location(3) layers: vec4<u32>,
    // overlay codes: layer | turns << 12 | stack << 14, first = on top
    @location(4) overlays: vec4<u32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) layers: vec4<u32>,
    @location(3) @interpolate(flat) overlays: vec4<u32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.uv = in.uv;
    out.layers = in.layers;
    out.overlays = in.overlays;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Rotate texture coordinates by `turns` quarter turns about the tile centre.
fn rotate_uv(uv: vec2<f32>, turns: u32) -> vec2<f32> {
    switch turns & 3u {
        case 1u: { return vec2<f32>(1.0 - uv.y, uv.x); }
        case 2u: { return vec2<f32>(1.0 - uv.x, 1.0 - uv.y); }
        case 3u: { return vec2<f32>(uv.y, 1.0 - uv.x); }
        default: { return uv; }
    }
}

fn clamp_layer(layer: u32, count: u32) -> i32 {
    return i32(min(layer, count - 1u));
}

// Rotate a texture-space derivative by `turns` quarter turns (the linear part of rotate_uv).
fn rotate_vec(d: vec2<f32>, turns: u32) -> vec2<f32> {
    switch turns & 3u {
        case 1u: { return vec2<f32>(-d.y, d.x); }
        case 2u: { return vec2<f32>(-d.x, -d.y); }
        case 3u: { return vec2<f32>(d.y, -d.x); }
        default: { return d; }
    }
}

// Premultiplied colour of one overlay code (transparent black when the slot is unused). Uses
// explicit gradients so it can sit behind a branch.
fn overlay_sample(code: u32, uv: vec2<f32>, ddx: vec2<f32>, ddy: vec2<f32>) -> vec4<f32> {
    if code == 0xffffffffu {
        return vec4<f32>(0.0);
    }
    let layer = code & 0xfffu;
    let turns = (code >> 12u) & 3u;
    let stack = code >> 14u;
    let ruv = rotate_uv(uv, turns);
    let rx = rotate_vec(ddx, turns);
    let ry = rotate_vec(ddy, turns);
    if stack == 0u {
        return textureSampleGrad(ground_tex, samp, ruv, clamp_layer(layer, textureNumLayers(ground_tex)), rx, ry);
    }
    if stack == 1u {
        // River: 5 frames of 18 pieces.
        return textureSampleGrad(extra_tex, samp, ruv, clamp_layer(layer + (u.anim % 5u) * 18u, textureNumLayers(extra_tex)), rx, ry);
    }
    if stack == 2u {
        return textureSampleGrad(extra_tex, samp, ruv, clamp_layer(90u + layer, textureNumLayers(extra_tex)), rx, ry);
    }
    // Shore: 32 frames of 8 pieces.
    return textureSampleGrad(beach_tex, samp, ruv, clamp_layer(layer + (u.anim & 31u) * 8u, textureNumLayers(beach_tex)), rx, ry);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Sampling must happen in uniform control flow, so everything is sampled and then selected.
    let g_layers = textureNumLayers(ground_tex);
    let o_layers = textureNumLayers(ocean_tex);
    let t_layers = textureNumLayers(track_tex);
    let ground = textureSample(ground_tex, samp, rotate_uv(in.uv, in.layers.w), clamp_layer(in.layers.x, g_layers));
    let ocean = textureSample(ocean_tex, samp, in.uv, clamp_layer(in.layers.y + u.water_frame * 4u, o_layers));
    let track = textureSample(track_tex, samp, in.uv, clamp_layer(in.layers.z, t_layers));

    // Premultiplied, gamma-encoded colours composited like the original: water, ground, track.
    var rgb = vec3<f32>(0.05, 0.1, 0.2);
    if in.layers.y != NO_LAYER {
        rgb = ocean.rgb;
    }
    if in.layers.x != NO_LAYER {
        rgb = ground.rgb + (1.0 - ground.a) * rgb;
    }
    // Overlays go under each other in list order: the first one is on top.
    let ddx = dpdx(in.uv);
    let ddy = dpdy(in.uv);
    for (var k = 3; k >= 0; k = k - 1) {
        let o = overlay_sample(in.overlays[k], in.uv, ddx, ddy);
        rgb = o.rgb + (1.0 - o.a) * rgb;
    }
    if in.layers.z != NO_LAYER {
        rgb = track.rgb + (1.0 - track.a) * rgb;
    }

    // Soft relief shading: flat ground is unchanged, slopes facing the light get brighter.
    let d = dot(normalize(in.normal), u.light.xyz) - u.light.y;
    let shade = clamp(1.0 + 0.9 * d, 0.55, 1.35);
    return vec4<f32>(srgb_to_linear(clamp(rgb * shade, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}
