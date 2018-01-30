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
use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Rect, Size2D, Vector2D};
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
    pub b_boxes: Vec<BBox>,
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
            b_boxes: vec![],
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
        self.b_boxes.clear();
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
        let BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_right_vertex_position: lr,
            ..
        } = self.get_b_quad_vertex_positions(b_quad);

        if ul.x.approx_eq(&ur.x) || ll.x.approx_eq(&lr.x) {
            return
        }

        self.b_quads.push(*b_quad);

        self.add_b_quad_vertex_positions(b_quad);
        self.add_b_box(b_quad);
    }

    fn add_b_quad_vertex_positions(&mut self, b_quad: &BQuad) {
        let b_quad_vertex_positions = self.get_b_quad_vertex_positions(b_quad);
        let first_b_quad_vertex_position_index = (self.b_quad_vertex_positions.len() as u32) * 6;
        self.push_b_quad_vertex_position_interior_indices(first_b_quad_vertex_position_index,
                                                          &b_quad_vertex_positions);
        self.b_quad_vertex_positions.push(b_quad_vertex_positions);
    }

    fn add_b_box(&mut self, b_quad: &BQuad) {
        let BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_control_point_position: uc,
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_control_point_position: lc,
            lower_right_vertex_position: lr,
        } = self.get_b_quad_vertex_positions(b_quad);

        let rect = Rect::from_points([ul, uc, ur, ll, lc, lr].into_iter());

        let (edge_ucl, edge_urc, edge_ulr) = (uc - ul, ur - uc, ul - ur);
        let (edge_lcl, edge_lrc, edge_llr) = (lc - ll, lr - lc, ll - lr);

        let (edge_len_ucl, edge_len_urc) = (edge_ucl.length(), edge_urc.length());
        let (edge_len_lcl, edge_len_lrc) = (edge_lcl.length(), edge_lrc.length());
        let (edge_len_ulr, edge_len_llr) = (edge_ulr.length(), edge_llr.length());

        let (uv_upper, uv_lower, sign_upper, sign_lower, mode_upper, mode_lower);

        if edge_len_ucl < 0.01 || edge_len_urc < 0.01 || edge_len_ulr < 0.01 ||
                edge_ucl.dot(-edge_ulr) > 0.9999 * edge_len_ucl * edge_len_ulr {
            uv_upper = Uv::line(&rect, &ul, &ur);
            sign_upper = -1.0;
            mode_upper = -1.0;
        } else {
            uv_upper = Uv::curve(&rect, &ul, &uc, &ur);
            sign_upper = (edge_ucl.cross(-edge_ulr)).signum();
            mode_upper = 1.0;
        }

        if edge_len_lcl < 0.01 || edge_len_lrc < 0.01 || edge_len_llr < 0.01 ||
                edge_lcl.dot(-edge_llr) > 0.9999 * edge_len_lcl * edge_len_llr {
            uv_lower = Uv::line(&rect, &ll, &lr);
            sign_lower = 1.0;
            mode_lower = -1.0;
        } else {
            uv_lower = Uv::curve(&rect, &ll, &lc, &lr);
            sign_lower = -(edge_lcl.cross(-edge_llr)).signum();
            mode_lower = 1.0;
        }

        let b_box = BBox {
            upper_left_position: rect.origin,
            lower_right_position: rect.bottom_right(),
            upper_left_uv_upper: uv_upper.origin,
            upper_left_uv_lower: uv_lower.origin,
            d_upper_uv_dx: uv_upper.d_uv_dx,
            d_lower_uv_dx: uv_lower.d_uv_dx,
            d_upper_uv_dy: uv_upper.d_uv_dy,
            d_lower_uv_dy: uv_lower.d_uv_dy,
            upper_sign: sign_upper,
            lower_sign: sign_lower,
            upper_mode: mode_upper,
            lower_mode: mode_lower,
        };

        self.b_boxes.push(b_box);
    }

    fn get_b_quad_vertex_positions(&self, b_quad: &BQuad) -> BQuadVertexPositions {
        let ul = self.b_vertex_positions[b_quad.upper_left_vertex_index as usize];
        let ur = self.b_vertex_positions[b_quad.upper_right_vertex_index as usize];
        let ll = self.b_vertex_positions[b_quad.lower_left_vertex_index as usize];
        let lr = self.b_vertex_positions[b_quad.lower_right_vertex_index as usize];

        let mut b_quad_vertex_positions = BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_control_point_position: ul,
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_control_point_position: lr,
            lower_right_vertex_position: lr,
        };

        if b_quad.upper_control_point_vertex_index != u32::MAX {
            let uc = &self.b_vertex_positions[b_quad.upper_control_point_vertex_index as usize];
            b_quad_vertex_positions.upper_control_point_position = *uc;
        }

        if b_quad.lower_control_point_vertex_index != u32::MAX {
            let lc = &self.b_vertex_positions[b_quad.lower_control_point_vertex_index as usize];
            b_quad_vertex_positions.lower_control_point_position = *lc;
        }

        b_quad_vertex_positions
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
        try!(write_simple_chunk(writer, b"bbox", &self.b_boxes));
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
            try!(write_path_range(writer, b"bbox", path_ranges, |ranges| &ranges.b_boxes));
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
            b_boxes: self.b_boxes.len() as u32,
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
    b_boxes: u32,
}

