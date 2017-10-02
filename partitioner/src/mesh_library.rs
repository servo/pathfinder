// pathfinder/partitioner/src/mesh_library.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bincode::{self, Infinite};
use byteorder::{LittleEndian, WriteBytesExt};
use euclid::Point2D;
use serde::Serialize;
use std::io::{self, ErrorKind, Seek, SeekFrom, Write};
use std::ops::Range;

use {BQuad, BVertexLoopBlinnData, CurveIndices, LineIndices};

#[derive(Debug, Clone)]
pub struct MeshLibrary {
    pub b_quads: Vec<BQuad>,
    pub b_vertex_positions: Vec<Point2D<f32>>,
    pub b_vertex_path_ids: Vec<u16>,
    pub b_vertex_loop_blinn_data: Vec<BVertexLoopBlinnData>,
    pub cover_indices: MeshLibraryCoverIndices,
    pub edge_indices: MeshLibraryEdgeIndices,
}

impl MeshLibrary {
    #[inline]
    pub fn new() -> MeshLibrary {
        MeshLibrary {
            b_quads: vec![],
            b_vertex_positions: vec![],
            b_vertex_path_ids: vec![],
            b_vertex_loop_blinn_data: vec![],
            cover_indices: MeshLibraryCoverIndices::new(),
            edge_indices: MeshLibraryEdgeIndices::new(),
        }
    }

    pub fn clear(&mut self) {
        self.b_quads.clear();
        self.b_vertex_positions.clear();
        self.b_vertex_path_ids.clear();
        self.b_vertex_loop_blinn_data.clear();
        self.cover_indices.clear();
        self.edge_indices.clear();
    }

    /// Writes this mesh library to a RIFF file.
    /// 
    /// RIFF is a dead-simple extensible binary format documented here:
    /// https://msdn.microsoft.com/en-us/library/windows/desktop/ee415713(v=vs.85).aspx
    pub fn serialize_into<W>(&self, writer: &mut W) -> io::Result<()> where W: Write + Seek {
        // `PFML` for "Pathfinder Mesh Library".
        try!(writer.write_all(b"RIFF\0\0\0\0PFML"));

        // NB: The RIFF spec requires that all chunks be padded to an even byte offset. However,
        // for us, this is guaranteed by construction because each instance of all of the data that
        // we're writing has a byte size that is a multiple of 4. So we don't bother with doing it
        // explicitly here.
        try!(write_chunk(writer, b"bqua", &self.b_quads));
        try!(write_chunk(writer, b"bvpo", &self.b_vertex_positions));
        try!(write_chunk(writer, b"bvpi", &self.b_vertex_path_ids));
        try!(write_chunk(writer, b"bvlb", &self.b_vertex_loop_blinn_data));
        try!(write_chunk(writer, b"cvii", &self.cover_indices.interior_indices));
        try!(write_chunk(writer, b"cvci", &self.cover_indices.curve_indices));
        try!(write_chunk(writer, b"euli", &self.edge_indices.upper_line_indices));
        try!(write_chunk(writer, b"euci", &self.edge_indices.upper_curve_indices));
        try!(write_chunk(writer, b"elli", &self.edge_indices.lower_line_indices));
        try!(write_chunk(writer, b"elci", &self.edge_indices.lower_curve_indices));

        let total_length = try!(writer.seek(SeekFrom::Current(0)));
        try!(writer.seek(SeekFrom::Start(4)));
        try!(writer.write_u32::<LittleEndian>((total_length - 8) as u32));
        return Ok(());

        fn write_chunk<W, T>(writer: &mut W, tag: &[u8; 4], data: &[T]) -> io::Result<()>
                             where W: Write + Seek, T: Serialize {
            try!(writer.write_all(tag));
            try!(writer.write_all(b"\0\0\0\0"));

            let start_position = try!(writer.seek(SeekFrom::Current(0)));
            for datum in data {
                try!(bincode::serialize_into(writer, datum, Infinite).map_err(|_| {
                    io::Error::from(ErrorKind::Other)
                }));
            }

            let end_position = try!(writer.seek(SeekFrom::Current(0)));
            try!(writer.seek(SeekFrom::Start(start_position - 4)));
            try!(writer.write_u32::<LittleEndian>((end_position - start_position) as u32));
            try!(writer.seek(SeekFrom::Start(end_position)));
            Ok(())
        }
    }

