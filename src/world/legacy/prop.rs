use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::chunk::BlockFace;
use super::coord::WorldBlockCoord;
use super::meshing::{CpuMesh, MeshVertex, RenderBounds};
use super::registry::{BlockMaterialKind, BlockRegistry};

const MICRO_UNITS_PER_BLOCK: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub struct MicrovoxelPropCatalog {
    props: Vec<MicrovoxelPropDef>,
    props_by_key: HashMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MicrovoxelPropDef {
    pub key: String,
    pub texture: String,
    pub material: BlockMaterialKind,
    pub tint: [u8; 4],
    pub cuboids: Vec<MicrovoxelCuboid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrovoxelCuboid {
    pub x: u8,
    pub y: u8,
    pub z: u8,
    pub w: u8,
    pub h: u8,
    pub d: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MicrovoxelPropPlacement {
    pub origin: WorldBlockCoord,
    pub quarter_turns: u8,
}

#[derive(Debug)]
pub enum MicrovoxelPropError {
    ReadIndex {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseIndex(toml::de::Error),
    ReadProp {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseProp {
        path: PathBuf,
        source: toml::de::Error,
    },
    DuplicateKey(String),
    EmptyProp {
        key: String,
    },
    InvalidCuboid {
        key: String,
        index: usize,
        cuboid: MicrovoxelCuboid,
    },
    UnknownMaterial {
        key: String,
        material: String,
    },
    UnknownTexture {
        prop_key: String,
        texture_key: String,
    },
}

impl MicrovoxelPropCatalog {
    pub fn load_default() -> Result<Self, MicrovoxelPropError> {
        Self::load_from_path(default_prop_manifest_path())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, MicrovoxelPropError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| MicrovoxelPropError::ReadIndex {
            path: path.to_path_buf(),
            source,
        })?;
        let manifest: MicrovoxelPropManifest =
            toml::from_str(&text).map_err(MicrovoxelPropError::ParseIndex)?;
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut props = Vec::with_capacity(manifest.prop_files.len());
        let mut props_by_key = HashMap::new();
        for prop_file in manifest.prop_files {
            let prop_path = base_dir.join(prop_file);
            let text =
                fs::read_to_string(&prop_path).map_err(|source| MicrovoxelPropError::ReadProp {
                    path: prop_path.clone(),
                    source,
                })?;
            let manifest: MicrovoxelPropFile =
                toml::from_str(&text).map_err(|source| MicrovoxelPropError::ParseProp {
                    path: prop_path,
                    source,
                })?;
            let prop = prop_from_manifest(manifest)?;
            if props_by_key.contains_key(&prop.key) {
                return Err(MicrovoxelPropError::DuplicateKey(prop.key));
            }
            props_by_key.insert(prop.key.clone(), props.len());
            props.push(prop);
        }

        Ok(Self {
            props,
            props_by_key,
        })
    }

    pub fn props(&self) -> &[MicrovoxelPropDef] {
        &self.props
    }

    pub fn prop(&self, key: &str) -> Option<&MicrovoxelPropDef> {
        self.props_by_key
            .get(key)
            .and_then(|index| self.props.get(*index))
    }
}

impl MicrovoxelPropPlacement {
    pub const fn new(origin: WorldBlockCoord, quarter_turns: u8) -> Self {
        Self {
            origin,
            quarter_turns,
        }
    }
}

pub fn build_microvoxel_prop_mesh(
    prop: &MicrovoxelPropDef,
    placement: MicrovoxelPropPlacement,
    registry: &BlockRegistry,
) -> Result<CpuMesh, MicrovoxelPropError> {
    let texture_layer =
        registry
            .texture_id(&prop.texture)
            .ok_or_else(|| MicrovoxelPropError::UnknownTexture {
                prop_key: prop.key.clone(),
                texture_key: prop.texture.clone(),
            })?;
    let mut mesh = CpuMesh::default();
    let color = tint_as_linear_rgba(prop.tint);

    for cuboid in &prop.cuboids {
        let (min, max) = placed_cuboid_bounds(*cuboid, placement);
        for face in faces() {
            append_prop_face(
                &mut mesh,
                min,
                max,
                face,
                color,
                u32::from(texture_layer.0),
                prop.material,
            );
        }
    }

    Ok(mesh)
}

pub fn default_prop_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("props")
        .join("index.toml")
}

fn prop_from_manifest(
    manifest: MicrovoxelPropFile,
) -> Result<MicrovoxelPropDef, MicrovoxelPropError> {
    let material = parse_material_kind(&manifest.material).ok_or_else(|| {
        MicrovoxelPropError::UnknownMaterial {
            key: manifest.key.clone(),
            material: manifest.material.clone(),
        }
    })?;
    if manifest.cuboids.is_empty() {
        return Err(MicrovoxelPropError::EmptyProp { key: manifest.key });
    }

    let mut cuboids = Vec::with_capacity(manifest.cuboids.len());
    for (index, cuboid) in manifest.cuboids.into_iter().enumerate() {
        let cuboid = MicrovoxelCuboid {
            x: cuboid.x,
            y: cuboid.y,
            z: cuboid.z,
            w: cuboid.w,
            h: cuboid.h,
            d: cuboid.d,
        };
        if !cuboid_fits(cuboid) {
            return Err(MicrovoxelPropError::InvalidCuboid {
                key: manifest.key,
                index,
                cuboid,
            });
        }
        cuboids.push(cuboid);
    }

    Ok(MicrovoxelPropDef {
        key: manifest.key,
        texture: manifest.texture,
        material,
        tint: manifest.tint.unwrap_or([255, 255, 255, 255]),
        cuboids,
    })
}

fn parse_material_kind(value: &str) -> Option<BlockMaterialKind> {
    match value {
        "generic_opaque" => Some(BlockMaterialKind::GenericOpaque),
        "grass" => Some(BlockMaterialKind::Grass),
        "soil" => Some(BlockMaterialKind::Soil),
        "stone" => Some(BlockMaterialKind::Stone),
        "sand" => Some(BlockMaterialKind::Sand),
        "foliage" => Some(BlockMaterialKind::Foliage),
        "water" => Some(BlockMaterialKind::Water),
        "emissive" => Some(BlockMaterialKind::Emissive),
        _ => None,
    }
}

fn cuboid_fits(cuboid: MicrovoxelCuboid) -> bool {
    cuboid.w > 0
        && cuboid.h > 0
        && cuboid.d > 0
        && u16::from(cuboid.x) + u16::from(cuboid.w) <= 16
        && u16::from(cuboid.y) + u16::from(cuboid.h) <= 16
        && u16::from(cuboid.z) + u16::from(cuboid.d) <= 16
}

fn placed_cuboid_bounds(
    cuboid: MicrovoxelCuboid,
    placement: MicrovoxelPropPlacement,
) -> ([f32; 3], [f32; 3]) {
    let raw_min = rotate_micro_coord(cuboid.x, cuboid.y, cuboid.z, placement.quarter_turns);
    let raw_max = rotate_micro_coord(
        cuboid.x.saturating_add(cuboid.w),
        cuboid.y.saturating_add(cuboid.h),
        cuboid.z.saturating_add(cuboid.d),
        placement.quarter_turns,
    );
    let min = [
        raw_min[0].min(raw_max[0]),
        raw_min[1].min(raw_max[1]),
        raw_min[2].min(raw_max[2]),
    ];
    let max = [
        raw_min[0].max(raw_max[0]),
        raw_min[1].max(raw_max[1]),
        raw_min[2].max(raw_max[2]),
    ];

    (
        [
            placement.origin.0 as f32 + min[0] / MICRO_UNITS_PER_BLOCK,
            placement.origin.1 as f32 + min[1] / MICRO_UNITS_PER_BLOCK,
            placement.origin.2 as f32 + min[2] / MICRO_UNITS_PER_BLOCK,
        ],
        [
            placement.origin.0 as f32 + max[0] / MICRO_UNITS_PER_BLOCK,
            placement.origin.1 as f32 + max[1] / MICRO_UNITS_PER_BLOCK,
            placement.origin.2 as f32 + max[2] / MICRO_UNITS_PER_BLOCK,
        ],
    )
}

fn rotate_micro_coord(x: u8, y: u8, z: u8, quarter_turns: u8) -> [f32; 3] {
    let x = f32::from(x);
    let y = f32::from(y);
    let z = f32::from(z);
    match quarter_turns % 4 {
        0 => [x, y, z],
        1 => [z, y, 16.0 - x],
        2 => [16.0 - x, y, 16.0 - z],
        3 => [16.0 - z, y, x],
        _ => unreachable!(),
    }
}

fn append_prop_face(
    mesh: &mut CpuMesh,
    min: [f32; 3],
    max: [f32; 3],
    face: BlockFace,
    color: [f32; 4],
    texture_layer: u32,
    material_kind: BlockMaterialKind,
) {
    let positions = face_positions(min, max, face);
    let base_index = mesh.vertices.len() as u32;
    let normal = face_normal(face);
    let uv = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    extend_bounds(&mut mesh.bounds, &positions);

    for (position, uv) in positions.into_iter().zip(uv) {
        mesh.vertices.push(MeshVertex {
            position,
            color,
            normal,
            uv,
            texture_layer,
            material_kind,
            contour_edges: 0,
        });
    }

    mesh.indices.extend_from_slice(&[
        base_index,
        base_index + 1,
        base_index + 2,
        base_index,
        base_index + 2,
        base_index + 3,
    ]);
}

fn face_positions(min: [f32; 3], max: [f32; 3], face: BlockFace) -> [[f32; 3]; 4] {
    match face {
        BlockFace::NegX => [
            [min[0], min[1], min[2]],
            [min[0], min[1], max[2]],
            [min[0], max[1], max[2]],
            [min[0], max[1], min[2]],
        ],
        BlockFace::PosX => [
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
        ],
        BlockFace::NegY => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
        ],
        BlockFace::PosY => [
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
        BlockFace::NegZ => [
            [max[0], min[1], min[2]],
            [min[0], min[1], min[2]],
            [min[0], max[1], min[2]],
            [max[0], max[1], min[2]],
        ],
        BlockFace::PosZ => [
            [min[0], min[1], max[2]],
            [max[0], min[1], max[2]],
            [max[0], max[1], max[2]],
            [min[0], max[1], max[2]],
        ],
    }
}

fn face_normal(face: BlockFace) -> [f32; 3] {
    match face {
        BlockFace::NegX => [-1.0, 0.0, 0.0],
        BlockFace::PosX => [1.0, 0.0, 0.0],
        BlockFace::NegY => [0.0, -1.0, 0.0],
        BlockFace::PosY => [0.0, 1.0, 0.0],
        BlockFace::NegZ => [0.0, 0.0, -1.0],
        BlockFace::PosZ => [0.0, 0.0, 1.0],
    }
}

fn extend_bounds(bounds: &mut Option<RenderBounds>, positions: &[[f32; 3]; 4]) {
    for position in positions {
        match bounds {
            Some(bounds) => {
                bounds.min[0] = bounds.min[0].min(position[0]);
                bounds.min[1] = bounds.min[1].min(position[1]);
                bounds.min[2] = bounds.min[2].min(position[2]);
                bounds.max[0] = bounds.max[0].max(position[0]);
                bounds.max[1] = bounds.max[1].max(position[1]);
                bounds.max[2] = bounds.max[2].max(position[2]);
            }
            None => {
                *bounds = Some(RenderBounds {
                    min: *position,
                    max: *position,
                });
            }
        }
    }
}

fn tint_as_linear_rgba(tint: [u8; 4]) -> [f32; 4] {
    [
        tint[0] as f32 / 255.0,
        tint[1] as f32 / 255.0,
        tint[2] as f32 / 255.0,
        tint[3] as f32 / 255.0,
    ]
}

fn faces() -> [BlockFace; 6] {
    [
        BlockFace::NegX,
        BlockFace::PosX,
        BlockFace::NegY,
        BlockFace::PosY,
        BlockFace::NegZ,
        BlockFace::PosZ,
    ]
}

#[derive(Debug, Deserialize)]
struct MicrovoxelPropManifest {
    #[serde(default)]
    prop_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MicrovoxelPropFile {
    key: String,
    texture: String,
    material: String,
    #[serde(default)]
    tint: Option<[u8; 4]>,
    #[serde(default)]
    cuboids: Vec<MicrovoxelCuboidFile>,
}

#[derive(Debug, Deserialize)]
struct MicrovoxelCuboidFile {
    x: u8,
    y: u8,
    z: u8,
    w: u8,
    h: u8,
    d: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_microvoxel_rock_catalog_loads() {
        let catalog =
            MicrovoxelPropCatalog::load_default().expect("default prop catalog should load");

        assert!(catalog.prop("rock_pebble_1x1x1").is_some());
        assert!(catalog.prop("rock_flat_2x2x1").is_some());
        assert!(catalog.prop("rock_stacked_2x2x1_plus_1").is_some());
        assert!(catalog.prop("rock_low_cluster_4x3").is_some());
    }

    #[test]
    fn stacked_rock_bakes_into_sub_block_mesh() {
        let catalog =
            MicrovoxelPropCatalog::load_default().expect("default prop catalog should load");
        let registry = BlockRegistry::load_default().expect("default block registry should load");
        let prop = catalog
            .prop("rock_stacked_2x2x1_plus_1")
            .expect("stacked rock should exist");

        let mesh = build_microvoxel_prop_mesh(
            prop,
            MicrovoxelPropPlacement::new(WorldBlockCoord(10, 4, -3), 0),
            &registry,
        )
        .expect("prop mesh should bake");

        assert_eq!(mesh.vertices.len(), 48);
        assert_eq!(mesh.triangle_count(), 24);
        assert_eq!(
            mesh.bounds,
            Some(RenderBounds {
                min: [10.4375, 4.0, -2.5625],
                max: [10.5625, 4.125, -2.4375],
            })
        );
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.material_kind == BlockMaterialKind::Stone)
        );
    }
}
