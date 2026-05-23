use std::collections::HashMap;

use super::height::{
    boundary_roughness_offset, is_lake_surface, lake_boundary_lowering_factor, lerp, smoothstep01,
};
use super::influence::RIDGE_FIELD_SOURCE_MIN_RIDGENESS;
use super::river::{
    RiverInfluenceSample, curve_bucket, curve_bucket_search_radius, flow_hint_from_plan,
    nearest_polyline_segment, polyline_distance, river_influence_sample, signed_side, site_bucket,
    squared_distance, usable_side,
};
use super::types::MacroFieldTileConfig;
use crate::world::generation::biome::GraphBiomeCell;
use crate::world::generation::boundary::{BoundaryCache, BoundaryJunction, NoisyBoundaryCurve};
use crate::world::generation::graph::{
    VoronoiEdgeId, VoronoiGraphPatch, VoronoiSiteId, WorldPlanePoint,
};
use crate::world::generation::hydrology::GraphDrainageNodeKind;
use crate::world::generation::macro_map::{GraphMacroMap, MacroSite};
use crate::world::generation::river_plan::RiverPlan;

pub(super) const MACRO_FIELD_CURVE_BUCKET_BLOCKS: f32 = 64.0;
pub(super) const MACRO_FIELD_SITE_BUCKET_BLOCKS: f32 = 256.0;
pub(super) const MACRO_FIELD_NOISY_OWNER_SEARCH_RADIUS_BLOCKS: f32 = 1024.0;
pub(super) const MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS: f32 = 768.0;
pub(super) const MACRO_FIELD_SCALAR_INTERPOLATION_BUCKET_RADIUS: i32 = 4;
pub(super) const MACRO_FIELD_SCALAR_INTERPOLATION_DISTANCE_POWER: f32 = 1.45;
pub(super) const MACRO_FIELD_JUNCTION_OWNER_MAX_RADIUS_BLOCKS: f32 = 8.0;
#[derive(Debug)]
pub struct MacroFieldRasterContext<'a> {
    pub(super) sites: &'a [MacroSite],
    pub(super) site_by_id: HashMap<VoronoiSiteId, MacroSite>,
    pub(super) biome_by_site_id: HashMap<VoronoiSiteId, GraphBiomeCell>,
    pub(super) site_grid: SiteIndexGrid,
    pub(super) boundary_edges: Vec<BoundaryEdgeRef<'a>>,
    pub(super) boundary_grid: CurveIndexGrid,
    pub(super) junctions: Vec<BoundaryJunction>,
    pub(super) junction_grid: JunctionIndexGrid,
    pub(super) coast_curves: Vec<&'a NoisyBoundaryCurve>,
    pub(super) coast_grid: CurveIndexGrid,
    pub(super) ridge_curves: Vec<&'a NoisyBoundaryCurve>,
    pub(super) ridge_grid: CurveIndexGrid,
    pub(super) river_curves: Vec<RiverCurveRef>,
    pub(super) river_grid: CurveIndexGrid,
    pub(super) estuary_fans: Vec<EstuaryFanRef>,
}

impl<'a> MacroFieldRasterContext<'a> {
    pub fn new(
        _patch: &'a VoronoiGraphPatch,
        macro_map: &'a GraphMacroMap,
        river_plan: &'a RiverPlan,
        boundary: &'a BoundaryCache,
    ) -> Self {
        let macro_edges = macro_map
            .edges
            .iter()
            .map(|edge| (edge.id, edge))
            .collect::<HashMap<_, _>>();
        let boundary_curves = boundary
            .curves
            .iter()
            .map(|curve| (curve.edge, curve))
            .collect::<HashMap<_, _>>();
        let site_by_id = macro_map
            .sites
            .iter()
            .map(|site| (site.id, *site))
            .collect::<HashMap<_, _>>();
        let biome_by_site_id = macro_map
            .biomes
            .iter()
            .map(|biome| (biome.site, *biome))
            .collect::<HashMap<_, _>>();
        let site_grid = SiteIndexGrid::from_sites(&macro_map.sites);
        let coast_curves = macro_map
            .edges
            .iter()
            .filter_map(|edge| {
                macro_edges
                    .get(&edge.id)
                    .is_some_and(|macro_edge| macro_edge.guide.is_coast)
                    .then(|| boundary_curves.get(&edge.id).copied())
                    .flatten()
            })
            .collect::<Vec<_>>();
        let ridge_curves = macro_map
            .edges
            .iter()
            .filter_map(|edge| {
                macro_edges
                    .get(&edge.id)
                    .is_some_and(|macro_edge| {
                        macro_edge.guide.is_ridge_candidate
                            && macro_edge.guide.ridgeness >= RIDGE_FIELD_SOURCE_MIN_RIDGENESS
                    })
                    .then(|| boundary_curves.get(&edge.id).copied())
                    .flatten()
            })
            .collect::<Vec<_>>();
        let mut chain_lengths = HashMap::new();
        for plan in &river_plan.segments {
            *chain_lengths.entry(plan.chain_id).or_insert(0.0) +=
                plan.segment_length_blocks.max(0.0);
        }
        let terminal_chain_ids = river_plan
            .chains
            .iter()
            .filter(|chain| chain.terminal_kind == Some(GraphDrainageNodeKind::CoastOutlet))
            .map(|chain| (chain.id, chain.segment_ids.last().copied()))
            .collect::<HashMap<_, _>>();
        let terminal_mouth_factors = river_plan
            .chains
            .iter()
            .filter(|chain| chain.terminal_kind == Some(GraphDrainageNodeKind::CoastOutlet))
            .flat_map(|chain| {
                let count = chain.segment_ids.len();
                chain
                    .segment_ids
                    .iter()
                    .enumerate()
                    .filter_map(move |(index, segment_id)| {
                        let from_end = count.checked_sub(index + 1)?;
                        match from_end {
                            0 => Some((*segment_id, 1.0)),
                            1 => Some((*segment_id, 0.45)),
                            _ => None,
                        }
                    })
            })
            .collect::<HashMap<_, _>>();
        let mut river_curves = river_plan
            .segments
            .iter()
            .filter_map(|plan| {
                let chain_length = chain_lengths.get(&plan.chain_id).copied().unwrap_or(0.0);
                let longitudinal_start_blocks = (plan.chain_downstream_progress * chain_length
                    - plan.segment_length_blocks.max(0.0) * 0.5)
                    .max(0.0);
                let endpoints = river_plan.endpoints(plan.segment_id);
                boundary_curves
                    .get(&plan.edge)
                    .copied()
                    .map(|curve| RiverCurveRef {
                        is_terminal_outlet: terminal_chain_ids
                            .get(&plan.chain_id)
                            .copied()
                            .flatten()
                            .is_some_and(|segment_id| segment_id == plan.segment_id),
                        edge: plan.edge,
                        points: endpoints
                            .map(|endpoints| {
                                orient_river_curve_points_downstream(
                                    curve.points.clone(),
                                    endpoints.from_position,
                                    endpoints.to_position,
                                )
                            })
                            .unwrap_or_else(|| curve.points.clone()),
                        longitudinal_start_blocks,
                        flow_hint: flow_hint_from_plan(plan),
                        water_width_blocks: plan.bed_width_blocks,
                        valley_width_blocks: plan.broad_valley_width_blocks,
                        bed_depth_blocks: plan.bed_depth_blocks,
                        terminal_mouth_factor: terminal_mouth_factors
                            .get(&plan.segment_id)
                            .copied()
                            .unwrap_or(0.0),
                    })
            })
            .collect::<Vec<_>>();
        river_curves.sort_by_key(|river| river.edge.0);
        let mut estuary_fans = river_plan
            .segments
            .iter()
            .filter_map(|plan| {
                let terminal_segment = terminal_chain_ids
                    .get(&plan.chain_id)
                    .copied()
                    .flatten()
                    .is_some_and(|segment_id| segment_id == plan.segment_id);
                if !terminal_segment {
                    return None;
                }
                let endpoints = river_plan.endpoints(plan.segment_id)?;
                let dx = endpoints.to_position.x - endpoints.from_position.x;
                let dz = endpoints.to_position.z - endpoints.from_position.z;
                let length = (dx * dx + dz * dz).sqrt();
                if length <= f32::EPSILON {
                    return None;
                }
                let flow_hint = flow_hint_from_plan(plan);
                let (start_half_width_blocks, end_half_width_blocks) =
                    estuary_fan_half_widths_blocks(
                        plan.bed_width_blocks,
                        plan.broad_valley_width_blocks,
                        flow_hint,
                    );
                let length_blocks = estuary_fan_length_blocks(
                    plan.bed_width_blocks,
                    plan.broad_valley_width_blocks,
                    flow_hint,
                );
                Some(EstuaryFanRef {
                    segment_id: plan.segment_id.0,
                    origin: endpoints.downstream_position,
                    direction_x: dx / length,
                    direction_z: dz / length,
                    start_half_width_blocks,
                    end_half_width_blocks,
                    length_blocks,
                    flow_hint,
                    bed_depth_hint: estuary_bed_depth_hint(
                        plan.bed_depth_blocks,
                        plan.segment_length_blocks,
                    ),
                    water_depth_hint: estuary_water_depth_hint(plan.bed_depth_blocks, flow_hint),
                })
            })
            .collect::<Vec<_>>();
        estuary_fans.sort_by_key(|fan| fan.segment_id);
        let mut boundary_edges = macro_map
            .edges
            .iter()
            .filter_map(|edge| {
                let left = site_by_id.get(&edge.sites[0]).copied()?;
                let right = site_by_id.get(&edge.sites[1]).copied()?;
                let curve = boundary_curves.get(&edge.id).copied()?;
                Some(BoundaryEdgeRef { curve, left, right })
            })
            .collect::<Vec<_>>();
        boundary_edges.sort_by_key(|edge| edge.curve.edge.0);
        let junctions = boundary.junctions();
        let boundary_grid = CurveIndexGrid::from_boundary_edges(&boundary_edges);
        let junction_grid = JunctionIndexGrid::from_junctions(&junctions);
        let coast_grid = CurveIndexGrid::from_curves(&coast_curves);
        let ridge_grid = CurveIndexGrid::from_curves(&ridge_curves);
        let river_grid = CurveIndexGrid::from_river_curves(&river_curves);

        Self {
            sites: &macro_map.sites,
            site_by_id,
            biome_by_site_id,
            site_grid,
            boundary_edges,
            boundary_grid,
            junctions,
            junction_grid,
            coast_curves,
            coast_grid,
            ridge_curves,
            ridge_grid,
            river_curves,
            river_grid,
            estuary_fans,
        }
    }

