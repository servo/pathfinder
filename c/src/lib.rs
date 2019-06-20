// pathfinder/c/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! C bindings to Pathfinder.

use gl;
use pathfinder_canvas::{CanvasFontContext, CanvasRenderingContext2D, LineJoin, Path2D};
use pathfinder_geometry::basic::rect::{RectF, RectI};
use pathfinder_geometry::basic::vector::{Vector2F, Vector2I};
use pathfinder_geometry::color::ColorF;
use pathfinder_geometry::outline::ArcDirection;
use pathfinder_geometry::stroke::LineCap;
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_gpu::resources::{FilesystemResourceLoader, ResourceLoader};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::options::{DestFramebuffer, RendererOptions};
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_renderer::options::BuildOptions;
use pathfinder_renderer::scene::Scene;
use pathfinder_simd::default::F32x4;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::slice;

// Constants

// `canvas`

pub const PF_LINE_CAP_BUTT:   u8 = 0;
pub const PF_LINE_CAP_SQUARE: u8 = 1;
pub const PF_LINE_CAP_ROUND:  u8 = 2;

pub const PF_LINE_JOIN_MITER: u8 = 0;
pub const PF_LINE_JOIN_BEVEL: u8 = 1;
pub const PF_LINE_JOIN_ROUND: u8 = 2;

// `geometry`

pub const PF_ARC_DIRECTION_CW:  u8 = 0;
pub const PF_ARC_DIRECTION_CCW: u8 = 1;

// `renderer`

pub const PF_RENDERER_OPTIONS_FLAGS_HAS_BACKGROUND_COLOR: u8 = 0x1;

// Types

// `canvas`
pub type PFCanvasRef = *mut CanvasRenderingContext2D;
pub type PFPathRef = *mut Path2D;
pub type PFCanvasFontContextRef = *mut CanvasFontContext;
pub type PFLineCap = u8;
pub type PFLineJoin = u8;
pub type PFArcDirection = u8;

