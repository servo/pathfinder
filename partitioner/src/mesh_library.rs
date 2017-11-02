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
use pathfinder_path_utils::{PathCommand, PathSegment, PathSegmentStream};
use serde::Serialize;
use std::io::{self, ErrorKind, Seek, SeekFrom, Write};
use std::ops::Range;
use std::u32;

use normal;
use {BQuad, BVertexLoopBlinnData};

#[derive(Debug, Clone)]
pub struct MeshLibrary {
    pub path_ranges: Vec<PathRanges>,
    pub b_quads: Vec<BQuad>,
    pub b_vertex_positions: Vec<Point2D<f32>>,
    pub b_vertex_loop_blinn_data: Vec<BVertexLoopBlinnData>,
    pub b_vertex_normals: Vec<f32>,
    pub cover_indices: MeshLibraryCoverIndices,
    pub edge_data: MeshLibraryEdgeData,
    pub segments: MeshLibrarySegments,
    pub segment_normals: MeshLibrarySegmentNormals,
}

impl MeshLibrary {
    #[inline]
    pub fn new() -> MeshLibrary {
        MeshLibrary {
            path_ranges: vec![],
            b_quads: vec![],
            b_vertex_positions: vec![],
            b_vertex_loop_blinn_data: vec![],
            b_vertex_normals: vec![],
            cover_indices: MeshLibraryCoverIndices::new(),
            edge_data: MeshLibraryEdgeData::new(),
            segments: MeshLibrarySegments::new(),
            segment_normals: MeshLibrarySegmentNormals::new(),
        }
    }

    pub fn clear(&mut self) {
        self.path_ranges.clear();
        self.b_quads.clear();
        self.b_vertex_positions.clear();
        self.b_vertex_loop_blinn_data.clear();
        self.b_vertex_normals.clear();
        self.cover_indices.clear();
        self.edge_data.clear();
        self.segments.clear();
        self.segment_normals.clear();
    }

    pub(crate) fn ensure_path_ranges(&mut self, path_id: u16) -> &mut PathRanges {
        let path_index = (path_id as usize) - 1;
        while path_index >= self.path_ranges.len() {
            self.path_ranges.push(PathRanges::new())
        }
        &mut self.path_ranges[path_index]
    }

    pub(crate) fn add_b_vertex(&mut self,
                               position: &Point2D<f32>,
                               loop_blinn_data: &BVertexLoopBlinnData,
                               normal: f32) {
        self.b_vertex_positions.push(*position);
        self.b_vertex_loop_blinn_data.push(*loop_blinn_data);
        self.b_vertex_normals.push(normal);
    }

    pub(crate) fn add_b_quad(&mut self, b_quad: &BQuad) {
        self.b_quads.push(*b_quad);

        let upper_left_position =
            &self.b_vertex_positions[b_quad.upper_left_vertex_index as usize];
        let upper_right_position =
            &self.b_vertex_positions[b_quad.upper_right_vertex_index as usize];
        let lower_left_position =
            &self.b_vertex_positions[b_quad.lower_left_vertex_index as usize];
        let lower_right_position =
            &self.b_vertex_positions[b_quad.lower_right_vertex_index as usize];

        let upper_left_bounding_box_position =
            Point2D::new(upper_left_position.x,
                         f32::max(upper_left_position.y, upper_right_position.y));
        let lower_right_bounding_box_position =
            Point2D::new(lower_right_position.x,
                         f32::max(lower_left_position.y, lower_right_position.y));

        self.edge_data.bounding_box_vertex_positions.push(EdgeBoundingBoxVertexPositions {
            upper_left: upper_left_bounding_box_position,
            lower_right: lower_right_bounding_box_position,
        });

        if b_quad.upper_control_point_vertex_index == u32::MAX {
            self.edge_data.upper_line_vertex_positions.push(EdgeLineVertexPositions {
                left: *upper_left_position,
                right: *upper_right_position,
            });
        } else {
            let upper_control_point_position =
                &self.b_vertex_positions[b_quad.upper_control_point_vertex_index as usize];
            self.edge_data.upper_curve_vertex_positions.push(EdgeCurveVertexPositions {
                left: *upper_left_position,
                control_point: *upper_control_point_position,
                right: *upper_right_position,
            });
        }

        if b_quad.lower_control_point_vertex_index == u32::MAX {
            self.edge_data.lower_line_vertex_positions.push(EdgeLineVertexPositions {
                left: *lower_left_position,
                right: *lower_right_position,
            });
        } else {
            let lower_control_point_position =
                &self.b_vertex_positions[b_quad.lower_control_point_vertex_index as usize];
            self.edge_data.lower_curve_vertex_positions.push(EdgeCurveVertexPositions {
                left: *lower_left_position,
                control_point: *lower_control_point_position,
                right: *lower_right_position,
            });
        }
    }

