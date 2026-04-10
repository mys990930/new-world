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
const MATERIAL_GRASS: u32 = 1u;
const MATERIAL_SOIL: u32 = 2u;
const MATERIAL_STONE: u32 = 3u;
const MATERIAL_SAND: u32 = 4u;
const MATERIAL_FOLIAGE: u32 = 5u;
const MATERIAL_WATER: u32 = 6u;
const MATERIAL_EMISSIVE: u32 = 7u;

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

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn tint_strength(material_kind: u32) -> f32 {
    switch material_kind {
        case MATERIAL_GRASS, MATERIAL_FOLIAGE: {
            return 0.28;
        }
        case MATERIAL_SOIL, MATERIAL_SAND: {
            return 0.18;
        }
        case MATERIAL_STONE: {
            return 0.12;
        }
        default: {
            return 0.16;
        }
    }
}

fn texture_contrast(material_kind: u32) -> f32 {
    switch material_kind {
        case MATERIAL_STONE: {
            return 1.18;
        }
        case MATERIAL_SOIL, MATERIAL_SAND: {
            return 1.12;
        }
        default: {
            return 1.08;
        }
    }
}

fn fog_resistance(material_kind: u32) -> f32 {
    switch material_kind {
        case MATERIAL_STONE: {
            return 0.20;
        }
        case MATERIAL_EMISSIVE: {
            return 0.35;
        }
        default: {
            return 0.0;
        }
    }
}

fn material_color_response(material_kind: u32, color: vec3<f32>) -> vec3<f32> {
    switch material_kind {
        case MATERIAL_GRASS: {
            return color * vec3<f32>(0.96, 1.04, 0.95);
        }
        case MATERIAL_SOIL: {
            return color * vec3<f32>(1.03, 0.97, 0.90);
        }
        case MATERIAL_STONE: {
            let gray = vec3<f32>(luminance(color));
            return mix(color, gray * vec3<f32>(0.95, 0.97, 1.02), 0.18);
        }
        case MATERIAL_SAND: {
            return color * vec3<f32>(1.06, 1.01, 0.90);
        }
        case MATERIAL_FOLIAGE: {
            return color * vec3<f32>(0.92, 1.05, 0.92);
        }
        case MATERIAL_WATER: {
            return mix(color, environment.sky_color_overcast.xyz, 0.24);
        }
        case MATERIAL_EMISSIVE: {
            return color * 1.08;
        }
        default: {
            return color;
        }
    }
}

fn resolve_albedo(sampled: vec3<f32>, tint: vec3<f32>, material_kind: u32) -> vec3<f32> {
    let tint_mix = tint_strength(material_kind);
    let tinted = sampled * mix(vec3<f32>(1.0, 1.0, 1.0), tint, tint_mix);
    let midpoint = vec3<f32>(0.5, 0.5, 0.5);
    let contrasted =
        clamp((tinted - midpoint) * texture_contrast(material_kind) + midpoint, vec3<f32>(0.0), vec3<f32>(1.0));
    return material_color_response(material_kind, contrasted);
}

fn apply_climate_and_weather(color: vec3<f32>, material_kind: u32) -> vec3<f32> {
    var result = color;

    if environment.quality_flags.z != 0u {
        let climate_mix = clamp(0.06 + environment.weather_climate_params.y * 0.12, 0.0, 0.22);
        result = mix(result, result * environment.climate_tint_weather.xyz, climate_mix);
    }

    if environment.quality_flags.w != 0u {
        let weather_mix =
            environment.climate_tint_weather.w * (0.06 + environment.weather_climate_params.x * 0.14);
        let moisture_tint = mix(
            vec3<f32>(1.0, 1.0, 1.0),
            environment.fog_color_density.xyz,
            0.08
        );
        let dampened_mix = weather_mix * (1.0 - fog_resistance(material_kind) * 0.5);
        result = mix(result, result * moisture_tint, clamp(dampened_mix, 0.0, 0.20));
    }

    return result;
}

fn apply_color_grade(color: vec3<f32>, world_position: vec3<f32>, material_kind: u32) -> vec3<f32> {
    if environment.quality_flags.y == 0u {
        return color;
    }

    let height_mix = clamp(world_position.y * 0.04 + 0.42, 0.0, 1.0);
    let warm_grade = mix(
        environment.horizon_color_height_falloff.xyz,
        environment.sky_color_overcast.xyz,
        height_mix
    );
    let gray = vec3<f32>(luminance(color));
    let saturated = mix(gray, color, 1.0 + environment.readability.w * 0.55);
    let grade_strength = 0.07 * (1.0 - fog_resistance(material_kind) * 0.4);
    return mix(saturated, saturated * warm_grade, grade_strength);
}

fn apply_fog(color: vec3<f32>, world_position: vec3<f32>, material_kind: u32) -> vec3<f32> {
    if environment.quality_flags.x == 0u {
        return color;
    }

    let distance_to_eye = distance(world_position, camera.eye_position.xyz);
    let height_term =
        exp(-max(world_position.y, 0.0) * environment.horizon_color_height_falloff.w);
    let fog_amount =
        1.0 -
        exp(-distance_to_eye * environment.fog_color_density.w * (0.42 + height_term * 0.24));
    let resisted = fog_amount * (1.0 - fog_resistance(material_kind));
    return mix(color, environment.fog_color_density.xyz, clamp(resisted, 0.0, 0.72));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(
        block_textures,
        block_sampler,
        input.uv,
        i32(input.texture_layer)
    );
    let alpha = sampled.a * input.color.a;
    if alpha <= 0.001 {
        discard;
    }

    let normal = normalize(input.normal);
    let to_light = sun_direction();
    let to_eye = normalize(camera.eye_position.xyz - input.world_position);
    let lambert = max(dot(normal, to_light), 0.0);
    let up_factor = max(normal.y, 0.0);
    let side_factor = 1.0 - up_factor;
    let albedo = resolve_albedo(sampled.rgb, input.color.rgb, input.material_kind);
    let ambient =
        environment.ambient_color_intensity.rgb * environment.ambient_color_intensity.w;
    let sunlight =
        environment.sun_color_intensity.rgb * environment.sun_color_intensity.w * lambert;
    let top_boost = 1.0 + up_factor * environment.readability.x * 0.82;
    let side_shadow = 1.0 - side_factor * environment.readability.y * 0.26;
    let warm_side_tint = mix(
        vec3<f32>(1.0, 1.0, 1.0),
        environment.horizon_color_height_falloff.xyz,
        side_factor * 0.10
    );
    let silhouette = pow(1.0 - max(dot(normal, to_eye), 0.0), 2.0);
    let rim =
        environment.horizon_color_height_falloff.xyz *
        silhouette *
        environment.readability.z *
        0.10;
    let low_light_detail = 0.10 * (1.0 - lambert);

    var shaded = albedo * (ambient + sunlight);
    shaded = shaded * top_boost * side_shadow * warm_side_tint + rim + albedo * low_light_detail;
    shaded = apply_climate_and_weather(shaded, input.material_kind);
    shaded = apply_color_grade(shaded, input.world_position, input.material_kind);
    shaded = apply_fog(shaded, input.world_position, input.material_kind);

    return vec4<f32>(clamp(shaded, vec3<f32>(0.0), vec3<f32>(1.0)), alpha);
}