    pub fn nearest_site(&self, position: WorldPlanePoint) -> Option<&'a MacroSite> {
        let mut nearest = None;
        self.site_grid
            .for_each_nearest_candidate_index(position, |index| {
                let Some(site) = self.sites.get(index) else {
                    return;
                };
                if nearest_site_order(position, site, nearest).is_lt() {
                    nearest = Some(site);
                }
            });
        if nearest.is_some() {
            return nearest;
        }

        self.sites.iter().min_by(|left, right| {
            squared_distance(position, left.position)
                .total_cmp(&squared_distance(position, right.position))
                .then_with(|| left.id.0.cmp(&right.id.0))
        })
    }

    pub fn biome_for_site(&self, site: VoronoiSiteId) -> Option<GraphBiomeCell> {
        self.biome_by_site_id.get(&site).copied()
    }

    pub(super) fn owner_sample(
        &self,
        position: WorldPlanePoint,
        config: MacroFieldTileConfig,
    ) -> OwnerSample {
        let fallback_site = self.nearest_site(position);

        if let Some(site) = self.nearest_junction_site(position, config) {
            return OwnerSample {
                primary: Some(site),
                macro_elevation: self.interpolated_macro_elevation(position),
                lake_lowering_factor: if is_lake_surface(site.surface_kind) {
                    1.0
                } else {
                    0.0
                },
            };
        }

        if let Some(boundary) = self.nearest_owner_boundary(position) {
            let primary_site = boundary.primary_site();
            let secondary = boundary.secondary_site();
            let is_lake_pair = is_lake_surface(primary_site.surface_kind)
                != is_lake_surface(secondary.surface_kind);
            let primary = self
                .site_by_id
                .get(&primary_site.id)
                .copied()
                .or_else(|| fallback_site.copied());

            return OwnerSample {
                primary,
                macro_elevation: self.interpolated_macro_elevation(position),
                lake_lowering_factor: if is_lake_pair
                    && boundary.distance <= config.boundary_blend_radius_blocks
                {
                    lake_boundary_lowering_factor(
                        primary_site,
                        boundary.distance,
                        config.boundary_blend_radius_blocks,
                        boundary_roughness_offset(
                            position,
                            config.boundary_roughness_blocks,
                            0x1A4E_0001,
                        ),
                    )
                } else if is_lake_surface(primary_site.surface_kind) {
                    1.0
                } else {
                    0.0
                },
            };
        }

        self.owner_sample_from_site(position, fallback_site)
    }

    fn nearest_junction_site(
        &self,
        position: WorldPlanePoint,
        config: MacroFieldTileConfig,
    ) -> Option<MacroSite> {
        let owner_radius_blocks = junction_owner_radius_blocks(config);
        self.junction_grid
            .candidate_indices(position)
            .into_iter()
            .filter_map(|index| self.junctions.get(index))
            .filter(|junction| {
                squared_distance(position, junction.position)
                    <= junction.radius_blocks.min(owner_radius_blocks).powi(2)
            })
            .filter_map(|junction| {
                junction
                    .sites
                    .iter()
                    .filter_map(|site| self.site_by_id.get(site))
                    .min_by(|left, right| {
                        squared_distance(position, left.position)
                            .total_cmp(&squared_distance(position, right.position))
                            .then_with(|| left.id.0.cmp(&right.id.0))
                    })
                    .map(|site| (junction, *site))
            })
            .min_by(|(left_junction, left_site), (right_junction, right_site)| {
                let left_distance = squared_distance(position, left_junction.position);
                let right_distance = squared_distance(position, right_junction.position);
                left_distance
                    .total_cmp(&right_distance)
                    .then_with(|| {
                        squared_distance(position, left_site.position)
                            .total_cmp(&squared_distance(position, right_site.position))
                    })
                    .then_with(|| left_site.id.0.cmp(&right_site.id.0))
            })
            .map(|(_, site)| site)
    }

    pub(super) fn owner_sample_from_site(
        &self,
        position: WorldPlanePoint,
        site: Option<&'a MacroSite>,
    ) -> OwnerSample {
        let Some(site) = site.copied() else {
            return OwnerSample::default();
        };
        OwnerSample {
            primary: Some(site),
            macro_elevation: self.interpolated_macro_elevation(position),
            lake_lowering_factor: if is_lake_surface(site.surface_kind) {
                1.0
            } else {
                0.0
            },
        }
    }

    fn interpolated_macro_elevation(&self, position: WorldPlanePoint) -> f32 {
        let mut exact_source = None;
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;
        self.site_grid.for_each_candidate_index_in_radius(
            position,
            MACRO_FIELD_SCALAR_INTERPOLATION_BUCKET_RADIUS,
            |index| {
                if let Some(site) = self.sites.get(index) {
                    let distance2 = squared_distance(position, site.position);
                    if distance2 <= 0.0001 {
                        exact_source = Some(site.signed_macro_elevation);
                    } else if exact_source.is_none() {
                        let distance = distance2.sqrt();
                        if distance <= MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS {
                            let radius_t = (distance
                                / MACRO_FIELD_SCALAR_INTERPOLATION_RADIUS_BLOCKS)
                                .clamp(0.0, 1.0);
                            let falloff = 1.0 - smoothstep01(radius_t);
                            let weight = falloff
                                / distance
                                    .max(1.0)
                                    .powf(MACRO_FIELD_SCALAR_INTERPOLATION_DISTANCE_POWER);
                            weighted_sum += site.signed_macro_elevation * weight;
                            weight_sum += weight;
                        }
                    }
                }
            },
        );

        if let Some(elevation) = exact_source {
            return elevation;
        }

        if weight_sum <= f32::EPSILON {
            self.nearest_site(position)
                .map(|site| site.signed_macro_elevation)
                .unwrap_or_default()
        } else {
            weighted_sum / weight_sum
        }
    }

    fn nearest_owner_boundary(&self, position: WorldPlanePoint) -> Option<BoundarySideSample> {
        let mut candidate_indices = self
            .boundary_grid
            .candidate_indices(position, MACRO_FIELD_NOISY_OWNER_SEARCH_RADIUS_BLOCKS);
        if candidate_indices.is_empty() {
            candidate_indices = (0..self.boundary_edges.len()).collect();
        }

        candidate_indices
            .into_iter()
            .filter_map(|index| self.boundary_edges[index].side_sample(position))
            .min_by(|left, right| left.distance.total_cmp(&right.distance))
    }

    pub(super) fn nearest_coast_distance(
        &self,
        position: WorldPlanePoint,
        radius: f32,
    ) -> Option<f32> {
        self.coast_grid
            .candidate_indices(position, radius)
            .into_iter()
            .filter_map(|index| self.coast_curves.get(index))
            .map(|curve| polyline_distance(position, &curve.points))
            .min_by(f32::total_cmp)
    }

    pub(super) fn river_valley(
        &self,
        position: WorldPlanePoint,
        config: MacroFieldTileConfig,
    ) -> RiverInfluenceSample {
        let Some((sample, _edge)) = self
            .river_grid
            .candidate_indices(position, config.river_radius_blocks)
            .into_iter()
            .filter_map(|index| self.river_curves.get(index))
            .map(|river| {
                Some((
                    river_influence_sample(
                        position,
                        &river.points,
                        river.flow_hint,
                        river.water_width_blocks,
                        river.valley_width_blocks,
                        river.bed_depth_blocks,
                        config.river_radius_blocks,
                        config.boundary_roughness_blocks,
                    ),
                    river.edge,
                ))
            })
            .flatten()
            .min_by(|left, right| {
                left.0
                    .distance_blocks
                    .total_cmp(&right.0.distance_blocks)
                    .then_with(|| left.1.0.cmp(&right.1.0))
            })
        else {
            return RiverInfluenceSample {
                distance_blocks: f32::INFINITY,
                ..RiverInfluenceSample::default()
            };
        };

        sample
    }
}

