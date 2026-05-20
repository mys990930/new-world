struct SunShadowUniform {
    light_view_projection: mat4x4<f32>,
    sun_direction_shadow_strength: vec4<f32>,
    sun_color_intensity: vec4<f32>,
    sun_screen_position_radius: vec4<f32>,
    shadow_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> sun_shadow: SunShadowUniform;
@group(1) @binding(0)
var block_textures: texture_2d_array<f32>;
@group(1) @binding(1)
var block_sampler: sampler;

const MATERIAL_FOLIAGE: u32 = 5u;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) texture_layer: u32,
    @location(5) material_kind: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) texture_layer: u32,
    @location(2) @interpolate(flat) material_kind: u32,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = sun_shadow.light_view_projection * vec4<f32>(input.position, 1.0);
    output.uv = input.uv;
    output.texture_layer = input.texture_layer;
    output.material_kind = input.material_kind;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) {
    if input.material_kind != MATERIAL_FOLIAGE {
        return;
    }

    let sampled = textureSample(
        block_textures,
        block_sampler,
        input.uv,
        i32(input.texture_layer)
    );
    if sampled.a * input.color.a < 0.42 {
        discard;
    }
}
