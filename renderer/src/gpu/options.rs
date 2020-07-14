// pathfinder/renderer/src/gpu/options.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Various options that control how the renderer behaves.

use pathfinder_color::ColorF;
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::Vector2I;
use pathfinder_gpu::{Device, FeatureLevel};

/// Renderer options that can't be changed after the renderer is created.
pub struct RendererMode {
    /// The level of hardware features that the renderer will attempt to use.
    pub level: RendererLevel,
}

/// Options that influence rendering that can be changed at runtime.
pub struct RendererOptions<D> where D: Device {
    /// Where the rendering should go: either to the default framebuffer (i.e. screen) or to a
    /// custom framebuffer.
    pub dest: DestFramebuffer<D>,
    /// The background color. If not present, transparent is assumed.
    pub background_color: Option<ColorF>,
    /// Whether to display the debug UI.
    pub show_debug_ui: bool,
}

/// The GPU API level that Pathfinder will use.
///
/// Note that this is a *level*, not a *backend*. Levels describe rough GPU feature requirements
/// instead of specific APIs. "D3D9" doesn't mean "Direct3D 9" specifically: rather, it's a more
/// convenient way to write something like "Direct3D 9/OpenGL 3.0/Metal/WebGL 2.0".
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RendererLevel {
    /// Direct3D 9/OpenGL 3.0/WebGL 2.0 compatibility. Bin on CPU, fill and composite on GPU.
    D3D9,
    /// Direct3D 11/OpenGL 4.3/Metal/Vulkan/WebGPU compatibility. Bin, fill, and composite on GPU.
    D3D11,
}

impl RendererMode {
    /// Creates a new `RendererMode` with a suitable API level for the given GPU device.
    #[inline]
    pub fn default_for_device<D>(device: &D) -> RendererMode where D: Device {
        RendererMode { level: RendererLevel::default_for_device(device) }
    }
}

impl<D> Default for RendererOptions<D> where D: Device {
    #[inline]
    fn default() -> RendererOptions<D> {
        RendererOptions {
            dest: DestFramebuffer::default(),
            background_color: None,
            show_debug_ui: false,
        }
    }
}

impl RendererLevel {
    /// Returns a suitable renderer level for the given device.
    pub fn default_for_device<D>(device: &D) -> RendererLevel where D: Device {
        match device.feature_level() {
            FeatureLevel::D3D10 => RendererLevel::D3D9,
            FeatureLevel::D3D11 => RendererLevel::D3D11,
        }
    }
}

/// Where the rendered content should go.
#[derive(Clone)]
pub enum DestFramebuffer<D> where D: Device {
    /// The rendered content should go to the default framebuffer (e.g. the window in OpenGL).
    Default {
        /// The rectangle within the window to draw in, in device pixels.
        viewport: RectI,
        /// The total size of the window in device pixels.
        window_size: Vector2I,
    },
    /// The rendered content should go to a non-default framebuffer (off-screen, typically).
    Other(D::Framebuffer),
}

impl<D> Default for DestFramebuffer<D> where D: Device {
    #[inline]
    fn default() -> DestFramebuffer<D> {
        DestFramebuffer::Default { viewport: RectI::default(), window_size: Vector2I::default() }
    }
}

impl<D> DestFramebuffer<D> where D: Device {
    /// Returns a `DestFramebuffer` object that renders to the entire contents of the default
    /// framebuffer.
    /// 
    /// The `window_size` parameter specifies the size of the window in device pixels.
    #[inline]
    pub fn full_window(window_size: Vector2I) -> DestFramebuffer<D> {
        let viewport = RectI::new(Vector2I::default(), window_size);
        DestFramebuffer::Default { viewport, window_size }
    }

    /// Returns the size of the destination buffer, in device pixels.
    #[inline]
    pub fn window_size(&self, device: &D) -> Vector2I {
        match *self {
            DestFramebuffer::Default { window_size, .. } => window_size,
            DestFramebuffer::Other(ref framebuffer) => {
                device.texture_size(device.framebuffer_texture(framebuffer))
            }
        }
    }
}
