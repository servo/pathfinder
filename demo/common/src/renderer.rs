// pathfinder/demo/common/src/renderer.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Rendering functionality for the demo.

use crate::camera::{Camera, Mode};
use crate::window::{View, Window};
use crate::{BackgroundColor, DemoApp, UIVisibility};
use image::ColorType;
use pathfinder_geometry::color::{ColorF, ColorU};
use pathfinder_gpu::{ClearParams, DepthFunc, DepthState, Device, Primitive, RenderState};
use pathfinder_gpu::{TextureFormat, UniformData};
use pathfinder_geometry::basic::transform3d::Transform3DF;
use pathfinder_renderer::gpu::renderer::{DestFramebuffer, RenderMode};
use pathfinder_renderer::gpu_data::RenderCommand;
use pathfinder_renderer::options::RenderTransform;
use pathfinder_renderer::post::DEFRINGING_KERNEL_CORE_GRAPHICS;
use std::path::PathBuf;

const GROUND_SOLID_COLOR: ColorU = ColorU {
    r: 80,
    g: 80,
    b: 80,
    a: 255,
};

const GROUND_LINE_COLOR: ColorU = ColorU {
    r: 127,
    g: 127,
    b: 127,
    a: 255,
};

const GRIDLINE_COUNT: i32 = 10;

impl<W> DemoApp<W> where W: Window {
    pub fn prepare_frame_rendering(&mut self) -> u32 {
        // Make the GL context current.
        let view = self.ui_model.mode.view(0);
        self.window.make_current(view);

        // Set up framebuffers.
        let window_size = self.window_size.device_size();
        let scene_count = match self.camera.mode() {
            Mode::VR => {
                let viewport = self.window.viewport(View::Stereo(0));
                if self.scene_framebuffer.is_none()
                    || self.renderer.device.texture_size(
                        &self
                            .renderer
                            .device
                            .framebuffer_texture(self.scene_framebuffer.as_ref().unwrap()),
                    ) != viewport.size()
                {
                    let scene_texture = self
                        .renderer
                        .device
                        .create_texture(TextureFormat::RGBA8, viewport.size());
                    self.scene_framebuffer =
                        Some(self.renderer.device.create_framebuffer(scene_texture));
                }
                self.renderer
                    .replace_dest_framebuffer(DestFramebuffer::Other(
                        self.scene_framebuffer.take().unwrap(),
                    ));
                2
            }
            _ => {
                self.renderer
                    .replace_dest_framebuffer(DestFramebuffer::Default {
                        viewport: self.window.viewport(View::Mono),
                        window_size,
                    });
                1
            }
        };

        // Begin drawing the scene.
        self.renderer.bind_dest_framebuffer();

        // Clear to the appropriate color.
        let clear_color = if scene_count == 2 {
            ColorF::transparent_black()
        } else {
            self.background_color().to_f32()
        };
        self.renderer.device.clear(&ClearParams {
            color: Some(clear_color),
            depth: Some(1.0),
            stencil: Some(0),
            ..ClearParams::default()
        });

        scene_count
    }

    pub fn draw_scene(&mut self) {
        let view = self.ui_model.mode.view(0);
        self.window.make_current(view);

        if self.camera.mode() != Mode::VR {
            self.draw_environment();
        }

        self.render_vector_scene();

        // Reattach default framebuffer.
        if self.camera.mode() != Mode::VR {
            return;
        }

        if let DestFramebuffer::Other(scene_framebuffer) =
            self.renderer
                .replace_dest_framebuffer(DestFramebuffer::Default {
                    viewport: self.window.viewport(View::Mono),
                    window_size: self.window_size.device_size(),
                })
        {
            self.scene_framebuffer = Some(scene_framebuffer);
        }
    }

    pub fn composite_scene(&mut self, render_scene_index: u32) {
        let (eye_transforms, scene_transform, modelview_transform) = match self.camera {
            Camera::ThreeD {
                ref eye_transforms,
                ref scene_transform,
                ref modelview_transform,
                ..
            } if eye_transforms.len() > 1 => (eye_transforms, scene_transform, modelview_transform),
            _ => return,
        };

        debug!(
            "scene_transform.perspective={:?}",
            scene_transform.perspective
        );
        debug!(
            "scene_transform.modelview_to_eye={:?}",
            scene_transform.modelview_to_eye
        );
        debug!("modelview transform={:?}", modelview_transform);

        let viewport = self.window.viewport(View::Stereo(render_scene_index));
        self.window.make_current(View::Stereo(render_scene_index));

        self.renderer
            .replace_dest_framebuffer(DestFramebuffer::Default {
                viewport,
                window_size: self.window_size.device_size(),
            });

        self.renderer.bind_draw_framebuffer();
        self.renderer.device.clear(&ClearParams {
            color: Some(self.background_color().to_f32()),
            depth: Some(1.0),
            stencil: Some(0),
            rect: Some(viewport),
        });

        self.draw_environment();

        let scene_framebuffer = self.scene_framebuffer.as_ref().unwrap();
        let scene_texture = self.renderer.device.framebuffer_texture(scene_framebuffer);

        let quad_scale_transform = Transform3DF::from_scale(
            self.scene_metadata.view_box.size().x(),
            self.scene_metadata.view_box.size().y(),
            1.0,
        );

        let scene_transform_matrix = scene_transform
            .perspective
            .post_mul(&scene_transform.modelview_to_eye)
            .post_mul(&modelview_transform.to_transform())
            .post_mul(&quad_scale_transform);

        let eye_transform = &eye_transforms[render_scene_index as usize];
        let eye_transform_matrix = eye_transform
            .perspective
            .post_mul(&eye_transform.modelview_to_eye)
            .post_mul(&modelview_transform.to_transform())
            .post_mul(&quad_scale_transform);

        debug!(
            "eye transform({}).modelview_to_eye={:?}",
            render_scene_index, eye_transform.modelview_to_eye
        );
        debug!(
            "eye transform_matrix({})={:?}",
            render_scene_index, eye_transform_matrix
        );
        debug!("---");

        self.renderer.reproject_texture(
            scene_texture,
            &scene_transform_matrix.transform,
            &eye_transform_matrix.transform,
        );
    }

