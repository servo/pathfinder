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
//! The debug font atlas was generated using: https://evanw.github.io/font-texture-generator/

use crate::gpu_data::Stats;
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_gpu::Device;
use pathfinder_gpu::resources::ResourceLoader;
use pathfinder_ui::{FONT_ASCENT, LINE_HEIGHT, PADDING, UI, WINDOW_COLOR};
use std::collections::VecDeque;
use std::ops::{Add, Div};
use std::time::Duration;

const SAMPLE_BUFFER_SIZE: usize = 60;

const PERF_WINDOW_WIDTH: i32 = 375;
const PERF_WINDOW_HEIGHT: i32 = LINE_HEIGHT * 6 + PADDING + 2;

pub struct DebugUI<D> where D: Device {
    pub ui: UI<D>,

    cpu_samples: SampleBuffer<CPUSample>,
    gpu_samples: SampleBuffer<GPUSample>,
}

impl<D> DebugUI<D> where D: Device {
    pub fn new(device: &D, resources: &dyn ResourceLoader, framebuffer_size: Point2DI32)
               -> DebugUI<D> {
        let ui = UI::new(device, resources, framebuffer_size);
        DebugUI { ui, cpu_samples: SampleBuffer::new(), gpu_samples: SampleBuffer::new() }
    }

    pub fn add_sample(&mut self,
                      stats: Stats,
                      tile_time: Duration,
                      rendering_time: Option<Duration>) {
        self.cpu_samples.push(CPUSample { stats, elapsed: tile_time });
        if let Some(rendering_time) = rendering_time {
            self.gpu_samples.push(GPUSample { elapsed: rendering_time });
        }
    }

    pub fn draw(&self, device: &D) {
        // Draw performance window.
        let framebuffer_size = self.ui.framebuffer_size();
        let bottom = framebuffer_size.y() - PADDING;
        let window_rect = RectI32::new(
            Point2DI32::new(framebuffer_size.x() - PADDING - PERF_WINDOW_WIDTH,
                            bottom - PERF_WINDOW_HEIGHT),
            Point2DI32::new(PERF_WINDOW_WIDTH, PERF_WINDOW_HEIGHT));
        self.ui.draw_solid_rounded_rect(device, window_rect, WINDOW_COLOR);
        let origin = window_rect.origin() + Point2DI32::new(PADDING, PADDING + FONT_ASCENT);

        let mean_cpu_sample = self.cpu_samples.mean();
        self.ui.draw_text(device,
                       &format!("Objects: {}", mean_cpu_sample.stats.object_count),
                       origin,
                       false);
        self.ui.draw_text(device,
                       &format!("Solid Tiles: {}", mean_cpu_sample.stats.solid_tile_count),
                       origin + Point2DI32::new(0, LINE_HEIGHT * 1),
                       false);
        self.ui.draw_text(device,
                          &format!("Alpha Tiles: {}", mean_cpu_sample.stats.alpha_tile_count),
                          origin + Point2DI32::new(0, LINE_HEIGHT * 2),
                          false);
        self.ui.draw_text(device,
                          &format!("Fills: {}", mean_cpu_sample.stats.fill_count),
                          origin + Point2DI32::new(0, LINE_HEIGHT * 3),
                          false);

        self.ui.draw_text(device,
                          &format!("CPU Time: {:.3} ms", duration_to_ms(mean_cpu_sample.elapsed)),
                          origin + Point2DI32::new(0, LINE_HEIGHT * 4),
                          false);

        let mean_gpu_sample = self.gpu_samples.mean();
        self.ui.draw_text(device,
                          &format!("GPU Time: {:.3} ms", duration_to_ms(mean_gpu_sample.elapsed)),
                          origin + Point2DI32::new(0, LINE_HEIGHT * 5),
                          false);
    }
}

struct SampleBuffer<S> where S: Add<S, Output=S> + Div<u32, Output=S> + Clone + Default {
    samples: VecDeque<S>,
}

impl<S> SampleBuffer<S> where S: Add<S, Output=S> + Div<u32, Output=S> + Clone + Default {
    fn new() -> SampleBuffer<S> {
        SampleBuffer { samples: VecDeque::with_capacity(SAMPLE_BUFFER_SIZE) }
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

        mean / self.samples.len() as u32
    }
}

#[derive(Clone, Default)]
struct CPUSample {
    elapsed: Duration,
    stats: Stats,
}

impl Add<CPUSample> for CPUSample {
    type Output = CPUSample;
    fn add(self, other: CPUSample) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed + other.elapsed,
            stats: Stats {
                object_count: self.stats.object_count + other.stats.object_count,
                solid_tile_count: self.stats.solid_tile_count + other.stats.solid_tile_count,
                alpha_tile_count: self.stats.alpha_tile_count + other.stats.alpha_tile_count,
                fill_count: self.stats.fill_count + other.stats.fill_count,
            },
        }
    }
}

impl Div<u32> for CPUSample {
    type Output = CPUSample;
    fn div(self, divisor: u32) -> CPUSample {
        CPUSample {
            elapsed: self.elapsed / divisor,
            stats: Stats {
                object_count: self.stats.object_count / divisor,
                solid_tile_count: self.stats.solid_tile_count / divisor,
                alpha_tile_count: self.stats.alpha_tile_count / divisor,
                fill_count: self.stats.fill_count / divisor,
            },
        }
    }
}

#[derive(Clone, Default)]
struct GPUSample {
    elapsed: Duration,
}

impl Add<GPUSample> for GPUSample {
    type Output = GPUSample;
    fn add(self, other: GPUSample) -> GPUSample {
        GPUSample { elapsed: self.elapsed + other.elapsed }
    }
}

impl Div<u32> for GPUSample {
    type Output = GPUSample;
    fn div(self, divisor: u32) -> GPUSample {
        GPUSample { elapsed: self.elapsed / divisor }
    }
}

fn duration_to_ms(time: Duration) -> f64 {
    time.as_secs() as f64 * 1000.0 + time.subsec_nanos() as f64 / 1000000.0
}
