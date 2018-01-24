// pathfinder/partitioner/src/mesh_library.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bincode::{self, Infinite};
use byteorder::{LittleEndian, WriteBytesExt};
use euclid::{Point2D, Vector2D};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIterator;
use pathfinder_path_utils::normals::PathNormals;
use pathfinder_path_utils::segments::{self, SegmentIter};
use serde::Serialize;
use std::f32;
use std::io::{self, ErrorKind, Seek, SeekFrom, Write};
use std::ops::Range;
use std::u32;

use {BQuad, BQuadVertexPositions, BVertexLoopBlinnData};

#[derive(Debug, Clone)]
pub struct MeshLibrary {
    pub path_ranges: Vec<PathRanges>,
    pub b_quads: Vec<BQuad>,
    // FIXME(pcwalton): Merge with `b_vertex_positions` below.
    pub b_quad_vertex_positions: Vec<BQuadVertexPositions>,
    pub b_quad_vertex_interior_indices: Vec<u32>,
    pub b_vertex_positions: Vec<Point2D<f32>>,
    pub b_vertex_loop_blinn_data: Vec<BVertexLoopBlinnData>,
    pub segments: MeshLibrarySegments,
    pub segment_normals: MeshLibrarySegmentNormals,
}

impl MeshLibrary {
    #[inline]
    pub fn new() -> MeshLibrary {
        MeshLibrary {
            path_ranges: vec![],
            b_quads: vec![],
            b_quad_vertex_positions: vec![],
            b_quad_vertex_interior_indices: vec![],
            b_vertex_positions: vec![],
            b_vertex_loop_blinn_data: vec![],
            segments: MeshLibrarySegments::new(),
            segment_normals: MeshLibrarySegmentNormals::new(),
        }
    }

    pub fn clear(&mut self) {
        self.path_ranges.clear();
        self.b_quads.clear();
        self.b_quad_vertex_positions.clear();
        self.b_quad_vertex_interior_indices.clear();
        self.b_vertex_positions.clear();
        self.b_vertex_loop_blinn_data.clear();
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
                               loop_blinn_data: &BVertexLoopBlinnData) {
        self.b_vertex_positions.push(*position);
        self.b_vertex_loop_blinn_data.push(*loop_blinn_data);
    }

    pub(crate) fn add_b_quad(&mut self, b_quad: &BQuad) {
        self.b_quads.push(*b_quad);

        let upper_left_position =
            self.b_vertex_positions[b_quad.upper_left_vertex_index as usize];
        let upper_right_position =
            self.b_vertex_positions[b_quad.upper_right_vertex_index as usize];
        let lower_left_position =
            self.b_vertex_positions[b_quad.lower_left_vertex_index as usize];
        let lower_right_position =
            self.b_vertex_positions[b_quad.lower_right_vertex_index as usize];

        let mut b_quad_vertex_positions = BQuadVertexPositions {
            upper_left_vertex_position: upper_left_position,
            upper_control_point_position: upper_left_position,
            upper_right_vertex_position: upper_right_position,
            lower_left_vertex_position: lower_left_position,
            lower_control_point_position: lower_right_position,
            lower_right_vertex_position: lower_right_position,
        };

        if b_quad.upper_control_point_vertex_index != u32::MAX {
            let upper_control_point_position =
                self.b_vertex_positions[b_quad.upper_control_point_vertex_index as usize];
            b_quad_vertex_positions.upper_control_point_position = upper_control_point_position;
        }

        if b_quad.lower_control_point_vertex_index != u32::MAX {
            let lower_control_point_position =
                self.b_vertex_positions[b_quad.lower_control_point_vertex_index as usize];
            b_quad_vertex_positions.lower_control_point_position = lower_control_point_position;
        }

        let first_b_quad_vertex_position_index = (self.b_quad_vertex_positions.len() as u32) * 6;
        self.push_b_quad_vertex_position_interior_indices(first_b_quad_vertex_position_index,
                                                          &b_quad_vertex_positions);

        self.b_quad_vertex_positions.push(b_quad_vertex_positions);
    }