    /// Reverses interior indices so that they draw front-to-back.
    ///
    /// This enables early Z optimizations.
    pub fn optimize(&mut self) {
        let mut new_interior_indices =
            Vec::with_capacity(self.cover_indices.interior_indices.len());

        for path_range in &mut self.path_ranges {
            let old_interior_indices = &self.cover_indices.interior_indices[..];
            let old_range = path_range.cover_interior_indices.clone();
            let old_range = (old_range.start as usize)..(old_range.end as usize);
            let new_start_index = new_interior_indices.len() as u32;
            new_interior_indices.extend_from_slice(&old_interior_indices[old_range]);
            let new_end_index = new_interior_indices.len() as u32;
            path_range.cover_interior_indices = new_start_index..new_end_index;
        }

        self.cover_indices.interior_indices = new_interior_indices
    }

    pub fn push_segments<I>(&mut self, path_id: u16, stream: I)
                            where I: Iterator<Item = PathCommand> {
        let first_line_index = self.segments.lines.len() as u32;
        let first_curve_index = self.segments.curves.len() as u32;

        let stream = PathSegmentStream::new(stream);
        for (segment, _) in stream {
            match segment {
                PathSegment::Line(endpoint_0, endpoint_1) => {
                    self.segments.lines.push(LineSegment {
                        endpoint_0: endpoint_0,
                        endpoint_1: endpoint_1,
                    });
                }
                PathSegment::Curve(endpoint_0, control_point, endpoint_1) => {
                    self.segments.curves.push(CurveSegment {
                        endpoint_0: endpoint_0,
                        control_point: control_point,
                        endpoint_1: endpoint_1,
                    });
                }
            }
        }

        let last_line_index = self.segments.lines.len() as u32;
        let last_curve_index = self.segments.curves.len() as u32;

        let path_ranges = self.ensure_path_ranges(path_id);
        path_ranges.segment_curves = first_curve_index..last_curve_index;
        path_ranges.segment_lines = first_line_index..last_line_index;
    }

