use std::path::PathBuf;
use std::sync::Arc;

use crate::world::{
    AtlasArea, BlockRegistry, ChunkCoord, ChunkSnapshot, CreateWorldConfig, NeighborChunks,
    TopdownChunkColumnCoord, WorldMeta,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRequest {
    CreateWorld {
        root: PathBuf,
        config: CreateWorldConfig,
        registry: Arc<BlockRegistry>,
    },
    LoadChunk {
        root: PathBuf,
        coord: ChunkCoord,
    },
    GenerateChunk {
        coord: ChunkCoord,
        meta: WorldMeta,
        registry: Arc<BlockRegistry>,
    },
    UnloadChunk {
        coord: ChunkCoord,
    },
    BuildChunkMesh {
        center: ChunkSnapshot,
        neighbors: NeighborChunks,
        registry: Arc<BlockRegistry>,
    },
    BuildMinimapChunkColumn {
        coord: TopdownChunkColumnCoord,
        chunks: Vec<ChunkSnapshot>,
        registry: Arc<BlockRegistry>,
    },
    ResolveRegionClassArea {
        meta: WorldMeta,
        area: AtlasArea,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobRequestCounts {
    pub create_world: usize,
    pub load_chunk: usize,
    pub generate_chunk: usize,
    pub unload_chunk: usize,
    pub build_chunk_mesh: usize,
    pub build_minimap_chunk_column: usize,
    pub resolve_region_class_area: usize,
}

impl JobRequestCounts {
    pub fn total(self) -> usize {
        self.create_world
            + self.load_chunk
            + self.generate_chunk
            + self.unload_chunk
            + self.build_chunk_mesh
            + self.build_minimap_chunk_column
            + self.resolve_region_class_area
    }

    pub(crate) fn add_request(&mut self, request: &JobRequest) {
        match request {
            JobRequest::CreateWorld { .. } => self.create_world += 1,
            JobRequest::LoadChunk { .. } => self.load_chunk += 1,
            JobRequest::GenerateChunk { .. } => self.generate_chunk += 1,
            JobRequest::UnloadChunk { .. } => self.unload_chunk += 1,
            JobRequest::BuildChunkMesh { .. } => self.build_chunk_mesh += 1,
            JobRequest::BuildMinimapChunkColumn { .. } => {
                self.build_minimap_chunk_column += 1;
            }
            JobRequest::ResolveRegionClassArea { .. } => {
                self.resolve_region_class_area += 1;
            }
        }
    }

    pub(crate) fn add_coalesce_key(&mut self, key: &JobCoalesceKey) {
        match key {
            JobCoalesceKey::CreateWorld(_) => self.create_world += 1,
            JobCoalesceKey::LoadChunk(_) => self.load_chunk += 1,
            JobCoalesceKey::GenerateChunk(_) => self.generate_chunk += 1,
            JobCoalesceKey::UnloadChunk(_) => self.unload_chunk += 1,
            JobCoalesceKey::BuildChunkMesh(_) => self.build_chunk_mesh += 1,
            JobCoalesceKey::BuildMinimapChunkColumn(_) => {
                self.build_minimap_chunk_column += 1;
            }
            JobCoalesceKey::ResolveRegionClassArea(_) => {
                self.resolve_region_class_area += 1;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum JobCoalesceKey {
    CreateWorld(PathBuf),
    LoadChunk(ChunkCoord),
    GenerateChunk(ChunkCoord),
    UnloadChunk(ChunkCoord),
    BuildChunkMesh(ChunkCoord),
    BuildMinimapChunkColumn(TopdownChunkColumnCoord),
    ResolveRegionClassArea(AtlasArea),
}

impl JobRequest {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::CreateWorld { .. } => "CreateWorld",
            Self::LoadChunk { .. } => "LoadChunk",
            Self::GenerateChunk { .. } => "GenerateChunk",
            Self::UnloadChunk { .. } => "UnloadChunk",
            Self::BuildChunkMesh { .. } => "BuildChunkMesh",
            Self::BuildMinimapChunkColumn { .. } => "BuildMinimapChunkColumn",
            Self::ResolveRegionClassArea { .. } => "ResolveRegionClassArea",
        }
    }

    pub fn diagnostic_label(&self) -> String {
        match self {
            Self::CreateWorld { root, config, .. } => format!(
                "CreateWorld(root={} seed={} center=({}, {}) radius={} y={}..{})",
                root.display(),
                config.seed,
                config.center_x,
                config.center_z,
                config.radius,
                config.min_y_chunk,
                config.max_y_chunk
            ),
            Self::LoadChunk { root, coord } => format!(
                "LoadChunk(root={} pos=({}, {}, {}))",
                root.display(),
                coord.0,
                coord.1,
                coord.2
            ),
            Self::GenerateChunk { coord, .. } => {
                format!("GenerateChunk(pos=({}, {}, {}))", coord.0, coord.1, coord.2)
            }
            Self::UnloadChunk { coord } => {
                format!("UnloadChunk(pos=({}, {}, {}))", coord.0, coord.1, coord.2)
            }
            Self::BuildChunkMesh { center, .. } => {
                let coord = center.coord();
                format!(
                    "BuildChunkMesh(pos=({}, {}, {}))",
                    coord.0, coord.1, coord.2
                )
            }
            Self::BuildMinimapChunkColumn { coord, chunks, .. } => format!(
                "BuildMinimapChunkColumn(column=({}, {}) chunks={})",
                coord.chunk_x,
                coord.chunk_z,
                chunks.len()
            ),
            Self::ResolveRegionClassArea { area, .. } => format!(
                "ResolveRegionClassArea(origin=({}, {}) size={}x{})",
                area.origin().x,
                area.origin().z,
                area.width(),
                area.height()
            ),
        }
    }

    pub fn coord(&self) -> ChunkCoord {
        match self {
            Self::CreateWorld { .. } => {
                panic!("CreateWorld request does not map to a single chunk coordinate")
            }
            Self::LoadChunk { coord, .. }
            | Self::GenerateChunk { coord, .. }
            | Self::UnloadChunk { coord } => *coord,
            Self::BuildChunkMesh { center, .. } => center.coord(),
            Self::BuildMinimapChunkColumn { .. } => {
                panic!("BuildMinimapChunkColumn request does not map to a single chunk coordinate")
            }
            Self::ResolveRegionClassArea { .. } => {
                panic!("ResolveRegionClassArea request does not map to a single chunk coordinate")
            }
        }
    }

    pub(crate) fn coalesce_key(&self) -> JobCoalesceKey {
        match self {
            Self::CreateWorld { root, .. } => JobCoalesceKey::CreateWorld(root.clone()),
            Self::LoadChunk { coord, .. } => JobCoalesceKey::LoadChunk(*coord),
            Self::GenerateChunk { coord, .. } => JobCoalesceKey::GenerateChunk(*coord),
            Self::UnloadChunk { coord } => JobCoalesceKey::UnloadChunk(*coord),
            Self::BuildChunkMesh { center, .. } => JobCoalesceKey::BuildChunkMesh(center.coord()),
            Self::BuildMinimapChunkColumn { coord, .. } => {
                JobCoalesceKey::BuildMinimapChunkColumn(*coord)
            }
            Self::ResolveRegionClassArea { area, .. } => {
                JobCoalesceKey::ResolveRegionClassArea(*area)
            }
        }
    }
}
