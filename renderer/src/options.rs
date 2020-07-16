// pathfinder/renderer/src/options.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Options that control how rendering is to be performed.

use crate::gpu::options::RendererLevel;
use crate::gpu_data::RenderCommand;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::transform3d::Perspective;
use pathfinder_geometry::vector::{Vector2F, Vector4F};

#[allow(deprecated)]
use pathfinder_content::clip::PolygonClipper3D;

/// A sink for the render commands that scenes build.
/// 
/// In single-threaded operation, this object typically buffers commands into an array and then,
/// once scene building is complete, commands are all sent to the output at once. In multithreaded
/// operation, on the other hand, commands are sent to the renderer on the fly as they're built.
/// The latter is generally preferable for performance, because it allows the CPU and GPU to run
/// concurrently. However, it requires a multithreaded environment, which may not always be
/// available.
pub struct RenderCommandListener<'a> {
    send_fn: RenderCommandSendFunction<'a>,
}

/// The callback function that receives the render commands from the scene builder.
pub type RenderCommandSendFunction<'a> = Box<dyn Fn(RenderCommand) + Send + Sync + 'a>;

impl<'a> RenderCommandListener<'a> {
    /// Wraps a render command callback in a `RenderCommandListener`.
    #[inline]
    pub fn new(send_fn: RenderCommandSendFunction<'a>) -> RenderCommandListener<'a> {
        RenderCommandListener { send_fn }
    }

    #[inline]
    pub(crate) fn send(&self, render_command: RenderCommand) {
        (self.send_fn)(render_command)
    }
}

/// Options that influence scene building.
#[derive(Clone, Default)]
pub struct BuildOptions {
    /// A global transform to be applied to the scene.
    pub transform: RenderTransform,
    /// Expands outlines by the given number of device pixels. This is useful to perform *stem
    /// darkening* for fonts, to mitigate the thinness of gamma-corrected fonts.
    pub dilation: Vector2F,
    /// True if subpixel antialiasing for LCD screens is to be performed.
    pub subpixel_aa_enabled: bool,
}

impl BuildOptions {
    pub(crate) fn prepare(self, bounds: RectF) -> PreparedBuildOptions {
        PreparedBuildOptions {
            transform: self.transform.prepare(bounds),
            dilation: self.dilation,
            subpixel_aa_enabled: self.subpixel_aa_enabled,
        }
    }
}

/// A global transform to apply to the scene.
#[derive(Clone)]
pub enum RenderTransform {
    /// A 2D transform.
    Transform2D(Transform2F),
    /// A perspective transform. (This will soon be removed in favor of a revised 3D approach.)
    Perspective(Perspective),
}

impl Default for RenderTransform {
    #[inline]
    fn default() -> RenderTransform {
        RenderTransform::Transform2D(Transform2F::default())
    }
}

impl RenderTransform {
    #[allow(deprecated)]
    fn prepare(&self, bounds: RectF) -> PreparedRenderTransform {
        let perspective = match self {
            RenderTransform::Transform2D(ref transform) => {
                if transform.is_identity() {
                    return PreparedRenderTransform::None;
                }
                return PreparedRenderTransform::Transform2D(*transform);
            }
            RenderTransform::Perspective(ref perspective) => *perspective,
        };

        let mut points = vec![
            bounds.origin().to_4d(),
            bounds.upper_right().to_4d(),
            bounds.lower_right().to_4d(),
            bounds.lower_left().to_4d(),
        ];
        debug!("-----");
        debug!("bounds={:?} ORIGINAL quad={:?}", bounds, points);
        for point in &mut points {
            *point = perspective.transform * *point;
        }
        debug!("... PERSPECTIVE quad={:?}", points);

        // Compute depth.
        let quad = [
            points[0].to_3d().to_4d(),
            points[1].to_3d().to_4d(),
            points[2].to_3d().to_4d(),
            points[3].to_3d().to_4d(),
        ];
        debug!("... PERSPECTIVE-DIVIDED points = {:?}", quad);

        points = PolygonClipper3D::new(points).clip();
        debug!("... CLIPPED quad={:?}", points);
        for point in &mut points {
            *point = point.to_3d().to_4d()
        }

        let inverse_transform = perspective.transform.inverse();
        let clip_polygon = points.into_iter()
                                 .map(|point| (inverse_transform * point).to_2d())
                                 .collect();
        return PreparedRenderTransform::Perspective {
            perspective,
            clip_polygon,
            quad,
        };
    }
}

pub(crate) struct PreparedBuildOptions {
    pub(crate) transform: PreparedRenderTransform,
    pub(crate) dilation: Vector2F,
    pub(crate) subpixel_aa_enabled: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum PrepareMode {
    CPU,
    TransformCPUBinGPU,
    GPU { transform: Transform2F },
}

impl PreparedBuildOptions {
    #[inline]
    pub(crate) fn bounding_quad(&self) -> BoundingQuad {
        match self.transform {
            PreparedRenderTransform::Perspective { quad, .. } => quad,
            _ => [Vector4F::default(); 4],
        }
    }

    #[inline]
    pub(crate) fn to_prepare_mode(&self, renderer_level: RendererLevel) -> PrepareMode {
        match renderer_level {
            RendererLevel::D3D9 => PrepareMode::CPU,
            RendererLevel::D3D11 => {
                match self.transform {
                    PreparedRenderTransform::Perspective { .. } => PrepareMode::TransformCPUBinGPU,
                    PreparedRenderTransform::None => {
                        PrepareMode::GPU { transform: Transform2F::default() }
                    }
                    PreparedRenderTransform::Transform2D(transform) => {
                        PrepareMode::GPU { transform }
                    }
                }
            }
        }
    }
}

pub(crate) type BoundingQuad = [Vector4F; 4];

pub(crate) enum PreparedRenderTransform {
    None,
    Transform2D(Transform2F),
    Perspective {
        perspective: Perspective,
        clip_polygon: Vec<Vector2F>,
        quad: [Vector4F; 4],
    },
}

impl PreparedRenderTransform {
    #[inline]
    pub(crate) fn is_2d(&self) -> bool {
        match *self {
            PreparedRenderTransform::Transform2D(_) => true,
            _ => false,
        }
    }
}