// `geometry`
#[repr(C)]
pub struct PFVector2F {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
pub struct PFVector2I {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
pub struct PFRectF {
    pub origin: PFVector2F,
    pub lower_right: PFVector2F,
}
#[repr(C)]
pub struct PFRectI {
    pub origin: PFVector2I,
    pub lower_right: PFVector2I,
}
#[repr(C)]
pub struct PFColorF {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

// `gl`
pub type PFGLDeviceRef = *mut GLDevice;
pub type PFGLVersion = GLVersion;
pub type PFGLFunctionLoader = extern "C" fn(name: *const c_char, userdata: *mut c_void)
                                            -> *const c_void;
// `gpu`
pub type PFGLDestFramebufferRef = *mut DestFramebuffer<GLDevice>;
pub type PFGLRendererRef = *mut Renderer<GLDevice>;
// FIXME(pcwalton): Double-boxing is unfortunate. Remove this when `std::raw::TraitObject` is
// stable?
pub type PFResourceLoaderRef = *mut Box<dyn ResourceLoader>;

// `renderer`
pub type PFSceneRef = *mut Scene;
pub type PFSceneProxyRef = *mut SceneProxy;
#[repr(C)]
pub struct PFRendererOptions {
    pub background_color: PFColorF,
    pub flags: PFRendererOptionsFlags,
}
pub type PFRendererOptionsFlags = u8;
// TODO(pcwalton)
#[repr(C)]
pub struct PFBuildOptions {
    pub placeholder: u32,
}

// `canvas`

/// Consumes the font context.
#[no_mangle]
pub unsafe extern "C" fn PFCanvasCreate(font_context: PFCanvasFontContextRef,
                                        size: *const PFVector2F)
                                        -> PFCanvasRef {
    Box::into_raw(Box::new(CanvasRenderingContext2D::new(*Box::from_raw(font_context),
                                                         (*size).to_rust())))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasDestroy(canvas: PFCanvasRef) {
    drop(Box::from_raw(canvas))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasFontContextCreate() -> PFCanvasFontContextRef {
    Box::into_raw(Box::new(CanvasFontContext::new()))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasFontContextDestroy(font_context: PFCanvasFontContextRef) {
    drop(Box::from_raw(font_context))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasFontContextClone(font_context: PFCanvasFontContextRef)
                                                  -> PFCanvasFontContextRef {
    Box::into_raw(Box::new((*font_context).clone()))
}

/// Consumes the canvas.
#[no_mangle]
pub unsafe extern "C" fn PFCanvasCreateScene(canvas: PFCanvasRef) -> PFSceneRef {
    Box::into_raw(Box::new(Box::from_raw(canvas).into_scene()))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasFillRect(canvas: PFCanvasRef, rect: *const PFRectF) {
    (*canvas).fill_rect((*rect).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasStrokeRect(canvas: PFCanvasRef, rect: *const PFRectF) {
    (*canvas).stroke_rect((*rect).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetLineWidth(canvas: PFCanvasRef, new_line_width: f32) {
    (*canvas).set_line_width(new_line_width)
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetLineCap(canvas: PFCanvasRef, new_line_cap: PFLineCap) {
    (*canvas).set_line_cap(match new_line_cap {
        PF_LINE_CAP_SQUARE => LineCap::Square,
        PF_LINE_CAP_ROUND  => LineCap::Round,
        _                  => LineCap::Butt,
    });
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetLineJoin(canvas: PFCanvasRef, new_line_join: PFLineJoin) {
    (*canvas).set_line_join(match new_line_join {
        PF_LINE_JOIN_BEVEL => LineJoin::Bevel,
        PF_LINE_JOIN_ROUND => LineJoin::Round,
        _                  => LineJoin::Miter,
    });
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetMiterLimit(canvas: PFCanvasRef, new_miter_limit: f32) {
    (*canvas).set_miter_limit(new_miter_limit);
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetLineDash(canvas: PFCanvasRef,
                                             new_line_dashes: *const f32,
                                             new_line_dash_count: usize) {
    (*canvas).set_line_dash(slice::from_raw_parts(new_line_dashes, new_line_dash_count).to_vec())
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasSetLineDashOffset(canvas: PFCanvasRef, new_offset: f32) {
    (*canvas).set_line_dash_offset(new_offset)
}

/// Consumes the path.
#[no_mangle]
pub unsafe extern "C" fn PFCanvasFillPath(canvas: PFCanvasRef, path: PFPathRef) {
    (*canvas).fill_path(*Box::from_raw(path))
}

/// Consumes the path.
#[no_mangle]
pub unsafe extern "C" fn PFCanvasStrokePath(canvas: PFCanvasRef, path: PFPathRef) {
    (*canvas).stroke_path(*Box::from_raw(path))
}

#[no_mangle]
pub unsafe extern "C" fn PFPathCreate() -> PFPathRef {
    Box::into_raw(Box::new(Path2D::new()))
}

#[no_mangle]
pub unsafe extern "C" fn PFPathDestroy(path: PFPathRef) {
    drop(Box::from_raw(path))
}

#[no_mangle]
pub unsafe extern "C" fn PFPathClone(path: PFPathRef) -> PFPathRef {
    Box::into_raw(Box::new((*path).clone()))
}

#[no_mangle]
pub unsafe extern "C" fn PFPathMoveTo(path: PFPathRef, to: *const PFVector2F) {
    (*path).move_to((*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathLineTo(path: PFPathRef, to: *const PFVector2F) {
    (*path).line_to((*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathQuadraticCurveTo(path: PFPathRef,
                                                ctrl: *const PFVector2F,
                                                to: *const PFVector2F) {
    (*path).quadratic_curve_to((*ctrl).to_rust(), (*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathBezierCurveTo(path: PFPathRef,
                                             ctrl0: *const PFVector2F,
                                             ctrl1: *const PFVector2F,
                                             to: *const PFVector2F) {
    (*path).bezier_curve_to((*ctrl0).to_rust(), (*ctrl1).to_rust(), (*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathArc(path: PFPathRef,
                                   center: *const PFVector2F,
                                   radius: f32,
                                   start_angle: f32,
                                   end_angle: f32,
                                   direction: PFArcDirection) {
    let direction = if direction == 0 { ArcDirection::CW } else { ArcDirection::CCW };
    (*path).arc((*center).to_rust(), radius, start_angle, end_angle, direction)
}

#[no_mangle]
pub unsafe extern "C" fn PFPathArcTo(path: PFPathRef,
                                     ctrl: *const PFVector2F,
                                     to: *const PFVector2F,
                                     radius: f32) {
    (*path).arc_to((*ctrl).to_rust(), (*to).to_rust(), radius)
}

#[no_mangle]
pub unsafe extern "C" fn PFPathRect(path: PFPathRef, rect: *const PFRectF) {
    (*path).rect((*rect).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathEllipse(path: PFPathRef,
                                       center: *const PFVector2F,
                                       axes: *const PFVector2F,
                                       rotation: f32,
                                       start_angle: f32,
                                       end_angle: f32) {
    (*path).ellipse((*center).to_rust(), (*axes).to_rust(), rotation, start_angle, end_angle)
}

#[no_mangle]
pub unsafe extern "C" fn PFPathClosePath(path: PFPathRef) {
    (*path).close_path()
}

// `gl`

#[no_mangle]
pub unsafe extern "C" fn PFFilesystemResourceLoaderLocate() -> PFResourceLoaderRef {
    let loader = Box::new(FilesystemResourceLoader::locate());
    Box::into_raw(Box::new(loader as Box<dyn ResourceLoader>))
}

#[no_mangle]
pub unsafe extern "C" fn PFGLLoadWith(loader: PFGLFunctionLoader, userdata: *mut c_void) {
    gl::load_with(|name| {
        let name = CString::new(name).unwrap();
        loader(name.as_ptr(), userdata)
    });
}

#[no_mangle]
pub unsafe extern "C" fn PFGLDeviceCreate(version: PFGLVersion, default_framebuffer: u32)
                                          -> PFGLDeviceRef {
    Box::into_raw(Box::new(GLDevice::new(version, default_framebuffer)))
}

#[no_mangle]
pub unsafe extern "C" fn PFGLDeviceDestroy(device: PFGLDeviceRef) {
    drop(Box::from_raw(device))
}

#[no_mangle]
pub unsafe extern "C" fn PFResourceLoaderDestroy(loader: PFResourceLoaderRef) {
    drop(Box::from_raw(loader))
}

// `gpu`

#[no_mangle]
pub unsafe extern "C" fn PFGLDestFramebufferCreateFullWindow(window_size: *const PFVector2I)
                                                             -> PFGLDestFramebufferRef {
    Box::into_raw(Box::new(DestFramebuffer::full_window((*window_size).to_rust())))
}

#[no_mangle]
pub unsafe extern "C" fn PFGLDestFramebufferDestroy(dest_framebuffer: PFGLDestFramebufferRef) {
    drop(Box::from_raw(dest_framebuffer))
}

/// Takes ownership of `device` and `dest_framebuffer`, but not `resources`.
#[no_mangle]
pub unsafe extern "C" fn PFGLRendererCreate(device: PFGLDeviceRef,
                                            resources: PFResourceLoaderRef,
                                            dest_framebuffer: PFGLDestFramebufferRef,
                                            options: *const PFRendererOptions)
                                            -> PFGLRendererRef {
    Box::into_raw(Box::new(Renderer::new(*Box::from_raw(device),
                                         &**resources,
                                         *Box::from_raw(dest_framebuffer),
                                         (*options).to_rust())))
}

#[no_mangle]
pub unsafe extern "C" fn PFGLRendererDestroy(renderer: PFGLRendererRef) {
    drop(Box::from_raw(renderer))
}

#[no_mangle]
pub unsafe extern "C" fn PFGLRendererGetDevice(renderer: PFGLRendererRef) -> PFGLDeviceRef {
    &mut (*renderer).device
}

#[no_mangle]
pub unsafe extern "C" fn PFSceneProxyBuildAndRenderGL(scene_proxy: PFSceneProxyRef,
                                                      renderer: PFGLRendererRef,
                                                      build_options: *const PFBuildOptions) {
    (*scene_proxy).build_and_render(&mut *renderer, (*build_options).to_rust())
}

// `renderer`

#[no_mangle]
pub unsafe extern "C" fn PFSceneDestroy(scene: PFSceneRef) {
    drop(Box::from_raw(scene))
}

#[no_mangle]
pub unsafe extern "C" fn PFSceneProxyCreateFromSceneAndRayonExecutor(scene: PFSceneRef)
                                                                     -> PFSceneProxyRef {
    Box::into_raw(Box::new(SceneProxy::from_scene(*Box::from_raw(scene), RayonExecutor)))
}

#[no_mangle]
pub unsafe extern "C" fn PFSceneProxyDestroy(scene_proxy: PFSceneProxyRef) {
    drop(Box::from_raw(scene_proxy))
}

// Helpers for `geometry`

impl PFColorF {
    #[inline]
    pub fn to_rust(&self) -> ColorF {
        ColorF(F32x4::new(self.r, self.g, self.b, self.a))
    }
}

impl PFRectF {
    #[inline]
    pub fn to_rust(&self) -> RectF {
        RectF::from_points(self.origin.to_rust(), self.lower_right.to_rust())
    }
}

impl PFRectI {
    #[inline]
    pub fn to_rust(&self) -> RectI {
        RectI::from_points(self.origin.to_rust(), self.lower_right.to_rust())
    }
}

impl PFVector2F {
    #[inline]
    pub fn to_rust(&self) -> Vector2F {
        Vector2F::new(self.x, self.y)
    }
}

impl PFVector2I {
    #[inline]
    pub fn to_rust(&self) -> Vector2I {
        Vector2I::new(self.x, self.y)
    }
}

// Helpers for `renderer`

impl PFRendererOptions {
    pub fn to_rust(&self) -> RendererOptions {
        let has_background_color = self.flags & PF_RENDERER_OPTIONS_FLAGS_HAS_BACKGROUND_COLOR;
        RendererOptions {
            background_color: if has_background_color != 0 {
                Some(self.background_color.to_rust())
            } else {
                None
            },
        }
    }
}

impl PFBuildOptions {
    pub fn to_rust(&self) -> BuildOptions {
        BuildOptions::default()
    }
}
