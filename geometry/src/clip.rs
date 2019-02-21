// pathfinder/geometry/src/clip.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::basic::line_segment::LineSegmentF32;
use crate::basic::point::{Point2DF32, Point3DF32};
use crate::basic::rect::RectF32;
use crate::outline::{Contour, PointFlags};
use crate::segment::{CubicSegment, Segment};
use crate::util::lerp;
use arrayvec::ArrayVec;
use smallvec::SmallVec;
use std::mem;

#[derive(Clone, Copy, Debug)]
struct Edge(LineSegmentF32);

impl TEdge for Edge {
    #[inline]
    fn point_is_inside(&self, point: &Point2DF32) -> bool {
        let area = (self.0.to() - self.0.from()).det(*point - self.0.from());
        //println!("point_is_inside({:?}, {:?}), area={}", self, point, area);
        area >= 0.0
    }

    fn intersect_line_segment(&self, segment: &LineSegmentF32) -> ArrayVec<[f32; 3]> {
        let mut results = ArrayVec::new();
        if let Some(t) = segment.intersection_t(&self.0) {
            if t >= 0.0 && t <= 1.0 {
                results.push(t);
            }
        }
        results
    }
}

#[derive(Clone, Copy, Debug)]
enum AxisAlignedEdge {
    Left(f32),
    Top(f32),
    Right(f32),
    Bottom(f32),
}

impl TEdge for AxisAlignedEdge {
    #[inline]
    fn point_is_inside(&self, point: &Point2DF32) -> bool {
        match *self {
            AxisAlignedEdge::Left(x)   => point.x() >= x,
            AxisAlignedEdge::Top(y)    => point.y() >= y,
            AxisAlignedEdge::Right(x)  => point.x() <= x,
            AxisAlignedEdge::Bottom(y) => point.y() <= y,
        }
    }

    fn intersect_line_segment(&self, segment: &LineSegmentF32) -> ArrayVec<[f32; 3]> {
        let mut results = ArrayVec::new();
        let t = match *self {
            AxisAlignedEdge::Left(x) | AxisAlignedEdge::Right(x)  => segment.solve_t_for_x(x),
            AxisAlignedEdge::Top(y)  | AxisAlignedEdge::Bottom(y) => segment.solve_t_for_y(y),
        };
        if t >= 0.0 && t <= 1.0 {
            results.push(t);
        }
        results
    }
}

trait TEdge {
    fn point_is_inside(&self, point: &Point2DF32) -> bool;
    fn intersect_line_segment(&self, segment: &LineSegmentF32) -> ArrayVec<[f32; 3]>;

    fn trivially_test_segment(&self, segment: &Segment) -> EdgeRelativeLocation {
        let from_inside = self.point_is_inside(&segment.baseline.from());
        //println!("point {:?} inside {:?}: {:?}", segment.baseline.from(), self, from_inside);
        if from_inside != self.point_is_inside(&segment.baseline.to()) {
            return EdgeRelativeLocation::Intersecting;
        }
        if !segment.is_line() {
            if from_inside != self.point_is_inside(&segment.ctrl.from()) {
                return EdgeRelativeLocation::Intersecting;
            }
            if !segment.is_quadratic() {
                if from_inside != self.point_is_inside(&segment.ctrl.to()) {
                    return EdgeRelativeLocation::Intersecting;
                }
            }
        }
        if from_inside { EdgeRelativeLocation::Inside } else { EdgeRelativeLocation::Outside }
    }

    fn intersect_segment(&self, segment: &Segment) -> ArrayVec<[f32; 3]> {
        if segment.is_line() {
            return self.intersect_line_segment(&segment.baseline);
        }

        let mut segment = *segment;
        if segment.is_quadratic() {
            segment = segment.to_cubic();
        }

        let mut results = ArrayVec::new();
        let mut prev_t = 0.0;
        while !results.is_full() {
            if prev_t >= 1.0 {
                break
            }
            let next_t = match self.intersect_cubic_segment(&segment, prev_t, 1.0) {
                None => break,
                Some(next_t) => next_t,
            };
            results.push(next_t);
            prev_t = next_t + EPSILON;
        }
        return results;

        const EPSILON: f32 = 0.0001;
    }

