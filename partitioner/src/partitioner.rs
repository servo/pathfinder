// pathfinder/partitioner/src/partitioner.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bit_vec::BitVec;
use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Vector2D};
use log::LogLevel;
use lyon_geom::{LineSegment, QuadraticBezierSegment};
use lyon_path::iterator::PathIterator;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::f32;
use std::iter;
use std::ops::{Add, AddAssign};
use std::u32;

use indexed_path::IndexedPath;
use mesh_library::MeshLibrary;
use monotonic::MonotonicPathIterator;
use {BQuad, BVertexLoopBlinnData, BVertexKind, FillRule};

const MAX_B_QUAD_SUBDIVISIONS: u8 = 8;

const INTERSECTION_TOLERANCE: f32 = 0.001;

pub struct Partitioner {
    path: IndexedPath,
    path_id: u16,

    library: MeshLibrary,

    fill_rule: FillRule,

    heap: BinaryHeap<Point>,
    visited_points: BitVec,
    active_edges: Vec<ActiveEdge>,
    vertex_normals: Vec<VertexNormal>,
}

impl Partitioner {
    #[inline]
    pub fn new(library: MeshLibrary) -> Partitioner {
        Partitioner {
            path: IndexedPath::new(),
            path_id: 0,
            fill_rule: FillRule::Winding,

            library: library,

            heap: BinaryHeap::new(),
            visited_points: BitVec::new(),
            active_edges: vec![],
            vertex_normals: vec![],
        }
    }

    #[inline]
    pub fn library(&self) -> &MeshLibrary {
        &self.library
    } 

    #[inline]
    pub fn library_mut(&mut self) -> &mut MeshLibrary {
        &mut self.library
    }

    #[inline]
    pub fn into_library(self) -> MeshLibrary {
        self.library
    }

    #[inline]
    pub fn partition<I>(&mut self, path: I, path_id: u16, fill_rule: FillRule)
                        where I: PathIterator {
        self.partition_monotonic(MonotonicPathIterator::new(path), path_id, fill_rule)
    }

    pub fn partition_monotonic<I>(&mut self, path: I, path_id: u16, fill_rule: FillRule)
                                  where I: PathIterator {
        self.path.clear();
        self.heap.clear();
        self.active_edges.clear();

        self.path.add_monotonic_path(path);
        self.path_id = path_id;
        self.fill_rule = fill_rule;

        // FIXME(pcwalton): Right now, this bit vector uses too much memory.
        self.visited_points = BitVec::from_elem(self.path.endpoints.len(), false);

        let start_lengths = self.library.snapshot_lengths();

        self.init_heap();

        while self.process_next_point() {}

        debug_assert_eq!(self.library.b_vertex_loop_blinn_data.len(),
                         self.library.b_vertex_positions.len());

        let end_lengths = self.library.snapshot_lengths();

        let path_ranges = self.library.ensure_path_ranges(path_id);
        path_ranges.set_partitioning_lengths(&start_lengths, &end_lengths);
    }

    fn process_next_point(&mut self) -> bool {
        let point = match self.heap.peek() {
            Some(point) => *point,
            None => return false,
        };

        if self.already_visited_point(&point) {
            self.heap.pop();
            return true
        }

        if log_enabled!(LogLevel::Debug) {
            let position = self.path.endpoints[point.endpoint_index as usize].to;
            debug!("processing point {}: {:?}", point.endpoint_index, position);
            debug!("... active edges at {}:", position.x);
            for (active_edge_index, active_edge) in self.active_edges.iter().enumerate() {
                let y = self.solve_active_edge_y_for_x(position.x, active_edge);
                debug!("... ... edge {}: {:?} @ ({}, {})",
                       active_edge_index,
                       active_edge,
                       position.x,
                       y);
            }
        }

        self.mark_point_as_visited(&point);

        self.sort_active_edge_list_and_emit_self_intersections(point.endpoint_index);

        let matching_active_edges = self.find_right_point_in_active_edge_list(point.endpoint_index);
        match matching_active_edges.count {
            0 => self.process_min_endpoint(point.endpoint_index),
            1 => {
                self.process_regular_endpoint(point.endpoint_index,
                                              matching_active_edges.indices[0])
            }
            2 => self.process_max_endpoint(point.endpoint_index, matching_active_edges.indices),
            _ => debug_assert!(false),
        }

        true
    }