    /// Computes vertex normals necessary for emboldening and/or stem darkening.
    pub fn push_normals<I>(&mut self, stream: I) where I: Iterator<Item = PathCommand> {
        normal::push_normals(self, stream)
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
        try!(write_chunk(writer, b"prng", |writer| write_path_ranges(writer, &self.path_ranges)));

        try!(write_simple_chunk(writer, b"bqua", &self.b_quads));
        try!(write_simple_chunk(writer, b"bvpo", &self.b_vertex_positions));
        try!(write_simple_chunk(writer, b"bvlb", &self.b_vertex_loop_blinn_data));
        try!(write_simple_chunk(writer, b"bvno", &self.b_vertex_normals));
        try!(write_simple_chunk(writer, b"cvii", &self.cover_indices.interior_indices));
        try!(write_simple_chunk(writer, b"cvci", &self.cover_indices.curve_indices));
        try!(write_simple_chunk(writer, b"ebbv", &self.edge_data.bounding_box_vertex_positions));
        try!(write_simple_chunk(writer, b"eulv", &self.edge_data.upper_line_vertex_positions));
        try!(write_simple_chunk(writer, b"ellv", &self.edge_data.lower_line_vertex_positions));
        try!(write_simple_chunk(writer, b"eucv", &self.edge_data.upper_curve_vertex_positions));
        try!(write_simple_chunk(writer, b"elcv", &self.edge_data.lower_curve_vertex_positions));
        try!(write_simple_chunk(writer, b"slin", &self.segments.lines));
        try!(write_simple_chunk(writer, b"scur", &self.segments.curves));
        try!(write_simple_chunk(writer, b"snli", &self.segment_normals.line_normals));
        try!(write_simple_chunk(writer, b"sncu", &self.segment_normals.curve_normals));

        let total_length = try!(writer.seek(SeekFrom::Current(0)));
        try!(writer.seek(SeekFrom::Start(4)));
        try!(writer.write_u32::<LittleEndian>((total_length - 8) as u32));
        return Ok(());

        fn write_chunk<W, F>(writer: &mut W, tag: &[u8; 4], mut closure: F) -> io::Result<()>
                             where W: Write + Seek, F: FnMut(&mut W) -> io::Result<()> {
            try!(writer.write_all(tag));
            try!(writer.write_all(b"\0\0\0\0"));

            let start_position = try!(writer.seek(SeekFrom::Current(0)));
            try!(closure(writer));

            let end_position = try!(writer.seek(SeekFrom::Current(0)));
            try!(writer.seek(SeekFrom::Start(start_position - 4)));
            try!(writer.write_u32::<LittleEndian>((end_position - start_position) as u32));
            try!(writer.seek(SeekFrom::Start(end_position)));
            Ok(())
        }

        fn write_simple_chunk<W, T>(writer: &mut W, tag: &[u8; 4], data: &[T]) -> io::Result<()>
                                    where W: Write + Seek, T: Serialize {
            write_chunk(writer, tag, |writer| {
                for datum in data {
                    try!(bincode::serialize_into(writer, datum, Infinite).map_err(|_| {
                        io::Error::from(ErrorKind::Other)
                    }));
                }
                Ok(())
            })
        }

        fn write_path_ranges<W>(writer: &mut W, path_ranges: &[PathRanges]) -> io::Result<()>
                                where W: Write + Seek {
            try!(write_path_range(writer, b"bqua", path_ranges, |ranges| &ranges.b_quads));
            try!(write_path_range(writer, b"bver", path_ranges, |ranges| &ranges.b_vertices));
            try!(write_path_range(writer,
                                  b"cvii",
                                  path_ranges,
                                  |ranges| &ranges.cover_interior_indices));
            try!(write_path_range(writer,
                                  b"cvci",
                                  path_ranges,
                                  |ranges| &ranges.cover_curve_indices));
            try!(write_path_range(writer,
                                  b"ebbo",
                                  path_ranges,
                                  |ranges| &ranges.edge_bounding_box_indices));
            try!(write_path_range(writer,
                                  b"euli",
                                  path_ranges,
                                  |ranges| &ranges.edge_upper_line_indices));
            try!(write_path_range(writer,
                                  b"euci",
                                  path_ranges,
                                  |ranges| &ranges.edge_upper_curve_indices));
            try!(write_path_range(writer,
                                  b"elli",
                                  path_ranges,
                                  |ranges| &ranges.edge_lower_line_indices));
            try!(write_path_range(writer,
                                  b"elci",
                                  path_ranges,
                                  |ranges| &ranges.edge_lower_curve_indices));
            try!(write_path_range(writer, b"slin", path_ranges, |ranges| &ranges.segment_lines));
            try!(write_path_range(writer, b"scur", path_ranges, |ranges| &ranges.segment_curves));
            Ok(())
        }

        fn write_path_range<W, F>(writer: &mut W,
                                  tag: &[u8; 4],
                                  all_path_ranges: &[PathRanges],
                                  mut get_range: F)
                                  -> io::Result<()>
                                  where W: Write + Seek, F: FnMut(&PathRanges) -> &Range<u32> {
            write_chunk(writer, tag, |writer| {
                for path_ranges in all_path_ranges {
                    let range = get_range(path_ranges);
                    try!(writer.write_u32::<LittleEndian>(range.start));
                    try!(writer.write_u32::<LittleEndian>(range.end));
                }
                Ok(())
            })
        }
    }

