// partitionfinder/partitioner.rs

use bit_vec::BitVec;
use euclid::Point2D;
use geometry::{self, SubdividedQuadraticBezier};
use log::LogLevel;
use std::collections::BinaryHeap;
use std::cmp::{self, Ordering};
use std::f32;
use std::u32;
use {BQuad, BVertex, Endpoint, Shape, Subpath};

pub struct Partitioner<'a> {
    endpoints: &'a [Endpoint],
    control_points: &'a [Point2D<f32>],
    subpaths: &'a [Subpath],

    b_quads: Vec<BQuad>,
    b_vertices: Vec<BVertex>,
    b_indices: Vec<u32>,

    heap: BinaryHeap<Point>,
    visited_points: BitVec,
    active_edges: Vec<ActiveEdge>,
    path_id: u32,
}

impl<'a> Partitioner<'a> {
    #[inline]
    pub fn new<'b>() -> Partitioner<'b> {
        Partitioner {
            endpoints: &[],
            control_points: &[],
            subpaths: &[],

            b_quads: vec![],
            b_vertices: vec![],
            b_indices: vec![],

            heap: BinaryHeap::new(),
            visited_points: BitVec::new(),
            active_edges: vec![],
            path_id: 0,
        }
    }

    pub fn init(&mut self,
                new_endpoints: &'a [Endpoint],
                new_control_points: &'a [Point2D<f32>],
                new_subpaths: &'a [Subpath]) {
        self.endpoints = new_endpoints;
        self.control_points = new_control_points;
        self.subpaths = new_subpaths;

        // FIXME(pcwalton): Move this initialization to `partition` below. Right now, this bit
        // vector uses too much memory.
        self.visited_points = BitVec::from_elem(self.endpoints.len() * 2, false);
    }

    pub fn partition(&mut self, path_id: u32, first_subpath_index: u32, last_subpath_index: u32) {
        self.b_quads.clear();
        self.b_vertices.clear();
        self.heap.clear();
        self.active_edges.clear();

        self.path_id = path_id;

        self.init_heap(first_subpath_index, last_subpath_index);

        while self.process_next_point() {}
    }

    #[inline]
    pub fn b_quads(&self) -> &[BQuad] {
        &self.b_quads
    }

    #[inline]
    pub fn b_vertices(&self) -> &[BVertex] {
        &self.b_vertices
    }

    #[inline]
    pub fn b_indices(&self) -> &[u32] {
        &self.b_indices
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

        let matching_active_edges = self.find_right_point_in_active_edge_list(point.endpoint_index);
        match point.point_type {
            PointType::Endpoint => {
                match matching_active_edges.count {
                    0 => self.process_min_endpoint(point.endpoint_index),
                    1 => {
                        self.process_regular_endpoint(point.endpoint_index,
                                                      matching_active_edges.indices[0])
                    }
                    2 => {
                        self.process_max_endpoint(point.endpoint_index,
                                                  matching_active_edges.indices)
                    }
                    _ => debug_assert!(false),
                }
            }
            PointType::CrossingBelow => {
                // FIXME(pcwalton): This condition should always pass, but it fails on the Dutch
                // rail map.
                if matching_active_edges.count > 0 {
                    self.process_crossing_point(point.position.x, matching_active_edges.indices[0])
                }
            }
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

        self.add_crossings_to_heap_if_necessary(next_active_edge_index + 0,
                                                next_active_edge_index + 2)
    }

    fn process_regular_endpoint(&mut self, endpoint_index: u32, active_edge_index: u32) {
        debug!("... REGULAR point: active edge {}", active_edge_index);

        let endpoint = &self.endpoints[endpoint_index as usize];
        if self.should_fill_below_active_edge(active_edge_index) {
            self.emit_b_quad_below(active_edge_index, endpoint.position.x)
        }
        if self.should_fill_above_active_edge(active_edge_index) {
            self.emit_b_quad_above(active_edge_index, endpoint.position.x)
        }

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);

        {
            let active_edge = &mut self.active_edges[active_edge_index as usize];
            active_edge.left_vertex_index = self.b_vertices.len() as u32;
            active_edge.control_point_vertex_index = active_edge.left_vertex_index + 1;

            let endpoint_position = self.endpoints[active_edge.right_endpoint_index as usize]
                                        .position;
            self.b_vertices.push(BVertex::new(&endpoint_position, self.path_id));

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
                    self.b_vertices.len() as u32;
                let b_vertex = BVertex::new(&self.control_points[control_point_index as usize],
                                            self.path_id);
                self.b_vertices.push(b_vertex)
            }
        }

