// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::{Buffer, BufferData, Protection};
use compute_shader::device::Device;
use euclid::size::Size2D;
use std::mem;

pub struct CoverageBuffer {
    pub buffer: Buffer,
}

impl CoverageBuffer {
    pub fn new(device: &Device, size: &Size2D<u32>) -> Result<CoverageBuffer, ()> {
        let size = size.width as usize * size.height as usize * mem::size_of::<u32>();
        let buffer = try!(device.create_buffer(Protection::ReadWrite,
                                               BufferData::Uninitialized(size)).map_err(drop));
        Ok(CoverageBuffer {
            buffer: buffer,
        })
    }
}

