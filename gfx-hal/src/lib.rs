// pathfinder/gl/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An OpenGL implementation of the device abstraction.

#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

use BackendBackend as Backend;

extern crate gfx_hal as hal;
extern crate shaderc;
extern crate log;
extern crate winit;

use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gpu as pf_gpu;
use pathfinder_simd::default::F32x4;
use rustache;
use std::ffi::CString;
use std::io::Cursor;
use std::mem;
use std::ptr;
use std::str;
use std::time::Duration;
use core::mem::ManuallyDrop;

use hal::{
    memory as hal_mem,
    command as hal_cmd,
};

use std::os::raw::c_void;
use std::os::raw::c_ulong;

pub struct HalDevice {
    buffer: ManuallyDrop<<Backend as hal::Backend>::Buffer>,
    memory: ManuallyDrop<<Backend as hal::Backend>::Memory>,
    descriptor_set_layouts: Vec<<Backend as hal::Backend>::DescriptorSetLayout>,
    pipeline_layout: ManuallyDrop<<Backend as hal::Backend>::PipelineLayout>,
    graphics_pipeline: ManuallyDrop<<Backend as hal::Backend>::GraphicsPipeline>,
    requirements: hal_mem::Requirements,
    current_frame: usize,
    frames_in_flight: usize,
    in_flight_fences: Vec<<Backend as hal::Backend>::Fence>,
    render_finished_semaphores: Vec<<Backend as hal::Backend>::Semaphore>,
    image_available_semaphores: Vec<<Backend as hal::Backend>::Semaphore>,
    submission_command_buffers: Vec<hal_cmd::CommandBuffer<Backend, hal::Graphics, hal_cmd::MultiShot, hal_cmd::Primary>>,
    command_pool: ManuallyDrop<hal::pool::CommandPool<Backend, hal::Graphics>>,
    swapchain_framebuffers: Vec<<Backend as hal::Backend>::Framebuffer>,
    swapchain_image_views: Vec<(<Backend as hal::Backend>::ImageView)>,
    render_pass: ManuallyDrop<<Backend as hal::Backend>::RenderPass>,
    render_area: hal::pso::Rect,
    queue_group: hal::queue::QueueGroup<Backend, hal::Graphics>,
    swapchain: ManuallyDrop<<Backend as hal::Backend>::Swapchain>,
    device: ManuallyDrop<BackendDevice>,
    _adapter: hal::Adapter<Backend>,
    _surface: <Backend as hal::Backend>::Surface,
    _instance: ManuallyDrop<Backend::Instance>
}

impl HalDevice {
    pub fn new(window: &winit::Window, instance_name: &str) -> Result<HalDevice, &'static str> {
        let instance = Backend::Instance::create(instance_name, 1);

        if cfg!(all(unix, not(target_os = "android"), feature = "vulkan")) {

        }

        let mut surface = instance.create_surface(w);

        let adapter = HalDevice::pick_adapter(&instance, &surface);

        let (mut device, queue_group) = HalDevice::create_device_with_graphics_queues(&adapter, &surface);

        // initialize swapchain, this is extra long
        let (swapchain, extent, backbuffer, format, frames_in_flight) = HalDevice::create_swapchain(&adapter, &device, &mut surface, None);

