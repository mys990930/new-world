pub const DEFAULT_GRAPH_REGION_SIZE_BLOCKS: i32 = 1024;
pub const DEFAULT_SITE_SPACING_BLOCKS: i32 = 192;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiSiteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiCornerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoronoiEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoronoiSite {
    pub id: VoronoiSiteId,
    pub owner_region: GraphRegionCoord,
    pub position: WorldPlanePoint,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
