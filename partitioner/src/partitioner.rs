// pathfinder/partitioner/src/partitioner.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bit_vec::BitVec;
use euclid::Point2D;
use geometry::{self, SubdividedQuadraticBezier};
use log::LogLevel;
use pathfinder_path_utils::PathBuffer;
use pathfinder_path_utils::curve::Curve;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::f32;
use std::iter;
use std::u32;
use {BQuad, BVertexLoopBlinnData, BVertexKind, CurveIndices, Endpoint, FillRule};
use {LineIndices, Subpath};

pub struct Partitioner<'a> {
    endpoints: &'a [Endpoint],
    control_points: &'a [Point2D<f32>],
    subpaths: &'a [Subpath],

    fill_rule: FillRule,

    b_quads: Vec<BQuad>,
    b_vertex_positions: Vec<Point2D<f32>>,
    b_vertex_path_ids: Vec<u16>,
    b_vertex_loop_blinn_data: Vec<BVertexLoopBlinnData>,
    cover_indices: CoverIndicesBuffer,
    edge_indices: EdgeIndicesBuffer,

    heap: BinaryHeap<Point>,
    visited_points: BitVec,
    active_edges: Vec<ActiveEdge>,
    path_id: u16,
}

impl<'a> Partitioner<'a> {
    #[inline]
    pub fn new<'b>() -> Partitioner<'b> {
        Partitioner {
            endpoints: &[],
            control_points: &[],
            subpaths: &[],

            fill_rule: FillRule::Winding,

            b_quads: vec![],
            b_vertex_positions: vec![],
            b_vertex_path_ids: vec![],
            b_vertex_loop_blinn_data: vec![],
            cover_indices: CoverIndicesBuffer::new(),
            edge_indices: EdgeIndicesBuffer::new(),

            heap: BinaryHeap::new(),
            visited_points: BitVec::new(),
            active_edges: vec![],
            path_id: 0,
        }
    }

    pub fn init_with_raw_data(&mut self,
                              new_endpoints: &'a [Endpoint],
                              new_control_points: &'a [Point2D<f32>],
                              new_subpaths: &'a [Subpath]) {
        self.endpoints = new_endpoints;
        self.control_points = new_control_points;
        self.subpaths = new_subpaths;

        // FIXME(pcwalton): Move this initialization to `partition` below. Right now, this bit
        // vector uses too much memory.
        self.visited_points = BitVec::from_elem(self.endpoints.len(), false);
    }

    pub fn init_with_path_buffer(&mut self, path_buffer: &'a PathBuffer) {
        self.init_with_raw_data(&path_buffer.endpoints,
                                &path_buffer.control_points,
                                &path_buffer.subpaths)
    }

    #[inline]
    pub fn set_fill_rule(&mut self, new_fill_rule: FillRule) {
        self.fill_rule = new_fill_rule
    }

    pub fn partition(&mut self, path_id: u16, first_subpath_index: u32, last_subpath_index: u32) {
        self.b_quads.clear();
        self.b_vertex_loop_blinn_data.clear();
        self.b_vertex_path_ids.clear();
        self.b_vertex_positions.clear();
        self.cover_indices.clear();
        self.edge_indices.clear();
        self.heap.clear();
        self.active_edges.clear();

        self.path_id = path_id;

        self.init_heap(first_subpath_index, last_subpath_index);

        while self.process_next_point() {}

        debug_assert!(self.b_vertex_loop_blinn_data.len() == self.b_vertex_path_ids.len());
        debug_assert!(self.b_vertex_loop_blinn_data.len() == self.b_vertex_positions.len());
    }

    #[inline]
    pub fn b_quads(&self) -> &[BQuad] {
        &self.b_quads
    }

    #[inline]
    pub fn b_vertex_positions(&self) -> &[Point2D<f32>] {
        &self.b_vertex_positions
    }

    #[inline]
    pub fn b_vertex_path_ids(&self) -> &[u16] {
        &self.b_vertex_path_ids
    }

    #[inline]
    pub fn b_vertex_loop_blinn_data(&self) -> &[BVertexLoopBlinnData] {
        &self.b_vertex_loop_blinn_data
    }

    #[inline]
    pub fn cover_indices(&self) -> CoverIndices {
        self.cover_indices.as_ref()
    }

