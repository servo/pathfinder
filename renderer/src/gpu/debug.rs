// pathfinder/renderer/src/gpu/debug.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A debug overlay.
//!
//! We don't render the demo UI text using Pathfinder itself so that we can use the debug UI to
//! debug Pathfinder if it's totally busted.
//!
//! The debug font atlas was generated using: <https://evanw.github.io/font-texture-generator/>

use crate::gpu::options::RendererLevel;
use crate::gpu::perf::{RenderStats, RenderTime};
use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::{Vector2I, vec2i};
use pathfinder_gpu::Device;
use pathfinder_gpu::allocator::GPUMemoryAllocator;
use pathfinder_resources::ResourceLoader;
use pathfinder_ui::{FONT_ASCENT, LINE_HEIGHT, PADDING, UIPresenter, WINDOW_COLOR};
use std::collections::VecDeque;
use std::ops::{Add, Div};
use std::time::Duration;

const SAMPLE_BUFFER_SIZE: usize = 60;

const STATS_WINDOW_WIDTH: i32 = 275;
const STATS_WINDOW_HEIGHT: i32 = LINE_HEIGHT * 4 + PADDING + 2;

const PERFORMANCE_WINDOW_WIDTH: i32 = 400;
const PERFORMANCE_WINDOW_HEIGHT_D3D9: i32 = LINE_HEIGHT * 8 + PADDING + 2;
const PERFORMANCE_WINDOW_HEIGHT_D3D11: i32 = LINE_HEIGHT * 10 + PADDING + 2;

const INFO_WINDOW_WIDTH: i32 = 425;
const INFO_WINDOW_HEIGHT: i32 = LINE_HEIGHT * 2 + PADDING + 2;

/// Manages the debug UI.
pub struct DebugUIPresenter<D> where D: Device {
    /// The general UI presenter object.
    /// 
    /// You can use this to draw your own application-specific debug widgets.
    pub ui_presenter: UIPresenter<D>,

    cpu_samples: SampleBuffer<RenderStats>,
    gpu_samples: SampleBuffer<RenderTime>,
    backend_name: &'static str,
    device_name: String,
    renderer_level: RendererLevel,
}

impl<D> DebugUIPresenter<D> where D: Device {
    pub(crate) fn new(device: &D,
                      resources: &dyn ResourceLoader,
                      framebuffer_size: Vector2I,
                      renderer_level: RendererLevel)
                      -> DebugUIPresenter<D> {
        let ui_presenter = UIPresenter::new(device, resources, framebuffer_size);
        DebugUIPresenter {
            ui_presenter,
            cpu_samples: SampleBuffer::new(),
            gpu_samples: SampleBuffer::new(),
            backend_name: device.backend_name(),
            device_name: device.device_name(),
            renderer_level,
        }
    }

    pub(crate) fn add_sample(&mut self, stats: RenderStats, rendering_time: RenderTime) {
        self.cpu_samples.push(stats);
        self.gpu_samples.push(rendering_time);
    }

    pub(crate) fn draw(&self, device: &D, allocator: &mut GPUMemoryAllocator<D>) {
        self.draw_stats_window(device, allocator);
        self.draw_performance_window(device, allocator);
        self.draw_info_window(device, allocator);
    }

    #[inline]
    pub(crate) fn set_framebuffer_size(&mut self, new_framebuffer_size: Vector2I) {
        self.ui_presenter.set_framebuffer_size(new_framebuffer_size)
    }