    // Draws the ground, if applicable.
    fn draw_environment(&self) {
        let frame = &self.current_frame.as_ref().unwrap();

        let perspective = match frame.transform {
            RenderTransform::Transform2D(..) => return,
            RenderTransform::Perspective(perspective) => perspective,
        };

        if self.ui_model.background_color == BackgroundColor::Transparent {
            return;
        }

        let ground_scale = self.scene_metadata.view_box.max_x() * 2.0;

        let mut base_transform = perspective.transform;
        base_transform = base_transform.post_mul(&Transform3DF::from_translation(
            -0.5 * self.scene_metadata.view_box.max_x(),
            self.scene_metadata.view_box.max_y(),
            -0.5 * ground_scale,
        ));

        // Fill ground.
        let mut transform = base_transform;
        transform =
            transform.post_mul(&Transform3DF::from_scale(ground_scale, 1.0, ground_scale));

        let device = &self.renderer.device;
        device.bind_vertex_array(&self.ground_vertex_array.vertex_array);
        device.use_program(&self.ground_program.program);
        device.set_uniform(
            &self.ground_program.program,
            &self.ground_program.transform_uniform,
            UniformData::from_transform_3d(&transform),
        );
        device.set_uniform(
            &self.ground_program.program,
            &self.ground_program.ground_color_uniform,
            UniformData::Vec4(GROUND_SOLID_COLOR.to_f32().0),
        );
        device.set_uniform(
            &self.ground_program.program,
            &self.ground_program.gridline_color_uniform,
            UniformData::Vec4(GROUND_LINE_COLOR.to_f32().0),
        );
        device.set_uniform(&self.ground_program.program,
                           &self.ground_program.gridline_count_uniform,
                           UniformData::Int(GRIDLINE_COUNT));
        device.draw_elements(
            Primitive::Triangles,
            6,
            &RenderState {
                depth: Some(DepthState { func: DepthFunc::Less, write: true }),
                ..RenderState::default()
            },
        );
    }

    fn render_vector_scene(&mut self) {
        match self.scene_metadata.monochrome_color {
            None => self.renderer.set_render_mode(RenderMode::Multicolor),
            Some(fg_color) => {
                self.renderer.set_render_mode(RenderMode::Monochrome {
                    fg_color: fg_color.to_f32(),
                    bg_color: self.background_color().to_f32(),
                    gamma_correction: self.ui_model.gamma_correction_effect_enabled,
                    defringing_kernel: if self.ui_model.subpixel_aa_effect_enabled {
                        // TODO(pcwalton): Select FreeType defringing kernel as necessary.
                        Some(DEFRINGING_KERNEL_CORE_GRAPHICS)
                    } else {
                        None
                    },
                })
            }
        }

        if self.ui_model.mode == Mode::TwoD {
            self.renderer.disable_depth();
        } else {
            self.renderer.enable_depth();
        }

        self.renderer.begin_scene();

        // Issue render commands!
        for command in self.render_command_stream.as_mut().unwrap() {
            self.renderer.render_command(&command);

            if let RenderCommand::Finish { build_time } = command {
                self.build_time = Some(build_time);
            }
        }

        self.current_frame
            .as_mut()
            .unwrap()
            .scene_stats
            .push(self.renderer.stats);
        self.renderer.end_scene();
    }

    pub fn take_raster_screenshot(&mut self, path: PathBuf) {
        let drawable_size = self.window_size.device_size();
        let pixels = self
            .renderer
            .device
            .read_pixels_from_default_framebuffer(drawable_size);
        image::save_buffer(
            path,
            &pixels,
            drawable_size.x() as u32,
            drawable_size.y() as u32,
            ColorType::RGBA(8),
        )
        .unwrap();
    }

    pub fn draw_debug_ui(&mut self) {
        if self.options.ui == UIVisibility::None {
            return;
        }

        let viewport = self.window.viewport(View::Mono);
        self.window.make_current(View::Mono);
        self.renderer.replace_dest_framebuffer(DestFramebuffer::Default {
            viewport,
            window_size: self.window_size.device_size(),
        });

        self.renderer.draw_debug_ui();
    }
}