#[derive(Debug, Clone)]
pub(super) struct RiverCurveRef {
    pub(super) is_terminal_outlet: bool,
    pub(super) edge: VoronoiEdgeId,
    pub(super) points: Vec<WorldPlanePoint>,
    pub(super) longitudinal_start_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) water_width_blocks: f32,
    pub(super) valley_width_blocks: f32,
    pub(super) bed_depth_blocks: f32,
    pub(super) terminal_mouth_factor: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EstuaryFanRef {
    pub(super) segment_id: u64,
    pub(super) origin: WorldPlanePoint,
    pub(super) direction_x: f32,
    pub(super) direction_z: f32,
    pub(super) start_half_width_blocks: f32,
    pub(super) end_half_width_blocks: f32,
    pub(super) length_blocks: f32,
    pub(super) flow_hint: f32,
    pub(super) bed_depth_hint: f32,
    pub(super) water_depth_hint: f32,
}

pub(super) fn estuary_fan_half_widths_blocks(
    bed_width_blocks: f32,
    broad_valley_width_blocks: f32,
    flow_hint: f32,
) -> (f32, f32) {
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let water_width = bed_width_blocks.max(1.0);
    let valley_width = broad_valley_width_blocks.max(water_width).max(8.0);
    let start_half_width_blocks = (water_width * lerp(0.62, 0.80, flow_t))
        .max(8.0)
        .min(valley_width * 0.90);
    let water_spread = water_width * lerp(1.35, 3.60, flow_t);
    let valley_spread = valley_width * lerp(0.35, 0.68, flow_t);
    let min_spread = start_half_width_blocks * lerp(2.20, 3.35, flow_t);
    let end_half_width_blocks = (start_half_width_blocks + water_spread + valley_spread)
        .max(min_spread)
        .max(start_half_width_blocks * 2.0);

    (start_half_width_blocks, end_half_width_blocks)
}

pub(super) fn estuary_fan_length_blocks(
    bed_width_blocks: f32,
    broad_valley_width_blocks: f32,
    flow_hint: f32,
) -> f32 {
    let flow_t = smoothstep01(flow_hint.clamp(0.0, 1.0));
    let low_flow_reach_floor = 144.0 - flow_t * 40.0;
    let valley_reach = broad_valley_width_blocks.max(8.0) * (1.15 + flow_t * 1.20);
    let bed_reach = bed_width_blocks.max(1.0) * (4.5 + flow_t * 3.5);
    valley_reach
        .max(bed_reach)
        .max(low_flow_reach_floor)
        .clamp(96.0, 768.0)
}

pub(super) fn estuary_bed_depth_hint(
    bed_depth_blocks: f32,
    _terminal_segment_length_blocks: f32,
) -> f32 {
    (bed_depth_blocks / 40.0).clamp(0.0, 1.0)
}

pub(super) fn estuary_water_depth_hint(bed_depth_blocks: f32, flow_hint: f32) -> f32 {
    let flow = flow_hint.clamp(0.0, 1.0);
    let flow_t = smoothstep01(flow);
    let planned_depth_blocks = bed_depth_blocks.max(0.0);
    let minimum_depth_blocks = 1.5 + flow_t * 4.5;
    let fill_ratio = (0.70 + flow * 0.10).clamp(0.58, 0.82);
    (planned_depth_blocks.max(minimum_depth_blocks) * fill_ratio / 40.0).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BoundaryEdgeRef<'a> {
    pub(super) curve: &'a NoisyBoundaryCurve,
    pub(super) left: MacroSite,
    pub(super) right: MacroSite,
}