    #[inline]
    pub fn edge_indices(&self) -> EdgeIndices {
        self.edge_indices.as_ref()
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

        debug!("processing point {}: {:?}",
               point.endpoint_index,
               self.endpoints[point.endpoint_index as usize].position);

        if log_enabled!(LogLevel::Debug) {
            debug!("... active edges:");
            for (active_edge_index, active_edge) in self.active_edges.iter().enumerate() {
                debug!("... ... edge {}: {:?}", active_edge_index, active_edge);
            }
        }

        self.mark_point_as_visited(&point);

        self.sort_active_edge_list(point.endpoint_index);

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

        let endpoint = &self.endpoints[endpoint_index as usize];
        if self.should_fill_above_active_edge(next_active_edge_index) {
            self.emit_b_quad_above(next_active_edge_index, endpoint.position.x)
        }

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

        let endpoint = &self.endpoints[endpoint_index as usize];
        let bottom = self.should_fill_above_active_edge(active_edge_index);
        if !bottom {
            self.emit_b_quad_below(active_edge_index, endpoint.position.x)
        } else {
            self.emit_b_quad_above(active_edge_index, endpoint.position.x)
        }

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);

        {
            let active_edge = &mut self.active_edges[active_edge_index as usize];
            active_edge.left_vertex_index = self.b_vertex_loop_blinn_data.len() as u32;
            active_edge.control_point_vertex_index = active_edge.left_vertex_index + 1;

            let endpoint_position = self.endpoints[active_edge.right_endpoint_index as usize]
                                        .position;
            self.b_vertex_positions.push(endpoint_position);
            self.b_vertex_path_ids.push(self.path_id);
            self.b_vertex_loop_blinn_data.push(BVertexLoopBlinnData::new(
                active_edge.endpoint_kind()));

            active_edge.toggle_parity();

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

        let control_point_index = if self.active_edges[active_edge_index as usize].left_to_right {
            self.control_point_index_before_endpoint(next_endpoint_index)
        } else {
            self.control_point_index_after_endpoint(prev_endpoint_index)
        };

        match control_point_index {
            u32::MAX => {
                self.active_edges[active_edge_index as usize].control_point_vertex_index = u32::MAX
            }
            control_point_index => {
                self.active_edges[active_edge_index as usize].control_point_vertex_index =
                    self.b_vertex_loop_blinn_data.len() as u32;

                let left_vertex_index = self.active_edges[active_edge_index as usize]
                                            .left_vertex_index;
                let control_point_position = &self.control_points[control_point_index as usize];
                let control_point_b_vertex_loop_blinn_data = BVertexLoopBlinnData::control_point(
                    &self.b_vertex_positions[left_vertex_index as usize],
                    &control_point_position,
                    &new_point.position,
                    bottom);
                self.b_vertex_positions.push(*control_point_position);
                self.b_vertex_path_ids.push(self.path_id);
                self.b_vertex_loop_blinn_data.push(control_point_b_vertex_loop_blinn_data);
            }
        }
    }

    fn process_max_endpoint(&mut self, endpoint_index: u32, active_edge_indices: [u32; 2]) {
        debug!("... MAX point: active edges {:?}", active_edge_indices);

        debug_assert!(active_edge_indices[0] < active_edge_indices[1],
                      "Matching active edge indices in wrong order when processing MAX point");

        let endpoint = &self.endpoints[endpoint_index as usize];

        if self.should_fill_above_active_edge(active_edge_indices[0]) {
            self.emit_b_quad_above(active_edge_indices[0], endpoint.position.x)
        }
        if self.should_fill_above_active_edge(active_edge_indices[1]) {
            self.emit_b_quad_above(active_edge_indices[1], endpoint.position.x)
        }
        if self.should_fill_below_active_edge(active_edge_indices[1]) {
            self.emit_b_quad_below(active_edge_indices[1], endpoint.position.x)
        }

        self.heap.pop();

        // FIXME(pcwalton): This is twice as slow as it needs to be.
        self.active_edges.remove(active_edge_indices[1] as usize);
        self.active_edges.remove(active_edge_indices[0] as usize);
    }

