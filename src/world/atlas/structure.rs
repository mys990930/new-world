use crate::world::WorldMeta;

use super::scale::AtlasArea;
use super::tuning::AtlasTuning;
use super::AtlasCoord;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountainChainId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountainChainScale {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountainSpineSegment {
    pub chain_id: MountainChainId,
    pub branch_order: u8,
    pub scale: MountainChainScale,
    pub start: AtlasCoord,
    pub end: AtlasCoord,
    pub strength: f32,
    pub half_width_cells: f32,
}

impl MountainSpineSegment {
    pub fn touches_area(self, area: AtlasArea) -> bool {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        let min_z = self.start.z.min(self.end.z);
        let max_z = self.start.z.max(self.end.z);
        let area_origin = area.origin();
        let area_max_x = area_origin.x + area.width() as i32 - 1;
        let area_max_z = area_origin.z + area.height() as i32 - 1;

        max_x >= area_origin.x && min_x <= area_max_x && max_z >= area_origin.z && min_z <= area_max_z
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MountainChainGraph {
    segments: Vec<MountainSpineSegment>,
}

impl MountainChainGraph {
    pub fn segments(&self) -> &[MountainSpineSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn push_segment(&mut self, segment: MountainSpineSegment) {
        self.segments.push(segment);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RiverPathId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrainageNodeKind {
    Headwater,
    Confluence,
    Outlet,
    Sink,
    PassOutlet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainageNode {
    pub coord: AtlasCoord,
    pub kind: DrainageNodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiverPathKind {
    Trunk,
    Tributary,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverPathSegment {
    pub river_id: RiverPathId,
    pub kind: RiverPathKind,
    pub order: u8,
    pub start: AtlasCoord,
    pub end: AtlasCoord,
    pub bankfull_width_cells: f32,
}

impl RiverPathSegment {
    pub fn touches_area(self, area: AtlasArea) -> bool {
        let min_x = self.start.x.min(self.end.x);
        let max_x = self.start.x.max(self.end.x);
        let min_z = self.start.z.min(self.end.z);
        let max_z = self.start.z.max(self.end.z);
        let area_origin = area.origin();
        let area_max_x = area_origin.x + area.width() as i32 - 1;
        let area_max_z = area_origin.z + area.height() as i32 - 1;

        max_x >= area_origin.x && min_x <= area_max_x && max_z >= area_origin.z && min_z <= area_max_z
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DrainageGraph {
    nodes: Vec<DrainageNode>,
    segments: Vec<RiverPathSegment>,
}

impl DrainageGraph {
    pub fn nodes(&self) -> &[DrainageNode] {
        &self.nodes
    }

    pub fn segments(&self) -> &[RiverPathSegment] {
        &self.segments
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.segments.is_empty()
    }

    pub fn push_node(&mut self, node: DrainageNode) {
        self.nodes.push(node);
    }

    pub fn push_segment(&mut self, segment: RiverPathSegment) {
        self.segments.push(segment);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtlasStructureMap {
    area: AtlasArea,
    mountain_chains: MountainChainGraph,
    drainage: DrainageGraph,
}

impl AtlasStructureMap {
    pub fn empty(area: AtlasArea) -> Self {
        Self {
            area,
            mountain_chains: MountainChainGraph::default(),
            drainage: DrainageGraph::default(),
        }
    }

    pub fn area(&self) -> AtlasArea {
        self.area
    }

    pub fn mountain_chains(&self) -> &MountainChainGraph {
        &self.mountain_chains
    }

    pub fn mountain_chains_mut(&mut self) -> &mut MountainChainGraph {
        &mut self.mountain_chains
    }

    pub fn drainage(&self) -> &DrainageGraph {
        &self.drainage
    }

    pub fn drainage_mut(&mut self) -> &mut DrainageGraph {
        &mut self.drainage
    }

    pub fn is_empty(&self) -> bool {
        self.mountain_chains.is_empty() && self.drainage.is_empty()
    }
}

pub fn generate_atlas_structure(meta: &WorldMeta, area: AtlasArea) -> AtlasStructureMap {
    generate_atlas_structure_with_tuning(meta, area, &AtlasTuning::default())
}

pub fn generate_atlas_structure_with_tuning(
    _meta: &WorldMeta,
    area: AtlasArea,
    _tuning: &AtlasTuning,
) -> AtlasStructureMap {
    AtlasStructureMap::empty(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_structure_preserves_requested_area() {
        let area = AtlasArea::new(AtlasCoord::new(-4, 7), 6, 5).unwrap();
        let structure = generate_atlas_structure(&WorldMeta::new(42), area);

        assert_eq!(structure.area(), area);
        assert!(structure.is_empty());
    }

    #[test]
    fn mountain_segments_report_area_intersection_from_bounds() {
        let area = AtlasArea::new(AtlasCoord::new(10, 10), 4, 4).unwrap();
        let segment = MountainSpineSegment {
            chain_id: MountainChainId(1),
            branch_order: 0,
            scale: MountainChainScale::Major,
            start: AtlasCoord::new(8, 12),
            end: AtlasCoord::new(12, 12),
            strength: 1.0,
            half_width_cells: 1.5,
        };

        assert!(segment.touches_area(area));
        assert!(!segment.touches_area(AtlasArea::new(AtlasCoord::new(20, 20), 4, 4).unwrap()));
    }

    #[test]
    fn river_segments_report_area_intersection_from_bounds() {
        let area = AtlasArea::new(AtlasCoord::new(-2, -2), 3, 3).unwrap();
        let segment = RiverPathSegment {
            river_id: RiverPathId(7),
            kind: RiverPathKind::Trunk,
            order: 2,
            start: AtlasCoord::new(-4, -1),
            end: AtlasCoord::new(0, -1),
            bankfull_width_cells: 1.0,
        };

        assert!(segment.touches_area(area));
        assert!(!segment.touches_area(AtlasArea::new(AtlasCoord::new(4, 4), 2, 2).unwrap()));
    }
}