impl BoundaryEdgeRef<'_> {
    fn side_sample(self, position: WorldPlanePoint) -> Option<BoundarySideSample> {
        let nearest = nearest_polyline_segment(position, &self.curve.points)?;
        let matches_left_side = self.matches_left_noisy_side(position, nearest.start, nearest.end);
        Some(BoundarySideSample {
            distance: nearest.distance,
            matches_left_side,
            left: self.left,
            right: self.right,
        })
    }

    fn matches_left_noisy_side(
        self,
        position: WorldPlanePoint,
        nearest_start: WorldPlanePoint,
        nearest_end: WorldPlanePoint,
    ) -> bool {
        let anchor_start = self.curve.anchors.start;
        let anchor_end = self.curve.anchors.end;
        if squared_distance(anchor_start, anchor_end) <= f32::EPSILON {
            return segment_side_matches_left(
                position,
                nearest_start,
                nearest_end,
                self.left,
                self.right,
            );
        }

        let left_side = usable_side(
            signed_side(self.left.position, anchor_start, anchor_end),
            signed_side(self.right.position, anchor_start, anchor_end),
        );
        let sample_side = usable_side(
            signed_side(position, anchor_start, anchor_end),
            signed_side(self.right.position, anchor_start, anchor_end),
        );
        let mut matches_left = sample_side.signum() == left_side.signum();
        if point_inside_noisy_boundary_ribbon(position, &self.curve.points) {
            matches_left = !matches_left;
        }
        matches_left
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BoundarySideSample {
    pub(super) distance: f32,
    pub(super) matches_left_side: bool,
    pub(super) left: MacroSite,
    pub(super) right: MacroSite,
}

impl BoundarySideSample {
    fn primary_site(self) -> MacroSite {
        if self.matches_left_side {
            self.left
        } else {
            self.right
        }
    }

    fn secondary_site(self) -> MacroSite {
        if self.matches_left_side {
            self.right
        } else {
            self.left
        }
    }
}

fn segment_side_matches_left(
    position: WorldPlanePoint,
    start: WorldPlanePoint,
    end: WorldPlanePoint,
    left: MacroSite,
    right: MacroSite,
) -> bool {
    let left_side = usable_side(
        signed_side(left.position, start, end),
        signed_side(right.position, start, end),
    );
    let sample_side = usable_side(signed_side(position, start, end), left_side);
    sample_side.signum() == left_side.signum()
}

fn point_inside_noisy_boundary_ribbon(
    position: WorldPlanePoint,
    points: &[WorldPlanePoint],
) -> bool {
    if points.len() < 3 {
        return false;
    }

    let mut inside = false;
    for (start, end) in points
        .windows(2)
        .map(|segment| (segment[0], segment[1]))
        .chain(
            points
                .last()
                .zip(points.first())
                .map(|(end, start)| (*end, *start)),
        )
    {
        if ((start.z > position.z) != (end.z > position.z))
            && position.x < (end.x - start.x) * (position.z - start.z) / (end.z - start.z) + start.x
        {
            inside = !inside;
        }
    }
    inside
}

fn junction_owner_radius_blocks(config: MacroFieldTileConfig) -> f32 {
    (config.sample_spacing_blocks * 0.5)
        .max(0.5)
        .min(MACRO_FIELD_JUNCTION_OWNER_MAX_RADIUS_BLOCKS)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct OwnerSample {
    pub(super) primary: Option<MacroSite>,
    pub(super) macro_elevation: f32,
    pub(super) lake_lowering_factor: f32,
}

impl Default for OwnerSample {
    fn default() -> Self {
        Self {
            primary: None,
            macro_elevation: 0.0,
            lake_lowering_factor: 0.0,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub(super) struct CurveIndexGrid {
    pub(super) buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl CurveIndexGrid {
    fn from_curves(curves: &[&NoisyBoundaryCurve]) -> Self {
        let mut grid = Self::default();
        for (index, curve) in curves.iter().enumerate() {
            grid.insert_curve(index, &curve.points);
        }
        grid.dedup_bucket_entries();
        grid
    }

    fn from_boundary_edges(edges: &[BoundaryEdgeRef<'_>]) -> Self {
        let mut grid = Self::default();
        for (index, edge) in edges.iter().enumerate() {
            grid.insert_curve(index, &edge.curve.points);
        }
        grid.dedup_bucket_entries();
        grid
    }

    fn from_river_curves(curves: &[RiverCurveRef]) -> Self {
        let mut grid = Self::default();
        for (index, curve) in curves.iter().enumerate() {
            grid.insert_curve(index, &curve.points);
        }
        grid.dedup_bucket_entries();
        grid
    }

    fn insert_curve(&mut self, index: usize, points: &[WorldPlanePoint]) {
        for point in points {
            self.insert_point(index, *point);
        }
        for segment in points.windows(2) {
            let start = segment[0];
            let end = segment[1];
            let distance = squared_distance(start, end).sqrt();
            let steps = (distance / (MACRO_FIELD_CURVE_BUCKET_BLOCKS * 0.5))
                .ceil()
                .max(1.0) as usize;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                self.insert_point(
                    index,
                    WorldPlanePoint::new(
                        start.x + (end.x - start.x) * t,
                        start.z + (end.z - start.z) * t,
                    ),
                );
            }
        }
    }

    fn insert_point(&mut self, index: usize, point: WorldPlanePoint) {
        self.buckets
            .entry(curve_bucket(point))
            .or_default()
            .push(index);
    }

    fn dedup_bucket_entries(&mut self) {
        for indices in self.buckets.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
    }

    pub(super) fn candidate_indices(&self, position: WorldPlanePoint, radius: f32) -> Vec<usize> {
        if self.buckets.is_empty() {
            return Vec::new();
        }
        let center = curve_bucket(position);
        let search = curve_bucket_search_radius(radius).max(1);
        let mut indices = Vec::new();
        for z in center.1 - search..=center.1 + search {
            for x in center.0 - search..=center.0 + search {
                if let Some(bucket) = self.buckets.get(&(x, z)) {
                    indices.extend(bucket.iter().copied());
                }
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct JunctionIndexGrid {
    pub(super) buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl JunctionIndexGrid {
    fn from_junctions(junctions: &[BoundaryJunction]) -> Self {
        let mut grid = Self::default();
        for (index, junction) in junctions.iter().enumerate() {
            let radius = junction.radius_blocks.max(0.0);
            let min = curve_bucket(WorldPlanePoint::new(
                junction.position.x - radius,
                junction.position.z - radius,
            ));
            let max = curve_bucket(WorldPlanePoint::new(
                junction.position.x + radius,
                junction.position.z + radius,
            ));
            for z in min.1..=max.1 {
                for x in min.0..=max.0 {
                    grid.buckets.entry((x, z)).or_default().push(index);
                }
            }
        }
        for indices in grid.buckets.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }
        grid
    }

    fn candidate_indices(&self, position: WorldPlanePoint) -> Vec<usize> {
        let mut indices = self
            .buckets
            .get(&curve_bucket(position))
            .cloned()
            .unwrap_or_default();
        indices.sort_unstable();
        indices.dedup();
        indices
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SiteIndexGrid {
    pub(super) buckets: HashMap<(i32, i32), Vec<usize>>,
}

impl SiteIndexGrid {
    fn from_sites(sites: &[MacroSite]) -> Self {
        let mut grid = Self::default();
        for (index, site) in sites.iter().enumerate() {
            grid.buckets
                .entry(site_bucket(site.position))
                .or_default()
                .push(index);
        }
        grid
    }

    // Each site is inserted into exactly one bucket, so callers can traverse the
    // bucket window directly. CurveIndexGrid still sorts/dedups because curves
    // are inserted into every bucket touched by their sampled polyline.
    fn for_each_nearest_candidate_index(
        &self,
        position: WorldPlanePoint,
        mut visit: impl FnMut(usize),
    ) {
        let center = site_bucket(position);
        for search in 0..=4 {
            for z in center.1 - search..=center.1 + search {
                for x in center.0 - search..=center.0 + search {
                    if let Some(bucket) = self.buckets.get(&(x, z)) {
                        for index in bucket {
                            visit(*index);
                        }
                    }
                }
            }
        }
    }

    fn for_each_candidate_index_in_radius(
        &self,
        position: WorldPlanePoint,
        bucket_radius: i32,
        mut visit: impl FnMut(usize),
    ) {
        if self.buckets.is_empty() {
            return;
        }
        let center = site_bucket(position);
        let radius = bucket_radius.max(0);
        for z in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                if let Some(bucket) = self.buckets.get(&(x, z)) {
                    for index in bucket {
                        visit(*index);
                    }
                }
            }
        }
    }
}

pub(super) fn nearest_site_order(
    position: WorldPlanePoint,
    candidate: &MacroSite,
    current: Option<&MacroSite>,
) -> std::cmp::Ordering {
    let Some(current) = current else {
        return std::cmp::Ordering::Less;
    };
    squared_distance(position, candidate.position)
        .total_cmp(&squared_distance(position, current.position))
        .then_with(|| candidate.id.0.cmp(&current.id.0))
}

fn orient_river_curve_points_downstream(
    mut points: Vec<WorldPlanePoint>,
    from_position: WorldPlanePoint,
    to_position: WorldPlanePoint,
) -> Vec<WorldPlanePoint> {
    let (Some(first), Some(last)) = (points.first().copied(), points.last().copied()) else {
        return points;
    };
    let forward_error =
        squared_distance(first, from_position) + squared_distance(last, to_position);
    let reverse_error =
        squared_distance(first, to_position) + squared_distance(last, from_position);
    if reverse_error < forward_error {
        points.reverse();
    }
    points
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use super::*;
    use crate::world::generation::boundary::{
        BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
    };
    use crate::world::generation::graph::{
        VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId, WorldPlanePoint,
    };
    use crate::world::generation::macro_field::test_support::*;
    use crate::world::generation::macro_field::{
        MacroFieldRasterContext, MacroFieldTileConfig, generate_macro_field_tile,
        sample_macro_field_point,
    };
    use crate::world::generation::macro_map::{
        GraphMacroMap, MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
    };
    use crate::world::generation::river_plan::{
        DEFAULT_RIVER_PLAN_DOWNSTREAM_WATER_WIDTH_BLOCKS, RiverPlan,
    };

    #[test]
    fn river_curve_points_are_oriented_to_hydrology_downstream() {
        let from = WorldPlanePoint::new(0.0, 0.0);
        let to = WorldPlanePoint::new(96.0, 0.0);
        let points = vec![
            to,
            WorldPlanePoint::new(64.0, 8.0),
            WorldPlanePoint::new(32.0, -8.0),
            from,
        ];

        let oriented = orient_river_curve_points_downstream(points, from, to);

        assert_eq!(oriented.first().copied(), Some(from));
        assert_eq!(oriented.last().copied(), Some(to));
    }

    #[test]
    fn low_flow_estuary_fan_keeps_practical_downstream_reach() {
        let headwater = estuary_fan_length_blocks(2.0, 14.0, 0.02);
        let downstream = estuary_fan_length_blocks(96.0, 280.0, 0.85);

        assert!(
            headwater >= 128.0,
            "low-Q river mouths still need enough fan reach to meet nearby connected ocean: {headwater}"
        );
        assert!(
            downstream > headwater,
            "large downstream mouths should still spread farther than the low-Q minimum: headwater={headwater} downstream={downstream}"
        );
    }

    #[test]
    fn estuary_fan_width_tracks_terminal_water_width() {
        let (small_start, small_end) = estuary_fan_half_widths_blocks(12.0, 36.0, 0.20);
        let (large_start, large_end) = estuary_fan_half_widths_blocks(120.0, 280.0, 0.85);

        assert!(
            small_start * 2.0 >= 12.0,
            "small mouth fan should begin at least as wide as the planned water width: {small_start}"
        );
        assert!(
            large_start * 2.0 >= 120.0 * 1.20,
            "large mouth fan lip should not pinch narrower than the existing downstream water: {large_start}"
        );
        assert!(
            large_end > small_end * 6.0,
            "fan spread should scale with terminal water width instead of using a nearly fixed mouth: small={small_end} large={large_end}"
        );
        assert!(
            large_end > large_start * 3.0,
            "large estuary fans should visibly spread downstream: start={large_start} end={large_end}"
        );
    }

    #[test]
    fn estuary_depth_hint_preserves_terminal_bed_context() {
        let short = estuary_bed_depth_hint(32.0, 24.0);
        let long = estuary_bed_depth_hint(32.0, 256.0);

        assert!(
            (short - long).abs() <= f32::EPSILON,
            "estuary fan slope is height-profile responsibility; the depth hint should keep terminal river context: short={short} long={long}"
        );
        assert!(
            long > 0.75,
            "long final segments keep the planned river-mouth bed depth hint: {long}"
        );
    }

    #[test]
    fn estuary_water_depth_hint_keeps_terminal_water_depth_separate_from_bed_carve() {
        let bed = estuary_bed_depth_hint(32.0, 24.0);
        let water = estuary_water_depth_hint(32.0, 0.82);

        assert!(
            water > 0.0 && water < bed,
            "water continuation should remain a separate filled-surface depth hint above the terminal bed: bed={bed} water={water}"
        );
        assert!(
            water < 1.0,
            "normalized estuary water depth hint must stay bounded: {water}"
        );
    }

    #[test]
    fn nearest_site_search_does_not_stop_at_first_nonempty_bucket_ring() {
        let far_ring_site = test_site(
            VoronoiSiteId(1),
            0.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.10,
        );
        let nearer_outer_ring_site = test_site(
            VoronoiSiteId(2),
            512.0,
            256.0,
            MacroSurfaceKind::Continent,
            0.20,
        );
        let macro_map = GraphMacroMap {
            sites: vec![far_ring_site, nearer_outer_ring_site],
            corners: Vec::new(),
            edges: Vec::new(),
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: Vec::new(),
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let position = WorldPlanePoint::new(255.0, 255.0);

        let nearest = context
            .nearest_site(position)
            .expect("test macro map should have sites");

        assert_eq!(
            nearest.id, nearer_outer_ring_site.id,
            "nearest-site lookup must consider all nearby bucket rings, otherwise 256-block site bucket boundaries become visible owner/mask seams"
        );
    }

    #[test]
    fn macro_field_owner_sampling_follows_noisy_boundary_curve() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let right = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::OceanBasin,
            -0.8,
        );
        let edge = VoronoiEdgeId(7);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Coast,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [left.id, right.id],
                start,
                end,
            },
            points: vec![start, WorldPlanePoint::new(5.0, 0.0), end],
            amplitude: 5.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -20.0,
                max_x: 20.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![left, right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [left.id, right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 1.6,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(4.0, 0.0));

        assert_eq!(
            sample.nearest_site,
            Some(left.id),
            "point is closer to the right site in straight Voronoi space, but the bowed noisy curve should classify it on the left side"
        );
        assert_eq!(sample.surface_kind, Some(MacroSurfaceKind::Continent));
        assert!(sample.coast_mask > 0.5);
    }

    #[test]
    fn macro_field_owner_sampling_follows_noisy_boundary_displacement_outside_blend_radius() {
        use crate::world::generation::biome::{
            GraphBiomeCell, GraphBiomeContext, GraphBiomeKind, GraphBiomeWaterRole,
        };
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let right = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.4,
        );
        let edge = VoronoiEdgeId(107);
        let start = WorldPlanePoint::new(0.0, -1000.0);
        let bend = WorldPlanePoint::new(160.0, 0.0);
        let end = WorldPlanePoint::new(0.0, 1000.0);
        let left_context = GraphBiomeContext {
            temperature: 0.55,
            hydration: 0.50,
            elevation: 0.0,
            continentality: 0.2,
            coastness: 0.0,
            mountainness: 0.0,
            ruggedness: 0.0,
            water_role: GraphBiomeWaterRole::Land,
        };
        let right_context = GraphBiomeContext {
            hydration: 0.20,
            ..left_context
        };
        let macro_map = GraphMacroMap {
            sites: vec![left, right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [left.id, right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.4,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: vec![
                GraphBiomeCell {
                    site: left.id,
                    context: left_context,
                    biome: GraphBiomeKind::TemperateGrassland,
                },
                GraphBiomeCell {
                    site: right.id,
                    context: right_context,
                    biome: GraphBiomeKind::SemiDesert,
                },
            ],
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge,
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [left.id, right.id],
                    start,
                    end,
                },
                points: vec![start, bend, end],
                amplitude: 160.0,
                seed: 1,
                guard: BoundaryGuard {
                    min_x: -8.0,
                    max_x: 168.0,
                    min_z: -1008.0,
                    max_z: 1008.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(20.0, 0.0));

        assert!(
            sample.position.x > 0.0,
            "test point should be on the raw nearest-site side of the right site"
        );
        assert_eq!(
            sample.nearest_site,
            Some(left.id),
            "owner and biome context should follow the visible noisy boundary, not the raw nearest-site line"
        );
        assert_eq!(sample.biome, Some(GraphBiomeKind::TemperateGrassland));
        assert_eq!(sample.biome_context, Some(left_context));
    }

    #[test]
    fn nearest_noisy_boundary_side_can_select_non_raw_owner() {
        let raw_owner = test_site(
            VoronoiSiteId(1),
            2.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.40,
        );
        let distant_left = test_site(
            VoronoiSiteId(2),
            -1000.0,
            -100.0,
            MacroSurfaceKind::DryBasin,
            -0.10,
        );
        let distant_right = test_site(
            VoronoiSiteId(3),
            1000.0,
            100.0,
            MacroSurfaceKind::CoastOcean,
            -0.70,
        );
        let unrelated_edge = VoronoiEdgeId(141);
        let start = WorldPlanePoint::new(0.0, -64.0);
        let end = WorldPlanePoint::new(0.0, 64.0);
        let macro_map = GraphMacroMap {
            sites: vec![raw_owner, distant_left, distant_right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: unrelated_edge,
                sites: [distant_left.id, distant_right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.60,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge: unrelated_edge,
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [distant_left.id, distant_right.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: 141,
                guard: BoundaryGuard {
                    min_x: -8.0,
                    max_x: 8.0,
                    min_z: -72.0,
                    max_z: 72.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 16.0;

        let sample = context.owner_sample(WorldPlanePoint::new(1.0, 0.0), config);

        assert_eq!(
            sample.primary.map(|site| site.id),
            Some(distant_right.id),
            "global owner sampling should follow the nearest noisy boundary side even when the raw nearest site differs"
        );
        assert!(
            sample.lake_lowering_factor <= f32::EPSILON,
            "ordinary non-lake boundary ownership must not leak lake lowering side effects"
        );
    }

    #[test]
    fn nearest_noisy_boundary_side_selection_keeps_scalar_elevation_continuous() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let primary = test_site(
            VoronoiSiteId(1),
            -100.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.02,
        );
        let coastland_secondary = test_site(
            VoronoiSiteId(2),
            100.0,
            -10.0,
            MacroSurfaceKind::CoastLand,
            -0.20,
        );
        let continent_secondary = test_site(
            VoronoiSiteId(3),
            100.0,
            10.0,
            MacroSurfaceKind::Continent,
            0.80,
        );
        let edge_a = VoronoiEdgeId(81);
        let edge_b = VoronoiEdgeId(82);
        let start_a = WorldPlanePoint::new(0.0, -32.0);
        let end_a = WorldPlanePoint::new(0.0, 32.0);
        let start_b = WorldPlanePoint::new(1.0, -32.0);
        let end_b = WorldPlanePoint::new(1.0, 32.0);
        let boundary = BoundaryCache {
            curves: vec![
                NoisyBoundaryCurve {
                    edge: edge_a,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                        sites: [primary.id, coastland_secondary.id],
                        start: start_a,
                        end: end_a,
                    },
                    points: vec![start_a, end_a],
                    amplitude: 0.0,
                    seed: 1,
                    guard: BoundaryGuard {
                        min_x: -8.0,
                        max_x: 8.0,
                        min_z: -40.0,
                        max_z: 40.0,
                    },
                },
                NoisyBoundaryCurve {
                    edge: edge_b,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                        sites: [primary.id, continent_secondary.id],
                        start: start_b,
                        end: end_b,
                    },
                    points: vec![start_b, end_b],
                    amplitude: 0.0,
                    seed: 2,
                    guard: BoundaryGuard {
                        min_x: -8.0,
                        max_x: 8.0,
                        min_z: -40.0,
                        max_z: 40.0,
                    },
                },
            ],
            stats: Default::default(),
        };
        let macro_map = GraphMacroMap {
            sites: vec![primary, coastland_secondary, continent_secondary],
            corners: Vec::new(),
            edges: vec![
                MacroEdge {
                    id: edge_a,
                    sites: [primary.id, coastland_secondary.id],
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.22,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
                MacroEdge {
                    id: edge_b,
                    sites: [primary.id, continent_secondary.id],
                    corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.78,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
            ],
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 8.0;
        let left_position = WorldPlanePoint::new(0.9, 0.0);
        let right_position = WorldPlanePoint::new(1.1, 0.0);

        let left = context.owner_sample(left_position, config);
        let right = context.owner_sample(right_position, config);

        assert_eq!(left.primary.map(|site| site.id), Some(primary.id));
        assert_eq!(
            right.primary.map(|site| site.id),
            Some(continent_secondary.id),
            "owner selection should follow the closest noisy boundary side, while scalar elevation remains interpolated"
        );
        let low = coastland_secondary
            .signed_macro_elevation
            .min(primary.signed_macro_elevation)
            .min(continent_secondary.signed_macro_elevation);
        let high = coastland_secondary
            .signed_macro_elevation
            .max(primary.signed_macro_elevation)
            .max(continent_secondary.signed_macro_elevation);
        assert!(
            (low..=high).contains(&left.macro_elevation),
            "ordinary coastland/land secondary selection must stay inside macro_map source range: {} not in {low}..={high}",
            left.macro_elevation
        );
        assert!(
            (low..=high).contains(&right.macro_elevation),
            "ordinary land/land secondary selection must stay inside macro_map source range: {} not in {low}..={high}",
            right.macro_elevation
        );
        assert!(
            (left.macro_elevation - right.macro_elevation).abs() < 0.005,
            "adjacent noisy-boundary side samples should stay continuous, left={} right={}",
            left.macro_elevation,
            right.macro_elevation
        );
    }

    #[test]
    fn junction_owner_uses_nearest_incident_site_before_boundary_side() {
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.10,
        );
        let right = test_site(VoronoiSiteId(2), 20.0, 0.0, MacroSurfaceKind::Island, 0.20);
        let corner_site = test_site(VoronoiSiteId(3), 1.0, 3.0, MacroSurfaceKind::DryBasin, 0.30);
        let shared_corner = VoronoiCornerId(100);
        let curves = vec![
            test_junction_curve(
                VoronoiEdgeId(10),
                shared_corner,
                VoronoiCornerId(101),
                [left.id, right.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(0.0, 100.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(11),
                shared_corner,
                VoronoiCornerId(102),
                [right.id, corner_site.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(80.0, -20.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(12),
                shared_corner,
                VoronoiCornerId(103),
                [corner_site.id, left.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(-80.0, -20.0),
            ),
        ];
        let macro_map = GraphMacroMap {
            sites: vec![left, right, corner_site],
            corners: Vec::new(),
            edges: curves
                .iter()
                .map(|curve| MacroEdge {
                    id: curve.edge,
                    sites: curve.anchors.sites,
                    corners: curve.anchors.corners,
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.0,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                })
                .collect(),
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves,
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let near_junction = context.owner_sample(WorldPlanePoint::new(1.0, 4.0), config);
        let outside_junction = context.owner_sample(WorldPlanePoint::new(1.0, 80.0), config);

        assert_eq!(
            near_junction.primary.map(|site| site.id),
            Some(corner_site.id),
            "inside the rounded junction radius, owner should be nearest incident site"
        );
        assert_eq!(
            outside_junction.primary.map(|site| site.id),
            Some(right.id),
            "outside the sample-scale junction radius, owner selection should fall back to nearest noisy boundary side"
        );
    }

    #[test]
    fn boundary_blend_radius_does_not_limit_global_noisy_owner_side() {
        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.30,
        );
        let right = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::DryBasin,
            0.10,
        );
        let edge = VoronoiEdgeId(180);
        let start = WorldPlanePoint::new(16.0, -64.0);
        let end = WorldPlanePoint::new(16.0, 64.0);
        let macro_map = GraphMacroMap {
            sites: vec![left, right],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [left.id, right.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.20,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge,
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [left.id, right.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 8.0,
                seed: 180,
                guard: BoundaryGuard {
                    min_x: -8.0,
                    max_x: 24.0,
                    min_z: -72.0,
                    max_z: 72.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = MacroFieldTileConfig::new(0.0, 0.0, 4, 4, 1.0);
        config.boundary_blend_radius_blocks = 96.0;

        let sample = context.owner_sample(WorldPlanePoint::new(4.0, 0.0), config);

        assert_eq!(
            sample.primary.map(|site| site.id),
            Some(left.id),
            "owner selection must follow the nearest noisy boundary side; boundary_blend_radius_blocks only controls local transition effects"
        );
        assert!(
            sample.lake_lowering_factor <= f32::EPSILON,
            "blend radius must not create lake lowering for a non-lake owner pair"
        );
    }

    #[test]
    fn junction_owner_selection_is_not_limited_by_raw_nearest_site() {
        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.10,
        );
        let right = test_site(VoronoiSiteId(2), 20.0, 0.0, MacroSurfaceKind::Island, 0.20);
        let corner_site = test_site(VoronoiSiteId(3), 1.0, 3.0, MacroSurfaceKind::DryBasin, 0.30);
        let outsider = test_site(
            VoronoiSiteId(4),
            1.0,
            4.0,
            MacroSurfaceKind::Continent,
            0.40,
        );
        let shared_corner = VoronoiCornerId(200);
        let curves = vec![
            test_junction_curve(
                VoronoiEdgeId(20),
                shared_corner,
                VoronoiCornerId(201),
                [left.id, right.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(0.0, 100.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(21),
                shared_corner,
                VoronoiCornerId(202),
                [right.id, corner_site.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(80.0, -20.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(22),
                shared_corner,
                VoronoiCornerId(203),
                [corner_site.id, left.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(-80.0, -20.0),
            ),
        ];
        let macro_map = GraphMacroMap {
            sites: vec![left, right, corner_site, outsider],
            corners: Vec::new(),
            edges: curves
                .iter()
                .map(|curve| MacroEdge {
                    id: curve.edge,
                    sites: curve.anchors.sites,
                    corners: curve.anchors.corners,
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.0,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                })
                .collect(),
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves,
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = test_tile_config();

        let sample = context.owner_sample(outsider.position, config);

        assert_eq!(
            sample.primary.map(|site| site.id),
            Some(corner_site.id),
            "inside junction support, global junction owner selection uses the nearest incident macro site even when raw nearest differs"
        );
    }

    #[test]
    fn junction_owner_selection_has_sample_scale_support() {
        let left = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.10,
        );
        let right = test_site(VoronoiSiteId(2), 20.0, 0.0, MacroSurfaceKind::Island, 0.20);
        let corner_site = test_site(VoronoiSiteId(3), 1.0, 3.0, MacroSurfaceKind::DryBasin, 0.30);
        let shared_corner = VoronoiCornerId(300);
        let curves = vec![
            test_junction_curve(
                VoronoiEdgeId(30),
                shared_corner,
                VoronoiCornerId(301),
                [left.id, right.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(0.0, 100.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(31),
                shared_corner,
                VoronoiCornerId(302),
                [right.id, corner_site.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(80.0, -20.0),
            ),
            test_junction_curve(
                VoronoiEdgeId(32),
                shared_corner,
                VoronoiCornerId(303),
                [corner_site.id, left.id],
                WorldPlanePoint::new(0.0, 0.0),
                WorldPlanePoint::new(-80.0, -20.0),
            ),
        ];
        let macro_map = GraphMacroMap {
            sites: vec![left, right, corner_site],
            corners: Vec::new(),
            edges: curves
                .iter()
                .map(|curve| MacroEdge {
                    id: curve.edge,
                    sites: curve.anchors.sites,
                    corners: curve.anchors.corners,
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.0,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                })
                .collect(),
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves,
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = MacroFieldTileConfig::new(0.0, 0.0, 8, 8, 1.0);

        let near_junction = context.owner_sample(WorldPlanePoint::new(-0.25, 0.0), config);
        let outside_sample_scale = context.owner_sample(WorldPlanePoint::new(-8.0, 40.0), config);

        assert_eq!(
            near_junction.primary.map(|site| site.id),
            Some(corner_site.id),
            "one-block preview samples directly at a junction can still use rounded incident-site owner selection"
        );
        assert_eq!(
            outside_sample_scale.primary.map(|site| site.id),
            Some(left.id),
            "outside sample-scale junction support, owner selection falls back to nearest noisy boundary side"
        );
    }

    #[test]
    fn macro_field_scalar_interpolates_between_macro_map_sites() {
        use crate::world::generation::graph::VoronoiSiteId;
        use crate::world::generation::macro_map::MacroSurfaceKind;

        let low_site = test_site(
            VoronoiSiteId(101),
            -128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.0,
        );
        let high_site = test_site(
            VoronoiSiteId(102),
            128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let macro_map = GraphMacroMap {
            sites: vec![low_site, high_site],
            corners: Vec::new(),
            edges: Vec::new(),
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let boundary = BoundaryCache {
            curves: Vec::new(),
            stats: Default::default(),
        };
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = test_tile_config();

        let low_sample = sample_macro_field_point(&context, config, low_site.position);
        let center_sample =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(0.0, 0.0));
        let high_sample = sample_macro_field_point(&context, config, high_site.position);

        assert!(
            (low_sample.macro_elevation - low_site.signed_macro_elevation).abs() <= f32::EPSILON,
            "source site center should preserve its macro_map elevation"
        );
        assert!(
            (high_sample.macro_elevation - high_site.signed_macro_elevation).abs() <= f32::EPSILON,
            "source site center should preserve its macro_map elevation"
        );
        assert!(
            center_sample.macro_elevation > low_site.signed_macro_elevation
                && center_sample.macro_elevation < high_site.signed_macro_elevation,
            "between-site scalar should be an interpolated contour-bearing value, got {}",
            center_sample.macro_elevation
        );
    }

    #[test]
    fn coast_boundary_preserves_owner_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let land = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let ocean = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::OceanBasin,
            -0.8,
        );
        let edge = VoronoiEdgeId(71);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Coast,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [land.id, ocean.id],
                start,
                end,
            },
            points: vec![start, end],
            amplitude: 0.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -48.0,
                max_x: 48.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![land, ocean],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [land.id, ocean.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 1.6,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let shoreline_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        let recovered_land =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-32.0, 0.0));
        let shoreline_ocean =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(1.0, 0.0));

        let low = land
            .signed_macro_elevation
            .min(ocean.signed_macro_elevation);
        let high = land
            .signed_macro_elevation
            .max(ocean.signed_macro_elevation);
        assert!(
            (low..=high).contains(&shoreline_land.macro_elevation),
            "land-side coast scalar should interpolate macro_map sources without creating a new profile: {} not in {low}..={high}",
            shoreline_land.macro_elevation
        );
        assert!(
            recovered_land.macro_elevation > shoreline_land.macro_elevation,
            "land sample farther from ocean should recover through local source interpolation: shoreline={} recovered={}",
            shoreline_land.macro_elevation,
            recovered_land.macro_elevation
        );
        assert!(
            (low..=high).contains(&shoreline_ocean.macro_elevation),
            "ocean side scalar should stay within macro_map source range, got {} not in {low}..={high}",
            shoreline_ocean.macro_elevation
        );
        assert!(
            shoreline_land.coast_mask > 0.0 && shoreline_ocean.coast_mask > 0.0,
            "explicit coast guide influence should keep coast mask data intact"
        );
    }

    #[test]
    fn nearby_ordinary_boundary_does_not_override_coast_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let land = test_site(
            VoronoiSiteId(1),
            -2.0,
            -10.0,
            MacroSurfaceKind::CoastLand,
            0.8,
        );
        let ocean = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::CoastOcean,
            -0.8,
        );
        let inland = test_site(
            VoronoiSiteId(3),
            -2.0,
            20.0,
            MacroSurfaceKind::Continent,
            0.8,
        );
        let coast_edge = VoronoiEdgeId(71);
        let ordinary_edge = VoronoiEdgeId(72);
        let coast_start = WorldPlanePoint::new(0.0, -10.0);
        let coast_end = WorldPlanePoint::new(0.0, 10.0);
        let ordinary_start = WorldPlanePoint::new(-20.0, 0.05);
        let ordinary_end = WorldPlanePoint::new(0.0, 0.05);
        let macro_map = GraphMacroMap {
            sites: vec![land, ocean, inland],
            corners: Vec::new(),
            edges: vec![
                MacroEdge {
                    id: coast_edge,
                    sites: [land.id, ocean.id],
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    guide: MacroEdgeGuide {
                        is_coast: true,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 1.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 1.6,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
                MacroEdge {
                    id: ordinary_edge,
                    sites: [land.id, inland.id],
                    corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                    guide: MacroEdgeGuide {
                        is_coast: false,
                        is_ridge_candidate: false,
                        is_river_candidate: false,
                        is_fault_candidate: false,
                        coastness: 0.0,
                        mountainness: 0.0,
                        ridgeness: 0.0,
                        signed_elevation_gradient: 0.0,
                        drainage_divide_potential: 0.0,
                        river_potential: 0.0,
                    },
                    lake_class: MacroLakeEdgeClass::NonLake,
                },
            ],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![
                NoisyBoundaryCurve {
                    edge: coast_edge,
                    profile: BoundaryProfile::Coast,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                        sites: [land.id, ocean.id],
                        start: coast_start,
                        end: coast_end,
                    },
                    points: vec![coast_start, coast_end],
                    amplitude: 0.0,
                    seed: 1,
                    guard: BoundaryGuard {
                        min_x: -48.0,
                        max_x: 48.0,
                        min_z: -20.0,
                        max_z: 20.0,
                    },
                },
                NoisyBoundaryCurve {
                    edge: ordinary_edge,
                    profile: BoundaryProfile::Ordinary,
                    anchors: BoundaryAnchors {
                        corners: [VoronoiCornerId(3), VoronoiCornerId(4)],
                        sites: [land.id, inland.id],
                        start: ordinary_start,
                        end: ordinary_end,
                    },
                    points: vec![ordinary_start, ordinary_end],
                    amplitude: 0.0,
                    seed: 2,
                    guard: BoundaryGuard {
                        min_x: -24.0,
                        max_x: 0.0,
                        min_z: -20.0,
                        max_z: 24.0,
                    },
                },
            ],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let near_ordinary_and_coast =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        assert_eq!(near_ordinary_and_coast.nearest_site, Some(land.id));
        assert!(
            (ocean.signed_macro_elevation..=land.signed_macro_elevation)
                .contains(&near_ordinary_and_coast.macro_elevation),
            "ordinary/coast boundary proximity should interpolate only existing source elevations, got {}",
            near_ordinary_and_coast.macro_elevation
        );
    }

    #[test]
    fn coast_adjacent_owner_samples_preserve_macro_map_source_elevation() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let ocean = test_site(
            VoronoiSiteId(1),
            96.0,
            0.0,
            MacroSurfaceKind::CoastOcean,
            -0.36,
        );
        let coast = test_site(
            VoronoiSiteId(2),
            -48.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.045,
        );
        let inland_a = test_site(
            VoronoiSiteId(3),
            -256.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.44,
        );
        let inland_b = test_site(
            VoronoiSiteId(4),
            -544.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.58,
        );
        let inland_c = test_site(
            VoronoiSiteId(5),
            -864.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.39,
        );
        let edge = VoronoiEdgeId(90);
        let start = WorldPlanePoint::new(0.0, -1024.0);
        let end = WorldPlanePoint::new(0.0, 1024.0);
        let macro_map = GraphMacroMap {
            sites: vec![ocean, coast, inland_a, inland_b, inland_c],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [coast.id, ocean.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: true,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 1.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.405,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge,
                profile: BoundaryProfile::Coast,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [coast.id, ocean.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: 7,
                guard: BoundaryGuard {
                    min_x: -128.0,
                    max_x: 128.0,
                    min_z: -1024.0,
                    max_z: 1024.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 96.0;
        let coast_x = [-8.0, -16.0, -24.0, -32.0, -40.0, -48.0, -56.0, -64.0];
        for x in coast_x {
            let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(x, 0.0));
            assert_eq!(sample.nearest_site, Some(coast.id));
            let source_min = ocean
                .signed_macro_elevation
                .min(coast.signed_macro_elevation);
            let source_max = inland_b
                .signed_macro_elevation
                .max(coast.signed_macro_elevation);
            assert!(
                (source_min..=source_max).contains(&sample.macro_elevation),
                "coast owner sample should remain within macro_map source elevation range at x={x}: {} not in {source_min}..={source_max}",
                sample.macro_elevation
            );
            assert!(
                (sample.combined_macro_height - sample.macro_elevation).abs() <= f32::EPSILON,
                "coast owner sample without river/lake should not get a macro_field height override"
            );
        }
    }

    #[test]
    fn coastland_continent_scalar_preserves_each_owner_source() {
        use crate::world::generation::graph::VoronoiSiteId;
        use crate::world::generation::macro_map::MacroSurfaceKind;

        let coast = test_site(
            VoronoiSiteId(31),
            0.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.04,
        );
        let inland = test_site(
            VoronoiSiteId(32),
            -128.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.80,
        );
        let macro_map = GraphMacroMap {
            sites: vec![coast, inland],
            corners: Vec::new(),
            edges: Vec::new(),
            biomes: Vec::new(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let boundary = BoundaryCache {
            curves: Vec::new(),
            stats: Default::default(),
        };
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let config = test_tile_config();
        let coast_sample = sample_macro_field_point(&context, config, coast.position);
        let inland_sample = sample_macro_field_point(&context, config, inland.position);

        assert_eq!(coast_sample.nearest_site, Some(coast.id));
        assert_eq!(inland_sample.nearest_site, Some(inland.id));
        assert!(
            (coast_sample.macro_elevation - coast.signed_macro_elevation).abs() <= f32::EPSILON,
            "coastland sample should preserve macro_map source elevation: sample={} source={}",
            coast_sample.macro_elevation,
            coast.signed_macro_elevation
        );
        assert!(
            (inland_sample.macro_elevation - inland.signed_macro_elevation).abs() <= f32::EPSILON,
            "continent sample should preserve macro_map source elevation: sample={} source={}",
            inland_sample.macro_elevation,
            inland.signed_macro_elevation
        );
    }

    #[test]
    fn coastland_continent_boundary_owner_switch_preserves_owner_sources() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let coast = test_site(
            VoronoiSiteId(41),
            -64.0,
            0.0,
            MacroSurfaceKind::CoastLand,
            0.045,
        );
        let inland = test_site(
            VoronoiSiteId(42),
            64.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.52,
        );
        let edge = VoronoiEdgeId(91);
        let start = WorldPlanePoint::new(0.0, -256.0);
        let end = WorldPlanePoint::new(0.0, 256.0);
        let macro_map = GraphMacroMap {
            sites: vec![coast, inland],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [coast.id, inland.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.475,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![NoisyBoundaryCurve {
                edge,
                profile: BoundaryProfile::Ordinary,
                anchors: BoundaryAnchors {
                    corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                    sites: [coast.id, inland.id],
                    start,
                    end,
                },
                points: vec![start, end],
                amplitude: 0.0,
                seed: 9,
                guard: BoundaryGuard {
                    min_x: -96.0,
                    max_x: 96.0,
                    min_z: -256.0,
                    max_z: 256.0,
                },
            }],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 48.0;

        let coast_side =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(-1.0, 0.0));
        let inland_side =
            sample_macro_field_point(&context, config, WorldPlanePoint::new(1.0, 0.0));

        assert_eq!(coast_side.nearest_site, Some(coast.id));
        assert_eq!(inland_side.nearest_site, Some(inland.id));
        assert!(
            (coast.signed_macro_elevation..=inland.signed_macro_elevation)
                .contains(&coast_side.macro_elevation),
            "coastland side should interpolate macro_map source elevation range: {}",
            coast_side.macro_elevation
        );
        assert!(
            (coast.signed_macro_elevation..=inland.signed_macro_elevation)
                .contains(&inland_side.macro_elevation),
            "continent side should interpolate macro_map source elevation range: {}",
            inland_side.macro_elevation
        );
        assert!(
            (coast_side.macro_elevation - inland_side.macro_elevation).abs() < 0.02,
            "ordinary boundary scalar should stay continuous across owner switch: coast_side={} inland_side={}",
            coast_side.macro_elevation,
            inland_side.macro_elevation
        );
    }

    #[test]
    fn dry_basin_boundary_blend_does_not_create_coast_mask() {
        use crate::world::generation::boundary::{
            BoundaryAnchors, BoundaryGuard, BoundaryProfile, NoisyBoundaryCurve,
        };
        use crate::world::generation::graph::{VoronoiCornerId, VoronoiEdgeId, VoronoiSiteId};
        use crate::world::generation::macro_map::{
            MacroEdge, MacroEdgeGuide, MacroLakeEdgeClass, MacroSurfaceKind,
        };

        let dry = test_site(
            VoronoiSiteId(1),
            -10.0,
            0.0,
            MacroSurfaceKind::DryBasin,
            0.08,
        );
        let land = test_site(
            VoronoiSiteId(2),
            10.0,
            0.0,
            MacroSurfaceKind::Continent,
            0.22,
        );
        let edge = VoronoiEdgeId(17);
        let start = WorldPlanePoint::new(0.0, -10.0);
        let end = WorldPlanePoint::new(0.0, 10.0);
        let curve = NoisyBoundaryCurve {
            edge,
            profile: BoundaryProfile::Ordinary,
            anchors: BoundaryAnchors {
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                sites: [dry.id, land.id],
                start,
                end,
            },
            points: vec![start, end],
            amplitude: 0.0,
            seed: 1,
            guard: BoundaryGuard {
                min_x: -20.0,
                max_x: 20.0,
                min_z: -20.0,
                max_z: 20.0,
            },
        };
        let macro_map = GraphMacroMap {
            sites: vec![dry, land],
            corners: Vec::new(),
            edges: vec![MacroEdge {
                id: edge,
                sites: [dry.id, land.id],
                corners: [VoronoiCornerId(1), VoronoiCornerId(2)],
                guide: MacroEdgeGuide {
                    is_coast: false,
                    is_ridge_candidate: false,
                    is_river_candidate: false,
                    is_fault_candidate: false,
                    coastness: 0.0,
                    mountainness: 0.0,
                    ridgeness: 0.0,
                    signed_elevation_gradient: 0.14,
                    drainage_divide_potential: 0.0,
                    river_potential: 0.0,
                },
                lake_class: MacroLakeEdgeClass::NonLake,
            }],
            biomes: Vec::new(),
        };
        let boundary = BoundaryCache {
            curves: vec![curve],
            stats: Default::default(),
        };
        let patch = Default::default();
        let river_plan = RiverPlan::default();
        let context = MacroFieldRasterContext::new(&patch, &macro_map, &river_plan, &boundary);
        let mut config = test_tile_config();
        config.boundary_blend_radius_blocks = 24.0;

        let sample = sample_macro_field_point(&context, config, WorldPlanePoint::new(0.0, 0.0));

        assert!(
            sample.coast_mask <= f32::EPSILON,
            "dry basin / land noisy boundary blend must not render as coast: {}",
            sample.coast_mask
        );
    }

    #[test]
    fn macro_field_sample_carries_nearest_site_biome() {
        let inputs = test_inputs(42);
        let context = MacroFieldRasterContext::new(
            &inputs.patch,
            &inputs.macro_map,
            &inputs.river_plan,
            &inputs.boundary,
        );
        let site = inputs
            .macro_map
            .sites
            .iter()
            .find(|site| inputs.macro_map.biome(site.id).is_some())
            .expect("generated macro map should expose biome cells");
        let sample = sample_macro_field_point(&context, test_tile_config(), site.position);
        let expected = inputs
            .macro_map
            .biome(site.id)
            .expect("site biome should be stable");

        assert_eq!(sample.nearest_site, Some(site.id));
        assert_eq!(sample.biome, Some(expected.biome));
        assert_eq!(sample.biome_context, Some(expected.context));
    }
}