        // create synchronization objects
        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) = HalDevice::create_synchronizers(&device);

        // create render pass
        let render_pass = HalDevice::create_renderpass(&device, Some(format));

        // create image views
        let swapchain_image_views: Vec<_> = HalDevice::create_image_views();

        let swapchain_framebuffers = HalDevice::create_framebuffers(&device, &render_pass, &image_views, extent);

        let mut command_pool = unsafe {
            device
                .create_command_pool_typed(&queue_group, hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL)
                .map_err(|_| "Could not create raw command pool.")?
        };

        let submission_command_buffers: Vec<_> = framebuffers
            .iter()
            .map(|_| command_pool.acquire_command_buffer())
            .collect();

        // Build our pipeline and vertex buffer
        let (descriptor_set_layouts, pipeline_layout, graphics_pipeline) =
            HalDevice::create_pipeline(&mut device, extent, &render_pass)?;
        let (buffer, memory, requirements) = unsafe {
            const F32_XY_TRIANGLE: u64 = (core::mem::size_of::<f32>() * 2 * 3) as u64;
            let mut buffer = device
                .create_buffer(F32_XY_TRIANGLE, hal::buffer::Usage::VERTEX)
                .map_err(|_| "Couldn't create a buffer for the vertices")?;
            let requirements = device.get_buffer_requirements(&buffer);
            let memory_type_id = adapter
                .physical_device
                .memory_properties()
                .memory_types
                .iter()
                .enumerate()
                .find(|&(id, memory_type)| {
                    requirements.type_mask & (1 << id) != 0
                        && memory_type.properties.contains(hal_mem::Properties::CPU_VISIBLE)
                })
                .map(|(id, _)| hal::adapter::MemoryTypeId(id))
                .ok_or("Couldn't find a memory type to support the vertex buffer!")?;
            let memory = device
                .allocate_memory(memory_type_id, requirements.size)
                .map_err(|_| "Couldn't allocate vertex buffer memory")?;
            device
                .bind_buffer_memory(&memory, 0, &mut buffer)
                .map_err(|_| "Couldn't bind the buffer memory!")?;
            (buffer, memory, requirements)
        };

        Ok(HalDevice {
            requirements,
            buffer: ManuallyDrop::new(buffer),
            memory: ManuallyDrop::new(memory),
            _instance: ManuallyDrop::new(instance),
            _surface: surface,
            _adapter: adapter,
            device: ManuallyDrop::new(device),
            queue_group,
            swapchain: ManuallyDrop::new(swapchain),
            render_area: extent.to_extent().rect(),
            render_pass: ManuallyDrop::new(render_pass),
            swapchain_image_views,
            swapchain_framebuffers,
            command_pool: ManuallyDrop::new(command_pool),
            submission_command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            frames_in_flight,
            current_frame: 0,
            descriptor_set_layouts,
            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            graphics_pipeline: ManuallyDrop::new(graphics_pipeline),
        })
    }

    fn pick_adapter(instance: &Backend::Instance, surface: &<Backend as hal::Backend>::Surface) -> Result<hal::adapter::Adapter<Backend>, &'static str>{
        // pick appropriate physical device (adapter)
        instance
            .enumerate_adapters()
            .into_iter()
            .find(|a| {
                a.queue_families
                    .iter()
                    .any(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
            })
            .ok_or("No physical device available with queue families which support graphics and presentation to surface.")?
    }


    fn create_device_with_graphics_queues(
        adapter: &mut hal::adapter::Adapter<Backend>,
        surface: &<Backend as hal::Backend>::Surface,
    ) -> (
        <Backend as hal::Backend>::Device,
        hal::queue::QueueGroup<Backend, hal::Graphics>,
        hal::queue::QueueType,
        hal::queue::family::QueueFamilyId,
    ) {
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                hal::Graphics::supported_by(family.queue_type())
                    && family.max_queues() > 0
                    && surface.supports_queue_family(family)
            })
            .expect("Could not find a queue family supporting graphics.");

        let priorities = vec![1.0; 1];
        let families = [(family, priorities.as_slice())];

        let hal::Gpu { device, mut queues } = unsafe {
            adapter
                .physical_device
                .open(&families, hal::Features::empty())
                .expect("Could not create device.")
        };

        let mut queue_group = queues
            .take::<hal::Graphics>(family.id())
            .expect("Could not take ownership of relevant queue group.");

        (device, queue_group, family.queue_type(), family.id())
    }

    fn create_swap_chain<W: Window>(
        adapter: &hal::adapter::Adapter<Backend>,
        device: &<Backend as hal::Backend>::Device,
        surface: &mut <Backend as hal::Backend>::Surface,
        previous_swapchain: Option<<Backend as hal::Backend>::Swapchain>,
        window: &winit::Window,
    ) -> (
        <Backend as hal::Backend>::Swapchain,
        hal::window::Extent2D,
        hal::window::Backbuffer<Backend>,
        hal::format::Format,
        usize,
    ) {
        let (caps, compatible_formats, compatible_present_modes, composite_alphas) =
            surface.compatibility(&adapter.physical_device);

        let present_mode = {
            use hal::window::PresentMode::{Mailbox, Fifo, Relaxed, Immediate};
            [Mailbox, Fifo, Relaxed, Immediate]
                .iter()
                .cloned()
                .find(|pm| compatible_present_modes.contains(pm))
                .ok_or("Surface does not support any known presentation mode.")?
        };

        let composite_alpha = {
            hal::window::CompositeAlpha::all()
                .iter()
                .cloned()
                .find(|ca| composite_alphas.contains(ca))
                .ok_or("Surface does not support any known alpha composition mode.")?
        };

        let format = match compatible_formats {
            None => hal::format::Format::Rgba8Srgb,
            Some(formats) => match formats
                .iter()
                .find(|format| format.base_format().1 == hal::format::ChannelType::Srgb)
                .cloned()
                {
                    Some(srgb_format) => srgb_format,
                    None => formats
                        .get(0)
                        .cloned()
                        .ok_or("Surface does not support any known format.")?,
                },
        };

        let extent = {
            let window_client_area = window
                .get_inner_size()
                .ok_or("Window doesn't exist!")?
                .to_physical(window.get_hidpi_factor());

            hal::window::Extent2D {
                width: caps.extents.end.width.min(window_client_area.width as u32),
                height: caps
                    .extents
                    .end
                    .height
                    .min(window_client_area.height as u32),
            }
        };

        let image_count = if present_mode == hal::window::PresentMode::Mailbox {
            (caps.image_count.end - 1).min(3)
        } else {
            (caps.image_count.end - 1).min(2)
        };

        let image_layers = 1;

        let image_usage = if caps.usage.contains(hal::image::Usage::COLOR_ATTACHMENT) {
            hal::image::Usage::COLOR_ATTACHMENT
        } else {
            Err("Surface does not support color attachments.")?
        };

        let swapchain_config = hal::window::SwapchainConfig {
            present_mode,
            composite_alpha,
            format,
            extent,
            image_count,
            image_layers,
            image_usage,
        };

        let (swapchain, backbuffer) = unsafe {
            device
                .create_swapchain(surface, swapchain_config, None)
                .map_err(|_| "Could not create swapchain.")?
        };

        (swapchain, extent, backbuffer, format, image_count as usize)
    }

    fn create_synchronizers(
        device: &<Backend as hal::Backend>::Device,
    ) -> (
        Vec<<Backend as hal::Backend>::Semaphore>,
        Vec<<Backend as hal::Backend>::Semaphore>,
        Vec<<Backend as hal::Backend>::Fence>,
    ) {
        let mut image_available_semaphores: Vec<<Backend as hal::Backend>::Semaphore> = Vec::new();
        let mut render_finished_semaphores: Vec<<Backend as hal::Backend>::Semaphore> = Vec::new();
        let mut in_flight_fences: Vec<<Backend as hal::Backend>::Fence> = Vec::new();

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_semaphores.push(device.create_semaphore().unwrap());
            render_finished_semaphores.push(device.create_semaphore().unwrap());
            in_flight_fences.push(device.create_fence(true).unwrap());
        }

        (
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
        )
    }

    fn create_render_pass(
        device: &<Backend as hal::Backend>::Device,
        format: Option<hal::format::Format>,
    ) -> <Backend as hal::Backend>::RenderPass {
        let samples: u8 = 1;

        let ops = hal::pass::AttachmentOps {
            load: hal::pass::AttachmentLoadOp::Clear,
            store: hal::pass::AttachmentStoreOp::Store,
        };

        let stencil_ops = hal::pass::AttachmentOps::DONT_CARE;

        let layouts = hal::image::Layout::Undefined..hal::image::Layout::Present;

        let color_attachment = hal::pass::Attachment {
            format,
            samples,
            ops,
            stencil_ops,
            layouts,
        };

        let color_attachment_ref: hal::pass::AttachmentRef = (0, hal::image::Layout::ColorAttachmentOptimal);

        // hal assumes pipeline bind point is GRAPHICS
        let subpass = hal::pass::SubpassDesc {
            colors: &[color_attachment_ref],
            depth_stencil: None,
            inputs: &[],
            resolves: &[],
            preserves: &[],
        };

        unsafe {
            device
                .create_render_pass(&[color_attachment], &[subpass], &[])
                .unwrap()
        }
    }

    unsafe fn create_image_views(
        backbuffer: hal::window::Backbuffer<Backend>,
        requested_format: hal::format::Format,
        device: &<Backend as hal::Backend>::Device,
    ) -> Vec<<Backend as hal::Backend>::ImageView> {
        match backbuffer {
            hal::window::Backbuffer::Images(images) => images
                .into_iter()
                .map(|image| {
                    let image_view = match device.create_image_view(
                        &image,
                        hal::image::ViewKind::D2,
                        requested_format,
                        hal::format::Swizzle::NO,
                        hal::image::SubresourceRange {
                            aspects: hal::format::Aspects::COLOR,
                            levels: 0..1,
                            layers: 0..1,
                        },
                    ) {
                        Ok(image_view) => image_view,
                        Err(_) => panic!("Error creating image view for an image."),
                    };

                    image_view
                })
                .collect(),
            _ => unimplemented!(),
        }
    }

    fn create_framebuffers(
        device: &<Backend as hal::Backend>::Device,
        render_pass: &<Backend as hal::Backend>::RenderPass,
        image_views: &[<Backend as hal::Backend>::ImageView],
        extent: hal::window::Extent2D,
    ) -> Vec<<Backend as hal::Backend>::Framebuffer> {
        let mut swapchain_framebuffers: Vec<<Backend as hal::Backend>::Framebuffer> = Vec::new();

        unsafe {
            for image_view in image_views.iter() {
                swapchain_framebuffers.push(
                    device
                        .create_framebuffer(
                            render_pass,
                            vec![image_view],
                            image::Extent {
                                width: extent.width as _,
                                height: extent.height as _,
                                depth: 1,
                            },
                        )
                        .expect("failed to create framebuffer!"),
                );
            }
        }

        swapchain_framebuffers
    }

    fn create_pipeline(
        device: &mut BackendDevice, extent: hal::window::Extent2D,
        render_pass: &<Backend as hal::Backend>::RenderPass,
    ) -> Result<
        (
            Vec<<Backend as hal::Backend>::DescriptorSetLayout>,
            <Backend as hal::Backend>::PipelineLayout,
            <Backend as hal::Backend>::GraphicsPipeline,
        ),
        &'static str,
    > {
        let mut compiler = shaderc::Compiler::new().ok_or("shaderc not found!")?;
        let vertex_compile_artifact = compiler
            .compile_into_spirv(
                VERTEX_SOURCE,
                shaderc::ShaderKind::Vertex,
                "vertex.vert",
                "main",
                None,
            )
            .map_err(|_| "Couldn't compile vertex shader!")?;
        let fragment_compile_artifact = compiler
            .compile_into_spirv(
                FRAGMENT_SOURCE,
                shaderc::ShaderKind::Fragment,
                "fragment.frag",
                "main",
                None,
            )
            .map_err(|e| {
                log::error!("{}", e);
                "Couldn't compile fragment shader!"
            })?;
        let vertex_shader_module = unsafe {
            device
                .create_shader_module(vertex_compile_artifact.as_binary_u8())
                .map_err(|_| "Couldn't make the vertex module")?
        };
        let fragment_shader_module = unsafe {
            device
                .create_shader_module(fragment_compile_artifact.as_binary_u8())
                .map_err(|_| "Couldn't make the fragment module")?
        };
        let (descriptor_set_layouts, pipeline_layout, graphics_pipeline) = {
            let (vs_entry, fs_entry) = (
                hal::pso::EntryPoint {
                    entry: "main",
                    module: &vertex_shader_module,
                    specialization: hal::pso::Specialization {
                        constants: &[],
                        data: &[],
                    },
                },
                hal::pso::EntryPoint {
                    entry: "main",
                    module: &fragment_shader_module,
                    specialization: hal::pso::Specialization {
                        constants: &[],
                        data: &[],
                    },
                },
            );
            let shaders = hal::pso::GraphicsShaderSet {
                vertex: vs_entry,
                hull: None,
                domain: None,
                geometry: None,
                fragment: Some(fs_entry),
            };

            let input_assembler = hal::pso::InputAssemblerDesc::new(hal::Primitive::TriangleList);

            let vertex_buffers: Vec<hal::pso::VertexBufferDesc> = vec![hal::pso::VertexBufferDesc {
                binding: 0,
                stride: (core::mem::size_of::<f32>() * 2) as u32,
                rate: hal::pso::VertexInputRate::Vertex,
            }];
            let attributes: Vec<hal::pso::AttributeDesc> = vec![hal::pso::AttributeDesc {
                location: 0,
                binding: 0,
                element: hal::pso::Element {
                    format: hal::format::Format::Rg32Float,
                    offset: 0,
                },
            }];

            let rasterizer = hal::pso::Rasterizer {
                depth_clamping: false,
                polygon_mode: hal::pso::PolygonMode::Fill,
                cull_face: hal::pso::Face::NONE,
                front_face: hal::pso::FrontFace::Clockwise,
                depth_bias: None,
                conservative: false,
            };

            let depth_stencil = hal::pso::DepthStencilDesc {
                depth: hal::pso::DepthTest::Off,
                depth_bounds: false,
                stencil: hal::pso::StencilTest::Off,
            };

            let blender = {
                let blend_state = hal::pso::BlendState::On {
                    color: hal::pso::BlendOp::Add {
                        src: hal::pso::Factor::One,
                        dst: hal::pso::Factor::Zero,
                    },
                    alpha: hal::pso::BlendOp::Add {
                        src: hal::pso::Factor::One,
                        dst: hal::pso::Factor::Zero,
                    },
                };
                hal::pso::BlendDesc {
                    logic_op: Some(hal::pso::LogicOp::Copy),
                    targets: vec![hal::pso::ColorBlendDesc(hal::pso::ColorMask::ALL, blend_state)],
                }
            };

            let baked_states = hal::pso::BakedStates {
                viewport: Some(hal::pso::Viewport {
                    rect: extent.to_extent().rect(),
                    depth: (0.0..1.0),
                }),
                scissor: Some(extent.to_extent().rect()),
                blend_color: None,
                depth_bounds: None,
            };

            let bindings = Vec::<hal::pso::DescriptorSetLayoutBinding>::new();
            let immutable_samplers = Vec::<<Backend as hal::Backend>::Sampler>::new();
            let descriptor_set_layouts: Vec<<Backend as hal::Backend>::DescriptorSetLayout> =
                vec![unsafe {
                    device
                        .create_descriptor_set_layout(bindings, immutable_samplers)
                        .map_err(|_| "Couldn't make a DescriptorSetLayout")?
                }];
            let push_constants = Vec::<(ShaderStageFlags, core::ops::Range<u32>)>::new();
            let layout = unsafe {
                device
                    .create_pipeline_layout(&descriptor_set_layouts, push_constants)
                    .map_err(|_| "Couldn't create a pipeline layout")?
            };

            let graphics_pipeline = {
                let desc = hal::pso::GraphicsPipelineDesc {
                    shaders,
                    rasterizer,
                    vertex_buffers,
                    attributes,
                    input_assembler,
                    blender,
                    depth_stencil,
                    multisampling: None,
                    baked_states,
                    layout: &layout,
                    subpass: hal::pass::Subpass {
                        index: 0,
                        main_pass: render_pass,
                    },
                    flags: hal::pso::PipelineCreationFlags::empty(),
                    parent: hal::pso::BasePipeline::None,
                };

                unsafe {
                    device
                        .create_graphics_pipeline(&desc, None)
                        .map_err(|_| "Couldn't create a graphics pipeline!")?
                }
            };

            (descriptor_set_layouts, layout, graphics_pipeline)
        };

        unsafe {
            device.destroy_shader_module(vertex_shader_module);
            device.destroy_shader_module(fragment_shader_module);
        }

        Ok((descriptor_set_layouts, pipeline_layout, graphics_pipeline))
    }
}