    pub(crate) fn snapshot_lengths(&self) -> MeshLibraryLengths {
        MeshLibraryLengths {
            b_quads: self.b_quads.len(),
            b_vertices: self.b_vertex_positions.len(),
            cover_interior_indices: self.cover_indices.interior_indices.len(),
            cover_curve_indices: self.cover_indices.curve_indices.len(),
            edge_upper_line_indices: self.edge_indices.upper_line_indices.len(),
            edge_upper_curve_indices: self.edge_indices.upper_curve_indices.len(),
            edge_lower_line_indices: self.edge_indices.lower_line_indices.len(),
            edge_lower_curve_indices: self.edge_indices.lower_curve_indices.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshLibraryCoverIndices {
    pub interior_indices: Vec<u32>,
    pub curve_indices: Vec<u32>,
}

impl MeshLibraryCoverIndices {
    #[inline]
    fn new() -> MeshLibraryCoverIndices {
        MeshLibraryCoverIndices {
            interior_indices: vec![],
            curve_indices: vec![],
        }
    }

    fn clear(&mut self) {
        self.interior_indices.clear();
        self.curve_indices.clear();
    }
}

#[derive(Debug, Clone)]
pub struct MeshLibraryEdgeIndices {
    pub upper_line_indices: Vec<LineIndices>,
    pub upper_curve_indices: Vec<CurveIndices>,
    pub lower_line_indices: Vec<LineIndices>,
    pub lower_curve_indices: Vec<CurveIndices>,
}

impl MeshLibraryEdgeIndices {
    #[inline]
    fn new() -> MeshLibraryEdgeIndices {
        MeshLibraryEdgeIndices {
            upper_line_indices: vec![],
            upper_curve_indices: vec![],
            lower_line_indices: vec![],
            lower_curve_indices: vec![],
        }
    }

    fn clear(&mut self) {
        self.upper_line_indices.clear();
        self.upper_curve_indices.clear();
        self.lower_line_indices.clear();
        self.lower_curve_indices.clear();
    }
}

pub(crate) struct MeshLibraryLengths {
    b_quads: usize,
    b_vertices: usize,
    cover_interior_indices: usize,
    cover_curve_indices: usize,
    edge_upper_line_indices: usize,
    edge_upper_curve_indices: usize,
    edge_lower_line_indices: usize,
    edge_lower_curve_indices: usize,
}

pub struct MeshLibraryIndexRanges {
    pub b_quads: Range<usize>,
    pub b_vertices: Range<usize>,
    pub cover_interior_indices: Range<usize>,
    pub cover_curve_indices: Range<usize>,
    pub edge_upper_line_indices: Range<usize>,
    pub edge_upper_curve_indices: Range<usize>,
    pub edge_lower_line_indices: Range<usize>,
    pub edge_lower_curve_indices: Range<usize>,
}

impl MeshLibraryIndexRanges {
    pub(crate) fn new(start: &MeshLibraryLengths, end: &MeshLibraryLengths)
                      -> MeshLibraryIndexRanges {
        MeshLibraryIndexRanges {
            b_quads: start.b_quads..end.b_quads,
            b_vertices: start.b_vertices..end.b_vertices,
            cover_interior_indices: start.cover_interior_indices..end.cover_interior_indices,
            cover_curve_indices: start.cover_curve_indices..end.cover_curve_indices,
            edge_upper_line_indices: start.edge_upper_line_indices..end.edge_upper_line_indices,
            edge_upper_curve_indices: start.edge_upper_curve_indices..end.edge_upper_curve_indices,
            edge_lower_line_indices: start.edge_lower_line_indices..end.edge_lower_line_indices,
            edge_lower_curve_indices: start.edge_lower_curve_indices..end.edge_lower_curve_indices,
        }
    }
}