    fn sort_active_edge_list(&mut self, endpoint_index: u32) {
        for index in 1..self.active_edges.len() {
            for sorted_index in (1..(index + 1)).rev() {
                if self.active_edges_are_ordered((sorted_index - 1) as u32,
                                                 sorted_index as u32,
                                                 endpoint_index) {
                    break
                }
                self.active_edges.swap(sorted_index - 1, sorted_index)
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

        let left_vertex_index = self.b_vertex_loop_blinn_data.len() as u32;
        new_active_edges[0].left_vertex_index = left_vertex_index;
        new_active_edges[1].left_vertex_index = left_vertex_index;

        let position = self.endpoints[endpoint_index as usize].position;
        self.b_vertex_positions.push(position);
        self.b_vertex_path_ids.push(self.path_id);
        self.b_vertex_loop_blinn_data.push(BVertexLoopBlinnData::new(BVertexKind::Endpoint0));

        new_active_edges[0].toggle_parity();
        new_active_edges[1].toggle_parity();

        let endpoint = &self.endpoints[endpoint_index as usize];
        let prev_endpoint = &self.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.endpoints[next_endpoint_index as usize];

        let prev_vector = prev_endpoint.position - endpoint.position;
        let next_vector = next_endpoint.position - endpoint.position;

        let (upper_control_point_index, lower_control_point_index);
        if prev_vector.cross(next_vector) >= 0.0 {
            new_active_edges[0].right_endpoint_index = prev_endpoint_index;
            new_active_edges[1].right_endpoint_index = next_endpoint_index;
            new_active_edges[0].left_to_right = false;
            new_active_edges[1].left_to_right = true;

            upper_control_point_index = self.endpoints[endpoint_index as usize].control_point_index;
            lower_control_point_index = self.endpoints[next_endpoint_index as usize]
                                            .control_point_index;
        } else {
            new_active_edges[0].right_endpoint_index = next_endpoint_index;
            new_active_edges[1].right_endpoint_index = prev_endpoint_index;
            new_active_edges[0].left_to_right = true;
            new_active_edges[1].left_to_right = false;

            upper_control_point_index = self.endpoints[next_endpoint_index as usize]
                                            .control_point_index;
            lower_control_point_index = self.endpoints[endpoint_index as usize].control_point_index;
        }

        match upper_control_point_index {
            u32::MAX => new_active_edges[0].control_point_vertex_index = u32::MAX,
            upper_control_point_index => {
                new_active_edges[0].control_point_vertex_index =
                    self.b_vertex_loop_blinn_data.len() as u32;

                let control_point_position =
                    self.control_points[upper_control_point_index as usize];
                let right_vertex_position =
                    self.endpoints[new_active_edges[0].right_endpoint_index as usize].position;
                let control_point_b_vertex_loop_blinn_data =
                    BVertexLoopBlinnData::control_point(&position,
                                                        &control_point_position,
                                                        &right_vertex_position,
                                                        false);
                self.b_vertex_positions.push(control_point_position);
                self.b_vertex_path_ids.push(self.path_id);
                self.b_vertex_loop_blinn_data.push(control_point_b_vertex_loop_blinn_data);
            }
        }

        match lower_control_point_index {
            u32::MAX => new_active_edges[1].control_point_vertex_index = u32::MAX,
            lower_control_point_index => {
                new_active_edges[1].control_point_vertex_index =
                    self.b_vertex_loop_blinn_data.len() as u32;

                let control_point_position =
                    self.control_points[lower_control_point_index as usize];
                let right_vertex_position =
                    self.endpoints[new_active_edges[1].right_endpoint_index as usize].position;
                let control_point_b_vertex_loop_blinn_data =
                    BVertexLoopBlinnData::control_point(&position,
                                                        &control_point_position,
                                                        &right_vertex_position,
                                                        true);
                self.b_vertex_positions.push(control_point_position);
                self.b_vertex_path_ids.push(self.path_id);
                self.b_vertex_loop_blinn_data.push(control_point_b_vertex_loop_blinn_data);
            }
        }
    }

    fn active_edges_are_ordered(&mut self,
                                prev_active_edge_index: u32,
                                next_active_edge_index: u32,
                                reference_endpoint_index: u32)
                                -> bool {
        let prev_active_edge = &self.active_edges[prev_active_edge_index as usize];
        let next_active_edge = &self.active_edges[next_active_edge_index as usize];
        if prev_active_edge.right_endpoint_index == next_active_edge.right_endpoint_index {
            // Always ordered.
            // FIXME(pcwalton): Is this true?
            return true
        }

        let prev_active_edge_right_endpoint =
            &self.endpoints[prev_active_edge.right_endpoint_index as usize];
        let next_active_edge_right_endpoint =
            &self.endpoints[next_active_edge.right_endpoint_index as usize];
        if prev_active_edge_right_endpoint.position.y <=
                next_active_edge_right_endpoint.position.y {
            // Guaranteed to be ordered.
            // FIXME(pcwalton): Is this true?
            return true
        }

        // Slow path.
        let reference_endpoint = &self.endpoints[reference_endpoint_index as usize];
        let prev_active_edge_y = self.solve_active_edge_y_for_x(reference_endpoint.position.x,
                                                                prev_active_edge);
        let next_active_edge_y = self.solve_active_edge_y_for_x(reference_endpoint.position.x,
                                                                next_active_edge);
        prev_active_edge_y <= next_active_edge_y
    }

    fn init_heap(&mut self, first_subpath_index: u32, last_subpath_index: u32) {
        for subpath in &self.subpaths[(first_subpath_index as usize)..
                                      (last_subpath_index as usize)] {
            for endpoint_index in subpath.first_endpoint_index..subpath.last_endpoint_index {
                match self.classify_endpoint(endpoint_index) {
                    EndpointClass::Min => {
                        let new_point = self.create_point_from_endpoint(endpoint_index);
                        self.heap.push(new_point)
                    }
                    EndpointClass::Regular | EndpointClass::Max => {}
                }
            }
        }
    }

    fn should_fill_below_active_edge(&self, active_edge_index: u32) -> bool {
        if (active_edge_index as usize) + 1 == self.active_edges.len() {
            return false
        }

        match self.fill_rule {
            FillRule::EvenOdd => active_edge_index % 2 == 0,
            FillRule::Winding => self.winding_number_below_active_edge(active_edge_index) != 0,
        }
    }

    fn should_fill_above_active_edge(&self, active_edge_index: u32) -> bool {
        active_edge_index > 0 && self.should_fill_below_active_edge(active_edge_index - 1)
    }

    fn winding_number_above_active_edge(&self, active_edge_index: u32) -> i32 {
        if active_edge_index == 0 {
            0
        } else {
            self.winding_number_below_active_edge(active_edge_index - 1)
        }
    }

    fn winding_number_below_active_edge(&self, active_edge_index: u32) -> i32 {
        let mut winding_number = 0;
        for active_edge_index in 0..(active_edge_index as usize + 1) {
            if self.active_edges[active_edge_index].left_to_right {
                winding_number += 1
            } else {
                winding_number -= 1
            }
        }
        winding_number
    }

    fn emit_b_quad_below(&mut self, upper_active_edge_index: u32, right_x: f32) {
        let mut lower_active_edge_index = upper_active_edge_index + 1;

        if self.fill_rule == FillRule::Winding {
            let active_edge_count = self.active_edges.len() as u32;
            let mut winding_number =
                self.winding_number_below_active_edge(lower_active_edge_index);
            while lower_active_edge_index + 1 < active_edge_count && winding_number != 0 {
                lower_active_edge_index += 1;
                if self.active_edges[lower_active_edge_index as usize].left_to_right {
                    winding_number += 1
                } else {
                    winding_number -= 1
                }
            }
        }

        self.emit_b_quad_above(lower_active_edge_index, right_x)
    }

    fn emit_b_quad_above(&mut self, lower_active_edge_index: u32, right_x: f32) {
        // TODO(pcwalton): Assert that the green X position is the same on both edges.
        debug_assert!(lower_active_edge_index > 0,
                      "Can't emit b_quads above the top active edge");

        let mut upper_active_edge_index = lower_active_edge_index - 1;

        if self.fill_rule == FillRule::Winding {
            let mut winding_number =
                self.winding_number_above_active_edge(upper_active_edge_index);
            while upper_active_edge_index > 0 && winding_number != 0 {
                upper_active_edge_index -= 1;
                if self.active_edges[upper_active_edge_index as usize].left_to_right {
                    winding_number -= 1
                } else {
                    winding_number += 1
                }
            }
        }

        let upper_curve = self.subdivide_active_edge_at(upper_active_edge_index, right_x);
        let lower_curve = self.subdivide_active_edge_at(lower_active_edge_index, right_x);

        let upper_shape = upper_curve.shape(&self.b_vertex_loop_blinn_data);
        let lower_shape = lower_curve.shape(&self.b_vertex_loop_blinn_data);

        match upper_shape {
            Shape::Flat => {
                self.edge_indices
                    .upper_line_indices
                    .push(LineIndices::new(upper_curve.left_curve_left, upper_curve.middle_point))
            }
            Shape::Convex | Shape::Concave => {
                self.edge_indices
                    .upper_curve_indices
                    .push(CurveIndices::new(upper_curve.left_curve_left,
                                            upper_curve.left_curve_control_point,
                                            upper_curve.middle_point))
            }
        }
        match lower_shape {
            Shape::Flat => {
                self.edge_indices
                    .lower_line_indices
                    .push(LineIndices::new(lower_curve.left_curve_left, lower_curve.middle_point))
            }
            Shape::Convex | Shape::Concave => {
                self.edge_indices
                    .lower_curve_indices
                    .push(CurveIndices::new(lower_curve.left_curve_left,
                                            lower_curve.left_curve_control_point,
                                            lower_curve.middle_point))
            }
        }

        debug!("... emitting B-quad: UL {} BL {} UR {} BR {}",
               upper_curve.left_curve_left,
               lower_curve.left_curve_left,
               upper_curve.middle_point,
               lower_curve.middle_point);

        match (upper_shape, lower_shape) {
            (Shape::Flat, Shape::Flat) |
            (Shape::Flat, Shape::Convex) |
            (Shape::Convex, Shape::Flat) |
            (Shape::Convex, Shape::Convex) => {
                self.cover_indices.interior_indices.extend([
                    upper_curve.left_curve_left,
                    upper_curve.middle_point,
                    lower_curve.left_curve_left,
                    lower_curve.middle_point,
                    lower_curve.left_curve_left,
                    upper_curve.middle_point,
                ].into_iter());
                if upper_shape != Shape::Flat {
                    self.cover_indices.curve_indices.extend([
                        upper_curve.left_curve_control_point,
                        upper_curve.middle_point,
                        upper_curve.left_curve_left,
                    ].into_iter())
                }
                if lower_shape != Shape::Flat {
                    self.cover_indices.curve_indices.extend([
                        lower_curve.left_curve_control_point,
                        lower_curve.left_curve_left,
                        lower_curve.middle_point,
                    ].into_iter())
                }
            }

            (Shape::Concave, Shape::Flat) |
            (Shape::Concave, Shape::Convex) => {
                self.cover_indices.interior_indices.extend([
                    upper_curve.left_curve_left,
                    upper_curve.left_curve_control_point,
                    lower_curve.left_curve_left,
                    upper_curve.middle_point,
                    lower_curve.middle_point,
                    upper_curve.left_curve_control_point,
                    lower_curve.middle_point,
                    lower_curve.left_curve_left,
                    upper_curve.left_curve_control_point,
                ].into_iter());
                self.cover_indices.curve_indices.extend([
                    upper_curve.left_curve_control_point,
                    upper_curve.left_curve_left,
                    upper_curve.middle_point,
                ].into_iter());
                if lower_shape != Shape::Flat {
                    self.cover_indices.curve_indices.extend([
                        lower_curve.left_curve_control_point,
                        lower_curve.left_curve_left,
                        lower_curve.middle_point,
                    ].into_iter())
                }
            }

            (Shape::Flat, Shape::Concave) |
            (Shape::Convex, Shape::Concave) => {
                self.cover_indices.interior_indices.extend([
                    upper_curve.left_curve_left,
                    upper_curve.middle_point,
                    lower_curve.left_curve_control_point,
                    upper_curve.middle_point,
                    lower_curve.middle_point,
                    lower_curve.left_curve_control_point,
                    upper_curve.left_curve_left,
                    lower_curve.left_curve_control_point,
                    lower_curve.left_curve_left,
                ].into_iter());
                self.cover_indices.curve_indices.extend([
                    lower_curve.left_curve_control_point,
                    lower_curve.middle_point,
                    lower_curve.left_curve_left,
                ].into_iter());
                if upper_shape != Shape::Flat {
                    self.cover_indices.curve_indices.extend([
                        upper_curve.left_curve_control_point,
                        upper_curve.middle_point,
                        upper_curve.left_curve_left,
                    ].into_iter())
                }
            }

            (Shape::Concave, Shape::Concave) => {
                self.cover_indices.interior_indices.extend([
                    upper_curve.left_curve_left,
                    upper_curve.left_curve_control_point,
                    lower_curve.left_curve_left,
                    lower_curve.left_curve_left,
                    upper_curve.left_curve_control_point,
                    lower_curve.left_curve_control_point,
                    upper_curve.middle_point,
                    lower_curve.left_curve_control_point,
                    upper_curve.left_curve_control_point,
                    upper_curve.middle_point,
                    lower_curve.middle_point,
                    lower_curve.left_curve_control_point,
                ].into_iter());
                self.cover_indices.curve_indices.extend([
                    upper_curve.left_curve_control_point,
                    upper_curve.left_curve_left,
                    upper_curve.middle_point,
                    lower_curve.left_curve_control_point,
                    lower_curve.middle_point,
                    lower_curve.left_curve_left,
                ].into_iter());
            }
        }

        self.b_quads.push(BQuad::new(upper_curve.left_curve_left,
                                     upper_curve.left_curve_control_point,
                                     upper_curve.middle_point,
                                     lower_curve.left_curve_left,
                                     lower_curve.left_curve_control_point,
                                     lower_curve.middle_point))
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
        let endpoint = &self.endpoints[endpoint_index as usize];
        match self.active_edges.iter().position(|active_edge| {
            self.solve_active_edge_y_for_x(endpoint.position.x, active_edge) > endpoint.position.y
        }) {
            Some(active_edge_index) => active_edge_index as u32,
            None => self.active_edges.len() as u32,
        }
    }

    fn solve_active_edge_t_for_x(&self, x: f32, active_edge: &ActiveEdge) -> f32 {
        let left_vertex_position = &self.b_vertex_positions[active_edge.left_vertex_index as
                                                            usize];
        let right_endpoint_position = &self.endpoints[active_edge.right_endpoint_index as usize]
                                           .position;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                geometry::solve_line_t_for_x(x, left_vertex_position, right_endpoint_position)
            }
            control_point_vertex_index => {
                let control_point = &self.b_vertex_positions[control_point_vertex_index as usize];
                geometry::solve_quadratic_bezier_t_for_x(x,
                                                         left_vertex_position,
                                                         control_point,
                                                         right_endpoint_position)
            }
        }
    }