#[derive(Default)]
struct QueueFamilyIds {
    graphics_family: Option<hal::queue::QueueFamilyId>,
}

impl QueueFamilyIds {
    fn is_complete(&self) -> bool {
        self.graphics_family.is_some()
    }
}

impl core::ops::Drop for HalDevice {
    fn drop(&mut self) {
        let _ = self.device.wait_idle();

        unsafe {
            for descriptor_set_layout in self.descriptor_set_layouts.drain(..) {
                self
                    .device
                    .destroy_descriptor_set_layout(descriptor_set_layout)
            }

            for fence in self.in_flight_fences.drain(..) {
                self.device.destroy_fence(fence)
            }

            for semaphore in self.render_finished_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore)
            }

            for semaphore in self.image_available_semaphores.drain(..) {
                self.device.destroy_semaphore(semaphore)
            }

            for framebuffer in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(framebuffer);
            }

            for image_view in self.image_views.drain(..) {
                self.device.destroy_image_view(image_view);
            }

            // very unsure if this is the right way to do things
            use core::ptr::read;
            self
                .device
                .destroy_buffer(ManuallyDrop::into_inner(read(&self.buffer)));
            self
                .device
                .free_memory(ManuallyDrop::into_inner(read(&self.memory)));
            self
                .device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));
            self
                .device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.graphics_pipeline)));
            self
                .device
                .destroy_command_pool(ManuallyDrop::into_inner(read(&self.command_pool)).into_raw());
            self
                .device
                .destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));
            self
                .device
                .destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));

            ManuallyDrop::drop(&mut self.device);
            ManuallyDrop::drop(&mut self._instance);
        }
    }
}