    fn intersect_cubic_segment(&self, segment: &Segment, mut t_min: f32, mut t_max: f32)
                               -> Option<f32> {
        /*println!("... intersect_cubic_segment({:?}, {:?}, t=({}, {}))",
                 self, segment, t_min, t_max);*/
        let mut segment = segment.as_cubic_segment().split_after(t_min);
        segment = segment.as_cubic_segment().split_before(t_max / (1.0 - t_min));

        if !self.intersects_cubic_segment_hull(segment.as_cubic_segment()) {
            return None
        }

        loop {
            let t_mid = lerp(t_min, t_max, 0.5);
            if t_max - t_min < 0.00001 {
                return Some(t_mid);
            }

            let (prev_segment, next_segment) = segment.as_cubic_segment().split(0.5);
            if self.intersects_cubic_segment_hull(prev_segment.as_cubic_segment()) {
                t_max = t_mid;
                segment = prev_segment;
            } else if self.intersects_cubic_segment_hull(next_segment.as_cubic_segment()) {
                t_min = t_mid;
                segment = next_segment;
            } else {
                return None;
            }
        }
    }

    fn intersects_cubic_segment_hull(&self, cubic_segment: CubicSegment) -> bool {
        let inside = self.point_is_inside(&cubic_segment.0.baseline.from());
        inside != self.point_is_inside(&cubic_segment.0.ctrl.from()) ||
            inside != self.point_is_inside(&cubic_segment.0.ctrl.to()) ||
            inside != self.point_is_inside(&cubic_segment.0.baseline.to())
    }
}

trait ContourClipper where Self::Edge: TEdge {
    type Edge;

    fn contour_mut(&mut self) -> &mut Contour;

    fn clip_against(&mut self, edge: Self::Edge) {
        // Fast path to avoid allocation in the no-clip case.
        match self.check_for_fast_clip(&edge) {
            FastClipResult::SlowPath => {}
            FastClipResult::AllInside => return,
            FastClipResult::AllOutside => {
                *self.contour_mut() = Contour::new();
                return;
            }
        }

        let input = self.contour_mut().take();
        for segment in input.iter() {
            self.clip_segment_against(segment, &edge);
        }
    }

    fn clip_segment_against(&mut self, mut segment: Segment, edge: &Self::Edge) {
        // Easy cases.
        match edge.trivially_test_segment(&segment) {
            EdgeRelativeLocation::Outside => return,
            EdgeRelativeLocation::Inside => {
                //println!("trivial test inside, pushing segment");
                self.push_segment(&segment);
                return;
            }
            EdgeRelativeLocation::Intersecting => {}
        }

        // We have a potential intersection.
        //println!("potential intersection: {:?} edge: {:?}", segment, edge);
        let mut starts_inside = edge.point_is_inside(&segment.baseline.from());
        let intersection_ts = edge.intersect_segment(&segment);
        let mut last_t = 0.0;
        //println!("... intersections: {:?}", intersection_ts);
        for t in intersection_ts {
            let (before_split, after_split) = segment.split((t - last_t) / (1.0 - last_t));

            // Push the split segment if appropriate.
            /*println!("... ... edge={:?} before_split={:?} t={:?} starts_inside={:?}",
                        edge.0,
                        before_split,
                        t,
                        starts_inside);*/
            if starts_inside {
                //println!("... split segment case, pushing segment");
                self.push_segment(&before_split);
            }

            // We've now transitioned from inside to outside or vice versa.
            starts_inside = !starts_inside;
            last_t = t;
            segment = after_split;
        }

        // No more intersections. Push the last segment if applicable.
        if starts_inside {
            //println!("... last segment case, pushing segment");
            self.push_segment(&segment);
        }
    }

    fn push_segment(&mut self, segment: &Segment) {
        //println!("... push_segment({:?}, edge={:?}", segment, edge);
        let contour = self.contour_mut();
        if let Some(last_position) = contour.last_position() {
            if last_position != segment.baseline.from() {
                // Add a line to join up segments.
                contour.push_point(segment.baseline.from(), PointFlags::empty(), true);
            }
        }

        contour.push_segment(*segment, true);
    }

