// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::device::Device;
use compute_shader::program::Program;
use compute_shader::queue::Queue;

// TODO(pcwalton): Don't force that these be compiled in.
// TODO(pcwalton): GLSL version.
static ACCUM_CL_SHADER: &'static str = include_str!("../resources/shaders/accum.cl");
static DRAW_CL_SHADER: &'static str = include_str!("../resources/shaders/draw.cl");

pub struct Rasterizer {
    pub device: Device,
    pub queue: Queue,
    accum_program: Program,
    draw_program: Program,
}

impl Rasterizer {
    pub fn new(device: Device, queue: Queue) -> Result<Rasterizer, ()> {
        // TODO(pcwalton): GLSL version.
        // FIXME(pcwalton): Don't panic if these fail to compile; just return an error.
        let accum_program = device.create_program(ACCUM_CL_SHADER).unwrap();
        let draw_program = device.create_program(DRAW_CL_SHADER).unwrap();
        Ok(Rasterizer {
            device: device,
            queue: queue,
            accum_program: accum_program,
            draw_program: draw_program,
        })
    }
}

