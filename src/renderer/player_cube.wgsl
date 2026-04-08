struct CameraUniform {
    view_projection: mat4x4<f32>,
};

struct LightUniform {
    direction_to_light: vec4<f32>,
    color: vec4<f32>,
    ambient: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;
@group(1) @binding(0)
var<uniform> light: LightUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.normal = normalize(input.normal);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let direction_to_light = normalize(light.direction_to_light.xyz);
    let lambert = max(dot(normalize(input.normal), direction_to_light), 0.0);
    let lighting = light.ambient.rgb + (light.color.rgb * lambert);
    let shaded_color = input.color.rgb * clamp(lighting, vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(shaded_color, input.color.a);
}