    fn draw_info_window(&self, device: &D, allocator: &mut GPUMemoryAllocator<D>) {
        let framebuffer_size = self.ui_presenter.framebuffer_size();
        let bottom = framebuffer_size.y() - PADDING;
        let window_rect = RectI::new(
            vec2i(framebuffer_size.x() - PADDING - INFO_WINDOW_WIDTH,
                  bottom - INFO_WINDOW_HEIGHT),
            vec2i(INFO_WINDOW_WIDTH, INFO_WINDOW_HEIGHT),
        );

        self.ui_presenter.draw_solid_rounded_rect(device, allocator, window_rect, WINDOW_COLOR);

        let origin = window_rect.origin() + vec2i(PADDING, PADDING + FONT_ASCENT);
        let level = match self.renderer_level {
            RendererLevel::D3D9 => "D3D9",
            RendererLevel::D3D11 => "D3D11",
        };
        self.ui_presenter.draw_text(device,
                                    allocator,
                                    &format!("{} ({} level)", self.backend_name, level),
                                    origin + vec2i(0, LINE_HEIGHT * 0),
                                    false);
        self.ui_presenter.draw_text(device,
                                    allocator,
                                    &self.device_name,
                                    origin + vec2i(0, LINE_HEIGHT * 1),
                                    false);

    }

    fn performance_window_size(&self) -> Vector2I {
        match self.renderer_level {
            RendererLevel::D3D9 => vec2i(PERFORMANCE_WINDOW_WIDTH, PERFORMANCE_WINDOW_HEIGHT_D3D9),
            RendererLevel::D3D11 => {
                vec2i(PERFORMANCE_WINDOW_WIDTH, PERFORMANCE_WINDOW_HEIGHT_D3D11)
            }
        }
    }