    fn process_min_endpoint(&mut self, endpoint_index: u32) {
        debug!("... MIN point");

        let next_active_edge_index = self.find_point_between_active_edges(endpoint_index);

        let endpoint = self.path.endpoints[endpoint_index as usize];
        self.emit_b_quads_around_active_edge(next_active_edge_index, endpoint.to.x);

        self.add_new_edges_for_min_point(endpoint_index, next_active_edge_index);

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);
        let new_point = self.create_point_from_endpoint(next_endpoint_index);
        *self.heap.peek_mut().unwrap() = new_point;
        if next_endpoint_index != prev_endpoint_index {
            let new_point = self.create_point_from_endpoint(prev_endpoint_index);
            self.heap.push(new_point)
        }
    }

    fn process_regular_endpoint(&mut self, endpoint_index: u32, active_edge_index: u32) {
        debug!("... REGULAR point: active edge {}", active_edge_index);

        let endpoint = self.path.endpoints[endpoint_index as usize];
        let bottom = self.emit_b_quads_around_active_edge(active_edge_index, endpoint.to.x) ==
            BQuadEmissionResult::BQuadEmittedAbove;

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);

        {
            let active_edge = &mut self.active_edges[active_edge_index as usize];
            let endpoint_position = self.path
                                        .endpoints[active_edge.right_endpoint_index as usize]
                                        .to;

            // If we already made a B-vertex point for this endpoint, reuse it instead of making a
            // new one.
            let old_left_position =
                self.library.b_vertex_positions[active_edge.left_vertex_index as usize];
            let should_update = (endpoint_position - old_left_position).square_length() >
                f32::approx_epsilon();
            if should_update {
                active_edge.left_vertex_index = self.library.b_vertex_loop_blinn_data.len() as u32;
                active_edge.control_point_vertex_index = active_edge.left_vertex_index + 1;

                // FIXME(pcwalton): Normal
                self.library.add_b_vertex(&endpoint_position,
                                          &BVertexLoopBlinnData::new(active_edge.endpoint_kind()));

                active_edge.toggle_parity();
            }

            if active_edge.left_to_right {
                active_edge.right_endpoint_index = next_endpoint_index;
            } else {
                active_edge.right_endpoint_index = prev_endpoint_index;
            }
        }

        let right_endpoint_index = self.active_edges[active_edge_index as usize]
                                       .right_endpoint_index;
        let new_point = self.create_point_from_endpoint(right_endpoint_index);
        *self.heap.peek_mut().unwrap() = new_point;

        let control_point = if self.active_edges[active_edge_index as usize].left_to_right {
            self.control_point_before_endpoint(next_endpoint_index)
        } else {
            self.control_point_after_endpoint(prev_endpoint_index)
        };

        match control_point {
            None => {
                self.active_edges[active_edge_index as usize].control_point_vertex_index = u32::MAX
            }
            Some(ref control_point_position) => {
                self.active_edges[active_edge_index as usize].control_point_vertex_index =
                    self.library.b_vertex_loop_blinn_data.len() as u32;

                let left_vertex_index = self.active_edges[active_edge_index as usize]
                                            .left_vertex_index;
                let control_point_b_vertex_loop_blinn_data = BVertexLoopBlinnData::control_point(
                    &self.library.b_vertex_positions[left_vertex_index as usize],
                    &control_point_position,
                    &new_point.position,
                    bottom);

                // FIXME(pcwalton): Normal
                self.library.add_b_vertex(control_point_position,
                                          &control_point_b_vertex_loop_blinn_data);
            }
        }
    }

    fn process_max_endpoint(&mut self, endpoint_index: u32, active_edge_indices: [u32; 2]) {
        debug!("... MAX point: active edges {:?}", active_edge_indices);

        debug_assert!(active_edge_indices[0] < active_edge_indices[1],
                      "Matching active edge indices in wrong order when processing MAX point");

        let endpoint = self.path.endpoints[endpoint_index as usize];

        // TODO(pcwalton): Collapse the two duplicate endpoints that this will create together if
        // possible (i.e. if they have the same parity).
        self.emit_b_quads_around_active_edge(active_edge_indices[0], endpoint.to.x);
        self.emit_b_quads_around_active_edge(active_edge_indices[1], endpoint.to.x);

        // Add supporting interior triangles if necessary.
        self.heap.pop();

        // FIXME(pcwalton): This is twice as slow as it needs to be.
        self.active_edges.remove(active_edge_indices[1] as usize);
        self.active_edges.remove(active_edge_indices[0] as usize);
    }

    fn sort_active_edge_list_and_emit_self_intersections(&mut self, endpoint_index: u32) {
        let x = self.path.endpoints[endpoint_index as usize].to.x;
        loop {
            let mut swapped = false;
            for lower_active_edge_index in 1..(self.active_edges.len() as u32) {
                let upper_active_edge_index = lower_active_edge_index - 1;

                if self.active_edges_are_ordered(upper_active_edge_index,
                                                 lower_active_edge_index,
                                                 x) {
                    continue
                }

                if let Some(crossing_point) =
                        self.crossing_point_for_active_edge(upper_active_edge_index, x) {
                    debug!("found SELF-INTERSECTION point for active edges {} & {}",
                           upper_active_edge_index,
                           lower_active_edge_index);
                    self.emit_b_quads_around_active_edge(upper_active_edge_index, crossing_point.x);
                    self.emit_b_quads_around_active_edge(lower_active_edge_index, crossing_point.x);
                } else {
                    warn!("swapped active edges {} & {} without finding intersection; rendering \
                           will probably be incorrect",
                          upper_active_edge_index,
                          lower_active_edge_index);
                }

                self.active_edges.swap(upper_active_edge_index as usize,
                                       lower_active_edge_index as usize);
                swapped = true;
            }

            if !swapped {
                break
            }
        }
    }

    fn add_new_edges_for_min_point(&mut self, endpoint_index: u32, next_active_edge_index: u32) {
        // FIXME(pcwalton): This is twice as slow as it needs to be.
        self.active_edges.insert(next_active_edge_index as usize, ActiveEdge::default());
        self.active_edges.insert(next_active_edge_index as usize, ActiveEdge::default());

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);

        let new_active_edges = &mut self.active_edges[next_active_edge_index as usize..
                                                      next_active_edge_index as usize + 2];

        let left_vertex_index = self.library.b_vertex_loop_blinn_data.len() as u32;
        new_active_edges[0].left_vertex_index = left_vertex_index;
        new_active_edges[1].left_vertex_index = left_vertex_index;

        // FIXME(pcwalton): Normal
        let position = self.path.endpoints[endpoint_index as usize].to;
        self.library.add_b_vertex(&position,
                                  &BVertexLoopBlinnData::new(BVertexKind::Endpoint0));

        new_active_edges[0].toggle_parity();
        new_active_edges[1].toggle_parity();

        let endpoint = &self.path.endpoints[endpoint_index as usize];
        let prev_endpoint = &self.path.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.path.endpoints[next_endpoint_index as usize];

        let prev_vector = prev_endpoint.to - endpoint.to;
        let next_vector = next_endpoint.to - endpoint.to;

        let (upper_control_point, lower_control_point);
        if prev_vector.cross(next_vector) >= 0.0 {
            new_active_edges[0].right_endpoint_index = prev_endpoint_index;
            new_active_edges[1].right_endpoint_index = next_endpoint_index;
            new_active_edges[0].left_to_right = false;
            new_active_edges[1].left_to_right = true;

            upper_control_point = self.path.endpoints[endpoint_index as usize].ctrl;
            lower_control_point = self.path.endpoints[next_endpoint_index as usize].ctrl;
        } else {
            new_active_edges[0].right_endpoint_index = next_endpoint_index;
            new_active_edges[1].right_endpoint_index = prev_endpoint_index;
            new_active_edges[0].left_to_right = true;
            new_active_edges[1].left_to_right = false;

            upper_control_point = self.path.endpoints[next_endpoint_index as usize].ctrl;
            lower_control_point = self.path.endpoints[endpoint_index as usize].ctrl;
        }

        match upper_control_point {
            None => new_active_edges[0].control_point_vertex_index = u32::MAX,
            Some(control_point_position) => {
                new_active_edges[0].control_point_vertex_index =
                    self.library.b_vertex_loop_blinn_data.len() as u32;

                let right_vertex_position =
                    self.path.endpoints[new_active_edges[0].right_endpoint_index as usize].to;
                let control_point_b_vertex_loop_blinn_data =
                    BVertexLoopBlinnData::control_point(&position,
                                                        &control_point_position,
                                                        &right_vertex_position,
                                                        false);

                // FIXME(pcwalton): Normal
                self.library.add_b_vertex(&control_point_position,
                                          &control_point_b_vertex_loop_blinn_data)
            }
        }

        match lower_control_point {
            None => new_active_edges[1].control_point_vertex_index = u32::MAX,
            Some(control_point_position) => {
                new_active_edges[1].control_point_vertex_index =
                    self.library.b_vertex_loop_blinn_data.len() as u32;

                let right_vertex_position =
                    self.path.endpoints[new_active_edges[1].right_endpoint_index as usize].to;
                let control_point_b_vertex_loop_blinn_data =
                    BVertexLoopBlinnData::control_point(&position,
                                                        &control_point_position,
                                                        &right_vertex_position,
                                                        true);

                // FIXME(pcwalton): Normal
                self.library.add_b_vertex(&control_point_position,
                                          &control_point_b_vertex_loop_blinn_data)
            }
        }
    }

    fn active_edges_are_ordered(&mut self,
                                prev_active_edge_index: u32,
                                next_active_edge_index: u32,
                                x: f32)
                                -> bool {
        let prev_active_edge = &self.active_edges[prev_active_edge_index as usize];
        let next_active_edge = &self.active_edges[next_active_edge_index as usize];
        if prev_active_edge.right_endpoint_index == next_active_edge.right_endpoint_index {
            // Always ordered.
            // FIXME(pcwalton): Is this true?
            return true
        }

        // TODO(pcwalton): See if we can speed this up. It's trickier than it seems, due to path
        // self intersection!
        let prev_active_edge_t = self.solve_active_edge_t_for_x(x, prev_active_edge);
        let next_active_edge_t = self.solve_active_edge_t_for_x(x, next_active_edge);
        let prev_active_edge_y = self.sample_active_edge(prev_active_edge_t, prev_active_edge).y;
        let next_active_edge_y = self.sample_active_edge(next_active_edge_t, next_active_edge).y;
        prev_active_edge_y <= next_active_edge_y
    }

    fn init_heap(&mut self) {
        for endpoint_index in 0..(self.path.endpoints.len() as u32) {
            if let EndpointClass::Min = self.classify_endpoint(endpoint_index) {
                let new_point = self.create_point_from_endpoint(endpoint_index);
                self.heap.push(new_point)
            }
        }
    }

    fn bounding_active_edges_for_fill(&self, active_edge_index: u32) -> (u32, u32) {
        match self.fill_rule {
            FillRule::EvenOdd if active_edge_index % 2 == 1 => {
                (active_edge_index - 1, active_edge_index)
            }
            FillRule::EvenOdd if (active_edge_index as usize) + 1 == self.active_edges.len() => {
                (active_edge_index, active_edge_index)
            }
            FillRule::EvenOdd => (active_edge_index, active_edge_index + 1),

            FillRule::Winding => {
                let (mut winding_number, mut upper_active_edge_index) = (0, 0);
                for (active_edge_index, active_edge) in
                        self.active_edges[0..active_edge_index as usize].iter().enumerate() {
                    if winding_number == 0 {
                        upper_active_edge_index = active_edge_index as u32
                    }
                    winding_number += active_edge.winding_number()
                }
                if winding_number == 0 {
                    upper_active_edge_index = active_edge_index as u32
                }

                let mut lower_active_edge_index = active_edge_index;
                for (active_edge_index, active_edge) in
                        self.active_edges.iter().enumerate().skip(active_edge_index as usize) {
                    winding_number += active_edge.winding_number();
                    if winding_number == 0 {
                        lower_active_edge_index = active_edge_index as u32;
                        break
                    }
                }

                (upper_active_edge_index, lower_active_edge_index)
            }
        }
    }

    fn emit_b_quads_around_active_edge(&mut self, active_edge_index: u32, right_x: f32)
                                       -> BQuadEmissionResult {
        if (active_edge_index as usize) >= self.active_edges.len() {
            return BQuadEmissionResult::NoBQuadEmitted
        }

        // TODO(pcwalton): Assert that the green X position is the same on both edges.
        let (upper_active_edge_index, lower_active_edge_index) =
            self.bounding_active_edges_for_fill(active_edge_index);
        debug!("... bounding active edges for fill = [{},{}] around {}",
               upper_active_edge_index,
               lower_active_edge_index,
               active_edge_index);

        let emission_result = BQuadEmissionResult::new(active_edge_index,
                                                       upper_active_edge_index,
                                                       lower_active_edge_index);
        if emission_result == BQuadEmissionResult::NoBQuadEmitted {
            return emission_result
        }

        if !self.should_subdivide_active_edge_at(upper_active_edge_index, right_x) ||
                !self.should_subdivide_active_edge_at(lower_active_edge_index, right_x) {
            return emission_result
        }

        let upper_curve = self.subdivide_active_edge_at(upper_active_edge_index,
                                                        right_x,
                                                        SubdivisionType::Upper);
        for active_edge_index in (upper_active_edge_index + 1)..lower_active_edge_index {
            if self.should_subdivide_active_edge_at(active_edge_index, right_x) {
                self.subdivide_active_edge_at(active_edge_index, right_x, SubdivisionType::Inside);
                self.active_edges[active_edge_index as usize].toggle_parity();
            }
        }
        let lower_curve = self.subdivide_active_edge_at(lower_active_edge_index,
                                                        right_x,
                                                        SubdivisionType::Lower);

        self.emit_b_quads(upper_active_edge_index,
                          lower_active_edge_index,
                          &upper_curve,
                          &lower_curve,
                          0);

        emission_result
    }

    /// Toggles parity at the end.
    fn emit_b_quads(&mut self,
                    upper_active_edge_index: u32,
                    lower_active_edge_index: u32,
                    upper_subdivision: &SubdividedActiveEdge,
                    lower_subdivision: &SubdividedActiveEdge,
                    iteration: u8) {
        let upper_shape = upper_subdivision.shape(&self.library.b_vertex_loop_blinn_data);
        let lower_shape = lower_subdivision.shape(&self.library.b_vertex_loop_blinn_data);

        // Make sure the convex hulls of the two curves do not intersect. If they do, subdivide and
        // recurse.
        if iteration < MAX_B_QUAD_SUBDIVISIONS {
            // TODO(pcwalton): Handle concave-line convex hull intersections.
            if let (Some(upper_curve), Some(lower_curve)) =
                    (upper_subdivision.to_curve(&self.library.b_vertex_positions),
                     lower_subdivision.to_curve(&self.library.b_vertex_positions)) {
                // TODO(pcwalton): Handle concave-concave convex hull intersections.
                if upper_shape == Shape::Concave &&
                        lower_curve.baseline()
                                   .to_line()
                                   .signed_distance_to_point(&upper_curve.ctrl) >
                        f32::approx_epsilon() {
                    let (upper_left_subsubdivision, upper_right_subsubdivision) =
                        self.subdivide_active_edge_again_at_t(&upper_subdivision,
                                                              0.5,
                                                              false);
                    let midpoint_x =
                        self.library
                            .b_vertex_positions[upper_left_subsubdivision.middle_point as usize].x;
                    let (lower_left_subsubdivision, lower_right_subsubdivision) =
                        self.subdivide_active_edge_again_at_x(&lower_subdivision,
                                                              midpoint_x,
                                                              true);

                    self.emit_b_quads(upper_active_edge_index,
                                      lower_active_edge_index,
                                      &upper_left_subsubdivision,
                                      &lower_left_subsubdivision,
                                      iteration + 1);
                    self.emit_b_quads(upper_active_edge_index,
                                      lower_active_edge_index,
                                      &upper_right_subsubdivision,
                                      &lower_right_subsubdivision,
                                      iteration + 1);
                    return;
                }

                if lower_shape == Shape::Concave &&
                        upper_curve.baseline()
                                   .to_line()
                                   .signed_distance_to_point(&lower_curve.ctrl) <
                        -f32::approx_epsilon() {
                    let (lower_left_subsubdivision, lower_right_subsubdivision) =
                        self.subdivide_active_edge_again_at_t(&lower_subdivision,
                                                              0.5,
                                                              true);
                    let midpoint_x =
                        self.library
                            .b_vertex_positions[lower_left_subsubdivision.middle_point as usize].x;
                    let (upper_left_subsubdivision, upper_right_subsubdivision) =
                        self.subdivide_active_edge_again_at_x(&upper_subdivision,
                                                              midpoint_x,
                                                              false);

                    self.emit_b_quads(upper_active_edge_index,
                                      lower_active_edge_index,
                                      &upper_left_subsubdivision,
                                      &lower_left_subsubdivision,
                                      iteration + 1);
                    self.emit_b_quads(upper_active_edge_index,
                                      lower_active_edge_index,
                                      &upper_right_subsubdivision,
                                      &lower_right_subsubdivision,
                                      iteration + 1);
                    return;
                }
            }
        }

        debug!("... emitting B-quad: UL {} BL {} UR {} BR {}",
               upper_subdivision.left_curve_left,
               lower_subdivision.left_curve_left,
               upper_subdivision.middle_point,
               lower_subdivision.middle_point);

        {
            let upper_active_edge = &mut self.active_edges[upper_active_edge_index as usize];
            self.library.b_vertex_loop_blinn_data[upper_subdivision.middle_point as usize] =
                BVertexLoopBlinnData::new(upper_active_edge.endpoint_kind());
            upper_active_edge.toggle_parity();
        }
        {
            let lower_active_edge = &mut self.active_edges[lower_active_edge_index as usize];
            self.library.b_vertex_loop_blinn_data[lower_subdivision.middle_point as usize] =
                BVertexLoopBlinnData::new(lower_active_edge.endpoint_kind());
            lower_active_edge.toggle_parity();
        }

        let b_quad = BQuad::new(upper_subdivision.left_curve_left,
                                upper_subdivision.left_curve_control_point,
                                upper_subdivision.middle_point,
                                lower_subdivision.left_curve_left,
                                lower_subdivision.left_curve_control_point,
                                lower_subdivision.middle_point);

        self.update_vertex_normals_for_new_b_quad(&b_quad);

        self.library.add_b_quad(&b_quad);
    }

    fn subdivide_active_edge_again_at_t(&mut self,
                                        subdivision: &SubdividedActiveEdge,
                                        t: f32,
                                        bottom: bool)
                                        -> (SubdividedActiveEdge, SubdividedActiveEdge) {
        let curve = subdivision.to_curve(&self.library.b_vertex_positions)
                               .expect("subdivide_active_edge_again_at_t(): not a curve!");
        let (left_curve, right_curve) = curve.assume_monotonic().split(t);

        let left_control_point_index = self.library.b_vertex_positions.len() as u32;
        let midpoint_index = left_control_point_index + 1;
        let right_control_point_index = midpoint_index + 1;
        self.library.b_vertex_positions.extend([
            left_curve.segment().ctrl,
            left_curve.segment().to,
            right_curve.segment().ctrl,
        ].into_iter());

        // Initially, assume that the parity is false. We will modify the Loop-Blinn data later if
        // that is incorrect.
        self.library.b_vertex_loop_blinn_data.extend([
            BVertexLoopBlinnData::control_point(&left_curve.segment().from,
                                                &left_curve.segment().ctrl,
                                                &left_curve.segment().to,
                                                bottom),
            BVertexLoopBlinnData::new(BVertexKind::Endpoint0),
            BVertexLoopBlinnData::control_point(&right_curve.segment().from,
                                                &right_curve.segment().ctrl,
                                                &right_curve.segment().to,
                                                bottom),
        ].into_iter());

        // FIXME(pcwalton): Normal

        (SubdividedActiveEdge {
            left_curve_left: subdivision.left_curve_left,
            left_curve_control_point: left_control_point_index,
            middle_point: midpoint_index,
        }, SubdividedActiveEdge {
            left_curve_left: midpoint_index,
            left_curve_control_point: right_control_point_index,
            middle_point: subdivision.middle_point,
        })
    }

    fn subdivide_active_edge_again_at_x(&mut self,
                                        subdivision: &SubdividedActiveEdge,
                                        x: f32,
                                        bottom: bool)
                                        -> (SubdividedActiveEdge, SubdividedActiveEdge) {
        let curve = subdivision.to_curve(&self.library.b_vertex_positions)
                               .expect("subdivide_active_edge_again_at_x(): not a curve!");
        let t = curve.assume_monotonic().solve_t_for_x(x);
        self.subdivide_active_edge_again_at_t(subdivision, t, bottom)
    }

    fn already_visited_point(&self, point: &Point) -> bool {
        // FIXME(pcwalton): This makes the visited vector too big.
        let index = point.endpoint_index as usize;
        match self.visited_points.get(index) {
            None => false,
            Some(visited) => visited,
        }
    }

    fn mark_point_as_visited(&mut self, point: &Point) {
        // FIXME(pcwalton): This makes the visited vector too big.
        self.visited_points.set(point.endpoint_index as usize, true)
    }

    fn find_right_point_in_active_edge_list(&self, endpoint_index: u32) -> MatchingActiveEdges {
        let mut matching_active_edges = MatchingActiveEdges {
            indices: [0, 0],
            count: 0,
        };

        for (active_edge_index, active_edge) in self.active_edges.iter().enumerate() {
            if active_edge.right_endpoint_index == endpoint_index {
                matching_active_edges.indices[matching_active_edges.count as usize] =
                    active_edge_index as u32;
                matching_active_edges.count += 1;
                if matching_active_edges.count == 2 {
                    break
                }
            }
        }

        matching_active_edges
    }

    fn classify_endpoint(&self, endpoint_index: u32) -> EndpointClass {
        // Create temporary points just for the comparison.
        let point = self.create_point_from_endpoint(endpoint_index);
        let prev_point = self.create_point_from_endpoint(self.prev_endpoint_of(endpoint_index));
        let next_point = self.create_point_from_endpoint(self.next_endpoint_of(endpoint_index));

        // Remember to reverse, because the comparison is reversed (as the heap is a max-heap).
        match (prev_point.cmp(&point).reverse(), next_point.cmp(&point).reverse()) {
            (Ordering::Less, Ordering::Less) => EndpointClass::Max,
            (Ordering::Less, _) | (_, Ordering::Less) => EndpointClass::Regular,
            (_, _) => EndpointClass::Min,
        }
    }

    fn find_point_between_active_edges(&self, endpoint_index: u32) -> u32 {
        let endpoint = &self.path.endpoints[endpoint_index as usize];
        match self.active_edges.iter().position(|active_edge| {
            self.solve_active_edge_y_for_x(endpoint.to.x, active_edge) > endpoint.to.y
        }) {
            Some(active_edge_index) => active_edge_index as u32,
            None => self.active_edges.len() as u32,
        }
    }

    fn solve_active_edge_t_for_x(&self, x: f32, active_edge: &ActiveEdge) -> f32 {
        let left_vertex_position =
            &self.library.b_vertex_positions[active_edge.left_vertex_index as usize];
        let right_endpoint_position =
            &self.path.endpoints[active_edge.right_endpoint_index as usize].to;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                LineSegment {
                    from: *left_vertex_position,
                    to: *right_endpoint_position,
                }.solve_t_for_x(x)
            }
            control_point_vertex_index => {
                let control_point = &self.library
                                         .b_vertex_positions[control_point_vertex_index as usize];
                QuadraticBezierSegment {
                    from: *left_vertex_position,
                    ctrl: *control_point,
                    to: *right_endpoint_position,
                }.assume_monotonic().solve_t_for_x(x)
            }
        }
    }

    fn solve_active_edge_y_for_x(&self, x: f32, active_edge: &ActiveEdge) -> f32 {
        self.sample_active_edge(self.solve_active_edge_t_for_x(x, active_edge), active_edge).y
    }

    fn sample_active_edge(&self, t: f32, active_edge: &ActiveEdge) -> Point2D<f32> {
        let left_vertex_position =
            &self.library.b_vertex_positions[active_edge.left_vertex_index as usize];
        let right_endpoint_position =
            &self.path.endpoints[active_edge.right_endpoint_index as usize].to;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                left_vertex_position.to_vector()
                                    .lerp(right_endpoint_position.to_vector(), t)
                                    .to_point()
            }
            control_point_vertex_index => {
                let control_point = &self.library
                                         .b_vertex_positions[control_point_vertex_index as usize];
                QuadraticBezierSegment {
                    from: *left_vertex_position,
                    ctrl: *control_point,
                    to: *right_endpoint_position,
                }.sample(t)
            }
        }
    }

    fn crossing_point_for_active_edge(&self, upper_active_edge_index: u32, max_x: f32)
                                      -> Option<Point2D<f32>> {
        let lower_active_edge_index = upper_active_edge_index + 1;

        let upper_active_edge = &self.active_edges[upper_active_edge_index as usize];
        let lower_active_edge = &self.active_edges[lower_active_edge_index as usize];
        if upper_active_edge.left_vertex_index == lower_active_edge.left_vertex_index ||
                upper_active_edge.right_endpoint_index == lower_active_edge.right_endpoint_index {
            return None
        }

        let upper_left_vertex_position =
            &self.library.b_vertex_positions[upper_active_edge.left_vertex_index as usize];
        let upper_right_endpoint_position =
            &self.path.endpoints[upper_active_edge.right_endpoint_index as usize].to;
        let lower_left_vertex_position =
            &self.library.b_vertex_positions[lower_active_edge.left_vertex_index as usize];
        let lower_right_endpoint_position =
            &self.path.endpoints[lower_active_edge.right_endpoint_index as usize].to;

        match (upper_active_edge.control_point_vertex_index,
               lower_active_edge.control_point_vertex_index) {
            (u32::MAX, u32::MAX) => {
                let (upper_line, _) = LineSegment {
                    from: *upper_left_vertex_position,
                    to: *upper_right_endpoint_position,
                }.split_at_x(max_x);
                let (lower_line, _) = LineSegment {
                    from: *lower_left_vertex_position,
                    to: *lower_right_endpoint_position,
                }.split_at_x(max_x);
                upper_line.intersection(&lower_line)
            }

            (upper_control_point_vertex_index, u32::MAX) => {
                let upper_control_point =
                    &self.library.b_vertex_positions[upper_control_point_vertex_index as usize];
                let (upper_curve, _) = QuadraticBezierSegment {
                    from: *upper_left_vertex_position,
                    ctrl: *upper_control_point,
                    to: *upper_right_endpoint_position,
                }.assume_monotonic().split_at_x(max_x);
                let (lower_line, _) = LineSegment {
                    from: *lower_left_vertex_position,
                    to: *lower_right_endpoint_position,
                }.split_at_x(max_x);
                upper_curve.segment().line_segment_intersections(&lower_line).pop()
            }

            (u32::MAX, lower_control_point_vertex_index) => {
                let lower_control_point =
                    &self.library.b_vertex_positions[lower_control_point_vertex_index as usize];
                let (lower_curve, _) = QuadraticBezierSegment {
                    from: *lower_left_vertex_position,
                    ctrl: *lower_control_point,
                    to: *lower_right_endpoint_position,
                }.assume_monotonic().split_at_x(max_x);
                let (upper_line, _) = LineSegment {
                    from: *upper_left_vertex_position,
                    to: *upper_right_endpoint_position,
                }.split_at_x(max_x);
                lower_curve.segment().line_segment_intersections(&upper_line).pop()
            }

            (upper_control_point_vertex_index, lower_control_point_vertex_index) => {
                let upper_control_point =
                    &self.library.b_vertex_positions[upper_control_point_vertex_index as usize];
                let lower_control_point =
                    &self.library.b_vertex_positions[lower_control_point_vertex_index as usize];
                let (upper_curve, _) = QuadraticBezierSegment {
                    from: *upper_left_vertex_position,
                    ctrl: *upper_control_point,
                    to: *upper_right_endpoint_position,
                }.assume_monotonic().split_at_x(max_x);
                let (lower_curve, _) = QuadraticBezierSegment {
                    from: *lower_left_vertex_position,
                    ctrl: *lower_control_point,
                    to: *lower_right_endpoint_position,
                }.assume_monotonic().split_at_x(max_x);
                upper_curve.first_intersection(0.0..1.0,
                                               &lower_curve,
                                               0.0..1.0,
                                               INTERSECTION_TOLERANCE)
            }
        }
    }

    fn should_subdivide_active_edge_at(&self, active_edge_index: u32, x: f32) -> bool {
        let left_curve_left = self.active_edges[active_edge_index as usize].left_vertex_index;
        let left_point_position = self.library.b_vertex_positions[left_curve_left as usize];
        x - left_point_position.x >= f32::approx_epsilon()
    }

    /// Does *not* toggle parity. You must do this after calling this function.
    fn subdivide_active_edge_at(&mut self,
                                active_edge_index: u32,
                                x: f32,
                                subdivision_type: SubdivisionType)
                                -> SubdividedActiveEdge {
        let left_curve_left = self.active_edges[active_edge_index as usize].left_vertex_index;
        let left_point_position = self.library.b_vertex_positions[left_curve_left as usize];

        let t = self.solve_active_edge_t_for_x(x, &self.active_edges[active_edge_index as usize]);

        let bottom = subdivision_type == SubdivisionType::Lower;
        let active_edge = &mut self.active_edges[active_edge_index as usize];

        let left_curve_control_point_vertex_index;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                let right_point =
                    self.path.endpoints[active_edge.right_endpoint_index as usize].to;
                let middle_point = left_point_position.to_vector()
                                                      .lerp(right_point.to_vector(), t);

                // FIXME(pcwalton): Normal
                active_edge.left_vertex_index = self.library.b_vertex_loop_blinn_data.len() as u32;
                self.library.add_b_vertex(&middle_point.to_point(),
                                          &BVertexLoopBlinnData::new(active_edge.endpoint_kind()));

                left_curve_control_point_vertex_index = u32::MAX;
            }
            _ => {
                let left_endpoint_position =
                    self.library.b_vertex_positions[active_edge.left_vertex_index as usize];
                let control_point_position =
                    self.library
                        .b_vertex_positions[active_edge.control_point_vertex_index as usize];
                let right_endpoint_position =
                    self.path.endpoints[active_edge.right_endpoint_index as usize].to;
                let (left_curve, right_curve) = QuadraticBezierSegment {
                    from: left_endpoint_position,
                    ctrl: control_point_position,
                    to: right_endpoint_position,
                }.split(t);

                left_curve_control_point_vertex_index =
                    self.library.b_vertex_loop_blinn_data.len() as u32;
                active_edge.left_vertex_index = left_curve_control_point_vertex_index + 1;
                active_edge.control_point_vertex_index = left_curve_control_point_vertex_index + 2;

                // FIXME(pcwalton): Normals
                self.library
                    .add_b_vertex(&left_curve.ctrl,
                                  &BVertexLoopBlinnData::control_point(&left_curve.from,
                                                                       &left_curve.ctrl,
                                                                       &left_curve.to,
                                                                       bottom));
                self.library.add_b_vertex(&left_curve.to,
                                          &BVertexLoopBlinnData::new(active_edge.endpoint_kind()));
                self.library
                    .add_b_vertex(&right_curve.ctrl,
                                  &BVertexLoopBlinnData::control_point(&right_curve.from,
                                                                       &right_curve.ctrl,
                                                                       &right_curve.to,
                                                                       bottom));
            }
        }

        SubdividedActiveEdge {
            left_curve_left: left_curve_left,
            left_curve_control_point: left_curve_control_point_vertex_index,
            middle_point: active_edge.left_vertex_index,
        }
    }

    // FIXME(pcwalton): This creates incorrect normals for vertical lines. I think we should
    // probably calculate normals for the path vertices first and then lerp them to calculate these
    // B-vertex normals. That would be simpler, faster, and more correct, I suspect.
    fn update_vertex_normals_for_new_b_quad(&mut self, b_quad: &BQuad) {
        self.update_vertex_normal_for_b_quad_edge(b_quad.upper_left_vertex_index,
                                                  b_quad.upper_control_point_vertex_index,
                                                  b_quad.upper_right_vertex_index);
        self.update_vertex_normal_for_b_quad_edge(b_quad.lower_right_vertex_index,
                                                  b_quad.lower_control_point_vertex_index,
                                                  b_quad.lower_left_vertex_index);
    }

    fn update_vertex_normal_for_b_quad_edge(&mut self,
                                            prev_vertex_index: u32,
                                            control_point_vertex_index: u32,
                                            next_vertex_index: u32) {
        if control_point_vertex_index == u32::MAX {
            let normal_vector = self.calculate_normal_for_edge(prev_vertex_index,
                                                               next_vertex_index);
            self.update_normal_for_vertex(prev_vertex_index, &normal_vector);
            self.update_normal_for_vertex(next_vertex_index, &normal_vector);
            return
        }

        let prev_normal_vector = self.calculate_normal_for_edge(prev_vertex_index,
                                                                control_point_vertex_index);
        let next_normal_vector = self.calculate_normal_for_edge(control_point_vertex_index,
                                                                next_vertex_index);
        self.update_normal_for_vertex(prev_vertex_index, &prev_normal_vector);
        self.update_normal_for_vertex(control_point_vertex_index, &prev_normal_vector);
        self.update_normal_for_vertex(control_point_vertex_index, &next_normal_vector);
        self.update_normal_for_vertex(next_vertex_index, &next_normal_vector);
    }

    fn update_normal_for_vertex(&mut self, vertex_index: u32, normal_vector: &VertexNormal) {
        let vertex_normal_count = self.vertex_normals.len();
        if vertex_index as usize >= vertex_normal_count {
            let new_vertex_normal_count = vertex_index as usize - vertex_normal_count + 1;
            self.vertex_normals
                .extend(iter::repeat(VertexNormal::zero()).take(new_vertex_normal_count));
        }

        self.vertex_normals[vertex_index as usize] += *normal_vector
    }

    fn calculate_normal_for_edge(&self, left_vertex_index: u32, right_vertex_index: u32)
                                 -> VertexNormal {
        let left_vertex_position = &self.library.b_vertex_positions[left_vertex_index as usize];
        let right_vertex_position = &self.library.b_vertex_positions[right_vertex_index as usize];
        VertexNormal::new(left_vertex_position, right_vertex_position)
    }

    fn prev_endpoint_of(&self, endpoint_index: u32) -> u32 {
        let endpoint = &self.path.endpoints[endpoint_index as usize];
        let subpath = &self.path.subpath_ranges[endpoint.subpath_index as usize];
        if endpoint_index > subpath.start {
            endpoint_index - 1
        } else {
            subpath.end - 1
        }
    }

    fn next_endpoint_of(&self, endpoint_index: u32) -> u32 {
        let endpoint = &self.path.endpoints[endpoint_index as usize];
        let subpath = &self.path.subpath_ranges[endpoint.subpath_index as usize];
        if endpoint_index + 1 < subpath.end {
            endpoint_index + 1
        } else {
            subpath.start
        }
    }

    fn create_point_from_endpoint(&self, endpoint_index: u32) -> Point {
        Point {
            position: self.path.endpoints[endpoint_index as usize].to,
            endpoint_index: endpoint_index,
        }
    }

    fn control_point_before_endpoint(&self, endpoint_index: u32) -> Option<Point2D<f32>> {
        self.path.endpoints[endpoint_index as usize].ctrl
    }

    fn control_point_after_endpoint(&self, endpoint_index: u32) -> Option<Point2D<f32>> {
        self.control_point_before_endpoint(self.next_endpoint_of(endpoint_index))
    }
}

