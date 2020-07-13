// pathfinder/renderer/src/gpu/perf.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Performance monitoring infrastructure.

use crate::gpu::options::RendererOptions;
use pathfinder_gpu::Device;
use std::mem;
use std::ops::{Add, Div};
use std::time::Duration;

/// Various GPU-side statistics about rendering.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    /// The total number of path objects in the scene.
    pub path_count: usize,
    /// The number of fill operations it took to render the scene.
    /// 
    /// A fill operation is a single edge in a 16x16 device pixel tile.
    pub fill_count: usize,
    /// The total number of 16x16 device pixel tile masks generated.
    pub alpha_tile_count: usize,
    /// The total number of 16x16 tiles needed to render the scene, including both alpha tiles and
    /// solid-color tiles.
    pub total_tile_count: usize,
    /// The amount of CPU time it took to build the scene.
    pub cpu_build_time: Duration,
    /// The number of GPU API draw calls it took to render the scene.
    pub drawcall_count: u32,
    /// The number of bytes of VRAM Pathfinder has allocated.
    /// 
    /// This may be higher than `gpu_bytes_committed` because Pathfinder caches some data for
    /// faster reuse.
    pub gpu_bytes_allocated: u64,
    /// The number of bytes of VRAM Pathfinder actually used for the frame.
    pub gpu_bytes_committed: u64,
}

impl Add<RenderStats> for RenderStats {
    type Output = RenderStats;
    fn add(self, other: RenderStats) -> RenderStats {
        RenderStats {
            path_count: self.path_count + other.path_count,
            alpha_tile_count: self.alpha_tile_count + other.alpha_tile_count,
            total_tile_count: self.total_tile_count + other.total_tile_count,
            fill_count: self.fill_count + other.fill_count,
            cpu_build_time: self.cpu_build_time + other.cpu_build_time,
            drawcall_count: self.drawcall_count + other.drawcall_count,
            gpu_bytes_allocated: self.gpu_bytes_allocated + other.gpu_bytes_allocated,
            gpu_bytes_committed: self.gpu_bytes_committed + other.gpu_bytes_committed,
        }
    }
}

impl Div<usize> for RenderStats {
    type Output = RenderStats;
    fn div(self, divisor: usize) -> RenderStats {
        RenderStats {
            path_count: self.path_count / divisor,
            alpha_tile_count: self.alpha_tile_count / divisor,
            total_tile_count: self.total_tile_count / divisor,
            fill_count: self.fill_count / divisor,
            cpu_build_time: self.cpu_build_time / divisor as u32,
            drawcall_count: self.drawcall_count / divisor as u32,
            gpu_bytes_allocated: self.gpu_bytes_allocated / divisor as u64,
            gpu_bytes_committed: self.gpu_bytes_committed / divisor as u64,
        }
    }
}

pub(crate) struct TimerQueryCache<D> where D: Device {
    free_queries: Vec<D::TimerQuery>,
}

pub(crate) struct PendingTimer<D> where D: Device {
    pub(crate) dice_times: Vec<TimerFuture<D>>,
    pub(crate) bin_times: Vec<TimerFuture<D>>,
    pub(crate) fill_times: Vec<TimerFuture<D>>,
    pub(crate) composite_times: Vec<TimerFuture<D>>,
    pub(crate) other_times: Vec<TimerFuture<D>>,
}

pub(crate) enum TimerFuture<D> where D: Device {
    Pending(D::TimerQuery),
    Resolved(Duration),
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum TimeCategory {
    Dice,
    Bin,
    Fill,
    Composite,
    Other,
}

impl<D> TimerQueryCache<D> where D: Device {
    pub(crate) fn new() -> TimerQueryCache<D> {
        TimerQueryCache { free_queries: vec![] }
    }

    pub(crate) fn alloc(&mut self, device: &D) -> D::TimerQuery {
        self.free_queries.pop().unwrap_or_else(|| device.create_timer_query())
    }

    pub(crate) fn free(&mut self, old_query: D::TimerQuery) {
        self.free_queries.push(old_query);
    }

    pub(crate) fn start_timing_draw_call(&mut self, device: &D, options: &RendererOptions<D>)
                                         -> Option<D::TimerQuery> {
        if !options.show_debug_ui {
            return None;
        }

        let timer_query = self.alloc(device);
        device.begin_timer_query(&timer_query);
        Some(timer_query)
    }
}

impl<D> PendingTimer<D> where D: Device {
    pub(crate) fn new() -> PendingTimer<D> {
        PendingTimer {
            dice_times: vec![],
            bin_times: vec![],
            fill_times: vec![],
            composite_times: vec![],
            other_times: vec![],
        }
    }

    pub(crate) fn poll(&mut self, device: &D) -> Vec<D::TimerQuery> {
        let mut old_queries = vec![];
        for future in self.dice_times.iter_mut().chain(self.bin_times.iter_mut())
                                                .chain(self.fill_times.iter_mut())
                                                .chain(self.composite_times.iter_mut())
                                                .chain(self.other_times.iter_mut()) {
            if let Some(old_query) = future.poll(device) {
                old_queries.push(old_query)
            }
        }
        old_queries
    }

