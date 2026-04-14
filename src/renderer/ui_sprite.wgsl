@group(0) @binding(0)
var ui_texture: texture_2d<f32>;

@group(0) @binding(1)
var ui_sampler: sampler;

struct UiVertexIn {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) tint: vec4<f32>,
};

struct UiVertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(input: UiVertexIn) -> UiVertexOut {
    var output: UiVertexOut;
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.tint = input.tint;
    return output;
}

@fragment
fn fs_main(input: UiVertexOut) -> @location(0) vec4<f32> {
    let sample = textureSample(ui_texture, ui_sampler, input.uv);
    return sample * input.tint;
}