    fn check_for_fast_clip(&mut self, edge: &Self::Edge) -> FastClipResult {
        let mut result = None;
        for segment in self.contour_mut().iter() {
            let location = edge.trivially_test_segment(&segment);
            match (result, location) {
                (None, EdgeRelativeLocation::Outside) => {
                    result = Some(FastClipResult::AllOutside);
                }
                (None, EdgeRelativeLocation::Inside) => {
                    result = Some(FastClipResult::AllInside);
                }
                (Some(FastClipResult::AllInside), EdgeRelativeLocation::Inside) |
                (Some(FastClipResult::AllOutside), EdgeRelativeLocation::Outside) => {}
                (_, _) => return FastClipResult::SlowPath,
            }
        }
        result.unwrap_or(FastClipResult::AllOutside)
    }
}

#[derive(Clone, Copy)]
enum FastClipResult {
    SlowPath,
    AllInside,
    AllOutside,
}

// General convex polygon clipping in 2D

pub(crate) struct ContourPolygonClipper {
    clip_polygon: SmallVec<[Point2DF32; 4]>,
    contour: Contour,
}

impl ContourClipper for ContourPolygonClipper {
    type Edge = Edge;

    #[inline]
    fn contour_mut(&mut self) -> &mut Contour {
        &mut self.contour
    }
}

impl ContourPolygonClipper {
    #[inline]
    pub(crate) fn new(clip_polygon: &[Point2DF32], contour: Contour) -> ContourPolygonClipper {
        ContourPolygonClipper { clip_polygon: SmallVec::from_slice(clip_polygon), contour }
    }

    pub(crate) fn clip(mut self) -> Contour {
        // TODO(pcwalton): Maybe have a coarse circumscribed rect and use that for clipping?

        let clip_polygon = mem::replace(&mut self.clip_polygon, SmallVec::default());
        let mut prev = match clip_polygon.last() {
            None => return Contour::new(),
            Some(prev) => *prev,
        };
        for &next in &clip_polygon {
            self.clip_against(Edge(LineSegmentF32::new(&prev, &next)));
            prev = next;
        }

        self.contour
    }
}

#[derive(PartialEq)]
enum EdgeRelativeLocation {
    Intersecting,
    Inside,
    Outside,
}

// Fast axis-aligned box 2D clipping

pub(crate) struct ContourRectClipper {
    clip_rect: RectF32,
    contour: Contour,
}

impl ContourClipper for ContourRectClipper {
    type Edge = AxisAlignedEdge;

    #[inline]
    fn contour_mut(&mut self) -> &mut Contour {
        &mut self.contour
    }
}

impl ContourRectClipper {
    #[inline]
    pub(crate) fn new(clip_rect: RectF32, contour: Contour) -> ContourRectClipper {
        ContourRectClipper { clip_rect, contour }
    }

    pub(crate) fn clip(mut self) -> Contour {
        if self.clip_rect.contains_rect(self.contour.bounds()) {
            return self.contour
        }

        self.clip_against(AxisAlignedEdge::Left(self.clip_rect.min_x()));
        self.clip_against(AxisAlignedEdge::Top(self.clip_rect.min_y()));
        self.clip_against(AxisAlignedEdge::Right(self.clip_rect.max_x()));
        self.clip_against(AxisAlignedEdge::Bottom(self.clip_rect.max_y()));

        self.contour
    }
}

// 3D quad clipping

pub struct PolygonClipper3D {
    subject: Vec<Point3DF32>,
}

impl PolygonClipper3D {
    #[inline]
    pub fn new(subject: Vec<Point3DF32>) -> PolygonClipper3D {
        PolygonClipper3D { subject }
    }

    pub fn clip(mut self) -> Vec<Point3DF32> {
        // TODO(pcwalton): Fast path for completely contained polygon?

        //println!("before clipping against bottom: {:?}", self.subject);
        self.clip_against(Edge3D::Bottom);
        //println!("before clipping against top: {:?}", self.subject);
        self.clip_against(Edge3D::Top);
        //println!("before clipping against left: {:?}", self.subject);
        self.clip_against(Edge3D::Left);
        //println!("before clipping against right: {:?}", self.subject);
        self.clip_against(Edge3D::Right);
        //println!("before clipping against far: {:?}", self.subject);
        self.clip_against(Edge3D::Far);
        //println!("before clipping against near: {:?}", self.subject);
        self.clip_against(Edge3D::Near);
        //println!("after clipping: {:?}", self.subject);

        self.subject
    }

