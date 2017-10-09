// pathfinder/partitioner/src/bold.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Infrastructure to enable on-GPU emboldening and stem darkening.

use euclid::{Point2D, Vector2D};
use pathfinder_path_utils::{PathCommand, PathSegment, PathSegmentStream};

use mesh_library::{CurveSegmentNormals, LineSegmentNormals, MeshLibrary};

pub fn push_normals<I>(library: &mut MeshLibrary, stream: I)
                       where I: Iterator<Item = PathCommand> {
    let mut stream = PathSegmentStream::new(stream).peekable();

    let mut first_segment_of_subpath = None;
    let mut index_of_first_segment_of_subpath = None;
    let mut index_of_prev_segment = None;

    while let Some((prev_segment, prev_subpath_index)) = stream.next() {
        let is_first_segment = index_of_first_segment_of_subpath.is_none();
        let is_last_segment = match stream.peek() {
            Some(&(_, next_subpath_index)) => prev_subpath_index != next_subpath_index,
            _ => true,
        };

        let next_segment = if is_last_segment {
            first_segment_of_subpath.unwrap_or(PathSegment::Line(Point2D::zero(), Point2D::zero()))
        } else {
            stream.peek().unwrap().0
        };

        if is_first_segment {
            first_segment_of_subpath = Some(prev_segment);
            index_of_first_segment_of_subpath = match prev_segment {
                PathSegment::Line(..) => {
                    Some(SegmentIndex::Line(library.segment_normals.line_normals.len()))
                }
                PathSegment::Curve(..) => {
                    Some(SegmentIndex::Curve(library.segment_normals.curve_normals.len()))
                }
            };
        }

        let prev_vector = match prev_segment {
            PathSegment::Line(endpoint_0, endpoint_1) => endpoint_1 - endpoint_0,
            PathSegment::Curve(_, control_point, endpoint_1) => endpoint_1 - control_point,
        };
        let next_vector = match next_segment {
            PathSegment::Line(endpoint_0, endpoint_1) => endpoint_1 - endpoint_0,
            PathSegment::Curve(endpoint_0, control_point, _) => control_point - endpoint_0,
        };

        let prev_edge_normal = Vector2D::new(-prev_vector.y, prev_vector.x).normalize();
        let next_edge_normal = Vector2D::new(-next_vector.y, next_vector.x).normalize();

        let next_vertex_normal = (prev_edge_normal + next_edge_normal) * 0.5;
        let next_vertex_angle = (-next_vertex_normal.y).atan2(next_vertex_normal.x);

        let prev_vertex_angle = if !is_first_segment {
            match index_of_prev_segment.unwrap() {
                SegmentIndex::Line(prev_line_index) => {
                    library.segment_normals.line_normals[prev_line_index].endpoint_1
                }
                SegmentIndex::Curve(prev_curve_index) => {
                    library.segment_normals.curve_normals[prev_curve_index].endpoint_1
                }
            }
        } else {
            // We'll patch this later.
            0.0
        };

        match prev_segment {
            PathSegment::Line(..) => {
                index_of_prev_segment =
                    Some(SegmentIndex::Line(library.segment_normals.line_normals.len()));
                library.segment_normals.line_normals.push(LineSegmentNormals {
                    endpoint_0: prev_vertex_angle,
                    endpoint_1: next_vertex_angle,
                });
            }
            PathSegment::Curve(endpoint_0, control_point, _) => {
                let prev_prev_vector = control_point - endpoint_0;
                let prev_prev_edge_normal = Vector2D::new(-prev_prev_vector.y,
                                                          prev_prev_vector.x).normalize();

                let control_point_vertex_normal = (prev_prev_edge_normal + prev_edge_normal) * 0.5;
                let control_point_vertex_angle =
                    (-control_point_vertex_normal.y).atan2(control_point_vertex_normal.x);

                index_of_prev_segment =
                    Some(SegmentIndex::Curve(library.segment_normals.curve_normals.len()));
                library.segment_normals.curve_normals.push(CurveSegmentNormals {
                    endpoint_0: prev_vertex_angle,
                    control_point: control_point_vertex_angle,
                    endpoint_1: next_vertex_angle,
                });
            }
        }

        // Patch that first segment if necessary.
        if is_last_segment {
            match index_of_first_segment_of_subpath.unwrap() {
                SegmentIndex::Line(index) => {
                    library.segment_normals.line_normals[index].endpoint_0 = next_vertex_angle
                }
                SegmentIndex::Curve(index) => {
                    library.segment_normals.curve_normals[index].endpoint_0 = next_vertex_angle
                }
            }
            index_of_first_segment_of_subpath = None;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum SegmentIndex {
    Line(usize),
    Curve(usize),
}
