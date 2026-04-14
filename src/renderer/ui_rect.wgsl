struct UiVertexIn {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct UiVertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(input: UiVertexIn) -> UiVertexOut {
    var output: UiVertexOut;
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: UiVertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
