struct Uniforms {
    view_proj: mat4x4<f32>,
    // Direction towards the light (xyz, normalised).
    light: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip = u.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let diffuse = max(dot(normalize(in.normal), u.light.xyz), 0.0);
    let shade = 0.35 + 0.85 * diffuse;
    return vec4<f32>(in.color * shade, 1.0);
}
