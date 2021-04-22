// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Images pipeline

use guillotiere::{AllocId, Allocation, AtlasAllocator};
use std::mem::size_of;
use std::num::NonZeroU64;
use std::ops::Range;

use super::ImageError;
use crate::draw::ShaderManager;
use kas::cast::{Cast, Conv};
use kas::draw::Pass;
use kas::geom::{Quad, Size, Vec2};

const TEXTURE_SIZE: (u32, u32) = (2048, 2048);

fn to_vec2(p: guillotiere::Point) -> Vec2 {
    Vec2(p.x.cast(), p.y.cast())
}

/// Screen and texture coordinates
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Instance {
    a: Vec2,
    b: Vec2,
    ta: Vec2,
    tb: Vec2,
}
unsafe impl bytemuck::Zeroable for Instance {}
unsafe impl bytemuck::Pod for Instance {}

pub struct Atlas {
    alloc: AtlasAllocator,
    tex: wgpu::Texture,
    bg: wgpu::BindGroup,
}

impl Atlas {
    /// Are image dimensions too large to fit in an Atlas?
    pub fn is_too_big(size: (u32, u32)) -> bool {
        size.0 > TEXTURE_SIZE.0 || size.1 > TEXTURE_SIZE.1
    }

    /// Construct a new allocator
    pub fn new_alloc() -> AtlasAllocator {
        let size_i32: (i32, i32) = (TEXTURE_SIZE.0.cast(), TEXTURE_SIZE.1.cast());
        AtlasAllocator::new(size_i32.into())
    }

