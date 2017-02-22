// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Apple Font Suitcase (`.dfont`) files.
//!
//! See: https://github.com/kreativekorp/ksfl/wiki/Macintosh-Resource-File-Format

use byteorder::{BigEndian, ReadBytesExt};
use error::FontError;
use font::Font;
use std::mem;
use util::Jump;

const SFNT: u32 = ((b's' as u32) << 24) |
                  ((b'f' as u32) << 16) |
                  ((b'n' as u32) << 8)  |
                   (b't' as u32);

impl<'a> Font<'a> {
    /// https://github.com/kreativekorp/ksfl/wiki/Macintosh-Resource-File-Format
    pub fn from_dfont_index<'b>(bytes: &'b [u8], index: u32) -> Result<Font<'b>, FontError> {
        let mut reader = bytes;

        // Read the Mac resource file header.
        let resource_data_offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        let resource_map_offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        let _resource_data_size = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        let _resource_map_size = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));

        // Move to the fields we care about in the resource map.
        reader = bytes;
        try!(reader.jump(resource_map_offset as usize + mem::size_of::<u32>() * 5 +
                         mem::size_of::<u16>() * 2).map_err(FontError::eof));

        // Read the type list and name list offsets.
        let type_list_offset = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        let _name_list_offset = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));

        // Move to the type list.
        reader = bytes;
        try!(reader.jump(resource_map_offset as usize + type_list_offset as usize)
                   .map_err(FontError::eof));

        // Find the 'sfnt' type.
        let type_count = (try!(reader.read_i16::<BigEndian>().map_err(FontError::eof)) + 1) as usize;
        let mut resource_count_and_list_offset = None;
        for _ in 0..type_count {
            let type_id = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let resource_count = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
            let resource_list_offset = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
            if type_id == SFNT {
                resource_count_and_list_offset = Some((resource_count, resource_list_offset));
                break
            }
        }

        // Unpack the resource count and list offset.
        let resource_count;
        match resource_count_and_list_offset {
            None => return Err(FontError::Failed),
            Some((count, resource_list_offset)) => {
                resource_count = count;
                reader = bytes;
                try!(reader.jump(resource_map_offset as usize + type_list_offset as usize +
                                 resource_list_offset as usize).map_err(FontError::eof));
            }
        }

        // Check whether the index is in bounds.
        if index >= resource_count as u32 + 1 {
            return Err(FontError::FontIndexOutOfBounds)
        }

        // Find the font we're interested in.
        try!(reader.jump(index as usize * (mem::size_of::<u16>() * 2 + mem::size_of::<u32>() * 2))
                   .map_err(FontError::eof));
        let _sfnt_id = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        let _sfnt_name_offset = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        let sfnt_data_offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof)) &
            0x00ffffff;
        let _sfnt_ptr = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));

        // Load the resource.
        reader = bytes;
        try!(reader.jump(resource_data_offset as usize + sfnt_data_offset as usize)
                   .map_err(FontError::eof));
        let sfnt_size = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        Font::from_otf(&reader[0..sfnt_size as usize], 0)
    }
}