#[derive(Debug, Clone, Copy)]
struct Point {
    position: Point2D<f32>,
    endpoint_index: u32,
}

impl PartialEq for Point {
    #[inline]
    fn eq(&self, other: &Point) -> bool {
        self.position == other.position && self.endpoint_index == other.endpoint_index
    }
    #[inline]
    fn ne(&self, other: &Point) -> bool {
        self.position != other.position || self.endpoint_index != other.endpoint_index
    }
}

impl Eq for Point {}

impl PartialOrd for Point {
    #[inline]
    fn partial_cmp(&self, other: &Point) -> Option<Ordering> {
        // Reverse, because `std::collections::BinaryHeap` is a *max*-heap!
        match other.position.x.partial_cmp(&self.position.x) {
            None | Some(Ordering::Equal) => {}
            Some(ordering) => return Some(ordering),
        }
        match other.position.y.partial_cmp(&self.position.y) {
            None | Some(Ordering::Equal) => {}
            Some(ordering) => return Some(ordering),
        }
        other.endpoint_index.partial_cmp(&self.endpoint_index)
    }
}

impl Ord for Point {
    #[inline]
    fn cmp(&self, other: &Point) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

#[derive(Debug, Clone, Copy)]
struct ActiveEdge {
    left_vertex_index: u32,
    control_point_vertex_index: u32,
    right_endpoint_index: u32,
    left_to_right: bool,
    parity: bool,
}

impl Default for ActiveEdge {
    fn default() -> ActiveEdge {
        ActiveEdge {
            left_vertex_index: 0,
            control_point_vertex_index: u32::MAX,
            right_endpoint_index: 0,
            left_to_right: false,
            parity: false,
        }
    }
}

impl ActiveEdge {
    fn toggle_parity(&mut self) {
        self.parity = !self.parity
    }

