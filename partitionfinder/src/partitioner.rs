// partitionfinder/partitioner.rs

use bit_vec::BitVec;
use euclid::Point2D;
use geometry;
use log::LogLevel;
use std::collections::BinaryHeap;
use std::cmp::{self, Ordering};
use std::u32;
use {Bezieroid, ControlPoints, Endpoint, Subpath};

pub struct Partitioner<'a> {
    endpoints: &'a [Endpoint],
    control_points: &'a [ControlPoints],
    subpaths: &'a [Subpath],

    bezieroids: Vec<Bezieroid>,

    heap: BinaryHeap<Point>,
    visited_points: BitVec,
    active_edges: Vec<ActiveEdge>,
}

impl<'a> Partitioner<'a> {
    #[inline]
    pub fn new<'b>() -> Partitioner<'b> {
        Partitioner {
            endpoints: &[],
            control_points: &[],
            subpaths: &[],

            bezieroids: vec![],

            heap: BinaryHeap::new(),
            visited_points: BitVec::new(),
            active_edges: vec![],
        }
    }

    pub fn init(&mut self,
                new_endpoints: &'a [Endpoint],
                new_control_points: &'a [ControlPoints],
                new_subpaths: &'a [Subpath]) {
        self.endpoints = new_endpoints;
        self.control_points = new_control_points;
        self.subpaths = new_subpaths;

        // FIXME(pcwalton): Move this initialization to `partition` below. Right now, this bit
        // vector uses too much memory.
        self.visited_points = BitVec::from_elem(self.endpoints.len() * 2, false);
    }

    pub fn partition(&mut self, first_subpath_index: u32, last_subpath_index: u32) {
        self.bezieroids.clear();
        self.heap.clear();
        self.active_edges.clear();

        self.init_heap(first_subpath_index, last_subpath_index);

        while self.process_next_point() {}
    }

    #[inline]
    pub fn bezieroids(&self) -> &[Bezieroid] {
        &self.bezieroids
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
            self.emit_bezieroid_above(next_active_edge_index, endpoint.position.x)
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
            self.emit_bezieroid_below(active_edge_index, endpoint.position.x)
        }
        if self.should_fill_above_active_edge(active_edge_index) {
            self.emit_bezieroid_above(active_edge_index, endpoint.position.x)
        }

        let prev_endpoint_index = self.prev_endpoint_of(endpoint_index);
        let next_endpoint_index = self.next_endpoint_of(endpoint_index);

        {
            let active_edge = &mut self.active_edges[active_edge_index as usize];
            active_edge.left_endpoint_index = active_edge.right_endpoint_index;
            if active_edge.left_to_right {
                active_edge.right_endpoint_index = next_endpoint_index;
                active_edge.time = 0.0
            } else {
                active_edge.right_endpoint_index = prev_endpoint_index;
                active_edge.time = 1.0
            }
        }

        let right_endpoint_index = self.active_edges[active_edge_index as usize]
                                       .right_endpoint_index;
        let new_point = self.create_point_from_endpoint(right_endpoint_index);
        *self.heap.peek_mut().unwrap() = new_point;

