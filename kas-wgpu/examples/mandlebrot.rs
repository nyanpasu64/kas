// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License in the LICENSE-APACHE file or at:
//     https://www.apache.org/licenses/LICENSE-2.0

//! Mandlebrot example
//!
//! Demonstrates use of a custom draw pipe.

use shaderc::{Compiler, ShaderKind};
use std::mem::size_of;
use wgpu::ShaderModule;

use kas::draw::{DrawHandle, SizeHandle};
use kas::event::ManagerState;
use kas::geom::{Rect, Size};
use kas::layout::{AxisInfo, SizeRules};
use kas::widget::Window;
use kas::{AlignHints, Layout};
use kas_wgpu::draw::{CustomPipe, DrawCustom, DrawPipe, Vec2};
use kas_wgpu::Options;

const VERTEX: &'static str = "
#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a1;

layout(location = 0) out vec2 b1;

layout(set = 0, binding = 0) uniform Locals {
    vec2 scale;
};

const vec2 offset = { 1.0, 1.0 };

void main() {
    gl_Position = vec4(scale * a_pos - offset, 0.0, 1.0);
    b1 = a1;
}
";
const FRAGMENT: &'static str = "
#version 450
#extension GL_ARB_separate_shader_objects : enable

precision highp float;

layout(location = 0) in vec2 c;

layout(location = 0) out vec4 outColor;

const int iter = 64;

void main() {
    vec2 z;

    int i;
    z = c;
    for(i=0; i<iter; i++) {
        float x = (z.x * z.x - z.y * z.y) + c.x;
        float y = (z.y * z.x + z.x * z.y) + c.y;

        if((x * x + y * y) > 4.0) break;
        z.x = x;
        z.y = y;
    }

    float r = (i == iter ? 0.0 : float(i)) / iter;
    outColor = vec4(r, 0.0, 0.0, 1.0);
}
";

struct Shaders {
    vertex: ShaderModule,
    fragment: ShaderModule,
}

