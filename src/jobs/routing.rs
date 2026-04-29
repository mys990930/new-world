use crate::world::{
    CreateWorldProgress, build_chunk_mesh, create_world_to_directory_with_progress,
    generate_atlas_fields, generate_atlas_structure, generate_chunk, load_created_world_chunk,
    resolve_region_classes, sample_topdown_chunk_column,
};

use super::request::JobRequest;
use super::result::{JobError, JobResult};

const CREATE_WORLD_PROGRESS_CHUNK_INTERVAL: u32 = 8;

pub(crate) fn execute(request: JobRequest, mut emit_progress: impl FnMut(JobResult)) -> JobResult {
    match request {
        JobRequest::CreateWorld {
            root,
            config,
            registry,
        } => {
            println!(
                "[jobs] create-world start: root={} seed={} center=({}, {}) radius={} y={}..{} chunks={}",
                root.display(),
                config.seed,
                config.center_x,
                config.center_z,
                config.radius,
                config.min_y_chunk,
                config.max_y_chunk,
                config.total_chunk_count().unwrap_or(0)
            );
            let progress_root = root.clone();
            let mut last_reported_chunks = None;
            let mut report_progress = |progress: CreateWorldProgress| {
                let should_report = progress.completed_chunks == 0
                    || progress.completed_chunks == progress.total_chunks
                    || last_reported_chunks
                        .map(|last| {
                            progress.completed_chunks.saturating_sub(last)
                                >= CREATE_WORLD_PROGRESS_CHUNK_INTERVAL
                        })
                        .unwrap_or(true);
                if should_report {
                    last_reported_chunks = Some(progress.completed_chunks);
                    emit_progress(JobResult::CreateWorldProgress {
                        root: progress_root.clone(),
                        completed_chunks: progress.completed_chunks,
                        total_chunks: progress.total_chunks,
                    });
                }
            };

            match create_world_to_directory_with_progress(
                root.as_path(),
                config,
                registry.as_ref(),
                &mut report_progress,
            ) {
                Ok(manifest) => {
                    println!(
                        "[jobs] create-world finished: root={} stacks={}",
                        root.display(),
                        manifest.stacks.len()
                    );
                    JobResult::WorldCreated { root, manifest }
                }
                Err(error) => JobResult::JobFailed {
                    request: JobRequest::CreateWorld {
                        root,
                        config,
                        registry,
                    },
                    error: JobError::ExecutionFailed {
                        message: error.to_string(),
                    },
                },
            }
        }
        JobRequest::LoadChunk { root, coord } => {
            match load_created_world_chunk(root.as_path(), coord) {
                Ok(chunk) => JobResult::ChunkLoaded { coord, chunk },
                Err(error) => JobResult::JobFailed {
                    request: JobRequest::LoadChunk { root, coord },
                    error: JobError::ExecutionFailed {
                        message: error.to_string(),
                    },
                },
            }
        }
        JobRequest::GenerateChunk {
            coord,
            meta,
            registry,
        } => JobResult::ChunkGenerated {
            coord,
            chunk: generate_chunk(coord, &meta, registry.as_ref()),
        },
        JobRequest::BuildChunkMesh {
            center,
            neighbors,
            registry,
        } => {
            let coord = center.coord();
            let mesh = build_chunk_mesh(&center, neighbors, registry.as_ref());
            JobResult::ChunkMeshBuilt { coord, mesh }
        }
        JobRequest::BuildMinimapChunkColumn {
            coord,
            chunks,
            registry,
        } => JobResult::MinimapChunkColumnBuilt {
            coord,
            patch: sample_topdown_chunk_column(registry.as_ref(), coord, &chunks),
        },
        JobRequest::ResolveRegionClassArea { meta, area } => {
            let fields = generate_atlas_fields(&meta, area);
            let structure = generate_atlas_structure(&meta, area);
            let classes = resolve_region_classes(&meta, area, &fields, &structure);
            JobResult::RegionClassResolved { area, classes }
        }
    }
}
