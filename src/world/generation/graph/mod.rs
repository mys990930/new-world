use delaunator::{EMPTY, Point, Triangulation, triangulate};
use rayon::prelude::*;
use std::collections::HashMap;

pub const DEFAULT_GRAPH_REGION_SIZE_BLOCKS: i32 = 1024;
pub const DEFAULT_SITE_SPACING_BLOCKS: i32 = 192;
pub const DEFAULT_GRAPH_PADDING_REGIONS: u32 = 1;
pub const DEFAULT_BASE_FIELD_SMOOTHING_PASSES: u32 = 2;
pub const DEFAULT_BASE_FIELD_SELF_WEIGHT: f32 = 0.55;

const SITE_JITTER_FRACTION: f64 = 0.43;
pub const MIN_NEAREST_SITE_SPACING_FRACTION: f32 = 0.14;
const TOPOLOGY_SITE_GUARD_CELLS: i64 = 3;
const HASH_SITE: u64 = 0x8f53_7a29_381d_55f7;
const HASH_SITE_FIELD: u64 = 0xa1b9_f4d2_0c73_17e5;
const HASH_CONTINENTALITY_FIELD: u64 = 0x92d3_4f11_6b9a_c807;
const HASH_ELEVATION_FIELD: u64 = 0x5e6f_18b2_a1c7_49d3;
const HASH_TEMPERATURE_FIELD: u64 = 0xb047_a3d9_2871_f6c5;
const HASH_HYDRATION_FIELD: u64 = 0x70c9_f51a_30de_4417;
const HASH_EDGE: u64 = 0x4d2c_6f01_9ab8_e327;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPlanePoint {
    pub x: f32,
    pub z: f32,
}

impl WorldPlanePoint {
    pub const fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphRegionCoord {
    pub x: i32,
    pub z: i32,
}

impl GraphRegionCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphRegionArea {
    pub min: GraphRegionCoord,
    pub max: GraphRegionCoord,
}

impl GraphRegionArea {
    pub fn new(min: GraphRegionCoord, max: GraphRegionCoord) -> Option<Self> {
        (min.x <= max.x && min.z <= max.z).then_some(Self { min, max })
    }

    pub fn contains(self, coord: GraphRegionCoord) -> bool {
        coord.x >= self.min.x
            && coord.x <= self.max.x
            && coord.z >= self.min.z
            && coord.z <= self.max.z
    }

    pub fn region_count(self) -> u32 {
        let width = (self.max.x - self.min.x + 1) as u32;
        let height = (self.max.z - self.min.z + 1) as u32;
        width.saturating_mul(height)
    }

