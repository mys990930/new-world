struct CameraUniform {
    view_projection: mat4x4<f32>,
    eye_position: vec4<f32>,
};

struct EnvironmentUniform {
    sun_direction_time: vec4<f32>,
    sun_color_intensity: vec4<f32>,
    ambient_color_intensity: vec4<f32>,
    fog_color_density: vec4<f32>,
    horizon_color_height_falloff: vec4<f32>,
    sky_color_overcast: vec4<f32>,
    climate_tint_weather: vec4<f32>,
    weather_climate_params: vec4<f32>,
    readability: vec4<f32>,
    quality_flags: vec4<u32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;
@group(1) @binding(0)
var<uniform> environment: EnvironmentUniform;
@group(2) @binding(0)
var block_textures: texture_2d_array<f32>;
@group(2) @binding(1)
var block_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) texture_layer: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) texture_layer: u32,
    @location(4) world_position: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.normal = normalize(input.normal);
    output.uv = input.uv;
    output.texture_layer = input.texture_layer;
    output.world_position = input.position;
    return output;
}

fn sun_direction() -> vec3<f32> {
    return normalize(environment.sun_direction_time.xyz);
}

fn apply_climate_and_weather(color: vec3<f32>) -> vec3<f32> {
    var result = color;

    if environment.quality_flags.z != 0u {
        let climate_mix = clamp(0.10 + environment.weather_climate_params.y * 0.18, 0.0, 0.3);
        result = mix(
            result,
            result * environment.climate_tint_weather.xyz,
            climate_mix
        );
    }

    if environment.quality_flags.w != 0u {
        let weather_mix =
            environment.climate_tint_weather.w * (0.12 + environment.weather_climate_params.x * 0.18);
        let moisture_tint = mix(
            vec3<f32>(1.0, 1.0, 1.0),
            environment.fog_color_density.xyz,
            0.12
        );
        result = mix(result, result * moisture_tint, clamp(weather_mix, 0.0, 0.3));
    }

    return result;
}

fn apply_color_grade(color: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    if environment.quality_flags.y == 0u {
        return color;
    }

    let height_mix = clamp(world_position.y * 0.05 + 0.4, 0.0, 1.0);
    let warm_grade = mix(
        environment.horizon_color_height_falloff.xyz,
        environment.sky_color_overcast.xyz,
        height_mix
    );
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = mix(vec3<f32>(luminance), color, 1.0 + environment.readability.w);
    let overcast_cool = mix(
        vec3<f32>(1.0, 1.0, 1.0),
        environment.sky_color_overcast.xyz,
        environment.sky_color_overcast.w * 0.1
    );
    return mix(saturated, saturated * warm_grade * overcast_cool, 0.12);
}

fn apply_fog(color: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    if environment.quality_flags.x == 0u {
        return color;
    }

    let distance_to_eye = distance(world_position, camera.eye_position.xyz);
    let height_term = exp(-max(world_position.y, 0.0) * environment.horizon_color_height_falloff.w);
    let fog_amount = 1.0 - exp(-distance_to_eye * environment.fog_color_density.w * (0.65 + height_term * 0.35));
    return mix(color, environment.fog_color_density.xyz, clamp(fog_amount, 0.0, 1.0));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(
        block_textures,
        block_sampler,
        input.uv,
        i32(input.texture_layer)
    );
    let base = vec4<f32>(sampled.rgb * input.color.rgb, sampled.a * input.color.a);
    if base.a <= 0.001 {
        discard;
    }

    let normal = normalize(input.normal);
    let to_light = sun_direction();
    let to_eye = normalize(camera.eye_position.xyz - input.world_position);
    let lambert = max(dot(normal, to_light), 0.0);
    let up_factor = max(normal.y, 0.0);
    let side_factor = 1.0 - up_factor;
    let ambient =
        environment.ambient_color_intensity.rgb * environment.ambient_color_intensity.w;
    let sunlight =
        environment.sun_color_intensity.rgb * environment.sun_color_intensity.w * lambert;
    let top_boost = 1.0 + up_factor * environment.readability.x;
    let side_shadow = 1.0 - side_factor * environment.readability.y * 0.45;
    let warm_side_tint = mix(
        vec3<f32>(1.0, 1.0, 1.0),
        environment.horizon_color_height_falloff.xyz,
        side_factor * 0.16
    );
    let silhouette = pow(1.0 - max(dot(normal, to_eye), 0.0), 2.0);
    let rim =
        environment.horizon_color_height_falloff.xyz *
        silhouette *
        environment.readability.z *
        0.16;

    var shaded = base.rgb * (ambient + sunlight);
    shaded = shaded * top_boost * side_shadow * warm_side_tint + rim;
    shaded = apply_climate_and_weather(shaded);
    shaded = apply_color_grade(shaded, input.world_position);
    shaded = apply_fog(shaded, input.world_position);

    return vec4<f32>(clamp(shaded, vec3<f32>(0.0), vec3<f32>(1.0)), base.a);
}
