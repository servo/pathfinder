// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Web Open Font Format 1.0 (`.woff`) files.
//!
//! See the specification: https://www.w3.org/TR/WOFF/
//!
//! TODO(pcwalton): WOFF 2.0.

use byteorder::{BigEndian, ReadBytesExt};
use containers::otf::{KNOWN_TABLES, KNOWN_TABLE_COUNT, SFNT_VERSIONS};
use error::FontError;
use flate2::FlateReadExt;
use font::{Font, FontTable};
use std::io::Read;
use std::iter;
use std::mem;
use util::Jump;

pub const MAGIC_NUMBER: u32 = ((b'w' as u32) << 24) |
                               ((b'O' as u32) << 16) |
                               ((b'F' as u32) << 8) |
                                (b'F' as u32);

impl<'a> Font<'a> {
    /// Creates a new font from a buffer containing data in the WOFF format.
    ///
    /// The given buffer will be used to decompress data.
    ///
    /// Decompresses eagerly.
    pub fn from_woff<'b>(bytes: &'b [u8], buffer: &'b mut Vec<u8>) -> Result<Font<'b>, FontError> {
        let mut reader = bytes;

        // Check magic number.
        let magic_number = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        if magic_number != MAGIC_NUMBER {
            return Err(FontError::UnknownFormat)
        }

        // Check the flavor.
        let flavor = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        if !SFNT_VERSIONS.contains(&flavor) {
            return Err(FontError::UnknownFormat)
        }

        // Get the number of tables.
        let _length = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        let num_tables = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));
        let _reserved = try!(reader.read_u16::<BigEndian>().map_err(FontError::eof));

        // Allocate size for uncompressed tables.
        let total_sfnt_size = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
        try!(reader.jump(mem::size_of::<u32>() * 6).map_err(FontError::eof));
        let buffer_start = buffer.len();
        buffer.extend(iter::repeat(0).take(total_sfnt_size as usize));
        let mut buffer = &mut buffer[buffer_start..];

        // Decompress and load tables as necessary.
        let mut tables = [None; KNOWN_TABLE_COUNT];
        for _ in 0..num_tables {
            let tag = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let offset = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let comp_length = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let orig_length = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));
            let _orig_checksum = try!(reader.read_u32::<BigEndian>().map_err(FontError::eof));

            // Find the table ID in our list of known IDs, which must be sorted.
            debug_assert!(KNOWN_TABLES.windows(2).all(|w| w[0] < w[1]));
            let slot = match KNOWN_TABLES.binary_search(&tag) {
                Err(_) => continue,
                Ok(table_index) => &mut tables[table_index],
            };

            // Make sure there isn't more than one copy of the table.
            if slot.is_some() {
                return Err(FontError::Failed)
            }

            // Allocate space in the buffer.
            let comp_end = offset as usize + comp_length as usize;
            let mut temp = buffer;  // borrow check black magic
            let (mut dest, mut rest) = temp.split_at_mut(orig_length as usize);
            buffer = rest;

            // Decompress or copy as applicable.
            //
            // FIXME(pcwalton): Errors here may be zlib errors, not EOFs.
            if comp_length != orig_length {
                let mut table_reader = bytes;
                try!(table_reader.jump(offset as usize).map_err(FontError::eof));
                let mut table_reader = table_reader.zlib_decode();
                try!(table_reader.read_exact(dest).map_err(FontError::eof));
            } else if comp_end <= bytes.len() {
                dest.clone_from_slice(&bytes[offset as usize..comp_end])
            } else {
                return Err(FontError::UnexpectedEof)
            }

            *slot = Some(FontTable {
                bytes: dest,
            })
        }

        Font::from_table_list(bytes, &tables)
    }
}