    fn push_b_quad_vertex_position_interior_indices(&mut self,
                                                    first_vertex_index: u32,
                                                    b_quad: &BQuadVertexPositions) {
        let upper_curve_is_concave =
            (b_quad.upper_right_vertex_position - b_quad.upper_left_vertex_position).cross(
                b_quad.upper_control_point_position - b_quad.upper_left_vertex_position) > 0.0;
        let lower_curve_is_concave =
            (b_quad.lower_left_vertex_position - b_quad.lower_right_vertex_position).cross(
                b_quad.lower_control_point_position - b_quad.lower_right_vertex_position) > 0.0;

        let indices: &'static [u32] = match (upper_curve_is_concave, lower_curve_is_concave) {
            (false, false) => &[UL, UR, LL, UR, LR, LL],
            (true, false) => &[UL, UC, LL, UC, LR, LL, UR, LR, UC],
            (false, true) => &[UL, LC, LL, UL, UR, LC, UR, LR, LC],
            (true, true) => &[UL, UC, LL, UC, LC, LL, UR, LC, UC, UR, LR, LC],
        };

        self.b_quad_vertex_interior_indices
            .extend(indices.into_iter().map(|index| index + first_vertex_index));

        const UL: u32 = 0;
        const UC: u32 = 1;
        const UR: u32 = 2;
        const LR: u32 = 3;
        const LC: u32 = 4;
        const LL: u32 = 5;
    }

    /// Reverses interior indices so that they draw front-to-back.
    ///
    /// This enables early Z optimizations.
    pub fn optimize(&mut self) {
        reverse_indices(&mut self.path_ranges,
                        &mut self.b_quad_vertex_interior_indices,
                        |path_ranges| path_ranges.b_quad_vertex_interior_indices.clone(),
                        |path_ranges, new_range| {
                            path_ranges.b_quad_vertex_interior_indices = new_range
                        });

        fn reverse_indices<G, S>(path_ranges: &mut [PathRanges],
                                 indices: &mut Vec<u32>,
                                 mut getter: G,
                                 mut setter: S)
                                 where G: FnMut(&PathRanges) -> Range<u32>,
                                       S: FnMut(&mut PathRanges, Range<u32>) {
            let mut new_indices = Vec::with_capacity(indices.len());
            for path_range in path_ranges.iter_mut().rev() {
                let old_range = getter(path_range);
                let old_range = (old_range.start as usize)..(old_range.end as usize);
                let new_start_index = new_indices.len() as u32;
                new_indices.extend_from_slice(&indices[old_range]);
                let new_end_index = new_indices.len() as u32;
                setter(path_range, new_start_index..new_end_index);
            }

            *indices = new_indices
        }
    }

    pub fn push_segments<I>(&mut self, path_id: u16, stream: I)
                            where I: Iterator<Item = PathEvent> {
        let first_line_index = self.segments.lines.len() as u32;
        let first_curve_index = self.segments.curves.len() as u32;

        let segment_iter = SegmentIter::new(stream);
        for segment in segment_iter {
            match segment {
                segments::Segment::Line(line_segment) => {
                    self.segments.lines.push(LineSegment {
                        endpoint_0: line_segment.from,
                        endpoint_1: line_segment.to,
                    })
                }
                segments::Segment::Quadratic(curve_segment) => {
                    self.segments.curves.push(CurveSegment {
                        endpoint_0: curve_segment.from,
                        control_point: curve_segment.ctrl,
                        endpoint_1: curve_segment.to,
                    })
                }
                segments::Segment::EndSubpath(..) => {}
                segments::Segment::Cubic(..) => {
                    panic!("push_segments(): Convert cubics to quadratics first!")
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
    pub fn push_normals<I>(&mut self, _path_id: u16, stream: I) where I: PathIterator {
        let path_events: Vec<_> = stream.collect();

        let mut normals = PathNormals::new();
        normals.add_path(path_events.iter().cloned());
        let normals = normals.normals();

        let mut current_point_normal_index = 0;
        let mut next_normal_index = 0;
        let mut first_normal_index_of_subpath = 0;

        for event in path_events {
            match event {
                PathEvent::MoveTo(..) => {
                    first_normal_index_of_subpath = next_normal_index;
                    current_point_normal_index = next_normal_index;
                    next_normal_index += 1;
                }
                PathEvent::LineTo(..) => {
                    self.segment_normals.line_normals.push(LineSegmentNormals {
                        endpoint_0: normal_angle(&normals[current_point_normal_index]),
                        endpoint_1: normal_angle(&normals[next_normal_index]),
                    });
                    current_point_normal_index = next_normal_index;
                    next_normal_index += 1;
                }
                PathEvent::QuadraticTo(..) => {
                    self.segment_normals.curve_normals.push(CurveSegmentNormals {
                        endpoint_0: normal_angle(&normals[current_point_normal_index]),
                        control_point: normal_angle(&normals[next_normal_index + 0]),
                        endpoint_1: normal_angle(&normals[next_normal_index + 1]),
                    });
                    current_point_normal_index = next_normal_index + 1;
                    next_normal_index += 2;
                }
                PathEvent::Close => {
                    self.segment_normals.line_normals.push(LineSegmentNormals {
                        endpoint_0: normal_angle(&normals[current_point_normal_index]),
                        endpoint_1: normal_angle(&normals[first_normal_index_of_subpath]),
                    });
                }
                PathEvent::CubicTo(..) | PathEvent::Arc(..) => {
                    panic!("push_normals(): Convert cubics and arcs to quadratics first!")
                }
            }
        }

        fn normal_angle(vector: &Vector2D<f32>) -> f32 {
            Vector2D::new(vector.x, -vector.y).angle_from_x_axis().get()
        }
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
        try!(write_simple_chunk(writer, b"bqvp", &self.b_quad_vertex_positions));
        try!(write_simple_chunk(writer, b"bqii", &self.b_quad_vertex_interior_indices));
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
            try!(write_path_range(writer,
                                  b"bqvp",
                                  path_ranges,
                                  |ranges| &ranges.b_quad_vertex_positions));
            try!(write_path_range(writer,
                                  b"bqii",
                                  path_ranges,
                                  |ranges| &ranges.b_quad_vertex_interior_indices));
            try!(write_path_range(writer, b"bver", path_ranges, |ranges| &ranges.b_vertices));
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
            b_quad_vertex_positions: self.b_quad_vertex_positions.len() as u32,
            b_quad_vertex_interior_indices: self.b_quad_vertex_interior_indices.len() as u32,
            b_vertices: self.b_vertex_positions.len() as u32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshLibraryCoverIndices {
    pub interior_indices: Vec<u32>,
    pub curve_indices: Vec<u32>,
}

pub(crate) struct MeshLibraryLengths {
    pub(crate) b_quads: u32,
    b_quad_vertex_positions: u32,
    b_quad_vertex_interior_indices: u32,
    b_vertices: u32,
}

#[derive(Clone, Debug)]
pub struct PathRanges {
    pub b_quads: Range<u32>,
    pub b_quad_vertex_positions: Range<u32>,
    pub b_quad_vertex_interior_indices: Range<u32>,
    pub b_vertices: Range<u32>,
    pub segment_lines: Range<u32>,
    pub segment_curves: Range<u32>,
}

impl PathRanges {
    fn new() -> PathRanges {
        PathRanges {
            b_quads: 0..0,
            b_quad_vertex_positions: 0..0,
            b_quad_vertex_interior_indices: 0..0,
            b_vertices: 0..0,
            segment_lines: 0..0,
            segment_curves: 0..0,
        }
    }

    pub(crate) fn set_partitioning_lengths(&mut self,
                                           start: &MeshLibraryLengths,
                                           end: &MeshLibraryLengths) {
        self.b_quads = start.b_quads..end.b_quads;
        self.b_quad_vertex_positions = start.b_quad_vertex_positions..end.b_quad_vertex_positions;
        self.b_quad_vertex_interior_indices =
            start.b_quad_vertex_interior_indices..end.b_quad_vertex_interior_indices;
        self.b_vertices = start.b_vertices..end.b_vertices;
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
