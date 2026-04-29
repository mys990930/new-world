use std::path::PathBuf;

use image::ImageReader;

use super::Renderer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTextureSource {
    BuiltinWhite,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTextureTile {
    pub layer: u32,
    pub key: String,
    pub source: RenderTextureSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTextureArraySource {
    pub tile_size: u32,
    pub tiles: Vec<RenderTextureTile>,
}

#[derive(Debug, Clone)]
pub(crate) struct BlockTextureSet {
    pub(crate) tile_size: u32,
    pub(crate) layers: Vec<BlockTextureLayer>,
}

#[derive(Debug, Clone)]
pub(crate) struct BlockTextureLayer {
    pub(crate) key: String,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) struct GpuBlockTextureResources {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
pub enum RenderTextureError {
    InvalidTileSize(u32),
    EmptyTextureList,
    MissingLayer {
        expected: u32,
        found: Option<u32>,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: image::ImageError,
    },
    WrongImageSize {
        path: PathBuf,
        expected: (u32, u32),
        found: (u32, u32),
    },
}

impl Default for BlockTextureSet {
    fn default() -> Self {
        Self::white_only(16)
    }
}

impl BlockTextureSet {
    pub(crate) fn white_only(tile_size: u32) -> Self {
        let pixel_count = tile_size.saturating_mul(tile_size) as usize;
        Self {
            tile_size,
            layers: vec![BlockTextureLayer {
                key: "__white".to_string(),
                rgba: vec![255; pixel_count * 4],
            }],
        }
    }

    pub(crate) fn from_source(
        source: &RenderTextureArraySource,
    ) -> Result<Self, RenderTextureError> {
        if source.tile_size == 0 {
            return Err(RenderTextureError::InvalidTileSize(0));
        }

        if source.tiles.is_empty() {
            return Err(RenderTextureError::EmptyTextureList);
        }

        let mut sorted = source.tiles.clone();
        sorted.sort_by_key(|tile| tile.layer);

        let mut layers = Vec::with_capacity(sorted.len());
        for (expected, tile) in sorted.into_iter().enumerate() {
            let expected = expected as u32;
            if tile.layer != expected {
                return Err(RenderTextureError::MissingLayer {
                    expected,
                    found: Some(tile.layer),
                });
            }

            let rgba = match &tile.source {
                RenderTextureSource::BuiltinWhite => {
                    let pixel_count = source.tile_size.saturating_mul(source.tile_size) as usize;
                    vec![255; pixel_count * 4]
                }
                RenderTextureSource::File(path) => {
                    let image = ImageReader::open(path)
                        .map_err(|source| RenderTextureError::Io {
                            path: path.clone(),
                            source,
                        })?
                        .decode()
                        .map_err(|source| RenderTextureError::Decode {
                            path: path.clone(),
                            source,
                        })?
                        .to_rgba8();

                    if image.width() != source.tile_size || image.height() != source.tile_size {
                        return Err(RenderTextureError::WrongImageSize {
                            path: path.clone(),
                            expected: (source.tile_size, source.tile_size),
                            found: (image.width(), image.height()),
                        });
                    }

                    image.into_raw()
                }
            };

            layers.push(BlockTextureLayer {
                key: tile.key,
                rgba,
            });
        }

        Ok(Self {
            tile_size: source.tile_size,
            layers,
        })
    }
}

impl Renderer {
    pub fn set_block_textures(
        &mut self,
        source: RenderTextureArraySource,
    ) -> Result<(), RenderTextureError> {
        let textures = BlockTextureSet::from_source(&source)?;
        if let Some(backend) = self.backend.as_mut() {
            backend.block_textures = create_gpu_block_texture_resources(
                &backend.device,
                &backend.queue,
                &backend.block_texture_bind_group_layout,
                &textures,
            );
        }
        self.block_textures = textures;
        Ok(())
    }
}

pub(crate) fn create_block_texture_bind_group_layout(
    device: &wgpu::Device,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("renderer_block_texture_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

pub(crate) fn create_gpu_block_texture_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    textures: &BlockTextureSet,
) -> GpuBlockTextureResources {
    let layer_count = u32::try_from(textures.layers.len())
        .unwrap_or(u32::MAX)
        .max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("renderer_block_texture_array"),
        size: wgpu::Extent3d {
            width: textures.tile_size.max(1),
            height: textures.tile_size.max(1),
            depth_or_array_layers: layer_count,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (layer_index, layer) in textures.layers.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer_index as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &layer.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(textures.tile_size * 4),
                rows_per_image: Some(textures.tile_size),
            },
            wgpu::Extent3d {
                width: textures.tile_size,
                height: textures.tile_size,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("renderer_block_texture_array_view"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("renderer_block_texture_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("renderer_block_texture_bind_group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    GpuBlockTextureResources {
        texture,
        view,
        sampler,
        bind_group,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_sources_must_start_at_layer_zero_and_be_contiguous() {
        let error = BlockTextureSet::from_source(&RenderTextureArraySource {
            tile_size: 16,
            tiles: vec![RenderTextureTile {
                layer: 1,
                key: "missing_zero".to_string(),
                source: RenderTextureSource::BuiltinWhite,
            }],
        })
        .expect_err("non-zero first layer should be rejected");

        assert!(matches!(
            error,
            RenderTextureError::MissingLayer {
                expected: 0,
                found: Some(1)
            }
        ));
    }
}
