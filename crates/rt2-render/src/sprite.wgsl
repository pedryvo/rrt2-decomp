struct Uniforms {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,
    viewport: vec2<f32>,
    // Screen pixels per native sprite pixel.
    sprite_scale: f32,
    water_frame: u32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

// How much the atlas was enlarged at load (`atlas.rs` BAKE_SCALE).
const BAKE: f32 = 2.0;
// Pull sprites this far towards the camera (world units along the view ray) so a building does not
// sink into the ground in front of its anchor.
const DEPTH_BIAS: f32 = 1.5;
const DEPTH_RANGE: f32 = 5999.0;

struct Inst {
    @location(0) pos: vec3<f32>,
    @location(1) shade: f32,
    // x, y, w, h at native scale relative to the anchor, y down
    @location(2) rect: vec4<f32>,
    @location(3) uv: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) shade: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Inst) -> VsOut {
    // Triangle strip: (0,0) (1,0) (0,1) (1,1)
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let c = u.view_proj * vec4<f32>(inst.pos, 1.0);
    // Snap the anchor to the pixel grid so sprites do not shimmer or blur when panning.
    let ndc = c.xy / c.w;
    let anchor_px = round((ndc * 0.5 + vec2<f32>(0.5)) * u.viewport);
    var out: VsOut;
    let sx_px = anchor_px.x + (inst.rect.x + corner.x * inst.rect.z) * u.sprite_scale;
    // Sprite y grows downwards, the clip space y upwards.
    let sy_px = anchor_px.y - (inst.rect.y + corner.y * inst.rect.w) * u.sprite_scale;
    out.clip = vec4<f32>(
        sx_px / u.viewport.x * 2.0 - 1.0,
        sy_px / u.viewport.y * 2.0 - 1.0,
        c.z / c.w - DEPTH_BIAS / DEPTH_RANGE,
        1.0,
    );
    out.uv = mix(inst.uv.xy, inst.uv.zw, corner);
    out.shade = inst.shade;
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(atlas, 0));
    let texel = in.uv * size;
    // Screen pixels per baked texel.
    let s = u.sprite_scale / BAKE;
    // Mip level from the screen-space footprint (uniform control flow).
    let footprint = max(length(dpdx(texel)), length(dpdy(texel)));
    let lod = max(log2(max(footprint, 1e-4)), 0.0);

    // Sharp bilinear: keep texels crisp and make the transition between two texels exactly one
    // screen pixel wide, at any scale above 1:1. Below that plain trilinear minification.
    var coord = in.uv;
    if s > 1.0 {
        let t = texel - vec2<f32>(0.5);
        let i = floor(t);
        let f = clamp((t - i - vec2<f32>(0.5)) * s + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
        coord = (i + f + vec2<f32>(0.5)) / size;
    }
    let c = textureSampleLevel(atlas, samp, coord, lod);
    if c.a < 0.004 {
        discard;
    }
    // Premultiplied, gamma encoded -> premultiplied linear.
    let straight = c.rgb / c.a;
    let lin = srgb_to_linear(clamp(straight * in.shade, vec3<f32>(0.0), vec3<f32>(1.0)));
    return vec4<f32>(lin * c.a, c.a);
}
