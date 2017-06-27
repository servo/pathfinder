// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! TrueType Collection (`.ttc`) files.
//!
//! See Microsoft's spec: https://www.microsoft.com/typography/otspec/otff.htm

use byteorder::{BigEndian, ReadBytesExt};
use error::FontError;
use font::Font;
use std::mem;
use util::Jump;

pub const MAGIC_NUMBER: u32 = ((b't' as u32) << 24) |
                              ((b't' as u32) << 16) |
                              ((b'c' as u32) << 8)  |
                               (b'f' as u32);

impl<'a> Font<'a> {
    /// Creates a new font from a single font within a byte buffer containing the contents of a
    /// font collection.
    pub fn from_ttc_index<'b>(bytes: &'b [u8], index: u32) -> Result<Font<'b>, FontError> {
        let mut reader = bytes;
        let magic_number = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        if magic_number != MAGIC_NUMBER {
            return Err(FontError::UnknownFormat)
        }

        let major_version = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        let minor_version = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        if (major_version != 1 && major_version != 2) || minor_version != 0 {
            return Err(FontError::UnsupportedVersion)
        }

        let num_fonts = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        if index >= num_fonts {
            return Err(FontError::FontIndexOutOfBounds)
        }

        try!(reader.jump(index as usize * mem::size_of::<u32>()).map_err(FontError::eof));
        let table_offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        Font::from_otf(&bytes, table_offset)
    }
}

