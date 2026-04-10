struct SunShadowUniform {
    light_view_projection: mat4x4<f32>,
    sun_direction_shadow_strength: vec4<f32>,
    sun_color_intensity: vec4<f32>,
    sun_screen_position_radius: vec4<f32>,
    shadow_params: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> sun_shadow: SunShadowUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc_position: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0)
    );

    var output: VertexOutput;
    let clip = positions[vertex_index];
    output.clip_position = vec4<f32>(clip, 0.0, 1.0);
    output.ndc_position = clip;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sun_center = sun_shadow.sun_screen_position_radius.xy;
    let sun_radius = sun_shadow.sun_screen_position_radius.z;
    let halo_radius = max(sun_shadow.sun_screen_position_radius.w, sun_radius + 0.001);
    let distance_to_sun = distance(input.ndc_position, sun_center);
    let disc = 1.0 - smoothstep(sun_radius * 0.78, sun_radius, distance_to_sun);
    let halo = 1.0 - smoothstep(sun_radius, halo_radius, distance_to_sun);
    let glow = max(halo * 0.55, disc);
    let sun_color =
        normalize(max(sun_shadow.sun_color_intensity.xyz, vec3<f32>(0.001, 0.001, 0.001))) *
        1.15;
    let alpha = clamp(glow * (0.18 + disc * 0.62), 0.0, 0.92);
    return vec4<f32>(sun_color, alpha);
}
