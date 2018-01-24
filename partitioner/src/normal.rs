// pathfinder/partitioner/src/normal.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utility functions for vertex normals.

/*
use euclid::{Point2D, Vector2D};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIterator;

use mesh_library::{CurveSegmentNormals, LineSegmentNormals, MeshLibrary};

pub fn push_normals<I>(library: &mut MeshLibrary, stream: I) where I: PathIterator {
    let mut first_segment_of_subpath = None;
    let mut index_of_first_segment_of_subpath = None;
    let mut index_of_prev_segment = None;

    for event in stream {
        let state = event.state();

        let is_first_segment = match event {
            PathEvent::MoveTo(..) => true,
            _ => false,
        };

        /*let is_last_segment = match stream.peek() {
            Some(&(_, next_subpath_index)) => prev_subpath_index != next_subpath_index,
            _ => true,
        };*/

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

        let next_vertex_normal = match (&prev_segment, &next_segment) {
            (&PathSegment::Line(ref prev_endpoint, ref vertex_endpoint),
             &PathSegment::Line(_, ref next_endpoint)) |
            (&PathSegment::Curve(_, ref prev_endpoint, ref vertex_endpoint),
             &PathSegment::Line(_, ref next_endpoint)) |
            (&PathSegment::Line(ref prev_endpoint, ref vertex_endpoint),
             &PathSegment::Curve(_, ref next_endpoint, _)) |
            (&PathSegment::Curve(_, ref prev_endpoint, ref vertex_endpoint),
             &PathSegment::Curve(_, ref next_endpoint, _)) => {
                calculate_vertex_normal(prev_endpoint, vertex_endpoint, next_endpoint)
            }
        };

        let next_vertex_angle = calculate_normal_angle(&next_vertex_normal);

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
            PathSegment::Curve(endpoint_0, control_point, endpoint_1) => {
                let control_point_vertex_normal = calculate_vertex_normal(&endpoint_0,
                                                                          &control_point,
                                                                          &endpoint_1);
                let control_point_vertex_angle =
                    calculate_normal_angle(&control_point_vertex_normal);

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

pub fn calculate_vertex_normal(prev_position: &Point2D<f32>,
                               vertex_position: &Point2D<f32>,
                               next_position: &Point2D<f32>)
                               -> Vector2D<f32> {
    let prev_edge_vector = *vertex_position - *prev_position;
    let next_edge_vector = *next_position - *vertex_position;

    let prev_edge_normal = Vector2D::new(-prev_edge_vector.y, prev_edge_vector.x).normalize();
    let next_edge_normal = Vector2D::new(-next_edge_vector.y, next_edge_vector.x).normalize();

    (prev_edge_normal + next_edge_normal) * 0.5
}

pub fn calculate_normal_angle(normal: &Vector2D<f32>) -> f32 {
    (-normal.y).atan2(normal.x)
}

#[derive(Clone, Copy, Debug)]
enum SegmentIndex {
    Line(usize),
    Curve(usize),
}
*/
