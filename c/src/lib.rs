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
use pathfinder_canvas::{CanvasRenderingContext2D, Path2D};
use pathfinder_geometry::basic::point::{Point2DF, Point2DI};
use pathfinder_geometry::basic::rect::{RectF, RectI};
use pathfinder_geometry::color::ColorF;
use pathfinder_gl::{GLDevice, GLVersion};
use pathfinder_gpu::resources::{FilesystemResourceLoader, ResourceLoader};
use pathfinder_gpu::{ClearParams, Device};
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::concurrent::scene_proxy::SceneProxy;
use pathfinder_renderer::gpu::renderer::{DestFramebuffer, Renderer};
use pathfinder_renderer::options::RenderOptions;
use pathfinder_renderer::scene::Scene;
use pathfinder_simd::default::F32x4;
use std::ffi::CString;
use std::os::raw::{c_char, c_void};

// Constants

pub const PF_CLEAR_FLAGS_HAS_COLOR:   u8 = 0x1;
pub const PF_CLEAR_FLAGS_HAS_DEPTH:   u8 = 0x2;
pub const PF_CLEAR_FLAGS_HAS_STENCIL: u8 = 0x4;
pub const PF_CLEAR_FLAGS_HAS_RECT:    u8 = 0x8;

// Types

// `canvas`
pub type PFCanvasRef = *mut CanvasRenderingContext2D;
pub type PFPathRef = *mut Path2D;

// `geometry`
#[repr(C)]
pub struct PFPoint2DF {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
pub struct PFPoint2DI {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
pub struct PFRectF {
    pub origin: PFPoint2DF,
    pub lower_right: PFPoint2DF,
}
#[repr(C)]
pub struct PFRectI {
    pub origin: PFPoint2DI,
    pub lower_right: PFPoint2DI,
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
#[repr(C)]
pub struct PFClearParams {
    pub color: PFColorF,
    pub depth: f32,
    pub stencil: u8,
    pub rect: PFRectI,
    pub flags: PFClearFlags,
}
pub type PFClearFlags = u8;

// `renderer`
pub type PFSceneRef = *mut Scene;
pub type PFSceneProxyRef = *mut SceneProxy;
// TODO(pcwalton)
#[repr(C)]
pub struct PFRenderOptions {
    pub placeholder: u32,
}

// `canvas`

#[no_mangle]
pub unsafe extern "C" fn PFCanvasCreate(size: *const PFPoint2DF) -> PFCanvasRef {
    Box::into_raw(Box::new(CanvasRenderingContext2D::new((*size).to_rust())))
}

#[no_mangle]
pub unsafe extern "C" fn PFCanvasDestroy(canvas: PFCanvasRef) {
    drop(Box::from_raw(canvas))
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
pub unsafe extern "C" fn PFPathMoveTo(path: PFPathRef, to: *const PFPoint2DF) {
    (*path).move_to((*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathLineTo(path: PFPathRef, to: *const PFPoint2DF) {
    (*path).line_to((*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathQuadraticCurveTo(path: PFPathRef,
                                                ctrl: *const PFPoint2DF,
                                                to: *const PFPoint2DF) {
    (*path).quadratic_curve_to((*ctrl).to_rust(), (*to).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFPathBezierCurveTo(path: PFPathRef,
                                             ctrl0: *const PFPoint2DF,
                                             ctrl1: *const PFPoint2DF,
                                             to: *const PFPoint2DF) {
    (*path).bezier_curve_to((*ctrl0).to_rust(), (*ctrl1).to_rust(), (*to).to_rust())
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
pub unsafe extern "C" fn PFGLDeviceClear(device: PFGLDeviceRef, params: *const PFClearParams) {
    (*device).clear(&(*params).to_rust())
}

#[no_mangle]
pub unsafe extern "C" fn PFResourceLoaderDestroy(loader: PFResourceLoaderRef) {
    drop(Box::from_raw(loader))
}

// `gpu`

#[no_mangle]
pub unsafe extern "C" fn PFGLDestFramebufferCreateFullWindow(window_size: *const PFPoint2DI)
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
                                            dest_framebuffer: PFGLDestFramebufferRef)
                                            -> PFGLRendererRef {
    Box::into_raw(Box::new(Renderer::new(*Box::from_raw(device),
                                         &**resources,
                                         *Box::from_raw(dest_framebuffer))))
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
                                                      options: *const PFRenderOptions) {
    (*scene_proxy).build_and_render(&mut *renderer, (*options).to_rust())
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

impl PFPoint2DF {
    #[inline]
    pub fn to_rust(&self) -> Point2DF {
        Point2DF::new(self.x, self.y)
    }
}

impl PFPoint2DI {
    #[inline]
    pub fn to_rust(&self) -> Point2DI {
        Point2DI::new(self.x, self.y)
    }
}

// Helpers for `gpu`

impl PFClearParams {
    pub fn to_rust(&self) -> ClearParams {
        ClearParams {
            color: if (self.flags & PF_CLEAR_FLAGS_HAS_COLOR) != 0 {
                Some(self.color.to_rust())
            } else {
                None
            },
            rect: if (self.flags & PF_CLEAR_FLAGS_HAS_RECT) != 0 {
                Some(self.rect.to_rust())
            } else {
                None
            },
            depth: if (self.flags & PF_CLEAR_FLAGS_HAS_DEPTH) != 0 {
                Some(self.depth)
            } else {
                None
            },
            stencil: if (self.flags & PF_CLEAR_FLAGS_HAS_STENCIL) != 0 {
                Some(self.stencil)
            } else {
                None
            },
        }
    }
}

// Helpers for `renderer`

impl PFRenderOptions {
    pub fn to_rust(&self) -> RenderOptions {
        RenderOptions::default()
    }
}