    pub(crate) fn snapshot_lengths(&self) -> MeshLibraryLengths {
        MeshLibraryLengths {
            b_quads: self.b_quads.len() as u32,
            b_vertices: self.b_vertex_positions.len() as u32,
            cover_interior_indices: self.cover_indices.interior_indices.len() as u32,
            cover_curve_indices: self.cover_indices.curve_indices.len() as u32,
            edge_bounding_box_indices: self.edge_data.bounding_box_vertex_positions.len() as u32,
            edge_upper_line_indices: self.edge_data.upper_line_vertex_positions.len() as u32,
            edge_upper_curve_indices: self.edge_data.upper_curve_vertex_positions.len() as u32,
            edge_lower_line_indices: self.edge_data.lower_line_vertex_positions.len() as u32,
            edge_lower_curve_indices: self.edge_data.lower_curve_vertex_positions.len() as u32,
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

pub(crate) struct MeshLibraryLengths {
    b_quads: u32,
    b_vertices: u32,
    cover_interior_indices: u32,
    cover_curve_indices: u32,
    edge_bounding_box_indices: u32,
    edge_upper_line_indices: u32,
    edge_upper_curve_indices: u32,
    edge_lower_line_indices: u32,
    edge_lower_curve_indices: u32,
}

#[derive(Clone, Debug)]
pub struct PathRanges {
    pub b_quads: Range<u32>,
    pub b_vertices: Range<u32>,
    pub cover_interior_indices: Range<u32>,
    pub cover_curve_indices: Range<u32>,
    pub edge_bounding_box_indices: Range<u32>,
    pub edge_upper_line_indices: Range<u32>,
    pub edge_upper_curve_indices: Range<u32>,
    pub edge_lower_line_indices: Range<u32>,
    pub edge_lower_curve_indices: Range<u32>,
    pub segment_lines: Range<u32>,
    pub segment_curves: Range<u32>,
}

impl PathRanges {
    fn new() -> PathRanges {
        PathRanges {
            b_quads: 0..0,
            b_vertices: 0..0,
            cover_interior_indices: 0..0,
            cover_curve_indices: 0..0,
            edge_bounding_box_indices: 0..0,
            edge_upper_line_indices: 0..0,
            edge_upper_curve_indices: 0..0,
            edge_lower_line_indices: 0..0,
            edge_lower_curve_indices: 0..0,
            segment_lines: 0..0,
            segment_curves: 0..0,
        }
    }

    pub(crate) fn set_partitioning_lengths(&mut self,
                                           start: &MeshLibraryLengths,
                                           end: &MeshLibraryLengths) {
        self.b_quads = start.b_quads..end.b_quads;
        self.b_vertices = start.b_vertices..end.b_vertices;
        self.cover_interior_indices = start.cover_interior_indices..end.cover_interior_indices;
        self.cover_curve_indices = start.cover_curve_indices..end.cover_curve_indices;
        self.edge_bounding_box_indices =
            start.edge_bounding_box_indices..end.edge_bounding_box_indices;
        self.edge_upper_line_indices = start.edge_upper_line_indices..end.edge_upper_line_indices;
        self.edge_upper_curve_indices =
            start.edge_upper_curve_indices..end.edge_upper_curve_indices;
        self.edge_lower_line_indices = start.edge_lower_line_indices..end.edge_lower_line_indices;
        self.edge_lower_curve_indices =
            start.edge_lower_curve_indices..end.edge_lower_curve_indices;
    }
}

#[derive(Clone, Debug)]
pub struct MeshLibraryEdgeData {
    pub bounding_box_vertex_positions: Vec<EdgeBoundingBoxVertexPositions>,
    pub upper_line_vertex_positions: Vec<EdgeLineVertexPositions>,
    pub lower_line_vertex_positions: Vec<EdgeLineVertexPositions>,
    pub upper_curve_vertex_positions: Vec<EdgeCurveVertexPositions>,
    pub lower_curve_vertex_positions: Vec<EdgeCurveVertexPositions>,
}

impl MeshLibraryEdgeData {
    fn new() -> MeshLibraryEdgeData {
        MeshLibraryEdgeData {
            bounding_box_vertex_positions: vec![],
            upper_line_vertex_positions: vec![],
            lower_line_vertex_positions: vec![],
            upper_curve_vertex_positions: vec![],
            lower_curve_vertex_positions: vec![],
        }
    }

    fn clear(&mut self) {
        self.bounding_box_vertex_positions.clear();
        self.upper_line_vertex_positions.clear();
        self.upper_curve_vertex_positions.clear();
        self.lower_line_vertex_positions.clear();
        self.lower_curve_vertex_positions.clear();
    }
}

#[derive(Clone, Debug)]
pub struct MeshLibrarySegments {
    pub lines: Vec<LineSegment>,
    pub curves: Vec<CurveSegment>,
}

impl MeshLibrarySegments {
    fn new() -> MeshLibrarySegments {
        MeshLibrarySegments {
            lines: vec![],
            curves: vec![],
        }
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.curves.clear();
    }
}

#[derive(Clone, Debug)]
pub struct MeshLibrarySegmentNormals {
    pub line_normals: Vec<LineSegmentNormals>,
    pub curve_normals: Vec<CurveSegmentNormals>,
}

impl MeshLibrarySegmentNormals {
    fn new() -> MeshLibrarySegmentNormals {
        MeshLibrarySegmentNormals {
            line_normals: vec![],
            curve_normals: vec![],
        }
    }

    fn clear(&mut self) {
        self.line_normals.clear();
        self.curve_normals.clear();
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EdgeBoundingBoxVertexPositions {
    pub upper_left: Point2D<f32>,
    pub lower_right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EdgeLineVertexPositions {
    pub left: Point2D<f32>,
    pub right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EdgeCurveVertexPositions {
    pub left: Point2D<f32>,
    pub control_point: Point2D<f32>,
    pub right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LineSegment {
    pub endpoint_0: Point2D<f32>,
    pub endpoint_1: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CurveSegment {
    pub endpoint_0: Point2D<f32>,
    pub control_point: Point2D<f32>,
    pub endpoint_1: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LineSegmentNormals {
    pub endpoint_0: f32,
    pub endpoint_1: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CurveSegmentNormals {
    pub endpoint_0: f32,
    pub control_point: f32,
    pub endpoint_1: f32,
}
