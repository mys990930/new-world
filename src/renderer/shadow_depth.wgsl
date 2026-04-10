struct SunShadowUniform {
    light_view_projection: mat4x4<f32>,
    sun_direction_shadow_strength: vec4<f32>,
    sun_color_intensity: vec4<f32>,
    sun_screen_position_radius: vec4<f32>,
    shadow_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> sun_shadow: SunShadowUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) texture_layer: u32,
    @location(5) material_kind: u32,
};

@vertex
fn vs_main(input: VertexInput) -> @builtin(position) vec4<f32> {
    return sun_shadow.light_view_projection * vec4<f32>(input.position, 1.0);
}