    fn endpoint_kind(&self) -> BVertexKind {
        if !self.parity {
            BVertexKind::Endpoint0
        } else {
            BVertexKind::Endpoint1
        }
    }

    #[inline]
    fn winding_number(&self) -> i32 {
        if self.left_to_right {
            1
        } else {
            -1
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SubdividedActiveEdge {
    left_curve_left: u32,
    left_curve_control_point: u32,
    middle_point: u32,
}

impl SubdividedActiveEdge {
    fn shape(&self, b_vertex_loop_blinn_data: &[BVertexLoopBlinnData]) -> Shape {
        if self.left_curve_control_point == u32::MAX {
            Shape::Flat
        } else if b_vertex_loop_blinn_data[self.left_curve_control_point as usize].sign < 0 {
            Shape::Convex
        } else {
            Shape::Concave
        }
    }

    fn to_curve(&self, b_vertex_positions: &[Point2D<f32>])
                -> Option<QuadraticBezierSegment<f32>> {
        if self.left_curve_control_point == u32::MAX {
            None
        } else {
            Some(QuadraticBezierSegment {
                from: b_vertex_positions[self.left_curve_left as usize],
                ctrl: b_vertex_positions[self.left_curve_control_point as usize],
                to: b_vertex_positions[self.middle_point as usize],
            })
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EndpointClass {
    Min,
    Regular,
    Max,
}

#[derive(Debug, Clone, Copy)]
struct MatchingActiveEdges {
    indices: [u32; 2],
    count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Flat,
    Convex,
    Concave,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BQuadEmissionResult {
    NoBQuadEmitted,
    BQuadEmittedBelow,
    BQuadEmittedAbove,
    BQuadEmittedAround,
}

impl BQuadEmissionResult {
    fn new(active_edge_index: u32, upper_active_edge_index: u32, lower_active_edge_index: u32)
           -> BQuadEmissionResult {
        if upper_active_edge_index == lower_active_edge_index {
            BQuadEmissionResult::NoBQuadEmitted
        } else if upper_active_edge_index == active_edge_index {
            BQuadEmissionResult::BQuadEmittedBelow
        } else if lower_active_edge_index == active_edge_index {
            BQuadEmissionResult::BQuadEmittedAbove
        } else {
            BQuadEmissionResult::BQuadEmittedAround
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SubdivisionType {
    Upper,
    Inside,
    Lower,
}

/// TODO(pcwalton): This could possibly be improved:
/// https://en.wikipedia.org/wiki/Mean_of_circular_quantities
#[derive(Debug, Clone, Copy)]
struct VertexNormal {
    sum: Vector2D<f32>,
}

impl VertexNormal {
    fn new(vertex_a: &Point2D<f32>, vertex_b: &Point2D<f32>) -> VertexNormal {
        let vector = *vertex_a - *vertex_b;
        VertexNormal {
            sum: Vector2D::new(-vector.y, vector.x).normalize(),
        }
    }

    fn zero() -> VertexNormal {
        VertexNormal {
            sum: Vector2D::zero(),
        }
    }
}

impl Add<VertexNormal> for VertexNormal {
    type Output = VertexNormal;
    fn add(self, rhs: VertexNormal) -> VertexNormal {
        VertexNormal {
            sum: self.sum + rhs.sum,
        }
    }
}

impl AddAssign<VertexNormal> for VertexNormal {
    fn add_assign(&mut self, rhs: VertexNormal) {
        *self = *self + rhs
    }
}