    fn solve_active_edge_y_for_x(&self, x: f32, active_edge: &ActiveEdge) -> f32 {
        self.sample_active_edge(self.solve_active_edge_t_for_x(x, active_edge), active_edge).y
    }

    fn sample_active_edge(&self, t: f32, active_edge: &ActiveEdge) -> Point2D<f32> {
        let left_vertex_position = &self.b_vertex_positions[active_edge.left_vertex_index as
                                                            usize];
        let right_endpoint_position = &self.endpoints[active_edge.right_endpoint_index as usize]
                                           .position;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                left_vertex_position.to_vector()
                                    .lerp(right_endpoint_position.to_vector(), t)
                                    .to_point()
            }
            control_point_vertex_index => {
                let control_point = &self.b_vertex_positions[control_point_vertex_index as usize];
                Curve::new(left_vertex_position, control_point, right_endpoint_position).sample(t)
            }
        }
    }

    fn crossing_point_for_active_edge(&self, upper_active_edge_index: u32)
                                      -> Option<Point2D<f32>> {
        let lower_active_edge_index = upper_active_edge_index + 1;

        let upper_active_edge = &self.active_edges[upper_active_edge_index as usize];
        let lower_active_edge = &self.active_edges[lower_active_edge_index as usize];
        if upper_active_edge.left_vertex_index == lower_active_edge.left_vertex_index ||
                upper_active_edge.right_endpoint_index == lower_active_edge.right_endpoint_index {
            return None
        }

        let upper_left_vertex_position =
            &self.b_vertex_positions[upper_active_edge.left_vertex_index as usize];
        let upper_right_endpoint_position =
            &self.endpoints[upper_active_edge.right_endpoint_index as usize].position;
        let lower_left_vertex_position =
            &self.b_vertex_positions[lower_active_edge.left_vertex_index as usize];
        let lower_right_endpoint_position =
            &self.endpoints[lower_active_edge.right_endpoint_index as usize].position;

        match (upper_active_edge.control_point_vertex_index,
               lower_active_edge.control_point_vertex_index) {
            (u32::MAX, u32::MAX) => {
                geometry::line_line_crossing_point(upper_left_vertex_position,
                                                   upper_right_endpoint_position,
                                                   lower_left_vertex_position,
                                                   lower_right_endpoint_position)
            }
            (upper_control_point_vertex_index, u32::MAX) => {
                let upper_control_point =
                    &self.b_vertex_positions[upper_control_point_vertex_index as usize];
                geometry::line_quadratic_bezier_crossing_point(lower_left_vertex_position,
                                                               lower_right_endpoint_position,
                                                               upper_left_vertex_position,
                                                               upper_control_point,
                                                               upper_right_endpoint_position)
            }
            (u32::MAX, lower_control_point_vertex_index) => {
                let lower_control_point =
                    &self.b_vertex_positions[lower_control_point_vertex_index as usize];
                geometry::line_quadratic_bezier_crossing_point(upper_left_vertex_position,
                                                               upper_right_endpoint_position,
                                                               lower_left_vertex_position,
                                                               lower_control_point,
                                                               lower_right_endpoint_position)
            }
            (upper_control_point_vertex_index, lower_control_point_vertex_index) => {
                let upper_control_point =
                    &self.b_vertex_positions[upper_control_point_vertex_index as usize];
                let lower_control_point =
                    &self.b_vertex_positions[lower_control_point_vertex_index as usize];
                geometry::quadratic_bezier_quadratic_bezier_crossing_point(
                    upper_left_vertex_position,
                    upper_control_point,
                    upper_right_endpoint_position,
                    lower_left_vertex_position,
                    lower_control_point,
                    lower_right_endpoint_position)
            }
        }
    }

    fn subdivide_active_edge_at(&mut self, active_edge_index: u32, x: f32)
                                -> SubdividedActiveEdge {
        let t = self.solve_active_edge_t_for_x(x, &self.active_edges[active_edge_index as usize]);

        let bottom = self.should_fill_above_active_edge(active_edge_index);

        let active_edge = &mut self.active_edges[active_edge_index as usize];
        let left_curve_left = active_edge.left_vertex_index;

        let left_curve_control_point_vertex_index;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                let path_id = self.b_vertex_path_ids[left_curve_left as usize];
                let left_point_position = self.b_vertex_positions[left_curve_left as usize];
                let right_point = self.endpoints[active_edge.right_endpoint_index as usize]
                                      .position;
                let middle_point = left_point_position.to_vector().lerp(right_point.to_vector(), t);

                active_edge.left_vertex_index = self.b_vertex_loop_blinn_data.len() as u32;
                self.b_vertex_positions.push(middle_point.to_point());
                self.b_vertex_path_ids.push(path_id);
                self.b_vertex_loop_blinn_data
                    .push(BVertexLoopBlinnData::new(active_edge.endpoint_kind()));

                active_edge.toggle_parity();

                left_curve_control_point_vertex_index = u32::MAX;
            }
            _ => {
                let left_endpoint_position =
                    self.b_vertex_positions[active_edge.left_vertex_index as usize];
                let right_endpoint_position =
                    self.endpoints[active_edge.right_endpoint_index as usize].position;
                let subdivided_quadratic_bezier = SubdividedQuadraticBezier::new(
                    t,
                    &left_endpoint_position,
                    &self.b_vertex_positions[active_edge.control_point_vertex_index as usize],
                    &right_endpoint_position);

                left_curve_control_point_vertex_index = self.b_vertex_loop_blinn_data.len() as u32;
                active_edge.left_vertex_index = left_curve_control_point_vertex_index + 1;
                active_edge.control_point_vertex_index = left_curve_control_point_vertex_index + 2;

                self.b_vertex_positions.extend([
                    subdivided_quadratic_bezier.ap1,
                    subdivided_quadratic_bezier.ap2bp0,
                    subdivided_quadratic_bezier.bp1,
                ].into_iter());
                self.b_vertex_path_ids.extend(iter::repeat(self.path_id).take(3));
                self.b_vertex_loop_blinn_data.extend([
                    BVertexLoopBlinnData::control_point(&left_endpoint_position,
                                                        &subdivided_quadratic_bezier.ap1,
                                                        &subdivided_quadratic_bezier.ap2bp0,
                                                        bottom),
                    BVertexLoopBlinnData::new(active_edge.endpoint_kind()),
                    BVertexLoopBlinnData::control_point(&subdivided_quadratic_bezier.ap2bp0,
                                                        &subdivided_quadratic_bezier.bp1,
                                                        &right_endpoint_position,
                                                        bottom),
                ].into_iter());

                active_edge.toggle_parity();
            }
        }

        SubdividedActiveEdge {
            left_curve_left: left_curve_left,
            left_curve_control_point: left_curve_control_point_vertex_index,
            middle_point: active_edge.left_vertex_index,
            right_curve_control_point: active_edge.control_point_vertex_index,
        }
    }

    fn prev_endpoint_of(&self, endpoint_index: u32) -> u32 {
        let endpoint = &self.endpoints[endpoint_index as usize];
        let subpath = &self.subpaths[endpoint.subpath_index as usize];
        if endpoint_index > subpath.first_endpoint_index {
            endpoint_index - 1
        } else {
            subpath.last_endpoint_index - 1
        }
    }

    fn next_endpoint_of(&self, endpoint_index: u32) -> u32 {
        let endpoint = &self.endpoints[endpoint_index as usize];
        let subpath = &self.subpaths[endpoint.subpath_index as usize];
        if endpoint_index + 1 < subpath.last_endpoint_index {
            endpoint_index + 1
        } else {
            subpath.first_endpoint_index
        }
    }

    fn create_point_from_endpoint(&self, endpoint_index: u32) -> Point {
        Point {
            position: self.endpoints[endpoint_index as usize].position,
            endpoint_index: endpoint_index,
        }
    }

    fn control_point_index_before_endpoint(&self, endpoint_index: u32) -> u32 {
        self.endpoints[endpoint_index as usize].control_point_index
    }

    fn control_point_index_after_endpoint(&self, endpoint_index: u32) -> u32 {
        self.control_point_index_before_endpoint(self.next_endpoint_of(endpoint_index))
    }
}

