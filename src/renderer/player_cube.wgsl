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

const MATERIAL_GENERIC_OPAQUE: u32 = 0u;
const MATERIAL_ACTOR: u32 = 8u;
const MATERIAL_SHADOW: u32 = 9u;
const MATERIAL_HIGHLIGHT: u32 = 10u;

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
    @location(5) material_kind: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) texture_layer: u32,
    @location(4) @interpolate(flat) material_kind: u32,
    @location(5) world_position: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = camera.view_projection * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.normal = normalize(input.normal);
    output.uv = input.uv;
    output.texture_layer = input.texture_layer;
    output.material_kind = input.material_kind;
    output.world_position = input.position;
    return output;
}

fn sun_direction() -> vec3<f32> {
    return normalize(environment.sun_direction_time.xyz);
}

fn apply_color_grade(color: vec3<f32>, world_position: vec3<f32>, material_kind: u32) -> vec3<f32> {
    if environment.quality_flags.y == 0u || material_kind == MATERIAL_SHADOW {
        return color;
    }

    let height_mix = clamp(world_position.y * 0.05 + 0.45, 0.0, 1.0);
    let warm_grade = mix(
        environment.horizon_color_height_falloff.xyz,
        environment.sky_color_overcast.xyz,
        height_mix
    );
    let luminance = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
    let saturated = mix(vec3<f32>(luminance), color, 1.0 + environment.readability.w * 0.35);
    let strength = select(0.05, 0.08, material_kind == MATERIAL_HIGHLIGHT);
    return mix(saturated, saturated * warm_grade, strength);
}

fn apply_fog(color: vec3<f32>, world_position: vec3<f32>, material_kind: u32) -> vec3<f32> {
    if environment.quality_flags.x == 0u || material_kind == MATERIAL_HIGHLIGHT {
        return color;
    }

    let distance_to_eye = distance(world_position, camera.eye_position.xyz);
    let height_term = exp(-max(world_position.y, 0.0) * environment.horizon_color_height_falloff.w);
    let fog_amount = 1.0 - exp(-distance_to_eye * environment.fog_color_density.w * (0.50 + height_term * 0.20));
    let resisted = select(fog_amount, fog_amount * 0.45, material_kind == MATERIAL_ACTOR);
    return mix(color, environment.fog_color_density.xyz, clamp(resisted, 0.0, 0.55));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(
        block_textures,
        block_sampler,
        input.uv,
        i32(input.texture_layer)
    );
    let base_color = sampled.rgb * input.color.rgb;
    let alpha = sampled.a * input.color.a;
    if alpha <= 0.001 {
        discard;
    }

    let normal = normalize(input.normal);
    let to_light = sun_direction();
    let to_eye = normalize(camera.eye_position.xyz - input.world_position);
    let lambert = max(dot(normal, to_light), 0.0);
    let ambient =
        environment.ambient_color_intensity.rgb * environment.ambient_color_intensity.w;
    let sunlight =
        environment.sun_color_intensity.rgb * environment.sun_color_intensity.w * lambert;
    let silhouette = pow(1.0 - max(dot(normal, to_eye), 0.0), 2.0);
    let rim =
        environment.horizon_color_height_falloff.xyz *
        silhouette *
        environment.readability.z *
        0.10;

    var shaded = base_color * (ambient + sunlight) + rim;

    if input.material_kind == MATERIAL_ACTOR {
        let top_boost = 1.0 + max(normal.y, 0.0) * environment.readability.x * 0.32;
        shaded = shaded * top_boost;
    } else if input.material_kind == MATERIAL_SHADOW {
        shaded =
            base_color *
            (environment.ambient_color_intensity.rgb * 0.32 + vec3<f32>(0.02, 0.02, 0.03));
    } else if input.material_kind == MATERIAL_HIGHLIGHT {
        let highlight_glow = environment.horizon_color_height_falloff.xyz * 0.42;
        shaded = max(base_color * 0.92 + highlight_glow, base_color);
    } else if input.material_kind == MATERIAL_GENERIC_OPAQUE {
        shaded = base_color * (ambient + sunlight);
    }

    shaded = apply_color_grade(shaded, input.world_position, input.material_kind);
    shaded = apply_fog(shaded, input.world_position, input.material_kind);

    return vec4<f32>(clamp(shaded, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}
