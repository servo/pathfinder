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
    pub b_quads: Vec<BQuad>,
    pub b_vertex_positions: Vec<Point2D<f32>>,
    pub b_vertex_path_ids: Vec<u16>,
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
            b_quads: vec![],
            b_vertex_positions: vec![],
            b_vertex_path_ids: vec![],
            b_vertex_loop_blinn_data: vec![],
            b_vertex_normals: vec![],
            cover_indices: MeshLibraryCoverIndices::new(),
            edge_data: MeshLibraryEdgeData::new(),
            segments: MeshLibrarySegments::new(),
            segment_normals: MeshLibrarySegmentNormals::new(),
        }
    }

    pub fn clear(&mut self) {
        self.b_quads.clear();
        self.b_vertex_positions.clear();
        self.b_vertex_path_ids.clear();
        self.b_vertex_loop_blinn_data.clear();
        self.b_vertex_normals.clear();
        self.cover_indices.clear();
        self.edge_data.clear();
        self.segments.clear();
        self.segment_normals.clear();
    }

    pub(crate) fn add_b_vertex(&mut self,
                               position: &Point2D<f32>,
                               path_id: u16,
                               loop_blinn_data: &BVertexLoopBlinnData,
                               normal: f32) {
        self.b_vertex_positions.push(*position);
        self.b_vertex_path_ids.push(path_id);
        self.b_vertex_loop_blinn_data.push(*loop_blinn_data);
        self.b_vertex_normals.push(normal);
    }

    pub(crate) fn add_b_quad(&mut self, b_quad: &BQuad) {
        self.b_quads.push(*b_quad);

        let path_id = self.b_vertex_path_ids[b_quad.upper_left_vertex_index as usize];

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
        self.edge_data.bounding_box_path_ids.push(path_id);

        if b_quad.upper_control_point_vertex_index == u32::MAX {
            self.edge_data.upper_line_vertex_positions.push(EdgeLineVertexPositions {
                left: *upper_left_position,
                right: *upper_right_position,
            });
            self.edge_data.upper_line_path_ids.push(path_id);
        } else {
            let upper_control_point_position =
                &self.b_vertex_positions[b_quad.upper_control_point_vertex_index as usize];
            self.edge_data.upper_curve_vertex_positions.push(EdgeCurveVertexPositions {
                left: *upper_left_position,
                control_point: *upper_control_point_position,
                right: *upper_right_position,
            });
            self.edge_data.upper_curve_path_ids.push(path_id);
        }

        if b_quad.lower_control_point_vertex_index == u32::MAX {
            self.edge_data.lower_line_vertex_positions.push(EdgeLineVertexPositions {
                left: *lower_left_position,
                right: *lower_right_position,
            });
            self.edge_data.lower_line_path_ids.push(path_id);
        } else {
            let lower_control_point_position =
                &self.b_vertex_positions[b_quad.lower_control_point_vertex_index as usize];
            self.edge_data.lower_curve_vertex_positions.push(EdgeCurveVertexPositions {
                left: *lower_left_position,
                control_point: *lower_control_point_position,
                right: *lower_right_position,
            });
            self.edge_data.lower_curve_path_ids.push(path_id);
        }
    }

    /// Reverses interior indices so that they draw front-to-back.
    ///
    /// This enables early Z optimizations.
    pub fn optimize(&mut self) {
        let mut new_cover_interior_indices =
            Vec::with_capacity(self.cover_indices.interior_indices.len());
        let mut last_cover_interior_index_index = self.cover_indices.interior_indices.len();
        while last_cover_interior_index_index != 0 {
            let mut first_cover_interior_index_index = last_cover_interior_index_index - 1;
            let path_id =
                self.b_vertex_path_ids[self.cover_indices
                                           .interior_indices[first_cover_interior_index_index] as
                                       usize];
            while first_cover_interior_index_index != 0 {
                let prev_path_id = self.b_vertex_path_ids[
                    self.cover_indices.interior_indices[first_cover_interior_index_index - 1] as
                    usize];
                if prev_path_id != path_id {
                    break
                }
                first_cover_interior_index_index -= 1
            }
            let range = first_cover_interior_index_index..last_cover_interior_index_index;
            new_cover_interior_indices.extend_from_slice(&self.cover_indices
                                                              .interior_indices[range]);
            last_cover_interior_index_index = first_cover_interior_index_index;
        }
        self.cover_indices.interior_indices = new_cover_interior_indices
    }

    pub fn push_segments<I>(&mut self, path_id: u16, stream: I)
                            where I: Iterator<Item = PathCommand> {
        let stream = PathSegmentStream::new(stream);
        for (segment, _) in stream {
            match segment {
                PathSegment::Line(endpoint_0, endpoint_1) => {
                    self.segments.lines.push(LineSegment {
                        endpoint_0: endpoint_0,
                        endpoint_1: endpoint_1,
                    });
                    self.segments.line_path_ids.push(path_id);
                }
                PathSegment::Curve(endpoint_0, control_point, endpoint_1) => {
                    self.segments.curves.push(CurveSegment {
                        endpoint_0: endpoint_0,
                        control_point: control_point,
                        endpoint_1: endpoint_1,
                    });
                    self.segments.curve_path_ids.push(path_id);
                }
            }
        }
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
        try!(write_chunk(writer, b"bqua", &self.b_quads));
        try!(write_chunk(writer, b"bvpo", &self.b_vertex_positions));
        try!(write_chunk(writer, b"bvpi", &self.b_vertex_path_ids));
        try!(write_chunk(writer, b"bvlb", &self.b_vertex_loop_blinn_data));
        try!(write_chunk(writer, b"bvno", &self.b_vertex_normals));
        try!(write_chunk(writer, b"cvii", &self.cover_indices.interior_indices));
        try!(write_chunk(writer, b"cvci", &self.cover_indices.curve_indices));
        try!(write_chunk(writer, b"ebbv", &self.edge_data.bounding_box_vertex_positions));
        try!(write_chunk(writer, b"eulv", &self.edge_data.upper_line_vertex_positions));
        try!(write_chunk(writer, b"ellv", &self.edge_data.lower_line_vertex_positions));
        try!(write_chunk(writer, b"eucv", &self.edge_data.upper_curve_vertex_positions));
        try!(write_chunk(writer, b"elcv", &self.edge_data.lower_curve_vertex_positions));
        try!(write_chunk(writer, b"ebbp", &self.edge_data.bounding_box_path_ids));
        try!(write_chunk(writer, b"eulp", &self.edge_data.upper_line_path_ids));
        try!(write_chunk(writer, b"ellp", &self.edge_data.lower_line_path_ids));
        try!(write_chunk(writer, b"eucp", &self.edge_data.upper_curve_path_ids));
        try!(write_chunk(writer, b"elcp", &self.edge_data.lower_curve_path_ids));
        try!(write_chunk(writer, b"slin", &self.segments.lines));
        try!(write_chunk(writer, b"scur", &self.segments.curves));
        try!(write_chunk(writer, b"slpi", &self.segments.line_path_ids));
        try!(write_chunk(writer, b"scpi", &self.segments.curve_path_ids));
        try!(write_chunk(writer, b"snli", &self.segment_normals.line_normals));
        try!(write_chunk(writer, b"sncu", &self.segment_normals.curve_normals));

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
            edge_bounding_box_indices: self.edge_data.bounding_box_vertex_positions.len(),
            edge_upper_line_indices: self.edge_data.upper_line_vertex_positions.len(),
            edge_upper_curve_indices: self.edge_data.upper_curve_vertex_positions.len(),
            edge_lower_line_indices: self.edge_data.lower_line_vertex_positions.len(),
            edge_lower_curve_indices: self.edge_data.lower_curve_vertex_positions.len(),
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
    b_quads: usize,
    b_vertices: usize,
    cover_interior_indices: usize,
    cover_curve_indices: usize,
    edge_bounding_box_indices: usize,
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
    pub edge_bounding_box_indices: Range<usize>,
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
            edge_bounding_box_indices:
                start.edge_bounding_box_indices..end.edge_bounding_box_indices,
            edge_upper_line_indices: start.edge_upper_line_indices..end.edge_upper_line_indices,
            edge_upper_curve_indices: start.edge_upper_curve_indices..end.edge_upper_curve_indices,
            edge_lower_line_indices: start.edge_lower_line_indices..end.edge_lower_line_indices,
            edge_lower_curve_indices: start.edge_lower_curve_indices..end.edge_lower_curve_indices,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MeshLibraryEdgeData {
    pub bounding_box_vertex_positions: Vec<EdgeBoundingBoxVertexPositions>,
    pub upper_line_vertex_positions: Vec<EdgeLineVertexPositions>,
    pub lower_line_vertex_positions: Vec<EdgeLineVertexPositions>,
    pub upper_curve_vertex_positions: Vec<EdgeCurveVertexPositions>,
    pub lower_curve_vertex_positions: Vec<EdgeCurveVertexPositions>,
    pub bounding_box_path_ids: Vec<u16>,
    pub upper_line_path_ids: Vec<u16>,
    pub lower_line_path_ids: Vec<u16>,
    pub upper_curve_path_ids: Vec<u16>,
    pub lower_curve_path_ids: Vec<u16>,
}

impl MeshLibraryEdgeData {
    fn new() -> MeshLibraryEdgeData {
        MeshLibraryEdgeData {
            bounding_box_vertex_positions: vec![],
            upper_line_vertex_positions: vec![],
            lower_line_vertex_positions: vec![],
            upper_curve_vertex_positions: vec![],
            lower_curve_vertex_positions: vec![],
            bounding_box_path_ids: vec![],
            upper_line_path_ids: vec![],
            lower_line_path_ids: vec![],
            upper_curve_path_ids: vec![],
            lower_curve_path_ids: vec![],
        }
    }

    fn clear(&mut self) {
        self.bounding_box_vertex_positions.clear();
        self.upper_line_vertex_positions.clear();
        self.upper_curve_vertex_positions.clear();
        self.lower_line_vertex_positions.clear();
        self.lower_curve_vertex_positions.clear();
        self.bounding_box_path_ids.clear();
        self.upper_line_path_ids.clear();
        self.upper_curve_path_ids.clear();
        self.lower_line_path_ids.clear();
        self.lower_curve_path_ids.clear();
    }
}

#[derive(Clone, Debug)]
pub struct MeshLibrarySegments {
    pub lines: Vec<LineSegment>,
    pub curves: Vec<CurveSegment>,
    pub line_path_ids: Vec<u16>,
    pub curve_path_ids: Vec<u16>,
}

impl MeshLibrarySegments {
    fn new() -> MeshLibrarySegments {
        MeshLibrarySegments {
            lines: vec![],
            curves: vec![],
            line_path_ids: vec![],
            curve_path_ids: vec![],
        }
    }

    fn clear(&mut self) {
        self.lines.clear();
        self.curves.clear();
        self.line_path_ids.clear();
        self.curve_path_ids.clear();
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