    pub fn expanded(self, padding_regions: u32) -> Option<Self> {
        let padding = i32::try_from(padding_regions).ok()?;
        let min = GraphRegionCoord {
            x: self.min.x.checked_sub(padding)?,
            z: self.min.z.checked_sub(padding)?,
        };
        let max = GraphRegionCoord {
            x: self.max.x.checked_add(padding)?,
            z: self.max.z.checked_add(padding)?,
        };

        Self::new(min, max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoronoiGraphConfig {
    pub seed: u64,
    pub generator_version: u32,
    pub region_size_blocks: i32,
    pub site_spacing_blocks: i32,
    pub padding_regions: u32,
}

impl VoronoiGraphConfig {
    pub const fn new(seed: u64, generator_version: u32) -> Self {
        Self {
            seed,
            generator_version,
            region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
            site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
            padding_regions: DEFAULT_GRAPH_PADDING_REGIONS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoronoiGraphPatchRequest {
    pub config: VoronoiGraphConfig,
    pub center_world_x: i32,
    pub center_world_z: i32,
}

impl VoronoiGraphPatchRequest {
    pub const fn new(config: VoronoiGraphConfig, center_world_x: i32, center_world_z: i32) -> Self {
        Self {
            config,
            center_world_x,
            center_world_z,
        }
    }

    pub fn center_region(self) -> GraphRegionCoord {
        graph_region_for_world_block(
            self.center_world_x,
            self.center_world_z,
            self.config.region_size_blocks,
        )
    }

    pub fn owner_area(self) -> GraphRegionArea {
        let center = self.center_region();
        GraphRegionArea::new(center, center).expect("single graph region area is valid")
    }

    pub fn padded_area(self) -> GraphRegionArea {
        self.owner_area()
            .expanded(self.config.padding_regions)
            .expect("graph patch padded area must fit i32 graph coordinates")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiSiteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiCornerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphBaseFieldConfig {
    pub smoothing_passes: u32,
    pub self_weight: f32,
}

impl Default for GraphBaseFieldConfig {
    fn default() -> Self {
        Self {
            smoothing_passes: DEFAULT_BASE_FIELD_SMOOTHING_PASSES,
            self_weight: DEFAULT_BASE_FIELD_SELF_WEIGHT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GraphBaseFields {
    pub temperature: f32,
    pub hydration: f32,
    pub continentality: f32,
    pub elevation_seed: f32,
}

impl GraphBaseFields {
    pub fn new(temperature: f32, hydration: f32, continentality: f32, elevation_seed: f32) -> Self {
        Self {
            temperature: clamp_unit(temperature),
            hydration: clamp_unit(hydration),
            continentality: clamp_signed_unit(continentality),
            elevation_seed: clamp_signed_unit(elevation_seed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoronoiSite {
    pub id: VoronoiSiteId,
    pub owner_region: GraphRegionCoord,
    pub position: WorldPlanePoint,
    pub raw_base_fields: GraphBaseFields,
    pub base_fields: GraphBaseFields,
    pub temperature: f32,
    pub hydration: f32,
    pub height_bias: f32,
    pub continentality: f32,
    pub ruggedness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoronoiCorner {
    pub id: VoronoiCornerId,
    pub position: WorldPlanePoint,
    pub raw_base_fields: GraphBaseFields,
    pub base_fields: GraphBaseFields,
    pub elevation: f32,
    pub water_accumulation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoronoiEdge {
    pub id: VoronoiEdgeId,
    pub sites: [VoronoiSiteId; 2],
    pub corners: [VoronoiCornerId; 2],
    pub boundary_curve_seed: u64,
    pub hydrology_bias: f32,
}

#[derive(Debug, Clone, Default)]
pub struct VoronoiGraphPatch {
    pub owner_regions: Vec<GraphRegionCoord>,
    pub sites: Vec<VoronoiSite>,
    pub corners: Vec<VoronoiCorner>,
    pub edges: Vec<VoronoiEdge>,
}

impl VoronoiGraphPatch {
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty() && self.corners.is_empty() && self.edges.is_empty()
    }

    pub fn site(&self, id: VoronoiSiteId) -> Option<&VoronoiSite> {
        self.sites.iter().find(|site| site.id == id)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GraphSiteSpacingStats {
    pub site_count: usize,
    pub min_nearest_distance_blocks: f32,
    pub max_nearest_distance_blocks: f32,
    pub average_nearest_distance_blocks: f32,
    pub nearest_distance_stddev_blocks: f32,
    pub nearest_distance_cv: f32,
}

pub fn generate_voronoi_graph_patch(request: VoronoiGraphPatchRequest) -> VoronoiGraphPatch {
    validate_graph_config(request.config);

    let owner_area = request.owner_area();
    let padded_area = request.padded_area();
    let owner_regions = regions_in_area(owner_area);
    let site_grid_bounds = site_grid_bounds_for_area(padded_area, request.config);

    let mut sites = site_grid_coords(site_grid_bounds)
        .into_par_iter()
        .map(|coord| generate_site(coord, request.config))
        .collect::<Vec<_>>();
    sites.sort_by_key(|site| site.id.0);

    let (corners, edges) = generate_delaunay_voronoi_dual(&sites, request.config);

    let mut patch = VoronoiGraphPatch {
        owner_regions,
        sites,
        corners,
        edges,
    };
    apply_base_graph_fields(&mut patch, GraphBaseFieldConfig::default());
    patch
}

pub fn graph_site_spacing_stats(patch: &VoronoiGraphPatch) -> GraphSiteSpacingStats {
    if patch.sites.len() < 2 {
        return GraphSiteSpacingStats {
            site_count: patch.sites.len(),
            ..GraphSiteSpacingStats::default()
        };
    }

    let nearest_distances = patch
        .sites
        .par_iter()
        .map(|site| {
            patch
                .sites
                .iter()
                .filter(|other| other.id != site.id)
                .map(|other| point_distance(site.position, other.position))
                .fold(f32::INFINITY, f32::min)
        })
        .collect::<Vec<_>>();

    let site_count = nearest_distances.len();
    let min_nearest_distance_blocks = nearest_distances
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max_nearest_distance_blocks = nearest_distances
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let average_nearest_distance_blocks = nearest_distances.iter().sum::<f32>() / site_count as f32;
    let variance = nearest_distances
        .iter()
        .map(|distance| {
            let delta = distance - average_nearest_distance_blocks;
            delta * delta
        })
        .sum::<f32>()
        / site_count as f32;
    let nearest_distance_stddev_blocks = variance.sqrt();
    let nearest_distance_cv =
        nearest_distance_stddev_blocks / average_nearest_distance_blocks.max(f32::EPSILON);

    GraphSiteSpacingStats {
        site_count,
        min_nearest_distance_blocks,
        max_nearest_distance_blocks,
        average_nearest_distance_blocks,
        nearest_distance_stddev_blocks,
        nearest_distance_cv,
    }
}

pub fn apply_base_graph_fields(patch: &mut VoronoiGraphPatch, config: GraphBaseFieldConfig) {
    validate_base_field_config(config);

    let adjacency = site_adjacency(&patch.sites, &patch.edges);
    let mut fields = patch
        .sites
        .iter()
        .map(|site| site.raw_base_fields)
        .collect::<Vec<_>>();

    for _ in 0..config.smoothing_passes {
        fields = smooth_base_field_pass(&fields, &adjacency, config.self_weight);
    }

    patch
        .sites
        .par_iter_mut()
        .zip(fields.into_par_iter())
        .for_each(|(site, fields)| {
            site.base_fields = fields;
            site.temperature = fields.temperature;
            site.hydration = fields.hydration;
            site.height_bias = fields.elevation_seed;
            site.continentality = fields.continentality;
        });

    assign_corner_base_fields(&mut patch.corners, &patch.sites, &patch.edges);
}

pub fn graph_region_for_world_block(
    world_x: i32,
    world_z: i32,
    region_size_blocks: i32,
) -> GraphRegionCoord {
    assert!(
        region_size_blocks > 0,
        "region_size_blocks must be positive"
    );
    GraphRegionCoord {
        x: world_x.div_euclid(region_size_blocks),
        z: world_z.div_euclid(region_size_blocks),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SiteGridCoord {
    x: i32,
    z: i32,
}

impl SiteGridCoord {
    const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }

    fn site_id(self) -> VoronoiSiteId {
        VoronoiSiteId(pack_grid_coord(self.x, self.z))
    }
}

#[derive(Debug, Clone, Copy)]
struct SiteGridBounds {
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
}

fn validate_graph_config(config: VoronoiGraphConfig) {
    assert!(
        config.region_size_blocks > 0,
        "region_size_blocks must be positive"
    );
    assert!(
        config.site_spacing_blocks > 0,
        "site_spacing_blocks must be positive"
    );
    assert!(
        i32::try_from(config.padding_regions).is_ok(),
        "padding_regions must fit i32 graph coordinates"
    );
}

fn validate_base_field_config(config: GraphBaseFieldConfig) {
    assert!(
        config.self_weight.is_finite(),
        "base field self_weight must be finite"
    );
    assert!(
        (0.0..=1.0).contains(&config.self_weight),
        "base field self_weight must be in 0..=1"
    );
}

fn regions_in_area(area: GraphRegionArea) -> Vec<GraphRegionCoord> {
    let mut regions = (area.min.z..=area.max.z)
        .flat_map(|z| (area.min.x..=area.max.x).map(move |x| GraphRegionCoord { x, z }))
        .collect::<Vec<_>>();
    regions.sort_by_key(|region| (region.z, region.x));
    regions
}

fn site_grid_bounds_for_area(area: GraphRegionArea, config: VoronoiGraphConfig) -> SiteGridBounds {
    let region_size = i64::from(config.region_size_blocks);
    let spacing = i64::from(config.site_spacing_blocks);
    let min_world_x = i64::from(area.min.x) * region_size;
    let min_world_z = i64::from(area.min.z) * region_size;
    let max_world_x_exclusive = (i64::from(area.max.x) + 1) * region_size;
    let max_world_z_exclusive = (i64::from(area.max.z) + 1) * region_size;
    let guard_cells = TOPOLOGY_SITE_GUARD_CELLS + i64::from(DEFAULT_BASE_FIELD_SMOOTHING_PASSES);

    SiteGridBounds {
        min_x: i32_from_i64(min_world_x.div_euclid(spacing) - guard_cells),
        max_x: i32_from_i64((max_world_x_exclusive - 1).div_euclid(spacing) + guard_cells),
        min_z: i32_from_i64(min_world_z.div_euclid(spacing) - guard_cells),
        max_z: i32_from_i64((max_world_z_exclusive - 1).div_euclid(spacing) + guard_cells),
    }
}

fn site_grid_coords(bounds: SiteGridBounds) -> Vec<SiteGridCoord> {
    (bounds.min_z..=bounds.max_z)
        .flat_map(|z| (bounds.min_x..=bounds.max_x).map(move |x| SiteGridCoord::new(x, z)))
        .collect()
}

fn generate_site(coord: SiteGridCoord, config: VoronoiGraphConfig) -> VoronoiSite {
    let spacing = f64::from(config.site_spacing_blocks);
    let jitter_radius = spacing * SITE_JITTER_FRACTION;
    let hash = graph_hash(config, HASH_SITE, coord.x, coord.z, 0);
    let jitter_x = (unit_f64(hash) * 2.0 - 1.0) * jitter_radius;
    let jitter_z = (unit_f64(splitmix64(hash ^ 0x9e37_79b9_7f4a_7c15)) * 2.0 - 1.0) * jitter_radius;
    let x = f64::from(coord.x) * spacing + spacing * 0.5 + jitter_x;
    let z = f64::from(coord.z) * spacing + spacing * 0.5 + jitter_z;
    let field_hash = graph_hash(config, HASH_SITE_FIELD, coord.x, coord.z, 0);
    let position = WorldPlanePoint::new(x as f32, z as f32);
    let raw_base_fields = raw_macro_friendly_base_fields(position, field_hash, config);

    VoronoiSite {
        id: coord.site_id(),
        owner_region: graph_region_for_world_plane(x, z, config.region_size_blocks),
        position,
        raw_base_fields,
        base_fields: raw_base_fields,
        temperature: raw_base_fields.temperature,
        hydration: raw_base_fields.hydration,
        height_bias: raw_base_fields.elevation_seed,
        continentality: raw_base_fields.continentality,
        ruggedness: unit_f32(splitmix64(field_hash ^ 0x082e_fa98_ec4e_6c89)),
    }
}

fn raw_macro_friendly_base_fields(
    position: WorldPlanePoint,
    field_hash: u64,
    config: VoronoiGraphConfig,
) -> GraphBaseFields {
    let spacing = config.site_spacing_blocks as f32;
    let continent_scale = (spacing * 34.0).max(1.0);
    let regional_scale = (spacing * 13.0).max(1.0);
    let island_scale = (spacing * 4.5).max(1.0);
    let site_variation = unit_f32(splitmix64(field_hash ^ 0xa409_3822_299f_31d0)) * 2.0 - 1.0;
    let broad_continent = fbm_signed(position, continent_scale, HASH_CONTINENTALITY_FIELD, config);
    let regional_continent = fbm_signed(
        WorldPlanePoint::new(position.x + spacing * 7.3, position.z - spacing * 3.1),
        regional_scale,
        HASH_CONTINENTALITY_FIELD ^ 0x9e37_79b9_7f4a_7c15,
        config,
    );
    let island_detail = fbm_signed(
        WorldPlanePoint::new(position.x - spacing * 2.7, position.z + spacing * 9.2),
        island_scale,
        HASH_CONTINENTALITY_FIELD ^ 0x243f_6a88_85a3_08d3,
        config,
    );
    let continentality = clamp_signed_unit(
        broad_continent * 0.64
            + regional_continent * 0.24
            + island_detail * 0.08
            + site_variation * 0.04,
    );

    let broad_elevation = fbm_signed(position, spacing * 22.0, HASH_ELEVATION_FIELD, config);
    let regional_elevation = fbm_signed(
        WorldPlanePoint::new(position.x + spacing * 11.0, position.z + spacing * 5.0),
        spacing * 7.0,
        HASH_ELEVATION_FIELD ^ 0x1319_8a2e_0370_7344,
        config,
    );
    let local_elevation = unit_f32(splitmix64(field_hash ^ 0x1319_8a2e_0370_7344)) * 2.0 - 1.0;
    let elevation_seed = clamp_signed_unit(
        broad_elevation * 0.42
            + regional_elevation * 0.28
            + continentality * 0.24
            + local_elevation * 0.06,
    );

    let latitude = ((position.z / (spacing * 96.0)).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
    let temperature_noise = fbm_signed(position, spacing * 18.0, HASH_TEMPERATURE_FIELD, config);
    let temperature = clamp_unit(
        0.68 - latitude * 0.36 - elevation_seed.max(0.0) * 0.12 + temperature_noise * 0.14,
    );
    let humidity_noise = fbm_signed(position, spacing * 16.0, HASH_HYDRATION_FIELD, config);
    let hydration = clamp_unit(
        0.50 + humidity_noise * 0.28 - continentality.max(0.0) * 0.10
            + (-continentality).max(0.0) * 0.08,
    );

    GraphBaseFields::new(temperature, hydration, continentality, elevation_seed)
}

fn fbm_signed(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: VoronoiGraphConfig,
) -> f32 {
    fbm_unit(position, scale_blocks, namespace, config) * 2.0 - 1.0
}

fn fbm_unit(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: VoronoiGraphConfig,
) -> f32 {
    let first = value_noise_unit(position, scale_blocks, namespace, config);
    let second = value_noise_unit(
        WorldPlanePoint::new(position.x + 997.0, position.z - 311.0),
        scale_blocks * 0.47,
        namespace ^ 0x6d2b_79f5_aa73_19c9,
        config,
    );
    let third = value_noise_unit(
        WorldPlanePoint::new(position.x - 521.0, position.z + 773.0),
        scale_blocks * 0.23,
        namespace ^ 0xf17b_1a2c_45f1_08ea,
        config,
    );

    (first * 0.58 + second * 0.29 + third * 0.13).clamp(0.0, 1.0)
}

fn value_noise_unit(
    position: WorldPlanePoint,
    scale_blocks: f32,
    namespace: u64,
    config: VoronoiGraphConfig,
) -> f32 {
    let scale = scale_blocks.max(1.0);
    let x = position.x / scale;
    let z = position.z / scale;
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let tx = smooth_unit(x - x0 as f32);
    let tz = smooth_unit(z - z0 as f32);
    let a = lattice_unit(config, namespace, x0, z0);
    let b = lattice_unit(config, namespace, x0 + 1, z0);
    let c = lattice_unit(config, namespace, x0, z0 + 1);
    let d = lattice_unit(config, namespace, x0 + 1, z0 + 1);
    let top = lerp(a, b, tx);
    let bottom = lerp(c, d, tx);

    lerp(top, bottom, tz).clamp(0.0, 1.0)
}

fn lattice_unit(config: VoronoiGraphConfig, namespace: u64, x: i32, z: i32) -> f32 {
    unit_f32(graph_hash(config, namespace, x, z, 0))
}

fn smooth_unit(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, amount: f32) -> f32 {
    a + (b - a) * amount
}

fn point_distance(a: WorldPlanePoint, b: WorldPlanePoint) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

fn site_adjacency(sites: &[VoronoiSite], edges: &[VoronoiEdge]) -> Vec<Vec<usize>> {
    let site_indices = sites
        .iter()
        .enumerate()
        .map(|(index, site)| (site.id, index))
        .collect::<HashMap<_, _>>();
    let mut adjacency = vec![Vec::new(); sites.len()];

    for edge in edges {
        let Some(&a) = site_indices.get(&edge.sites[0]) else {
            continue;
        };
        let Some(&b) = site_indices.get(&edge.sites[1]) else {
            continue;
        };

        adjacency[a].push(b);
        adjacency[b].push(a);
    }

    adjacency.par_iter_mut().for_each(|neighbors| {
        neighbors.sort_unstable();
        neighbors.dedup();
    });

    adjacency
}

fn smooth_base_field_pass(
    fields: &[GraphBaseFields],
    adjacency: &[Vec<usize>],
    self_weight: f32,
) -> Vec<GraphBaseFields> {
    fields
        .par_iter()
        .enumerate()
        .map(|(index, current)| {
            let neighbors = &adjacency[index];
            if neighbors.is_empty() {
                return *current;
            }

            let neighbor_weight = 1.0 / neighbors.len() as f32;
            let neighbor_average = neighbors
                .iter()
                .fold(GraphBaseFields::default(), |sum, &neighbor| {
                    add_weighted_base_fields(sum, fields[neighbor], neighbor_weight)
                });

            blend_base_fields(*current, neighbor_average, self_weight)
        })
        .collect()
}

fn assign_corner_base_fields(
    corners: &mut [VoronoiCorner],
    sites: &[VoronoiSite],
    edges: &[VoronoiEdge],
) {
    let site_indices = sites
        .iter()
        .enumerate()
        .map(|(index, site)| (site.id, index))
        .collect::<HashMap<_, _>>();
    let corner_sites = corner_site_adjacency(corners, edges);

    corners.par_iter_mut().for_each(|corner| {
        let adjacent_sites = corner_sites
            .get(&corner.id)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let raw_base_fields = corner_base_fields_from_sites(
            corner.position,
            adjacent_sites,
            &site_indices,
            sites,
            true,
        )
        .unwrap_or_default();
        let base_fields = corner_base_fields_from_sites(
            corner.position,
            adjacent_sites,
            &site_indices,
            sites,
            false,
        )
        .unwrap_or(raw_base_fields);

        corner.raw_base_fields = raw_base_fields;
        corner.base_fields = base_fields;
        corner.elevation = base_fields.elevation_seed;
    });
}

fn corner_site_adjacency(
    corners: &[VoronoiCorner],
    edges: &[VoronoiEdge],
) -> HashMap<VoronoiCornerId, Vec<VoronoiSiteId>> {
    let mut corner_site_ids = corners
        .iter()
        .map(|corner| (corner.id, Vec::<VoronoiSiteId>::new()))
        .collect::<HashMap<_, _>>();

    for edge in edges {
        for corner in edge.corners {
            let Some(adjacent) = corner_site_ids.get_mut(&corner) else {
                continue;
            };
            adjacent.extend(edge.sites);
        }
    }

    corner_site_ids.par_iter_mut().for_each(|(_, adjacent)| {
        adjacent.sort_by_key(|site| site.0);
        adjacent.dedup();
    });

    corner_site_ids
}

fn corner_base_fields_from_sites(
    position: WorldPlanePoint,
    adjacent: &[VoronoiSiteId],
    site_indices: &HashMap<VoronoiSiteId, usize>,
    sites: &[VoronoiSite],
    use_raw_fields: bool,
) -> Option<GraphBaseFields> {
    let mut total_weight = 0.0;
    let mut sum = GraphBaseFields::default();

    for site_id in adjacent {
        let site = sites.get(*site_indices.get(site_id)?)?;
        let dx = position.x - site.position.x;
        let dz = position.z - site.position.z;
        let distance_squared = dx.mul_add(dx, dz * dz).max(1.0);
        let weight = 1.0 / distance_squared;
        let fields = if use_raw_fields {
            site.raw_base_fields
        } else {
            site.base_fields
        };

        total_weight += weight;
        sum = add_weighted_base_fields(sum, fields, weight);
    }

    (total_weight > f32::EPSILON).then(|| scale_base_fields(sum, 1.0 / total_weight))
}

fn add_weighted_base_fields(
    sum: GraphBaseFields,
    fields: GraphBaseFields,
    weight: f32,
) -> GraphBaseFields {
    GraphBaseFields {
        temperature: sum.temperature + fields.temperature * weight,
        hydration: sum.hydration + fields.hydration * weight,
        continentality: sum.continentality + fields.continentality * weight,
        elevation_seed: sum.elevation_seed + fields.elevation_seed * weight,
    }
}

fn scale_base_fields(fields: GraphBaseFields, scale: f32) -> GraphBaseFields {
    GraphBaseFields::new(
        fields.temperature * scale,
        fields.hydration * scale,
        fields.continentality * scale,
        fields.elevation_seed * scale,
    )
}

fn blend_base_fields(
    fields: GraphBaseFields,
    neighbor_average: GraphBaseFields,
    self_weight: f32,
) -> GraphBaseFields {
    let neighbor_weight = 1.0 - self_weight;

    GraphBaseFields::new(
        fields.temperature * self_weight + neighbor_average.temperature * neighbor_weight,
        fields.hydration * self_weight + neighbor_average.hydration * neighbor_weight,
        fields.continentality * self_weight + neighbor_average.continentality * neighbor_weight,
        fields.elevation_seed * self_weight + neighbor_average.elevation_seed * neighbor_weight,
    )
}

fn generate_delaunay_voronoi_dual(
    sites: &[VoronoiSite],
    config: VoronoiGraphConfig,
) -> (Vec<VoronoiCorner>, Vec<VoronoiEdge>) {
    let points = sites
        .iter()
        .map(|site| Point {
            x: f64::from(site.position.x),
            y: f64::from(site.position.z),
        })
        .collect::<Vec<_>>();
    let triangulation = triangulate(&points);
    let mut corners = (0..triangulation.triangles.len() / 3)
        .into_par_iter()
        .filter_map(|triangle_index| triangle_corner(triangle_index, &triangulation, sites, config))
        .collect::<Vec<_>>();
    corners.sort_by_key(|corner| corner.id.0);
    corners.dedup_by_key(|corner| corner.id.0);

    let corner_ids = (0..triangulation.triangles.len() / 3)
        .filter_map(|triangle_index| {
            let triangle_sites = triangle_site_ids(triangle_index, &triangulation, sites)?;
            Some((triangle_index, triangle_corner_id(triangle_sites, config)))
        })
        .collect::<HashMap<_, _>>();
    let mut edges = delaunay_voronoi_edges(&triangulation, sites, &corner_ids, config);
    edges.sort_by_key(|edge| edge.id.0);
    edges.dedup_by_key(|edge| edge.id.0);

    (corners, edges)
}

fn triangle_corner(
    triangle_index: usize,
    triangulation: &Triangulation,
    sites: &[VoronoiSite],
    config: VoronoiGraphConfig,
) -> Option<VoronoiCorner> {
    let triangle_sites = triangle_site_ids(triangle_index, triangulation, sites)?;
    let a = sites[triangulation.triangles[triangle_index * 3]].position;
    let b = sites[triangulation.triangles[triangle_index * 3 + 1]].position;
    let c = sites[triangulation.triangles[triangle_index * 3 + 2]].position;
    let position = circumcenter(a, b, c)?;

    Some(VoronoiCorner {
        id: triangle_corner_id(triangle_sites, config),
        position,
        raw_base_fields: GraphBaseFields::default(),
        base_fields: GraphBaseFields::default(),
        elevation: 0.0,
        water_accumulation: 0.0,
    })
}

fn triangle_site_ids(
    triangle_index: usize,
    triangulation: &Triangulation,
    sites: &[VoronoiSite],
) -> Option<[VoronoiSiteId; 3]> {
    let offset = triangle_index.checked_mul(3)?;
    let mut ids = [
        sites.get(*triangulation.triangles.get(offset)?)?.id,
        sites.get(*triangulation.triangles.get(offset + 1)?)?.id,
        sites.get(*triangulation.triangles.get(offset + 2)?)?.id,
    ];
    ids.sort_by_key(|site| site.0);
    Some(ids)
}

fn triangle_corner_id(sites: [VoronoiSiteId; 3], config: VoronoiGraphConfig) -> VoronoiCornerId {
    VoronoiCornerId(graph_hash_u64s(
        config,
        HASH_SITE ^ 0x6a09_e667_f3bc_c909,
        &[sites[0].0, sites[1].0, sites[2].0],
    ))
}

fn delaunay_voronoi_edges(
    triangulation: &Triangulation,
    sites: &[VoronoiSite],
    corner_ids: &HashMap<usize, VoronoiCornerId>,
    config: VoronoiGraphConfig,
) -> Vec<VoronoiEdge> {
    triangulation
        .halfedges
        .par_iter()
        .enumerate()
        .filter_map(|(edge_index, &opposite)| {
            if opposite == EMPTY || edge_index > opposite {
                return None;
            }
            let triangle_index = edge_index / 3;
            let opposite_triangle_index = opposite / 3;
            let a = sites[triangulation.triangles[edge_index]].id;
            let b = sites[triangulation.triangles[next_halfedge(edge_index)]].id;
            let corners = [
                *corner_ids.get(&triangle_index)?,
                *corner_ids.get(&opposite_triangle_index)?,
            ];
            Some(make_edge(a, b, corners, config))
        })
        .collect()
}

fn make_edge(
    a: VoronoiSiteId,
    b: VoronoiSiteId,
    mut corners: [VoronoiCornerId; 2],
    config: VoronoiGraphConfig,
) -> VoronoiEdge {
    let mut sites = [a, b];
    sites.sort_by_key(|site| site.0);
    corners.sort_by_key(|corner| corner.0);
    let boundary_curve_seed = graph_hash_u64s(
        config,
        HASH_EDGE,
        &[sites[0].0, sites[1].0, corners[0].0, corners[1].0],
    );

    VoronoiEdge {
        id: VoronoiEdgeId(boundary_curve_seed),
        sites,
        corners,
        boundary_curve_seed,
        hydrology_bias: unit_f32(splitmix64(boundary_curve_seed ^ 0x4528_21e6_38d0_1377)),
    }
}

fn next_halfedge(edge: usize) -> usize {
    if edge % 3 == 2 { edge - 2 } else { edge + 1 }
}

fn circumcenter(
    a: WorldPlanePoint,
    b: WorldPlanePoint,
    c: WorldPlanePoint,
) -> Option<WorldPlanePoint> {
    let ax = f64::from(a.x);
    let ay = f64::from(a.z);
    let bx = f64::from(b.x);
    let by = f64::from(b.z);
    let cx = f64::from(c.x);
    let cy = f64::from(c.z);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() <= f64::EPSILON {
        return None;
    }

    let ax2ay2 = ax * ax + ay * ay;
    let bx2by2 = bx * bx + by * by;
    let cx2cy2 = cx * cx + cy * cy;
    let x = (ax2ay2 * (by - cy) + bx2by2 * (cy - ay) + cx2cy2 * (ay - by)) / d;
    let z = (ax2ay2 * (cx - bx) + bx2by2 * (ax - cx) + cx2cy2 * (bx - ax)) / d;

    (x.is_finite() && z.is_finite()).then(|| WorldPlanePoint::new(x as f32, z as f32))
}

fn graph_region_for_world_plane(x: f64, z: f64, region_size_blocks: i32) -> GraphRegionCoord {
    let world_x = x.floor().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    let world_z = z.floor().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
    graph_region_for_world_block(world_x, world_z, region_size_blocks)
}

fn graph_hash(config: VoronoiGraphConfig, namespace: u64, x: i32, z: i32, extra: i32) -> u64 {
    let mut state = splitmix64(config.seed ^ namespace);
    state = splitmix64(state ^ u64::from(config.generator_version));
    state = splitmix64(state ^ u64::from(config.site_spacing_blocks as u32));
    state = splitmix64(state ^ u64::from(zigzag_i32(x)));
    state = splitmix64(state ^ u64::from(zigzag_i32(z)));
    splitmix64(state ^ u64::from(zigzag_i32(extra)))
}

fn graph_hash_u64s(config: VoronoiGraphConfig, namespace: u64, values: &[u64]) -> u64 {
    let mut state = splitmix64(config.seed ^ namespace);
    state = splitmix64(state ^ u64::from(config.generator_version));
    state = splitmix64(state ^ u64::from(config.site_spacing_blocks as u32));
    for &value in values {
        state = splitmix64(state ^ value);
    }
    state
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit_f32(value: u64) -> f32 {
    unit_f64(value) as f32
}

fn unit_f64(value: u64) -> f64 {
    const SCALE: f64 = 1.0 / ((1u64 << 53) as f64);
    ((value >> 11) as f64) * SCALE
}

fn clamp_unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn clamp_signed_unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-1.0, 1.0)
    }
}

fn pack_grid_coord(x: i32, z: i32) -> u64 {
    (u64::from(zigzag_i32(x)) << 32) | u64::from(zigzag_i32(z))
}

fn zigzag_i32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

fn i32_from_i64(value: i64) -> i32 {
    i32::try_from(value).expect("graph lattice coordinate must fit i32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn graph_region_mapping_uses_euclidean_negative_coordinates() {
        let size = 1024;

        assert_eq!(
            graph_region_for_world_block(0, 0, size),
            GraphRegionCoord::new(0, 0)
        );
        assert_eq!(
            graph_region_for_world_block(1023, 1023, size),
            GraphRegionCoord::new(0, 0)
        );
        assert_eq!(
            graph_region_for_world_block(1024, -1, size),
            GraphRegionCoord::new(1, -1)
        );
        assert_eq!(
            graph_region_for_world_block(-1, -1024, size),
            GraphRegionCoord::new(-1, -1)
        );
    }

    #[test]
    fn graph_region_area_is_inclusive() {
        let area = GraphRegionArea::new(GraphRegionCoord::new(-1, 2), GraphRegionCoord::new(1, 3))
            .unwrap();

        assert_eq!(area.region_count(), 6);
        assert!(area.contains(GraphRegionCoord::new(1, 3)));
        assert!(!area.contains(GraphRegionCoord::new(2, 3)));
    }

    #[test]
    fn graph_patch_generation_is_deterministic_for_same_request() {
        let request = test_request(42, 0, 0);

        let first = generate_voronoi_graph_patch(request);
        let second = generate_voronoi_graph_patch(request);

        assert_eq!(first.owner_regions, second.owner_regions);
        assert_eq!(first.sites, second.sites);
        assert_eq!(first.corners, second.corners);
        assert_eq!(first.edges, second.edges);
    }

    #[test]
    fn graph_patch_generation_changes_with_seed() {
        let first = generate_voronoi_graph_patch(test_request(42, 0, 0));
        let second = generate_voronoi_graph_patch(test_request(43, 0, 0));

        assert_ne!(first.sites, second.sites);
    }

    #[test]
    fn base_graph_field_smoothing_reduces_adjacent_site_differences() {
        let patch = generate_voronoi_graph_patch(test_request(177, 0, 0));

        let raw_difference =
            average_edge_base_field_difference(&patch, |site| site.raw_base_fields);
        let smoothed_difference =
            average_edge_base_field_difference(&patch, |site| site.base_fields);

        assert!(raw_difference > 0.0);
        assert!(
            smoothed_difference < raw_difference,
            "smoothed={smoothed_difference} raw={raw_difference}"
        );
    }

    #[test]
    fn corner_base_fields_are_weighted_from_surrounding_sites() {
        let patch = generate_voronoi_graph_patch(test_request(211, 0, 0));
        let corner = patch
            .corners
            .iter()
            .find(|corner| corner_neighbor_sites(&patch, corner.id).len() >= 3)
            .expect("test patch should contain an interior Delaunay triangle corner");
        let expected_raw = expected_corner_fields(&patch, *corner, true);
        let expected_smoothed = expected_corner_fields(&patch, *corner, false);

        assert_base_fields_close(corner.raw_base_fields, expected_raw);
        assert_base_fields_close(corner.base_fields, expected_smoothed);
        assert_close(corner.elevation, corner.base_fields.elevation_seed);
    }

    #[test]
    fn graph_edges_use_delaunay_voronoi_dual_not_square_grid_only_valence() {
        let patch = generate_voronoi_graph_patch(test_request(512, 0, 0));
        let adjacency = site_adjacency(&patch.sites, &patch.edges);
        let diagonal_or_oblique_edges = patch
            .edges
            .iter()
            .filter(|edge| {
                let [a, b] = edge_sites(&patch, edge);
                let dx = (a.position.x - b.position.x).abs();
                let dz = (a.position.z - b.position.z).abs();
                dx > DEFAULT_SITE_SPACING_BLOCKS as f32 * 0.35
                    && dz > DEFAULT_SITE_SPACING_BLOCKS as f32 * 0.35
            })
            .count();

        assert!(
            adjacency.iter().any(|neighbors| neighbors.len() >= 5),
            "Delaunay center graph should include non-grid valence"
        );
        assert!(
            diagonal_or_oblique_edges > patch.edges.len() / 8,
            "Delaunay edges should not collapse to only horizontal/vertical lattice adjacencies"
        );
    }

    #[test]
    fn site_spacing_has_visible_variability_but_keeps_minimum_guard() {
        let patch = generate_voronoi_graph_patch(test_request(42, 0, 0));
        let stats = graph_site_spacing_stats(&patch);
        let spacing = DEFAULT_SITE_SPACING_BLOCKS as f32;

        assert!(
            stats.nearest_distance_cv >= 0.16,
            "site spacing should avoid an overly uniform grid look: {stats:?}"
        );
        assert!(
            stats.min_nearest_distance_blocks >= spacing * MIN_NEAREST_SITE_SPACING_FRACTION,
            "site jitter should keep a conservative minimum spacing guard: {stats:?}"
        );
        assert!(
            stats.max_nearest_distance_blocks >= stats.average_nearest_distance_blocks * 1.18,
            "site spacing should include visibly larger cells as well as smaller ones: {stats:?}"
        );
    }

    #[test]
    fn base_graph_fields_change_with_seed_for_same_site_id() {
        let first = generate_voronoi_graph_patch(test_request(42, 0, 0));
        let second = generate_voronoi_graph_patch(test_request(43, 0, 0));
        let first_site = first
            .sites
            .first()
            .expect("test patch should contain sites");
        let second_site = second
            .site(first_site.id)
            .expect("same lattice site id should exist for both seeds");

        assert_ne!(first_site.raw_base_fields, second_site.raw_base_fields);
        assert_ne!(first_site.base_fields, second_site.base_fields);
    }

    #[test]
    fn base_graph_continentality_is_long_range_coherent() {
        let patch = generate_voronoi_graph_patch(test_request(310, 0, 0));
        let adjacent_difference =
            average_edge_component_difference(&patch, |fields| fields.continentality);
        let global_span = component_span(&patch, |fields| fields.continentality);

        assert!(
            global_span >= 0.35,
            "continentality should expose broad land/ocean range; span={global_span}"
        );
        assert!(
            adjacent_difference <= global_span * 0.38,
            "adjacent sites should vary less than the patch-scale field; adjacent={adjacent_difference} span={global_span}"
        );
    }

    #[test]
    fn base_graph_elevation_seed_is_coherent_with_continentality() {
        let patch = generate_voronoi_graph_patch(test_request(311, 0, 0));
        let adjacent_difference =
            average_edge_component_difference(&patch, |fields| fields.elevation_seed);
        let global_span = component_span(&patch, |fields| fields.elevation_seed);
        let correlation = component_correlation(
            &patch,
            |fields| fields.continentality,
            |fields| fields.elevation_seed,
        );

        assert!(
            global_span >= 0.20,
            "elevation_seed should retain macro relief range; span={global_span}"
        );
        assert!(
            adjacent_difference <= global_span * 0.55,
            "elevation_seed should not be salt-and-pepper noise; adjacent={adjacent_difference} span={global_span}"
        );
        assert!(
            correlation > 0.12,
            "elevation_seed should carry land/ocean context from continentality; correlation={correlation}"
        );
    }

    #[test]
    fn adjacent_center_requests_keep_overlapping_sites_stable() {
        let left = generate_voronoi_graph_patch(test_request(77, 0, 0));
        let right =
            generate_voronoi_graph_patch(test_request(77, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0));
        let overlap_region = GraphRegionCoord::new(1, 0);
        let left_sites = sites_in_region_by_id(&left, overlap_region);
        let right_sites = sites_in_region_by_id(&right, overlap_region);

        assert!(!left_sites.is_empty());
        assert_eq!(left_sites, right_sites);
    }

    #[test]
    fn adjacent_center_requests_keep_overlapping_base_fields_stable() {
        let left = generate_voronoi_graph_patch(test_request(88, 0, 0));
        let right =
            generate_voronoi_graph_patch(test_request(88, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0));
        let overlap_region = GraphRegionCoord::new(1, 0);
        let left_sites = site_fields_in_region_by_id(&left, overlap_region);
        let right_sites = site_fields_in_region_by_id(&right, overlap_region);

        assert!(!left_sites.is_empty());
        assert_eq!(left_sites, right_sites);
    }

    #[test]
    fn adjacent_center_requests_keep_overlapping_internal_edges_stable() {
        let left = generate_voronoi_graph_patch(test_request(89, 0, 0));
        let right =
            generate_voronoi_graph_patch(test_request(89, DEFAULT_GRAPH_REGION_SIZE_BLOCKS, 0));
        let overlap_region = GraphRegionCoord::new(1, 0);
        let left_sites = sites_in_region_by_id(&left, overlap_region);
        let right_sites = sites_in_region_by_id(&right, overlap_region);
        let left_edges = internal_edges_by_id(&left, &left_sites);
        let right_edges = internal_edges_by_id(&right, &right_sites);

        assert!(!left_edges.is_empty());
        assert_eq!(left_edges, right_edges);
    }

    #[test]
    fn negative_coordinate_request_generates_negative_region_patch() {
        let patch = generate_voronoi_graph_patch(test_request(11, -1, -1));

        assert_eq!(patch.owner_regions, vec![GraphRegionCoord::new(-1, -1)]);
        assert!(
            patch
                .sites
                .iter()
                .any(|site| site.owner_region.x < 0 && site.owner_region.z < 0)
        );
    }

    #[test]
    fn generated_graph_patch_is_not_empty() {
        let patch = generate_voronoi_graph_patch(test_request(5, 0, 0));

        assert!(!patch.sites.is_empty());
        assert!(!patch.corners.is_empty());
        assert!(!patch.edges.is_empty());
        assert!(!patch.is_empty());
    }

    fn test_request(
        seed: u64,
        center_world_x: i32,
        center_world_z: i32,
    ) -> VoronoiGraphPatchRequest {
        VoronoiGraphPatchRequest::new(
            VoronoiGraphConfig {
                seed,
                generator_version: 3,
                region_size_blocks: DEFAULT_GRAPH_REGION_SIZE_BLOCKS,
                site_spacing_blocks: DEFAULT_SITE_SPACING_BLOCKS,
                padding_regions: 1,
            },
            center_world_x,
            center_world_z,
        )
    }

    fn sites_in_region_by_id(
        patch: &VoronoiGraphPatch,
        region: GraphRegionCoord,
    ) -> HashMap<VoronoiSiteId, WorldPlanePoint> {
        patch
            .sites
            .iter()
            .filter(|site| site.owner_region == region)
            .map(|site| (site.id, site.position))
            .collect()
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct SiteFieldSnapshot {
        position: WorldPlanePoint,
        raw_base_fields: GraphBaseFields,
        base_fields: GraphBaseFields,
        temperature: f32,
        hydration: f32,
        height_bias: f32,
        continentality: f32,
    }

    fn site_fields_in_region_by_id(
        patch: &VoronoiGraphPatch,
        region: GraphRegionCoord,
    ) -> HashMap<VoronoiSiteId, SiteFieldSnapshot> {
        patch
            .sites
            .iter()
            .filter(|site| site.owner_region == region)
            .map(|site| {
                (
                    site.id,
                    SiteFieldSnapshot {
                        position: site.position,
                        raw_base_fields: site.raw_base_fields,
                        base_fields: site.base_fields,
                        temperature: site.temperature,
                        hydration: site.hydration,
                        height_bias: site.height_bias,
                        continentality: site.continentality,
                    },
                )
            })
            .collect()
    }

    fn internal_edges_by_id(
        patch: &VoronoiGraphPatch,
        sites: &HashMap<VoronoiSiteId, WorldPlanePoint>,
    ) -> HashMap<VoronoiEdgeId, VoronoiEdge> {
        patch
            .edges
            .iter()
            .filter(|edge| sites.contains_key(&edge.sites[0]) && sites.contains_key(&edge.sites[1]))
            .map(|edge| (edge.id, *edge))
            .collect()
    }

    fn average_edge_base_field_difference(
        patch: &VoronoiGraphPatch,
        select_fields: impl Fn(&VoronoiSite) -> GraphBaseFields,
    ) -> f32 {
        let site_map = patch
            .sites
            .iter()
            .map(|site| (site.id, site))
            .collect::<HashMap<_, _>>();
        let mut total = 0.0;
        let mut count = 0;

        for edge in &patch.edges {
            let Some(a) = site_map.get(&edge.sites[0]) else {
                continue;
            };
            let Some(b) = site_map.get(&edge.sites[1]) else {
                continue;
            };

            total += base_field_distance(select_fields(a), select_fields(b));
            count += 1;
        }

        total / count as f32
    }

    fn average_edge_component_difference(
        patch: &VoronoiGraphPatch,
        select_component: impl Fn(GraphBaseFields) -> f32,
    ) -> f32 {
        let site_map = patch
            .sites
            .iter()
            .map(|site| (site.id, site))
            .collect::<HashMap<_, _>>();
        let mut total = 0.0;
        let mut count = 0;

        for edge in &patch.edges {
            let Some(a) = site_map.get(&edge.sites[0]) else {
                continue;
            };
            let Some(b) = site_map.get(&edge.sites[1]) else {
                continue;
            };

            total += (select_component(a.base_fields) - select_component(b.base_fields)).abs();
            count += 1;
        }

        total / count as f32
    }

    fn component_span(
        patch: &VoronoiGraphPatch,
        select_component: impl Fn(GraphBaseFields) -> f32,
    ) -> f32 {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;

        for site in &patch.sites {
            let value = select_component(site.base_fields);
            min = min.min(value);
            max = max.max(value);
        }

        max - min
    }

    fn component_correlation(
        patch: &VoronoiGraphPatch,
        select_a: impl Fn(GraphBaseFields) -> f32,
        select_b: impl Fn(GraphBaseFields) -> f32,
    ) -> f32 {
        let count = patch.sites.len() as f32;
        let mean_a = patch
            .sites
            .iter()
            .map(|site| select_a(site.base_fields))
            .sum::<f32>()
            / count;
        let mean_b = patch
            .sites
            .iter()
            .map(|site| select_b(site.base_fields))
            .sum::<f32>()
            / count;
        let mut covariance = 0.0;
        let mut variance_a = 0.0;
        let mut variance_b = 0.0;

        for site in &patch.sites {
            let a = select_a(site.base_fields) - mean_a;
            let b = select_b(site.base_fields) - mean_b;
            covariance += a * b;
            variance_a += a * a;
            variance_b += b * b;
        }

        covariance / (variance_a.sqrt() * variance_b.sqrt()).max(f32::EPSILON)
    }

    fn base_field_distance(a: GraphBaseFields, b: GraphBaseFields) -> f32 {
        (a.temperature - b.temperature).abs()
            + (a.hydration - b.hydration).abs()
            + (a.continentality - b.continentality).abs()
            + (a.elevation_seed - b.elevation_seed).abs()
    }

    fn expected_corner_fields(
        patch: &VoronoiGraphPatch,
        corner: VoronoiCorner,
        use_raw_fields: bool,
    ) -> GraphBaseFields {
        let site_indices = patch
            .sites
            .iter()
            .enumerate()
            .map(|(index, site)| (site.id, index))
            .collect::<HashMap<_, _>>();
        let adjacent = corner_neighbor_sites(patch, corner.id);

        corner_base_fields_from_sites(
            corner.position,
            &adjacent,
            &site_indices,
            &patch.sites,
            use_raw_fields,
        )
        .expect("corner should have surrounding Delaunay sites")
    }

    fn corner_neighbor_sites(
        patch: &VoronoiGraphPatch,
        corner_id: VoronoiCornerId,
    ) -> Vec<VoronoiSiteId> {
        let mut sites = patch
            .edges
            .iter()
            .filter(|edge| edge.corners.contains(&corner_id))
            .flat_map(|edge| edge.sites)
            .collect::<Vec<_>>();
        sites.sort_by_key(|site| site.0);
        sites.dedup();
        sites
    }

    fn edge_sites<'a>(patch: &'a VoronoiGraphPatch, edge: &VoronoiEdge) -> [&'a VoronoiSite; 2] {
        let sites = patch
            .sites
            .iter()
            .map(|site| (site.id, site))
            .collect::<HashMap<_, _>>();
        [sites[&edge.sites[0]], sites[&edge.sites[1]]]
    }

    fn assert_base_fields_close(actual: GraphBaseFields, expected: GraphBaseFields) {
        assert_close(actual.temperature, expected.temperature);
        assert_close(actual.hydration, expected.hydration);
        assert_close(actual.continentality, expected.continentality);
        assert_close(actual.elevation_seed, expected.elevation_seed);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.000_001,
            "actual={actual} expected={expected}"
        );
    }
}