    pub(crate) fn total_time(&self) -> Option<RenderTime> {
        let dice_time = total_time_of_timer_futures(&self.dice_times);
        let bin_time = total_time_of_timer_futures(&self.bin_times);
        let fill_time = total_time_of_timer_futures(&self.fill_times);
        let composite_time = total_time_of_timer_futures(&self.composite_times);
        let other_time = total_time_of_timer_futures(&self.other_times);
        match (dice_time, bin_time, fill_time, composite_time, other_time) {
            (Some(dice_time),
             Some(bin_time),
             Some(fill_time),
             Some(composite_time),
             Some(other_time)) => {
                Some(RenderTime { dice_time, bin_time, fill_time, composite_time, other_time })
            }
            _ => None,
        }
    }

    pub(crate) fn push_query(&mut self,
                             time_category: TimeCategory,
                             timer_query: Option<D::TimerQuery>) {
        let timer_future = match timer_query {
            None => return,
            Some(timer_query) => TimerFuture::new(timer_query),
        };
        match time_category {
            TimeCategory::Dice => self.dice_times.push(timer_future),
            TimeCategory::Bin => self.bin_times.push(timer_future),
            TimeCategory::Fill => self.fill_times.push(timer_future),
            TimeCategory::Composite => self.composite_times.push(timer_future),
            TimeCategory::Other => self.other_times.push(timer_future),
        }
    }
}

impl<D> TimerFuture<D> where D: Device {
    pub(crate) fn new(query: D::TimerQuery) -> TimerFuture<D> {
        TimerFuture::Pending(query)
    }

    fn poll(&mut self, device: &D) -> Option<D::TimerQuery> {
        let duration = match *self {
            TimerFuture::Pending(ref query) => device.try_recv_timer_query(query),
            TimerFuture::Resolved(_) => None,
        };
        match duration {
            None => None,
            Some(duration) => {
                match mem::replace(self, TimerFuture::Resolved(duration)) {
                    TimerFuture::Resolved(_) => unreachable!(),
                    TimerFuture::Pending(old_query) => Some(old_query),
                }
            }
        }
    }
}

fn total_time_of_timer_futures<D>(futures: &[TimerFuture<D>]) -> Option<Duration> where D: Device {
    let mut total = Duration::default();
    for future in futures {
        match *future {
            TimerFuture::Pending(_) => return None,
            TimerFuture::Resolved(time) => total += time,
        }
    }
    Some(total)
}

/// The amount of GPU time it took to render the scene, broken up into stages.
#[derive(Clone, Copy, Debug)]
pub struct RenderTime {
    /// How much GPU time it took to divide all edges in the scene into small lines.
    /// 
    /// This will be zero in the D3D9-level backend, since in that backend dicing is done on CPU.
    pub dice_time: Duration,
    /// How much GPU time it took to assign those diced microlines to tiles.
    /// 
    /// This will be zero in the D3D9-level backend, since in that backend binning is done on CPU.
    pub bin_time: Duration,
    /// How much GPU time it took to draw fills (i.e. render edges) to masks.
    pub fill_time: Duration,
    /// How much GPU time it took to draw the contents of the tiles to the output.
    pub composite_time: Duration,
    /// How much GPU time it took to execute miscellaneous tasks other than dicing, binning,
    /// filling, and compositing.
    pub other_time: Duration,
}

impl RenderTime {
    /// The total GPU time it took to render the scene.
    #[inline]
    pub fn total_time(&self) -> Duration {
        self.dice_time + self.bin_time + self.fill_time + self.composite_time + self.other_time
    }
}

impl Default for RenderTime {
    #[inline]
    fn default() -> RenderTime {
        RenderTime {
            dice_time: Duration::new(0, 0),
            bin_time: Duration::new(0, 0),
            fill_time: Duration::new(0, 0),
            composite_time: Duration::new(0, 0),
            other_time: Duration::new(0, 0),
        }
    }
}

impl Add<RenderTime> for RenderTime {
    type Output = RenderTime;

    #[inline]
    fn add(self, other: RenderTime) -> RenderTime {
        RenderTime {
            dice_time: self.dice_time + other.dice_time,
            bin_time: self.bin_time + other.bin_time,
            fill_time: self.fill_time + other.fill_time,
            composite_time: self.composite_time + other.composite_time,
            other_time: self.other_time + other.other_time,
        }
    }
}

impl Div<usize> for RenderTime {
    type Output = RenderTime;

    #[inline]
    fn div(self, divisor: usize) -> RenderTime {
        let divisor = divisor as u32;
        RenderTime {
            dice_time: self.dice_time / divisor,
            bin_time: self.bin_time / divisor,
            fill_time: self.fill_time / divisor,
            composite_time: self.composite_time / divisor,
            other_time: self.other_time / divisor,
        }
    }
}