    fn clip_against(&mut self, edge: Edge3D) {
        let input = mem::replace(&mut self.subject, vec![]);
        let mut prev = match input.last() {
            None => return,
            Some(point) => *point,
        };
        for next in input {
            if edge.point_is_inside(next) {
                if !edge.point_is_inside(prev) {
                    self.subject.push(edge.line_intersection(prev, next));
                }
                self.subject.push(next);
            } else if edge.point_is_inside(prev) {
                self.subject.push(edge.line_intersection(prev, next));
            }
            prev = next;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Edge3D {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far
}

impl Edge3D {
    #[inline]
    fn point_is_inside(self, point: Point3DF32) -> bool {
        let w = point.w();
        match self {
            Edge3D::Left   => point.x() >= -w, Edge3D::Right => point.x() <= w,
            Edge3D::Bottom => point.y() >= -w, Edge3D::Top   => point.y() <= w,
            Edge3D::Near   => point.z() >= -w, Edge3D::Far   => point.z() <= w,
        }
    }

    // Blinn & Newell, "Clipping using homogeneous coordinates", SIGGRAPH 1978.
    fn line_intersection(self, prev: Point3DF32, next: Point3DF32) -> Point3DF32 {
        let (x0, x1) = match self {
            Edge3D::Left   | Edge3D::Right => (prev.x(), next.x()),
            Edge3D::Bottom | Edge3D::Top   => (prev.y(), next.y()),
            Edge3D::Near   | Edge3D::Far   => (prev.z(), next.z()),
        };
        let (w0, w1) = (prev.w(), next.w());
        let sign = match self {
            Edge3D::Left  | Edge3D::Bottom | Edge3D::Near => -1.0,
            Edge3D::Right | Edge3D::Top    | Edge3D::Far  =>  1.0,
        };
        let alpha = ((x0 - sign * w0) as f64) / ((sign * (w1 - w0) - (x1 - x0)) as f64);
        prev.lerp(next, alpha as f32)
    }
}

/// Coarse collision detection

// Separating axis theorem. Requires that the polygon be convex.
pub(crate) fn rect_is_outside_polygon(rect: RectF32, polygon_points: &[Point2DF32]) -> bool {
    let mut outcode = Outcode::all();
    for point in polygon_points {
        if point.x() > rect.min_x() { outcode.remove(Outcode::LEFT);   }
        if point.x() < rect.max_x() { outcode.remove(Outcode::RIGHT);  }
        if point.y() > rect.min_y() { outcode.remove(Outcode::TOP);    }
        if point.y() < rect.max_y() { outcode.remove(Outcode::BOTTOM); }
    }
    if !outcode.is_empty() {
        return true
    }

    // FIXME(pcwalton): Check winding!
    let rect_points = [rect.origin(), rect.upper_right(), rect.lower_left(), rect.lower_right()];
    for (next_point_index, &next) in polygon_points.iter().enumerate() {
        let prev_point_index = if next_point_index == 0 {
            polygon_points.len() - 1
        } else {
            next_point_index - 1
        };
        let prev = polygon_points[prev_point_index];
        let polygon_edge_vector = next - prev;
        if rect_points.iter().all(|&rect_point| polygon_edge_vector.det(rect_point - prev) < 0.0) {
            return true
        }
    }

    false
}

// Edge equation method. Requires that the polygon be convex.
pub(crate) fn rect_is_inside_polygon(rect: RectF32, polygon_points: &[Point2DF32]) -> bool {
    // FIXME(pcwalton): Check winding!
    let rect_points = [rect.origin(), rect.upper_right(), rect.lower_left(), rect.lower_right()];
    for (next_point_index, &next) in polygon_points.iter().enumerate() {
        let prev_point_index = if next_point_index == 0 {
            polygon_points.len() - 1
        } else {
            next_point_index - 1
        };
        let prev = polygon_points[prev_point_index];
        let polygon_edge_vector = next - prev;
        for &rect_point in &rect_points {
            if polygon_edge_vector.det(rect_point - prev) < 0.0 {
                return false;
            }
        }
    }

    true
}

bitflags! {
    struct Outcode: u8 {
        const LEFT   = 0x01;
        const RIGHT  = 0x02;
        const TOP    = 0x04;
        const BOTTOM = 0x08;
    }
}