impl Shaders {
    fn compile(device: &wgpu::Device) -> Self {
        let mut compiler = Compiler::new().unwrap();

        let artifact = compiler
            .compile_into_spirv(VERTEX, ShaderKind::Vertex, "VERTEX", "main", None)
            .unwrap();
        let vertex = device.create_shader_module(&artifact.as_binary());

        let artifact = compiler
            .compile_into_spirv(FRAGMENT, ShaderKind::Fragment, "FRAGMENT", "main", None)
            .unwrap();
        let fragment = device.create_shader_module(&artifact.as_binary());

        Shaders { vertex, fragment }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex(Vec2, Vec2);

struct PipeRes {
    bind_group: wgpu::BindGroup,
    scale_buf: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
}

struct Pipe {
    res: Option<PipeRes>,
    passes: Vec<Vec<Vertex>>,
}

impl Clone for Pipe {
    fn clone(&self) -> Self {
        Pipe {
            res: None,
            passes: vec![],
        }
    }
}

impl CustomPipe for Pipe {
    type Param = (Vec2, f32);

    fn init(&mut self, device: &wgpu::Device, size: Size) {
        // Note: real apps should compile shaders once and share between windows
        let shaders = Shaders::compile(device);

        type Scale = [f32; 2];
        let scale_factor: Scale = [2.0 / size.0 as f32, 2.0 / size.1 as f32];
        let scale_buf = device
            .create_buffer_mapped(
                scale_factor.len(),
                wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            )
            .fill_from_slice(&scale_factor);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[wgpu::BindGroupLayoutBinding {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &scale_buf,
                    range: 0..(size_of::<Scale>() as u64),
                },
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            bind_group_layouts: &[&bind_group_layout],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &shaders.vertex,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &shaders.fragment,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: 0,
                        shader_location: 0,
                    },
                    wgpu::VertexAttributeDescriptor {
                        format: wgpu::VertexFormat::Float2,
                        offset: (size_of::<Vec2>()) as u64,
                        shader_location: 1,
                    },
                ],
            }],
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        self.res = Some(PipeRes {
            bind_group,
            scale_buf,
            render_pipeline,
        });
    }

    fn resize(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, size: Size) {
        type Scale = [f32; 2];
        let scale_factor: Scale = [2.0 / size.0 as f32, 2.0 / size.1 as f32];
        let scale_buf = device
            .create_buffer_mapped(scale_factor.len(), wgpu::BufferUsage::COPY_SRC)
            .fill_from_slice(&scale_factor);
        let byte_len = size_of::<Scale>() as u64;

        let res = self.res.as_ref().unwrap();
        encoder.copy_buffer_to_buffer(&scale_buf, 0, &res.scale_buf, 0, byte_len);
    }

    fn invoke(&mut self, pass: usize, rect: Rect, p: Self::Param) {
        let aa = Vec2::from(rect.pos);
        let bb = aa + Vec2::from(rect.size);

        let ab = Vec2(aa.0, bb.1);
        let ba = Vec2(bb.0, aa.1);

        let scale = p.1 / (rect.size.0.min(rect.size.1) as f32);
        let cxy = (bb - aa) * scale;

        let caa = p.0 - cxy;
        let cbb = p.0 + cxy;
        let cab = Vec2(caa.0, cbb.1);
        let cba = Vec2(cbb.0, caa.1);

        #[rustfmt::skip]
        self.add_vertices(pass, &[
            Vertex(aa, caa), Vertex(ba, cba), Vertex(ab, cab),
            Vertex(ab, cab), Vertex(ba, cba), Vertex(bb, cbb),
        ]);
    }

    fn render(&mut self, device: &wgpu::Device, pass: usize, rpass: &mut wgpu::RenderPass) {
        if pass >= self.passes.len() {
            return;
        }
        let v = &mut self.passes[pass];
        let buffer = device
            .create_buffer_mapped(v.len(), wgpu::BufferUsage::VERTEX)
            .fill_from_slice(&v);
        let count = v.len() as u32;

        let res = self.res.as_ref().unwrap();
        rpass.set_pipeline(&res.render_pipeline);
        rpass.set_bind_group(0, &res.bind_group, &[]);
        rpass.set_vertex_buffers(0, &[(&buffer, 0)]);
        rpass.draw(0..count, 0..1);

        v.clear();
    }
}

impl Pipe {
    fn new() -> Self {
        Pipe {
            res: None,
            passes: vec![],
        }
    }

    fn add_vertices(&mut self, pass: usize, slice: &[Vertex]) {
        if self.passes.len() <= pass {
            // We only need one more, but no harm in adding extra
            self.passes.resize(pass + 8, vec![]);
        }

        self.passes[pass].extend_from_slice(slice);
    }
}

#[widget]
#[handler]
#[derive(Clone, Debug, kas :: macros :: Widget)]
struct Mandlebrot {
    #[core]
    core: kas::CoreData,
}

impl Layout for Mandlebrot {
    fn size_rules(&mut self, _: &mut dyn SizeHandle, _: AxisInfo) -> SizeRules {
        SizeRules::fixed(100)
    }

    #[inline]
    fn set_rect(&mut self, _size_handle: &mut dyn SizeHandle, rect: Rect, _align: AlignHints) {
        self.core.rect = rect;
    }

    fn draw(&self, draw_handle: &mut dyn DrawHandle, _: &ManagerState) {
        let (region, offset, draw) = draw_handle.draw_device();
        let draw = draw.as_any_mut().downcast_mut::<DrawPipe<Pipe>>().unwrap();
        let p = (Vec2(-0.5, 0.0), 1.0);
        draw.custom(region, self.core.rect + offset, p);
    }
}

impl Mandlebrot {
    fn new() -> Self {
        Mandlebrot {
            core: Default::default(),
        }
    }
}

fn main() -> Result<(), kas_wgpu::Error> {
    env_logger::init();

    let window = Window::new("Mandlebrot", Mandlebrot::new());

    let pipe = Pipe::new();
    let theme = kas_theme::FlatTheme::new();
    let mut toolkit = kas_wgpu::Toolkit::new_custom(pipe, theme, Options::from_env())?;
    toolkit.add(window)?;
    toolkit.run()
}