    /// Construct from an allocator
    pub fn new(
        alloc: AtlasAllocator,
        device: &wgpu::Device,
        bg_tex_layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Self {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loaded image"),
            size: wgpu::Extent3d {
                width: TEXTURE_SIZE.0,
                height: TEXTURE_SIZE.1,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("image atlas bind group"),
            layout: bg_tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        Atlas { alloc, tex, bg }
    }
}

/// A pipeline for rendering from image atlases
pub struct Pipeline {
    bg_tex_layout: wgpu::BindGroupLayout,
    render_pipeline: wgpu::RenderPipeline,
    atlases: Vec<Atlas>,
    new_aa: Vec<AtlasAllocator>,
    sampler: wgpu::Sampler,
}

impl Pipeline {
    /// Construct
    pub fn new(
        device: &wgpu::Device,
        shaders: &ShaderManager,
        bg_common: &wgpu::BindGroupLayout,
    ) -> Self {
        let bg_tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("images texture bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        filtering: false,
                        comparison: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("images pipeline layout"),
            bind_group_layouts: &[bg_common, &bg_tex_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("images render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shaders.vert_tex_quad,
                entry_point: "main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<Instance>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float2,
                        1 => Float2,
                        2 => Float2,
                        3 => Float2,
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Cw,
                cull_mode: wgpu::CullMode::Back,
                polygon_mode: wgpu::PolygonMode::Fill,
            },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shaders.frag_image,
                entry_point: "main",
                targets: &[wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Pipeline {
            bg_tex_layout,
            render_pipeline,
            atlases: vec![],
            new_aa: vec![],
            sampler,
        }
    }

    fn allocate_space(&mut self, size: (i32, i32)) -> (usize, Allocation) {
        let size = size.into();
        let mut atlas = 0;
        while atlas < self.atlases.len() {
            if let Some(alloc) = self.atlases[atlas].alloc.allocate(size) {
                return (atlas, alloc);
            }
            atlas += 1;
        }

        // New_aa are atlas allocators which haven't been assigned textures yet
        for new_aa in &mut self.new_aa {
            if let Some(alloc) = new_aa.allocate(size) {
                return (atlas, alloc);
            }
            atlas += 1;
        }

        self.new_aa.push(Atlas::new_alloc());
        match self.new_aa.last_mut().unwrap().allocate(size) {
            Some(alloc) => return (atlas, alloc),
            None => unreachable!(),
        }
    }

    /// Allocate space within a texture atlas
    ///
    /// Fails with `ImageError::Allocation` if size is too large.
    ///
    /// On success, returns:
    ///
    /// -   `atlas` number
    /// -   allocation identifier within the atlas
    /// -   `origin` within texture (integer coordinates, for use when uploading)
    /// -   texture coordinates (for use when drawing)
    pub fn allocate(
        &mut self,
        size: (u32, u32),
    ) -> Result<(usize, AllocId, (u32, u32), Quad), ImageError> {
        if Atlas::is_too_big(size) {
            return Err(ImageError::Allocation);
        }
        let (atlas, alloc) = self.allocate_space((size.0.cast(), size.1.cast()));

        let origin = (alloc.rectangle.min.x.cast(), alloc.rectangle.min.y.cast());

        let tex_size = Vec2::from(Size::from(TEXTURE_SIZE));
        let a = to_vec2(alloc.rectangle.min) / tex_size;
        let b = to_vec2(alloc.rectangle.max) / tex_size;
        debug_assert!(Vec2::ZERO.le(a) && a.le(b) && b.le(Vec2::splat(1.0)));
        let tex_quad = Quad { a, b };

        Ok((atlas, alloc.id, origin, tex_quad))
    }

    pub fn deallocate(&mut self, atlas: usize, alloc: AllocId) {
        self.atlases[atlas].alloc.deallocate(alloc);
    }

    /// Prepare textures
    pub fn prepare(&mut self, device: &wgpu::Device) {
        for alloc in self.new_aa.drain(..) {
            let atlas = Atlas::new(alloc, device, &self.bg_tex_layout, &self.sampler);
            self.atlases.push(atlas);
        }
    }

    pub fn get_texture(&self, atlas: usize) -> &wgpu::Texture {
        &self.atlases[atlas].tex
    }

    /// Enqueue render commands
    pub fn render<'a>(
        &'a self,
        window: &'a Window,
        pass: usize,
        rpass: &mut wgpu::RenderPass<'a>,
        bg_common: &'a wgpu::BindGroup,
    ) {
        if let Some(buffer) = window.buffer.as_ref() {
            if let Some(pass) = window.passes.get(pass) {
                rpass.set_pipeline(&self.render_pipeline);
                rpass.set_bind_group(0, bg_common, &[]);
                rpass.set_vertex_buffer(0, buffer.slice(pass.data_range.clone()));
                for (a, atlas) in pass.atlases.iter().enumerate() {
                    rpass.set_bind_group(1, &self.atlases[a].bg, &[]);
                    rpass.draw(0..4, atlas.range.clone());
                }
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
struct AtlasPassData {
    instances: Vec<Instance>,
    range: Range<u32>,
}

#[derive(Clone, Debug, Default)]
struct PassData {
    atlases: Vec<AtlasPassData>,
    data_range: Range<u64>,
}

/// Per-window state
#[derive(Debug, Default)]
pub struct Window {
    passes: Vec<PassData>,
    buffer: Option<wgpu::Buffer>,
    buffer_size: u64,
}

impl Window {
    /// Prepare vertex buffers
    pub fn write_buffers(
        &mut self,
        device: &wgpu::Device,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut req_len = 0;
        for pass in self.passes.iter() {
            for atlas in pass.atlases.iter() {
                req_len += u64::conv(atlas.instances.len() * size_of::<Instance>());
            }
        }
        let byte_len = match NonZeroU64::new(req_len) {
            Some(nz) => nz,
            None => {
                for pass in self.passes.iter_mut() {
                    for atlas in pass.atlases.iter_mut() {
                        atlas.range = 0..0;
                    }
                }
                return;
            }
        };

        if req_len <= self.buffer_size {
            let buffer = self.buffer.as_ref().unwrap();
            let mut slice = staging_belt.write_buffer(encoder, buffer, 0, byte_len, device);
            copy_to_slice(&mut self.passes, &mut slice);
        } else {
            // Size must be a multiple of alignment
            let mask = wgpu::COPY_BUFFER_ALIGNMENT - 1;
            let buffer_size = (byte_len.get() + mask) & !mask;
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("atlases vertex buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            });

            let mut slice = buffer.slice(..byte_len.get()).get_mapped_range_mut();
            copy_to_slice(&mut self.passes, &mut slice);
            drop(slice);

            buffer.unmap();
            self.buffer = Some(buffer);
            self.buffer_size = buffer_size;
        }

        fn copy_to_slice(passes: &mut [PassData], slice: &mut [u8]) {
            let mut byte_offset = 0;
            for pass in passes.iter_mut() {
                let byte_start = byte_offset;
                let mut index = 0;
                for atlas in pass.atlases.iter_mut() {
                    let len = u32::conv(atlas.instances.len());
                    let byte_len = u64::from(len) * u64::conv(size_of::<Instance>());
                    let byte_end = byte_offset + byte_len;

                    slice[usize::conv(byte_offset)..usize::conv(byte_end)]
                        .copy_from_slice(bytemuck::cast_slice(&atlas.instances));

                    byte_offset = byte_end;
                    atlas.instances.clear();
                    let end = index + len;
                    atlas.range = index..end;
                    index = end;
                }
                pass.data_range = byte_start..byte_offset;
            }
        }
    }

    /// Add a rectangle to the buffer
    pub fn rect(&mut self, pass: Pass, atlas: usize, tex: Quad, rect: Quad) {
        if !rect.a.lt(rect.b) {
            // zero / negative size: nothing to draw
            return;
        }

        let instance = Instance {
            a: rect.a,
            b: rect.b,
            ta: tex.a,
            tb: tex.b,
        };

        let pass = pass.pass();
        if self.passes.len() <= pass {
            // We only need one more, but no harm in adding extra
            self.passes.resize(pass + 8, Default::default());
        }
        let pass = &mut self.passes[pass];

        if pass.atlases.len() <= atlas {
            // Warning: length must not excced number of atlases
            pass.atlases.resize(atlas + 1, Default::default());
        }

        pass.atlases[atlas].instances.push(instance);
    }
}