// render state?
impl pf_gpu::Device for HalDevice {
    type Buffer = <Backend as hal::Backend>::Buffer;
    type Framebuffer = <Backend as hal::Backend>::Framebuffer;
    type Program = <Backend as hal::Backend>::GraphicsPipeline;
    type Shader = <Backend as hal::Backend>::ShaderModule;
    type Texture = <Backend as hal::Backend>::Image;
    type TimerQuery = hal::query::Query<'a, <Backend as hal::Backend>::QueryPool>; // query
    type Uniform = <Backend as hal::Backend>::Buffer; // staging resource?
    type VertexArray = <Backend as hal::Backend>::Buffer; // buffer
    type VertexAttr = usize; //usize

    fn create_texture(&self, format: pf_gpu::TextureFormat, size: Point2DI32) -> <Backend as hal::Backend>::Image {
    }

    fn create_texture_from_data(&self, size: Point2DI32, data: &[u8]) -> <Backend as hal::Backend>::Image {
    }

    fn create_shader_from_source(&self,
                                 name: &str,
                                 source: &[u8],
                                 kind: pf_gpu::ShaderKind,
                                 mut template_input: rustache::HashBuilder)
                                 -> ShaderModule {
    }

    fn create_program_from_shaders(&self,
                                   name: &str,
                                   vertex_shader: GLShader,
                                   fragment_shader: GLShader)
                                   -> <Backend as hal::Backend>::GraphicsPipeline {
    }

