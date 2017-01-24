// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::Protection;
use compute_shader::device::Device;
use compute_shader::texture::{Format, Texture};
use euclid::size::Size2D;
use std::mem;

pub struct CoverageBuffer {
    pub texture: Texture,
}

impl CoverageBuffer {
    pub fn new(device: &Device, size: &Size2D<u32>) -> Result<CoverageBuffer, ()> {
        let texture = try!(device.create_texture(Format::R32F, Protection::ReadWrite, size)
                                 .map_err(drop));
        Ok(CoverageBuffer {
            texture: texture,
        })
    }
}

