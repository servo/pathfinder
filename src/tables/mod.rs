// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! OpenType fonts.

// These tables need no parsing and so don't need separate files.
pub mod cvt {
    pub const TAG: u32 = ((b'c' as u32) << 24) |
                          ((b'v' as u32) << 16) |
                          ((b't' as u32) << 8)  |
                           (b' ' as u32);
}

pub mod fpgm {
    pub const TAG: u32 = ((b'f' as u32) << 24) |
                          ((b'p' as u32) << 16) |
                          ((b'g' as u32) << 8)  |
                           (b'm' as u32);
}

pub mod prep {
    pub const TAG: u32 = ((b'p' as u32) << 24) |
                          ((b'r' as u32) << 16) |
                          ((b'e' as u32) << 8)  |
                           (b'p' as u32);
}

pub mod cff;
pub mod cmap;
pub mod glyf;
pub mod head;
pub mod hhea;
pub mod hmtx;
pub mod kern;
pub mod loca;
pub mod os_2;