        self.add_crossings_to_heap_if_necessary(active_edge_index + 0, active_edge_index + 2)
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

        self.add_crossings_to_heap_if_necessary(active_edge_indices[0], active_edge_indices[0] + 2)
    }

    fn process_crossing_point(&mut self, x: f32, upper_active_edge_index: u32) {
        if self.should_fill_above_active_edge(upper_active_edge_index) {
            self.emit_b_quad_above(upper_active_edge_index, x)
        }
        if self.should_fill_below_active_edge(upper_active_edge_index) {
            self.emit_b_quad_below(upper_active_edge_index, x)
        }

        // Swap the two edges.
        //
        // FIXME(pcwalton): This condition should always pass, but it fails on the Dutch rail map.
        let lower_active_edge_index = upper_active_edge_index + 1;
        if (lower_active_edge_index as usize) < self.active_edges.len() {
            self.active_edges.swap(upper_active_edge_index as usize,
                                   lower_active_edge_index as usize)
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

        new_active_edges[0].left_vertex_index = self.b_vertices.len() as u32;
        new_active_edges[1].left_vertex_index = new_active_edges[0].left_vertex_index;

        let position = self.endpoints[endpoint_index as usize].position;
        self.b_vertices.push(BVertex::new(&position, self.path_id));

        let endpoint = &self.endpoints[endpoint_index as usize];
        let prev_endpoint = &self.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.endpoints[next_endpoint_index as usize];

        // TODO(pcwalton): There's a faster way to do this with no divisions, almost certainly.
        let prev_vector = (prev_endpoint.position - endpoint.position).normalize();
        let next_vector = (next_endpoint.position - endpoint.position).normalize();

        let (upper_control_point_index, lower_control_point_index);
        if prev_vector.y <= next_vector.y {
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
                new_active_edges[0].control_point_vertex_index = self.b_vertices.len() as u32;
                let b_vertex =
                    BVertex::new(&self.control_points[upper_control_point_index as usize],
                                 self.path_id);
                self.b_vertices.push(b_vertex)
            }
        }

        match lower_control_point_index {
            u32::MAX => new_active_edges[1].control_point_vertex_index = u32::MAX,
            lower_control_point_index => {
                new_active_edges[1].control_point_vertex_index = self.b_vertices.len() as u32;
                let b_vertex =
                    BVertex::new(&self.control_points[lower_control_point_index as usize],
                                 self.path_id);
                self.b_vertices.push(b_vertex)
            }
        }
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
        // TODO(pcwalton): Support the winding fill rule.
        active_edge_index % 2 == 0
    }

    fn should_fill_above_active_edge(&self, active_edge_index: u32) -> bool {
        // TODO(pcwalton): Support the winding fill rule.
        active_edge_index % 2 == 1
    }

    fn emit_b_quad_below(&mut self, upper_active_edge_index: u32, right_x: f32) {
        self.emit_b_quad_above(upper_active_edge_index + 1, right_x)
    }

    fn emit_b_quad_above(&mut self, lower_active_edge_index: u32, right_x: f32) {
        // TODO(pcwalton): Assert that the green X position is the same on both edges.
        debug_assert!(lower_active_edge_index > 0,
                      "Can't emit b_quads above the top active edge");
        let upper_active_edge_index = lower_active_edge_index - 1;

        let upper_curve = self.subdivide_active_edge_at(upper_active_edge_index, right_x);
        let lower_curve = self.subdivide_active_edge_at(lower_active_edge_index, right_x);

        // TODO(pcwalton): Concave.
        let upper_shape = if upper_curve.left_curve_control_point == u32::MAX {
            Shape::Flat
        } else {
            Shape::Convex
        };
        let lower_shape = if lower_curve.left_curve_control_point == u32::MAX {
            Shape::Flat
        } else {
            Shape::Convex
        };

        let start_index = self.b_indices.len() as u32;
        self.b_indices.extend([
            upper_curve.left_curve_left,
            lower_curve.left_curve_left,
            upper_curve.middle_point,
            upper_curve.middle_point,
            lower_curve.middle_point,
            lower_curve.left_curve_left,
        ].into_iter());

        if upper_shape != Shape::Flat {
            self.b_indices.extend([
                upper_curve.left_curve_control_point,
                upper_curve.middle_point,
                upper_curve.left_curve_left,
            ].into_iter())
        }
        if lower_shape != Shape::Flat {
            self.b_indices.extend([
                lower_curve.left_curve_control_point,
                lower_curve.middle_point,
                lower_curve.left_curve_left,
            ].into_iter())
        }

        self.b_quads.push(BQuad::new(start_index, upper_shape, lower_shape))
    }

    fn already_visited_point(&self, point: &Point) -> bool {
        // FIXME(pcwalton): This makes the visited vector too big.
        let index = point.endpoint_index as usize * 2 + point.point_type as usize;
        match self.visited_points.get(index) {
            None => false,
            Some(visited) => visited,
        }
    }

    fn mark_point_as_visited(&mut self, point: &Point) {
        // FIXME(pcwalton): This makes the visited vector too big.
        self.visited_points.set(point.endpoint_index as usize * 2 + point.point_type as usize,
                                true)
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
        let left_vertex_position = &self.b_vertices[active_edge.left_vertex_index as usize]
                                        .position;
        let right_endpoint_position = &self.endpoints[active_edge.right_endpoint_index as usize]
                                           .position;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                geometry::solve_line_t_for_x(x, left_vertex_position, right_endpoint_position)
            }
            control_point_vertex_index => {
                let control_point = &self.b_vertices[control_point_vertex_index as usize].position;
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
        let left_vertex_position = &self.b_vertices[active_edge.left_vertex_index as usize]
                                        .position;
        let right_endpoint_position = &self.endpoints[active_edge.right_endpoint_index as usize]
                                           .position;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                left_vertex_position.to_vector()
                                    .lerp(right_endpoint_position.to_vector(), t)
                                    .to_point()
            }
            control_point_vertex_index => {
                let control_point = &self.b_vertices[control_point_vertex_index as usize].position;
                geometry::sample_quadratic_bezier(t,
                                                  left_vertex_position,
                                                  control_point,
                                                  right_endpoint_position)
            }
        }
    }

    fn add_crossings_to_heap_if_necessary(&mut self,
                                          mut first_active_edge_index: u32,
                                          mut last_active_edge_index: u32) {
        if self.active_edges.is_empty() {
            return
        }

        first_active_edge_index = first_active_edge_index.checked_sub(1)
                                                         .unwrap_or(first_active_edge_index);
        last_active_edge_index = cmp::min(last_active_edge_index + 1,
                                          self.active_edges.len() as u32);

        for (upper_active_edge_index, upper_active_edge) in
                self.active_edges[(first_active_edge_index as usize)..
                                  (last_active_edge_index as usize - 1)]
                    .iter()
                    .enumerate() {
            let crossing_position =
                match self.crossing_point_for_active_edge(upper_active_edge_index as u32) {
                    None => continue,
                    Some(crossing_point) => crossing_point,
                };

            let new_point = Point {
                position: crossing_position,
                endpoint_index: upper_active_edge.right_endpoint_index,
                point_type: PointType::CrossingBelow,
            };

            self.heap.push(new_point);
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
            &self.b_vertices[upper_active_edge.left_vertex_index as usize].position;
        let upper_right_endpoint_position =
            &self.endpoints[upper_active_edge.right_endpoint_index as usize].position;
        let lower_left_vertex_position =
            &self.b_vertices[lower_active_edge.left_vertex_index as usize].position;
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
                    &self.b_vertices[upper_control_point_vertex_index as usize].position;
                geometry::line_quadratic_bezier_crossing_point(lower_left_vertex_position,
                                                               lower_right_endpoint_position,
                                                               upper_left_vertex_position,
                                                               upper_control_point,
                                                               upper_right_endpoint_position)
            }
            (u32::MAX, lower_control_point_vertex_index) => {
                let lower_control_point =
                    &self.b_vertices[lower_control_point_vertex_index as usize].position;
                geometry::line_quadratic_bezier_crossing_point(upper_left_vertex_position,
                                                               upper_right_endpoint_position,
                                                               lower_left_vertex_position,
                                                               lower_control_point,
                                                               lower_right_endpoint_position)
            }
            (upper_control_point_vertex_index, lower_control_point_vertex_index) => {
                let upper_control_point =
                    &self.b_vertices[upper_control_point_vertex_index as usize].position;
                let lower_control_point =
                    &self.b_vertices[lower_control_point_vertex_index as usize].position;
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

    fn subdivide_active_edge_at(&mut self, active_edge_index: u32, x: f32) -> SubdividedActiveEdge {
        let t = self.solve_active_edge_t_for_x(x, &self.active_edges[active_edge_index as usize]);

        let active_edge = &mut self.active_edges[active_edge_index as usize];
        let left_curve_left = active_edge.left_vertex_index;

        let left_curve_control_point_vertex_index;
        match active_edge.control_point_vertex_index {
            u32::MAX => {
                let left_point = self.b_vertices[left_curve_left as usize];
                let right_point = self.endpoints[active_edge.right_endpoint_index as usize]
                                      .position;
                let middle_point = left_point.position.to_vector().lerp(right_point.to_vector(), t);

                active_edge.left_vertex_index = self.b_vertices.len() as u32;
                self.b_vertices.push(BVertex::new(&middle_point.to_point(), left_point.path_id));

                left_curve_control_point_vertex_index = u32::MAX;
            }
            _ => {
                let subdivided_quadratic_bezier = SubdividedQuadraticBezier::new(
                    t,
                    &self.b_vertices[active_edge.left_vertex_index as usize].position,
                    &self.b_vertices[active_edge.control_point_vertex_index as usize].position,
                    &self.endpoints[active_edge.right_endpoint_index as usize].position);

                left_curve_control_point_vertex_index = self.b_vertices.len() as u32;
                active_edge.left_vertex_index = left_curve_control_point_vertex_index + 1;
                active_edge.control_point_vertex_index = left_curve_control_point_vertex_index + 2;

                let path_id = self.path_id;
                self.b_vertices.extend([
                    BVertex::new(&subdivided_quadratic_bezier.ap1, path_id),
                    BVertex::new(&subdivided_quadratic_bezier.ap2bp0, path_id),
                    BVertex::new(&subdivided_quadratic_bezier.bp1, path_id),
                ].into_iter());
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
            point_type: PointType::Endpoint,
        }
    }

    fn control_point_index_before_endpoint(&self, endpoint_index: u32) -> u32 {
        self.endpoints[endpoint_index as usize].control_point_index
    }

    fn control_point_index_after_endpoint(&self, endpoint_index: u32) -> u32 {
        self.control_point_index_before_endpoint(self.next_endpoint_of(endpoint_index))
    }
}

#[derive(Debug, Clone, Copy)]
struct Point {
    position: Point2D<f32>,
    endpoint_index: u32,
    point_type: PointType,
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
}

impl Default for ActiveEdge {
    fn default() -> ActiveEdge {
        ActiveEdge {
            left_vertex_index: 0,
            control_point_vertex_index: u32::MAX,
            right_endpoint_index: 0,
            left_to_right: false,
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

#[derive(Debug, Clone, Copy)]
enum PointType {
    Endpoint = 0,
    CrossingBelow = 1,
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