        self.add_crossings_to_heap_if_necessary(active_edge_index + 0, active_edge_index + 2)
    }

    fn process_max_endpoint(&mut self, endpoint_index: u32, active_edge_indices: [u32; 2]) {
        debug!("... MAX point: active edges {:?}", active_edge_indices);

        debug_assert!(active_edge_indices[0] < active_edge_indices[1],
                      "Matching active edge indices in wrong order when processing MAX point");

        let endpoint = &self.endpoints[endpoint_index as usize];

        if self.should_fill_above_active_edge(active_edge_indices[0]) {
            self.emit_bezieroid_above(active_edge_indices[0], endpoint.position.x)
        }
        if self.should_fill_above_active_edge(active_edge_indices[1]) {
            self.emit_bezieroid_above(active_edge_indices[1], endpoint.position.x)
        }
        if self.should_fill_below_active_edge(active_edge_indices[1]) {
            self.emit_bezieroid_below(active_edge_indices[1], endpoint.position.x)
        }

        self.heap.pop();

        // FIXME(pcwalton): This is twice as slow as it needs to be.
        self.active_edges.remove(active_edge_indices[1] as usize);
        self.active_edges.remove(active_edge_indices[0] as usize);

        self.add_crossings_to_heap_if_necessary(active_edge_indices[0], active_edge_indices[0] + 2)
    }

    fn process_crossing_point(&mut self, x: f32, upper_active_edge_index: u32) {
        if self.should_fill_above_active_edge(upper_active_edge_index) {
            self.emit_bezieroid_above(upper_active_edge_index, x)
        }
        if self.should_fill_below_active_edge(upper_active_edge_index) {
            self.emit_bezieroid_below(upper_active_edge_index, x)
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

        new_active_edges[0].left_endpoint_index = endpoint_index;
        new_active_edges[1].left_endpoint_index = endpoint_index;

        let endpoint = &self.endpoints[endpoint_index as usize];
        let prev_endpoint = &self.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.endpoints[next_endpoint_index as usize];

        // TODO(pcwalton): There's a faster way to do this with no divisions, almost certainly.
        let prev_vector = (prev_endpoint.position - endpoint.position).normalize();
        let next_vector = (next_endpoint.position - endpoint.position).normalize();

        if prev_vector.y <= next_vector.y {
            new_active_edges[0].right_endpoint_index = prev_endpoint_index;
            new_active_edges[1].right_endpoint_index = next_endpoint_index;
            new_active_edges[0].left_to_right = false;
            new_active_edges[1].left_to_right = true;
            new_active_edges[0].time = 1.0;
            new_active_edges[1].time = 0.0;
        } else {
            new_active_edges[0].right_endpoint_index = next_endpoint_index;
            new_active_edges[1].right_endpoint_index = prev_endpoint_index;
            new_active_edges[0].left_to_right = true;
            new_active_edges[1].left_to_right = false;
            new_active_edges[0].time = 0.0;
            new_active_edges[1].time = 1.0;
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

    fn emit_bezieroid_below(&mut self, upper_active_edge_index: u32, right_x: f32) {
        self.emit_bezieroid_above(upper_active_edge_index + 1, right_x)
    }

    fn emit_bezieroid_above(&mut self, lower_active_edge_index: u32, right_x: f32) {
        // TODO(pcwalton): Assert that the green X position is the same on both edges.
        debug_assert!(lower_active_edge_index > 0,
                      "Can't emit bezieroids above the top active edge");
        let upper_active_edge_index = lower_active_edge_index - 1;

        let new_bezieroid;

        {
            let lower_active_edge = &self.active_edges[lower_active_edge_index as usize];
            let upper_active_edge = &self.active_edges[upper_active_edge_index as usize];

            new_bezieroid = Bezieroid {
                upper_prev_endpoint: upper_active_edge.prev_endpoint_index(),
                upper_next_endpoint: upper_active_edge.next_endpoint_index(),
                lower_prev_endpoint: lower_active_edge.prev_endpoint_index(),
                lower_next_endpoint: lower_active_edge.next_endpoint_index(),
                upper_left_time: upper_active_edge.time,
                upper_right_time: self.solve_t_for_active_edge(upper_active_edge_index, right_x),
                lower_left_time: lower_active_edge.time,
                lower_right_time: self.solve_t_for_active_edge(lower_active_edge_index, right_x),
            };

            self.bezieroids.push(new_bezieroid);
        }

        self.active_edges[upper_active_edge_index as usize].time = new_bezieroid.upper_right_time;
        self.active_edges[lower_active_edge_index as usize].time = new_bezieroid.lower_right_time;
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

    fn solve_t_for_active_edge(&self, active_edge_index: u32, x: f32) -> f32 {
        let active_edge = &self.active_edges[active_edge_index as usize];
        let prev_endpoint_index = active_edge.prev_endpoint_index();
        let next_endpoint_index = active_edge.next_endpoint_index();
        let prev_endpoint = &self.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.endpoints[next_endpoint_index as usize];
        match self.control_points_index(next_endpoint_index) {
            None => {
                let x_vector = next_endpoint.position.x - prev_endpoint.position.x;
                (x - prev_endpoint.position.x) / x_vector
            }
            Some(control_points_index) => {
                let control_points = &self.control_points[control_points_index as usize];
                geometry::solve_cubic_bezier_t_for_x(x,
                                                     &prev_endpoint.position,
                                                     &control_points.point1,
                                                     &control_points.point2,
                                                     &next_endpoint.position)
            }
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

    fn solve_active_edge_y_for_x(&self, x: f32, active_edge: &ActiveEdge) -> f32 {
        let prev_endpoint_index = active_edge.prev_endpoint_index();
        let next_endpoint_index = active_edge.next_endpoint_index();
        if self.control_points_index(next_endpoint_index).is_none() {
            self.solve_line_y_for_x(x, prev_endpoint_index, next_endpoint_index)
        } else {
            self.solve_cubic_bezier_y_for_x(x, prev_endpoint_index, next_endpoint_index)
        }
    }

    fn solve_line_y_for_x(&self, x: f32, prev_endpoint_index: u32, next_endpoint_index: u32)
                          -> f32 {
        geometry::solve_line_y_for_x(x,
                                     &self.endpoints[prev_endpoint_index as usize].position,
                                     &self.endpoints[next_endpoint_index as usize].position)
    }

    fn solve_cubic_bezier_y_for_x(&self, x: f32, prev_endpoint_index: u32, next_endpoint_index: u32)
                                  -> f32 {
        let prev_endpoint = &self.endpoints[prev_endpoint_index as usize];
        let next_endpoint = &self.endpoints[next_endpoint_index as usize];
        let control_points_index = self.control_points_index(next_endpoint_index)
                                       .expect("Edge not a cubic bezier!");
        let control_points = &self.control_points[control_points_index as usize];
        geometry::solve_cubic_bezier_y_for_x(x,
                                             &prev_endpoint.position,
                                             &control_points.point1,
                                             &control_points.point2,
                                             &next_endpoint.position)
    }

    fn control_points_index(&self, next_endpoint_index: u32) -> Option<u32> {
        match self.endpoints[next_endpoint_index as usize].control_points_index {
            u32::MAX => None,
            control_points_index => Some(control_points_index),
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
        if upper_active_edge.left_endpoint_index == lower_active_edge.left_endpoint_index ||
                upper_active_edge.right_endpoint_index == lower_active_edge.right_endpoint_index {
            return None
        }

        let prev_upper_endpoint_index = upper_active_edge.prev_endpoint_index();
        let next_upper_endpoint_index = upper_active_edge.next_endpoint_index();
        let prev_lower_endpoint_index = lower_active_edge.prev_endpoint_index();
        let next_lower_endpoint_index = lower_active_edge.next_endpoint_index();
        let upper_control_points_index = self.endpoints[next_upper_endpoint_index as usize]
                                             .control_points_index;
        let lower_control_points_index = self.endpoints[next_lower_endpoint_index as usize]
                                             .control_points_index;

        match (upper_control_points_index, lower_control_points_index) {
            (u32::MAX, u32::MAX) => {
                self.line_line_crossing_point(prev_upper_endpoint_index,
                                              next_upper_endpoint_index,
                                              prev_lower_endpoint_index,
                                              next_lower_endpoint_index)
            }
            (u32::MAX, _) => {
                self.line_cubic_bezier_crossing_point(prev_upper_endpoint_index,
                                                      next_upper_endpoint_index,
                                                      next_lower_endpoint_index,
                                                      next_lower_endpoint_index)
            }
            (_, u32::MAX) => {
                self.line_cubic_bezier_crossing_point(prev_lower_endpoint_index,
                                                      next_lower_endpoint_index,
                                                      next_upper_endpoint_index,
                                                      next_upper_endpoint_index)
            }
            (_, _) => {
                self.cubic_bezier_cubic_bezier_crossing_point(prev_upper_endpoint_index,
                                                              next_upper_endpoint_index,
                                                              prev_lower_endpoint_index,
                                                              next_lower_endpoint_index)
            }
        }
    }

    fn line_line_crossing_point(&self,
                                prev_upper_endpoint_index: u32,
                                next_upper_endpoint_index: u32,
                                prev_lower_endpoint_index: u32,
                                next_lower_endpoint_index: u32)
                                -> Option<Point2D<f32>> {
        let endpoints = &self.endpoints;
        geometry::line_line_crossing_point(&endpoints[prev_upper_endpoint_index as usize].position,
                                           &endpoints[next_upper_endpoint_index as usize].position,
                                           &endpoints[prev_lower_endpoint_index as usize].position,
                                           &endpoints[next_lower_endpoint_index as usize].position)
    }

    fn line_cubic_bezier_crossing_point(&self,
                                        prev_line_endpoint_index: u32,
                                        next_line_endpoint_index: u32,
                                        prev_bezier_endpoint_index: u32,
                                        next_bezier_endpoint_index: u32)
                                        -> Option<Point2D<f32>> {
        let control_points_index = self.control_points_index(next_bezier_endpoint_index)
                                       .expect("Edge not a cubic Bezier!");
        let control_points = &self.control_points[control_points_index as usize];
        geometry::line_cubic_bezier_crossing_point(
            &self.endpoints[prev_line_endpoint_index as usize].position,
            &self.endpoints[next_line_endpoint_index as usize].position,
            &self.endpoints[prev_bezier_endpoint_index as usize].position,
            &control_points.point1,
            &control_points.point2,
            &self.endpoints[next_bezier_endpoint_index as usize].position)
    }

    fn cubic_bezier_cubic_bezier_crossing_point(&self,
                                                prev_upper_endpoint_index: u32,
                                                next_upper_endpoint_index: u32,
                                                prev_lower_endpoint_index: u32,
                                                next_lower_endpoint_index: u32)
                                                -> Option<Point2D<f32>> {
        let upper_control_points_index = self.control_points_index(next_upper_endpoint_index)
                                             .expect("Upper edge not a cubic Bezier!");
        let upper_control_points = &self.control_points[upper_control_points_index as usize];
        let lower_control_points_index = self.control_points_index(next_lower_endpoint_index)
                                             .expect("Lower edge not a cubic Bezier!");
        let lower_control_points = &self.control_points[lower_control_points_index as usize];
        geometry::cubic_bezier_cubic_bezier_crossing_point(
            &self.endpoints[prev_upper_endpoint_index as usize].position,
            &upper_control_points.point1,
            &upper_control_points.point2,
            &self.endpoints[next_upper_endpoint_index as usize].position,
            &self.endpoints[prev_lower_endpoint_index as usize].position,
            &lower_control_points.point1,
            &lower_control_points.point2,
            &self.endpoints[next_lower_endpoint_index as usize].position)
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

#[derive(Debug, Clone, Copy, Default)]
struct ActiveEdge {
    left_endpoint_index: u32,
    right_endpoint_index: u32,
    time: f32,
    left_to_right: bool,
}

impl ActiveEdge {
    fn prev_endpoint_index(&self) -> u32 {
        if self.left_to_right {
            self.left_endpoint_index
        } else {
            self.right_endpoint_index
        }
    }

    fn next_endpoint_index(&self) -> u32 {
        if self.left_to_right {
            self.right_endpoint_index
        } else {
            self.left_endpoint_index
        }
    }
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