    fn create_vertex_array(&self) -> <Backend as hal::Backend>::Buffer {
    }

    fn get_vertex_attr(&self, program: &<Backend as hal::Backend>::GraphicsPipeline, name: &str) -> usize {
    }

    fn get_uniform(&self, program: &<Backend as hal::Backend>::GraphicsPipeline, name: &str) -> <Backend as hal::Backend>::Buffer {
    }

    fn use_program(&self, program: &<Backend as hal::Backend>::GraphicsPipeline) {
    }

    fn configure_float_vertex_attr(&self,
                                   attr: &usize,
                                   size: usize,
                                   attr_type: pf_gpu::VertexAttrType,
                                   normalized: bool,
                                   stride: usize,
                                   offset: usize,
                                   divisor: u32) {
    }

    fn configure_int_vertex_attr(&self,
                                 attr: &usize,
                                 size: usize,
                                 attr_type: pf_gpu::VertexAttrType,
                                 stride: usize,
                                 offset: usize,
                                 divisor: u32) {
    }

    fn set_uniform(&self, uniform: &<Backend as hal::Backend>::Buffer, data: pf_gpu::UniformData) {
    }

    fn create_framebuffer(&self, texture: <Backend as hal::Backend>::Buffer ) -> <Backend as hal::Backend>::Framebuffer {
    }