    fn draw_stats_window(&self, device: &D, allocator: &mut GPUMemoryAllocator<D>) {
        let performance_window_height = self.performance_window_size().y();

        let framebuffer_size = self.ui_presenter.framebuffer_size();
        let bottom = framebuffer_size.y() - PADDING;
        let window_rect = RectI::new(
            vec2i(framebuffer_size.x() - PADDING - STATS_WINDOW_WIDTH,
                  bottom -
                    PADDING -
                    INFO_WINDOW_HEIGHT -
                    performance_window_height -
                    PADDING -
                    STATS_WINDOW_HEIGHT),
            vec2i(STATS_WINDOW_WIDTH, STATS_WINDOW_HEIGHT));

        self.ui_presenter.draw_solid_rounded_rect(device, allocator, window_rect, WINDOW_COLOR);

        let mean_cpu_sample = self.cpu_samples.mean();
        let origin = window_rect.origin() + vec2i(PADDING, PADDING + FONT_ASCENT);
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Paths: {}", mean_cpu_sample.path_count),
            origin,
            false,
        );
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Tiles: {}", mean_cpu_sample.total_tile_count),
            origin + vec2i(0, LINE_HEIGHT * 1),
            false,
        );
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Masks: {}", mean_cpu_sample.alpha_tile_count),
            origin + vec2i(0, LINE_HEIGHT * 2),
            false,
        );
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Fills: {}", mean_cpu_sample.fill_count),
            origin + vec2i(0, LINE_HEIGHT * 3),
            false,
        );
    }

    fn draw_performance_window(&self, device: &D, allocator: &mut GPUMemoryAllocator<D>) {
        let performance_window_size = self.performance_window_size();

        let framebuffer_size = self.ui_presenter.framebuffer_size();
        let bottom = framebuffer_size.y() - PADDING;
        let window_rect = RectI::new(
            vec2i(framebuffer_size.x() - PADDING - performance_window_size.x(),
                  bottom - INFO_WINDOW_HEIGHT - PADDING - performance_window_size.y()),
            performance_window_size);

        self.ui_presenter.draw_solid_rounded_rect(device, allocator, window_rect, WINDOW_COLOR);

        let mean_cpu_sample = self.cpu_samples.mean();
        let mean_gpu_sample = self.gpu_samples.mean();
        let origin = window_rect.origin() + vec2i(PADDING, PADDING + FONT_ASCENT);

        let mut current_y = 0;
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Drawcalls: {}", mean_cpu_sample.drawcall_count),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("VRAM Alloc.: {:.1} MB",
                     mean_cpu_sample.gpu_bytes_allocated as f64 / (1024.0 * 1024.0)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("VRAM Commit: {:.1} MB",
                     mean_cpu_sample.gpu_bytes_committed as f64 / (1024.0 * 1024.0)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;

        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("CPU: {:.3} ms", duration_to_ms(mean_cpu_sample.cpu_build_time)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;

        match self.renderer_level {
            RendererLevel::D3D11 => {
                self.ui_presenter.draw_text(
                    device,
                    allocator,
                    &format!("GPU Dice: {:.3} ms", duration_to_ms(mean_gpu_sample.dice_time)),
                    origin + vec2i(0, current_y),
                    false,
                );
                current_y += LINE_HEIGHT;
                self.ui_presenter.draw_text(
                    device,
                    allocator,
                    &format!("GPU Bin: {:.3} ms", duration_to_ms(mean_gpu_sample.bin_time)),
                    origin + vec2i(0, current_y),
                    false,
                );
                current_y += LINE_HEIGHT;
            }
            RendererLevel::D3D9 => {}
        }
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("GPU Fill: {:.3} ms", duration_to_ms(mean_gpu_sample.fill_time)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("GPU Comp.: {:.3} ms", duration_to_ms(mean_gpu_sample.composite_time)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("GPU Other: {:.3} ms", duration_to_ms(mean_gpu_sample.other_time)),
            origin + vec2i(0, current_y),
            false,
        );
        current_y += LINE_HEIGHT;

        let mut wallclock_time = match self.renderer_level {
            RendererLevel::D3D11 => {
                duration_to_ms(mean_cpu_sample.cpu_build_time) +
                    duration_to_ms(mean_gpu_sample.fill_time)
            }
            RendererLevel::D3D9 => {
                f64::max(duration_to_ms(mean_cpu_sample.cpu_build_time),
                         duration_to_ms(mean_gpu_sample.fill_time))
            }
        };
        wallclock_time += duration_to_ms(mean_gpu_sample.composite_time) +
                          duration_to_ms(mean_gpu_sample.dice_time) +
                          duration_to_ms(mean_gpu_sample.bin_time) +
                          duration_to_ms(mean_gpu_sample.other_time);
        self.ui_presenter.draw_text(
            device,
            allocator,
            &format!("Wallclock: {:.3} ms", wallclock_time),
            origin + vec2i(0, current_y),
            false,
        );
    }

}

struct SampleBuffer<S>
where
    S: Add<S, Output = S> + Div<usize, Output = S> + Clone + Default,
{
    samples: VecDeque<S>,
}

impl<S> SampleBuffer<S>
where
    S: Add<S, Output = S> + Div<usize, Output = S> + Clone + Default,
{
    fn new() -> SampleBuffer<S> {
        SampleBuffer {
            samples: VecDeque::with_capacity(SAMPLE_BUFFER_SIZE),
        }
    }

    fn push(&mut self, time: S) {
        self.samples.push_back(time);
        while self.samples.len() > SAMPLE_BUFFER_SIZE {
            self.samples.pop_front();
        }
    }

    fn mean(&self) -> S {
        let mut mean = Default::default();
        if self.samples.is_empty() {
            return mean;
        }

        for time in &self.samples {
            mean = mean + (*time).clone();
        }

        mean / self.samples.len()
    }
}

#[derive(Clone, Default)]
struct CPUSample {
    elapsed: Duration,
    stats: RenderStats,
}

impl Add<CPUSample> for CPUSample {
    type Output = CPUSample;
    fn add(self, other: CPUSample) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed + other.elapsed,
            stats: self.stats + other.stats,
        }
    }
}

impl Div<usize> for CPUSample {
    type Output = CPUSample;
    fn div(self, divisor: usize) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed / (divisor as u32),
            stats: self.stats / divisor,
        }
    }
}

fn duration_to_ms(time: Duration) -> f64 {
    time.as_secs() as f64 * 1000.0 + time.subsec_nanos() as f64 / 1000000.0
}
