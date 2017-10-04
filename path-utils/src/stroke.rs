// pathfinder/path-utils/src/stroke.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::u32;

use {Endpoint, PathBuffer, PathCommand, Subpath};
use line::Line;

pub struct Stroke {
    pub width: f32,
}

impl Stroke {
    #[inline]
    pub fn new(width: f32) -> Stroke {
        Stroke {
            width: width,
        }
    }

    pub fn apply<I>(&self, output: &mut PathBuffer, stream: I)
                    where I: Iterator<Item = PathCommand> {
        let mut input = PathBuffer::new();
        input.add_stream(stream);

        for subpath_index in 0..(input.subpaths.len() as u32) {
            let first_endpoint_index = output.endpoints.len() as u32;

            // Compute offset curves.
            //
            // TODO(pcwalton): Support line caps.
            self.offset_subpath(output, &input, subpath_index);
            input.reverse_subpath(subpath_index);
            self.offset_subpath(output, &input, subpath_index);

            // Close the path.
            if !output.endpoints.is_empty() {
                let first_endpoint = output.endpoints[first_endpoint_index as usize];
                output.endpoints.push(first_endpoint);
            }

            let last_endpoint_index = output.endpoints.len() as u32;
            output.subpaths.push(Subpath {
                first_endpoint_index: first_endpoint_index,
                last_endpoint_index: last_endpoint_index,
            });
        }
    }

    /// TODO(pcwalton): Miter and round joins.
    fn offset_subpath(&self, output: &mut PathBuffer, input: &PathBuffer, subpath_index: u32) {
        let subpath = &input.subpaths[subpath_index as usize];

        let mut prev_position = None;
        for endpoint_index in subpath.first_endpoint_index..subpath.last_endpoint_index {
            let endpoint = &input.endpoints[endpoint_index as usize];
            let position = &endpoint.position;

            if let Some(ref prev_position) = prev_position {
                if endpoint.control_point_index == u32::MAX {
                    let offset_line = Line::new(&prev_position, position).offset(self.width);
                    output.endpoints.extend_from_slice(&[
                        Endpoint {
                            position: offset_line.endpoints[0],
                            control_point_index: u32::MAX,
                            subpath_index: 0,
                        },
                        Endpoint {
                            position: offset_line.endpoints[1],
                            control_point_index: u32::MAX,
                            subpath_index: 0,
                        },
                    ]);
                } else {
                    // This is the Tiller & Hanson 1984 algorithm for approximate Bézier offset
                    // curves. It's beautifully simple: just take the cage (i.e. convex hull) and
                    // push its edges out along their normals, then recompute the control point
                    // with a miter join.

                    let control_point_position =
                        &input.control_points[endpoint.control_point_index as usize];
                    let offset_line_0 =
                        Line::new(&prev_position, control_point_position).offset(self.width);
                    let offset_line_1 =
                        Line::new(control_point_position, position).offset(self.width);

                    // FIXME(pcwalton): Can the `None` case ever happen?
                    let offset_control_point =
                        offset_line_0.intersect_at_infinity(&offset_line_1).unwrap_or_else(|| {
                        offset_line_0.endpoints[1].lerp(offset_line_1.endpoints[0], 0.5)
                    });

                    output.endpoints.extend_from_slice(&[
                        Endpoint {
                            position: offset_line_0.endpoints[0],
                            control_point_index: u32::MAX,
                            subpath_index: 0,
                        },
                        Endpoint {
                            position: offset_line_1.endpoints[1],
                            control_point_index: output.control_points.len() as u32,
                            subpath_index: 0,
                        },
                    ]);

                    output.control_points.push(offset_control_point);
                }
            }

            prev_position = Some(*position)
        }
    }
}