    fn create_buffer(&self) -> <Backend as hal::Backend>::Buffer {
    }

    fn upload_to_buffer<T>(&self,
                           buffer: &<Backend as hal::Backend>::Buffer,
                           data: &[T],
                           target: pf_gpu::BufferTarget,
                           mode: pf_gpu::BufferUploadMode) {
    }

    #[inline]
    fn framebuffer_texture<'f>(&self, framebuffer: &'f <Backend as hal::Backend>::Buffer ) -> &'f <Backend as hal::Backend>::Buffer  {
        &framebuffer.texture
    }

    #[inline]
    fn texture_size(&self, texture: &<Backend as hal::Backend>::Buffer ) -> Point2DI32 {
        texture.size
    }

    fn upload_to_texture(&self, texture: &<Backend as hal::Backend>::Buffer, size: Point2DI32, data: &[u8]) {
        unimplemented!();
    }

    fn read_pixels_from_default_framebuffer(&self, size: Point2DI32) -> Vec<u8> {
        unimplemented!();
    }

    // TODO(pcwalton): Switch to `ColorF`!
    fn clear(&self, color: Option<F32x4>, depth: Option<f32>, stencil: Option<u8>) {
        unimplemented!();
    }

    fn draw_arrays(&self, primitive: pf_gpu::Primitive, index_count: u32, render_state: &pf_gpu::RenderState) {
        unimplemented!();
    }

    fn draw_elements(&self, primitive: pf_gpu::Primitive, index_count: u32, render_state: &pf_gpu::RenderState) {
        unimplemented!();
    }

    fn draw_arrays_instanced(&self,
                             primitive: pf_gpu::Primitive,
                             index_count: u32,
                             instance_count: u32,
                             render_state: &pf_gpu::RenderState) {
        unimplemented!();

    }

    #[inline]
    fn create_timer_query(&self) -> hal::query::Query<'a, <Backend as hal::Backend>::QueryPool> {
        unimplemented!();
    }

    #[inline]
    fn begin_timer_query(&self, query: &hal::query::Query<'a, <Backend as hal::Backend>::QueryPool>) {
        unimplemented!();
    }

    #[inline]
    fn end_timer_query(&self, _: &hal::query::Query<'a, <Backend as hal::Backend>::QueryPool>) {
        unimplemented!();
    }

    #[inline]
    fn timer_query_is_available(&self, query: &hal::query::Query<'a, <Backend as hal::Backend>::QueryPool>) -> bool {
        unimplemented!();
    }

    #[inline]
    fn get_timer_query(&self, query: &hal::query::Query<'a, <Backend as hal::Backend>::QueryPool>) -> Duration {
        unimplemented!();
    }

    #[inline]
    fn bind_vertex_array(&self, vertex_array: &<Backend as hal::Backend>::Buffer) {
        unimplemented!();
    }

    #[inline]
    fn bind_buffer(&self, buffer: &GLBuffer, target: pf_gpu::BufferTarget) {
        unimplemented!();
    }

    #[inline]
    fn bind_default_framebuffer(&self, viewport: RectI32) {
        unimplemented!();
    }

    #[inline]
    fn bind_framebuffer(&self, framebuffer: &GLFramebuffer) {
        unimplemented!();
    }

    #[inline]
    fn bind_texture(&self, texture: &<Backend as hal::Backend>::Buffer, unit: u32) {
        unimplemented!();
    }
}
