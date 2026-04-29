use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::chunk::{BlockFace, BlockId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextureTileId(pub u16);

impl TextureTileId {
    pub const WHITE: Self = Self(0);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureTileSource {
    BuiltinWhite,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureTileDef {
    pub id: TextureTileId,
    pub key: String,
    pub source: TextureTileSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRenderKind {
    Empty,
    Cube,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMaterialKind {
    GenericOpaque = 0,
    Grass = 1,
    Soil = 2,
    Stone = 3,
    Sand = 4,
    Foliage = 5,
    Water = 6,
    Emissive = 7,
}

impl BlockMaterialKind {
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceTextureSet {
    pub top: TextureTileId,
    pub bottom: TextureTileId,
    pub side: TextureTileId,
}

impl FaceTextureSet {
    pub const WHITE: Self = Self {
        top: TextureTileId::WHITE,
        bottom: TextureTileId::WHITE,
        side: TextureTileId::WHITE,
    };

    pub const fn for_face(self, face: BlockFace) -> TextureTileId {
        match face {
            BlockFace::PosY => self.top,
            BlockFace::NegY => self.bottom,
            BlockFace::NegX | BlockFace::PosX | BlockFace::NegZ | BlockFace::PosZ => self.side,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockDef {
    pub id: BlockId,
    pub key: String,
    pub solid: bool,
    pub opaque: bool,
    pub render_kind: BlockRenderKind,
    pub material: BlockMaterialKind,
    pub surface_height: f32,
    pub face_textures: FaceTextureSet,
    pub tint: [u8; 4],
}

impl Eq for BlockDef {}

impl BlockDef {
    pub fn is_rendered_cube(&self) -> bool {
        matches!(self.render_kind, BlockRenderKind::Cube)
    }

    pub fn is_opaque(&self) -> bool {
        self.opaque
    }

    pub fn surface_height(&self) -> f32 {
        self.surface_height
    }

    pub fn tint_as_linear_rgba(&self) -> [f32; 4] {
        [
            self.tint[0] as f32 / 255.0,
            self.tint[1] as f32 / 255.0,
            self.tint[2] as f32 / 255.0,
            self.tint[3] as f32 / 255.0,
        ]
    }

    pub fn texture_for_face(&self, face: BlockFace) -> TextureTileId {
        self.face_textures.for_face(face)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockRegistry {
    tile_size: u32,
    textures: Vec<TextureTileDef>,
    blocks: Vec<Option<BlockDef>>,
    blocks_by_key: HashMap<String, BlockId>,
    textures_by_key: HashMap<String, TextureTileId>,
    missing_block: BlockDef,
}

impl Eq for BlockRegistry {}

impl BlockRegistry {
    pub fn load_default() -> Result<Self, BlockRegistryError> {
        Self::load_from_path(default_manifest_path())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, BlockRegistryError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| BlockRegistryError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        let manifest: BlockRegistryManifest =
            toml::from_str(&text).map_err(BlockRegistryError::ParseManifest)?;
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::from_manifest(manifest, &base_dir)
    }

    pub fn from_manifest(
        manifest: BlockRegistryManifest,
        base_dir: &Path,
    ) -> Result<Self, BlockRegistryError> {
        if manifest.tile_size == 0 {
            return Err(BlockRegistryError::InvalidTileSize(0));
        }

        let mut textures = Vec::new();
        let mut textures_by_key = HashMap::new();
        textures.push(TextureTileDef {
            id: TextureTileId::WHITE,
            key: "__white".to_string(),
            source: TextureTileSource::BuiltinWhite,
        });
        textures_by_key.insert("__white".to_string(), TextureTileId::WHITE);

        for (index, texture) in manifest.textures.into_iter().enumerate() {
            if textures_by_key.contains_key(&texture.key) {
                return Err(BlockRegistryError::DuplicateTextureKey(texture.key));
            }

            let id = TextureTileId((index as u16).saturating_add(1));
            let path = base_dir.join(&texture.path);
            textures.push(TextureTileDef {
                id,
                key: texture.key.clone(),
                source: TextureTileSource::File(path),
            });
            textures_by_key.insert(texture.key, id);
        }

        let block_defs = load_block_defs(&manifest.block_files, base_dir)?;
        let max_id = block_defs.iter().map(|block| block.id).max().unwrap_or(0) as usize;
        let mut blocks = vec![None; max_id.saturating_add(1)];
        let mut blocks_by_key = HashMap::new();

        for block in block_defs {
            let id = BlockId::new(block.id);
            if blocks_by_key.contains_key(&block.key) {
                return Err(BlockRegistryError::DuplicateBlockKey(block.key));
            }
            if blocks[id.raw() as usize].is_some() {
                return Err(BlockRegistryError::DuplicateBlockId(id.raw()));
            }

            let render_kind = match block.render {
                ManifestRenderKind::Empty => BlockRenderKind::Empty,
                ManifestRenderKind::Cube => BlockRenderKind::Cube,
            };

            let face_textures = match render_kind {
                BlockRenderKind::Empty => FaceTextureSet::WHITE,
                BlockRenderKind::Cube => FaceTextureSet {
                    top: lookup_texture(&textures_by_key, &block.top)?,
                    bottom: lookup_texture(&textures_by_key, &block.bottom)?,
                    side: lookup_texture(&textures_by_key, &block.side)?,
                },
            };
            let material = block
                .material
                .map(BlockMaterialKind::from_manifest)
                .unwrap_or_else(|| infer_block_material_kind(&block.key, render_kind));
            let surface_height = resolve_surface_height(block.surface_height, block.key.as_str())?;

            let tint = block.tint.unwrap_or([255, 255, 255, 255]);
            let def = BlockDef {
                id,
                key: block.key.clone(),
                solid: block.solid,
                opaque: block.opaque,
                render_kind,
                material,
                surface_height,
                face_textures,
                tint,
            };
            blocks[id.raw() as usize] = Some(def);
            blocks_by_key.insert(block.key, id);
        }

        if blocks_by_key.get("air") != Some(&BlockId::AIR) {
            return Err(BlockRegistryError::MissingRequiredBlock("air"));
        }

        Ok(Self {
            tile_size: manifest.tile_size,
            textures,
            blocks,
            blocks_by_key,
            textures_by_key,
            missing_block: BlockDef {
                id: BlockId::new(u16::MAX),
                key: "__missing".to_string(),
                solid: true,
                opaque: true,
                render_kind: BlockRenderKind::Cube,
                material: BlockMaterialKind::GenericOpaque,
                surface_height: 1.0,
                face_textures: FaceTextureSet::WHITE,
                tint: [255, 0, 255, 255],
            },
        })
    }

    pub fn tile_size(&self) -> u32 {
        self.tile_size
    }

    pub fn texture_tiles(&self) -> &[TextureTileDef] {
        &self.textures
    }

    pub fn block_id(&self, key: &str) -> Option<BlockId> {
        self.blocks_by_key.get(key).copied()
    }

    pub fn texture_id(&self, key: &str) -> Option<TextureTileId> {
        self.textures_by_key.get(key).copied()
    }

    pub fn block(&self, id: BlockId) -> Option<&BlockDef> {
        self.blocks.get(id.raw() as usize).and_then(Option::as_ref)
    }

    pub fn block_or_missing(&self, id: BlockId) -> &BlockDef {
        self.block(id).unwrap_or(&self.missing_block)
    }

    pub fn is_solid(&self, id: BlockId) -> bool {
        self.block_or_missing(id).solid
    }

    pub fn is_opaque(&self, id: BlockId) -> bool {
        self.block_or_missing(id).opaque
    }
}

fn load_block_defs(
    block_files: &[String],
    base_dir: &Path,
) -> Result<Vec<ManifestBlockDef>, BlockRegistryError> {
    let mut blocks = Vec::with_capacity(block_files.len());

    for block_file in block_files {
        let path = base_dir.join(block_file);
        let text =
            fs::read_to_string(&path).map_err(|source| BlockRegistryError::ReadBlockDef {
                path: path.clone(),
                source,
            })?;
        let block = toml::from_str(&text)
            .map_err(|source| BlockRegistryError::ParseBlockDef { path, source })?;
        blocks.push(block);
    }

    Ok(blocks)
}

fn lookup_texture(
    textures_by_key: &HashMap<String, TextureTileId>,
    key: &str,
) -> Result<TextureTileId, BlockRegistryError> {
    textures_by_key
        .get(key)
        .copied()
        .ok_or_else(|| BlockRegistryError::UnknownTextureKey(key.to_string()))
}

fn infer_block_material_kind(key: &str, render_kind: BlockRenderKind) -> BlockMaterialKind {
    if matches!(render_kind, BlockRenderKind::Empty) {
        return BlockMaterialKind::GenericOpaque;
    }

    let key = key.to_ascii_lowercase();
    if key.contains("grass") || key.contains("moss") {
        BlockMaterialKind::Grass
    } else if key.contains("dirt")
        || key.contains("soil")
        || key.contains("mud")
        || key.contains("clay")
    {
        BlockMaterialKind::Soil
    } else if key.contains("stone")
        || key.contains("rock")
        || key.contains("ore")
        || key.contains("slate")
    {
        BlockMaterialKind::Stone
    } else if key.contains("sand") {
        BlockMaterialKind::Sand
    } else if key.contains("leaf")
        || key.contains("leaves")
        || key.contains("foliage")
        || key.contains("vine")
    {
        BlockMaterialKind::Foliage
    } else if key.contains("water") || key.contains("ice") {
        BlockMaterialKind::Water
    } else if key.contains("lava")
        || key.contains("lamp")
        || key.contains("lantern")
        || key.contains("glow")
    {
        BlockMaterialKind::Emissive
    } else {
        BlockMaterialKind::GenericOpaque
    }
}

fn resolve_surface_height(
    surface_height: Option<f32>,
    block_key: &str,
) -> Result<f32, BlockRegistryError> {
    let surface_height = surface_height.unwrap_or(1.0);
    if !surface_height.is_finite()
        || !(0.0..=1.0).contains(&surface_height)
        || surface_height <= 0.0
    {
        return Err(BlockRegistryError::InvalidSurfaceHeight {
            block_key: block_key.to_string(),
            value: surface_height,
        });
    }

    Ok(surface_height)
}

pub fn default_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("blocks")
        .join("index.toml")
}

#[derive(Debug)]
pub enum BlockRegistryError {
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseManifest(toml::de::Error),
    ReadBlockDef {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseBlockDef {
        path: PathBuf,
        source: toml::de::Error,
    },
    InvalidTileSize(u32),
    DuplicateTextureKey(String),
    UnknownTextureKey(String),
    DuplicateBlockKey(String),
    DuplicateBlockId(u16),
    MissingRequiredBlock(&'static str),
    InvalidSurfaceHeight {
        block_key: String,
        value: f32,
    },
}

#[derive(Debug, Deserialize)]
pub struct BlockRegistryManifest {
    pub tile_size: u32,
    #[serde(default)]
    pub textures: Vec<ManifestTextureDef>,
    #[serde(default)]
    pub block_files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ManifestTextureDef {
    pub key: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestRenderKind {
    Empty,
    Cube,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManifestBlockMaterialKind {
    GenericOpaque,
    Grass,
    Soil,
    Stone,
    Sand,
    Foliage,
    Water,
    Emissive,
}

impl BlockMaterialKind {
    fn from_manifest(material: ManifestBlockMaterialKind) -> Self {
        match material {
            ManifestBlockMaterialKind::GenericOpaque => Self::GenericOpaque,
            ManifestBlockMaterialKind::Grass => Self::Grass,
            ManifestBlockMaterialKind::Soil => Self::Soil,
            ManifestBlockMaterialKind::Stone => Self::Stone,
            ManifestBlockMaterialKind::Sand => Self::Sand,
            ManifestBlockMaterialKind::Foliage => Self::Foliage,
            ManifestBlockMaterialKind::Water => Self::Water,
            ManifestBlockMaterialKind::Emissive => Self::Emissive,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ManifestBlockDef {
    pub id: u16,
    pub key: String,
    pub solid: bool,
    pub opaque: bool,
    pub render: ManifestRenderKind,
    #[serde(default)]
    pub material: Option<ManifestBlockMaterialKind>,
    #[serde(default)]
    pub surface_height: Option<f32>,
    #[serde(default)]
    pub top: String,
    #[serde(default)]
    pub bottom: String,
    #[serde(default)]
    pub side: String,
    #[serde(default)]
    pub tint: Option<[u8; 4]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_reserves_white_tile_zero() {
        let registry = BlockRegistry::load_default().expect("default registry should load");

        assert_eq!(registry.texture_tiles()[0].id, TextureTileId::WHITE);
        assert_eq!(registry.texture_tiles()[0].key, "__white");
        assert_eq!(registry.block_id("grass"), Some(BlockId::GRASS));
        assert_eq!(
            registry
                .block_or_missing(BlockId::GRASS)
                .texture_for_face(BlockFace::PosY),
            registry.texture_id("grass_top").unwrap()
        );
        assert!(registry.block_id("jungle_grass").is_some());
        assert_eq!(
            registry
                .block(
                    registry
                        .block_id("jungle_grass")
                        .expect("jungle grass block should exist")
                )
                .expect("jungle grass definition should exist")
                .texture_for_face(BlockFace::PosY),
            registry.texture_id("jungle_grass_top").unwrap()
        );
        assert_eq!(
            registry.block_or_missing(BlockId::GRASS).material,
            BlockMaterialKind::Grass
        );
        assert_eq!(
            registry.block_or_missing(BlockId::DIRT).material,
            BlockMaterialKind::Soil
        );
        assert_eq!(
            registry.block_or_missing(BlockId::STONE).material,
            BlockMaterialKind::Stone
        );
        assert!(registry.block_id("sand").is_some());
        assert!(registry.block_id("gravel").is_some());
        assert!(registry.block_id("mud").is_some());
        assert!(registry.block_id("snow").is_some());
        assert!(registry.block_id("water").is_some());
        assert_eq!(
            registry
                .block(
                    registry
                        .block_id("water")
                        .expect("water block should exist")
                )
                .expect("water definition should exist")
                .surface_height(),
            0.9
        );
    }
}