#[derive(Clone, Debug)]
pub struct PathRanges {
    pub b_quads: Range<u32>,
    pub b_quad_vertex_positions: Range<u32>,
    pub b_quad_vertex_interior_indices: Range<u32>,
    pub b_vertices: Range<u32>,
    pub b_boxes: Range<u32>,
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
            b_boxes: 0..0,
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
        self.b_boxes = start.b_boxes..end.b_boxes;
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BBox {
    pub upper_left_position: Point2D<f32>,
    pub lower_right_position: Point2D<f32>,
    pub upper_left_uv_upper: Point2D<f32>,
    pub upper_left_uv_lower: Point2D<f32>,
    pub d_upper_uv_dx: Vector2D<f32>,
    pub d_lower_uv_dx: Vector2D<f32>,
    pub d_upper_uv_dy: Vector2D<f32>,
    pub d_lower_uv_dy: Vector2D<f32>,
    pub upper_sign: f32,
    pub lower_sign: f32,
    pub upper_mode: f32,
    pub lower_mode: f32,
}

#[derive(Clone, Copy, Debug)]
struct CornerPositions {
    upper_left: Point2D<f32>,
    upper_right: Point2D<f32>,
    lower_left: Point2D<f32>,
    lower_right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug)]
struct CornerValues {
    upper_left: Point2D<f32>,
    upper_right: Point2D<f32>,
    lower_left: Point2D<f32>,
    lower_right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Uv {
    origin: Point2D<f32>,
    d_uv_dx: Vector2D<f32>,
    d_uv_dy: Vector2D<f32>,
}

impl Uv {
    fn from_values(origin: &Point2D<f32>, origin_right: &Point2D<f32>, origin_down: &Point2D<f32>)
                   -> Uv {
        Uv {
            origin: *origin,
            d_uv_dx: *origin_right - *origin,
            d_uv_dy: *origin_down - *origin,
        }
    }

    fn curve(rect: &Rect<f32>, left: &Point2D<f32>, ctrl: &Point2D<f32>, right: &Point2D<f32>)
             -> Uv {
        let origin_right = rect.top_right();
        let origin_down = rect.bottom_left();

        let (lambda_origin, denom) = to_barycentric(left, ctrl, right, &rect.origin);
        let (lambda_origin_right, _) = to_barycentric(left, ctrl, right, &origin_right);
        let (lambda_origin_down, _) = to_barycentric(left, ctrl, right, &origin_down);

        let uv_origin = lambda_to_uv(&lambda_origin, denom);
        let uv_origin_right = lambda_to_uv(&lambda_origin_right, denom);
        let uv_origin_down = lambda_to_uv(&lambda_origin_down, denom);

        return Uv::from_values(&uv_origin, &uv_origin_right, &uv_origin_down);

        // https://gamedev.stackexchange.com/a/23745
        fn to_barycentric(a: &Point2D<f32>, b: &Point2D<f32>, c: &Point2D<f32>, p: &Point2D<f32>)
                        -> ([f64; 2], f64) {
            let (a, b, c, p) = (a.to_f64(), b.to_f64(), c.to_f64(), p.to_f64());
            let (v0, v1, v2) = (b - a, c - a, p - a);
            let (d00, d01) = (v0.dot(v0), v0.dot(v1));
            let d11 = v1.dot(v1);
            let (d20, d21) = (v2.dot(v0), v2.dot(v1));
            let denom = d00 * d11 - d01 * d01;
            ([(d11 * d20 - d01 * d21), (d00 * d21 - d01 * d20)], denom)
        }

        fn lambda_to_uv(lambda: &[f64; 2], denom: f64) -> Point2D<f32> {
            (Point2D::new(lambda[0] * 0.5 + lambda[1], lambda[1]) / denom).to_f32()
        }
    }

    fn line(rect: &Rect<f32>, left: &Point2D<f32>, right: &Point2D<f32>) -> Uv {
        let (values, line_bounds);
        if f32::abs(left.y - right.y) < 0.01 {
            values = CornerValues {
                upper_left: Point2D::new(0.0, 0.5),
                upper_right: Point2D::new(0.5, 1.0),
                lower_right: Point2D::new(1.0, 0.5),
                lower_left: Point2D::new(0.5, 0.0),
            };
            line_bounds = Rect::new(*left + Vector2D::new(0.0, -1.0),
                                    Size2D::new(right.x - left.x, 2.0));
        } else {
            if left.y < right.y {
                values = CornerValues {
                    upper_left: Point2D::new(1.0, 1.0),
                    upper_right: Point2D::new(0.0, 1.0),
                    lower_left: Point2D::new(1.0, 0.0),
                    lower_right: Point2D::new(0.0, 0.0),
                };
            } else {
                values = CornerValues {
                    upper_left: Point2D::new(0.0, 1.0),
                    upper_right: Point2D::new(1.0, 1.0),
                    lower_left: Point2D::new(0.0, 0.0),
                    lower_right: Point2D::new(1.0, 0.0),
                };
            }
            line_bounds = Rect::from_points([*left, *right].into_iter());
        }

        let origin_right = rect.top_right();
        let origin_down = rect.bottom_left();

        let uv_origin = bilerp(&line_bounds, &values, &rect.origin);
        let uv_origin_right = bilerp(&line_bounds, &values, &origin_right);
        let uv_origin_down = bilerp(&line_bounds, &values, &origin_down);

        return Uv::from_values(&uv_origin, &uv_origin_right, &uv_origin_down);

        fn bilerp(rect: &Rect<f32>, values: &CornerValues, position: &Point2D<f32>)
                  -> Point2D<f32> {
            let upper = values.upper_left.lerp(values.upper_right,
                                               (position.x - rect.min_x()) / rect.size.width);
            let lower = values.lower_left.lerp(values.lower_right,
                                               (position.x - rect.min_x()) / rect.size.width);
            upper.lerp(lower, (position.y - rect.min_y()) / rect.size.height)
        }
    }
}
