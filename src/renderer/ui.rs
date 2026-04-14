use std::path::PathBuf;

use bytemuck::{Pod, Zeroable};
use image::ImageReader;

use super::Renderer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderUiTextureSource {
    BuiltinWhite,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderUiSprite {
    pub min_screen_px: [f32; 2],
    pub max_screen_px: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub tint: [f32; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct UiTextureSet {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

pub(crate) struct GpuUiTextureResources {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
pub enum RenderUiTextureError {
    Io { path: PathBuf, source: std::io::Error },
    Decode { path: PathBuf, source: image::ImageError },
}

impl Default for UiTextureSet {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 255],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct UiVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub tint: [f32; 4],
}

impl UiVertex {
    pub(crate) fn vertex_buffer_layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UiVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

impl UiTextureSet {
    pub(crate) fn from_source(source: &RenderUiTextureSource) -> Result<Self, RenderUiTextureError> {
        match source {
            RenderUiTextureSource::BuiltinWhite => Ok(Self::default()),
            RenderUiTextureSource::File(path) => {
                let image = ImageReader::open(path)
                    .map_err(|source| RenderUiTextureError::Io {
                        path: path.clone(),
                        source,
                    })?
                    .decode()
                    .map_err(|source| RenderUiTextureError::Decode {
                        path: path.clone(),
                        source,
                    })?
                    .to_rgba8();

                Ok(Self {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                })
            }
        }
    }
}

impl Renderer {
    pub fn set_ui_texture(
        &mut self,
        source: RenderUiTextureSource,
    ) -> Result<(), RenderUiTextureError> {
        let texture = UiTextureSet::from_source(&source)?;
        if let Some(backend) = self.backend.as_mut() {
            backend.ui_texture = create_gpu_ui_texture_resources(
                &backend.device,
                &backend.queue,
                &backend.ui_texture_bind_group_layout,
                &texture,
            );
        }
        self.ui_texture = texture;
        Ok(())
    }
}

pub(crate) fn create_ui_texture_bind_group_layout(
    device: &wgpu::Device,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("renderer_ui_texture_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
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

pub(crate) fn create_gpu_ui_texture_resources(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    texture_set: &UiTextureSet,
) -> GpuUiTextureResources {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("renderer_ui_texture"),
        size: wgpu::Extent3d {
            width: texture_set.width.max(1),
            height: texture_set.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &texture_set.rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture_set.width.max(1) * 4),
            rows_per_image: Some(texture_set.height.max(1)),
        },
        wgpu::Extent3d {
            width: texture_set.width.max(1),
            height: texture_set.height.max(1),
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("renderer_ui_texture_sampler"),
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("renderer_ui_texture_bind_group"),
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

    GpuUiTextureResources {
        texture,
        view,
        sampler,
        bind_group,
    }
}