#[derive(Debug, Clone)]
struct CoverIndicesBuffer {
    interior_indices: Vec<u32>,
    curve_indices: Vec<u32>,
}

impl CoverIndicesBuffer {
    fn new() -> CoverIndicesBuffer {
        CoverIndicesBuffer {
            interior_indices: vec![],
            curve_indices: vec![],
        }
    }

    fn clear(&mut self) {
        self.interior_indices.clear();
        self.curve_indices.clear();
    }

    fn as_ref(&self) -> CoverIndices {
        CoverIndices {
            interior_indices: &self.interior_indices,
            curve_indices: &self.curve_indices,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoverIndices<'a> {
    pub interior_indices: &'a [u32],
    pub curve_indices: &'a [u32],
}

#[derive(Debug, Clone)]
struct EdgeIndicesBuffer {
    upper_line_indices: Vec<LineIndices>,
    upper_curve_indices: Vec<CurveIndices>,
    lower_line_indices: Vec<LineIndices>,
    lower_curve_indices: Vec<CurveIndices>,
}

impl EdgeIndicesBuffer {
    fn new() -> EdgeIndicesBuffer {
        EdgeIndicesBuffer {
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

    fn as_ref(&self) -> EdgeIndices {
        EdgeIndices {
            upper_line_indices: &self.upper_line_indices,
            upper_curve_indices: &self.upper_curve_indices,
            lower_line_indices: &self.lower_line_indices,
            lower_curve_indices: &self.lower_curve_indices,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeIndices<'a> {
    pub upper_line_indices: &'a [LineIndices],
    pub upper_curve_indices: &'a [CurveIndices],
    pub lower_line_indices: &'a [LineIndices],
    pub lower_curve_indices: &'a [CurveIndices],
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
}

#[derive(Debug, Clone, Copy)]
struct SubdividedActiveEdge {
    left_curve_left: u32,
    left_curve_control_point: u32,
    middle_point: u32,
    right_curve_control_point: u32,
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
