// pathfinder/geometry/src/clip.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::line_segment::LineSegmentF32;
use crate::outline::{Contour, PointFlags};
use crate::point::{Point2DF32, Point3DF32};
use crate::segment::{CubicSegment, Segment};
use crate::util::lerp;
use arrayvec::ArrayVec;
use euclid::Rect;
use lyon_path::PathEvent;
use smallvec::SmallVec;
use std::mem;

pub struct RectClipper<'a> {
    clip_rect: Rect<f32>,
    subject: &'a [PathEvent],
}

impl<'a> RectClipper<'a> {
    pub fn new<'aa>(clip_rect: &Rect<f32>, subject: &'aa [PathEvent]) -> RectClipper<'aa> {
        RectClipper {
            clip_rect: *clip_rect,
            subject,
        }
    }

    pub fn clip(&self) -> Vec<PathEvent> {
        let mut output = self.subject.to_vec();
        self.clip_against(Edge::left(&self.clip_rect), &mut output);
        self.clip_against(Edge::top(&self.clip_rect), &mut output);
        self.clip_against(Edge::right(&self.clip_rect), &mut output);
        self.clip_against(Edge::bottom(&self.clip_rect), &mut output);
        output
    }

    fn clip_against(&self, edge: Edge, output: &mut Vec<PathEvent>) {
        let (mut from, mut path_start, mut first_point) = (Point2DF32::default(), None, false);
        let input = mem::replace(output, vec![]);
        for event in input {
            let to = match event {
                PathEvent::MoveTo(to) => {
                    let to = Point2DF32::from_euclid(to);
                    path_start = Some(to);
                    from = to;
                    first_point = true;
                    continue
                }
                PathEvent::Close => {
                    match path_start {
                        None => continue,
                        Some(path_start) => path_start,
                    }
                }
                PathEvent::LineTo(to) |
                PathEvent::QuadraticTo(_, to) |
                PathEvent::CubicTo(_, _, to) => Point2DF32::from_euclid(to),
                PathEvent::Arc(..) => panic!("Arcs unsupported!"),
            };

            if edge.point_is_inside(&to) {
                if !edge.point_is_inside(&from) {
                    let line_segment = LineSegmentF32::new(&from, &to);
                    for t in edge.intersect_line_segment(&line_segment) {
                        let intersection = line_segment.sample(t);
                        add_line(&intersection, output, &mut first_point);
                    }
                }
                add_line(&to, output, &mut first_point);
            } else if edge.point_is_inside(&from) {
                let line_segment = LineSegmentF32::new(&from, &to);
                for t in edge.intersect_line_segment(&line_segment) {
                    let intersection = line_segment.sample(t);
                    add_line(&intersection, output, &mut first_point);
                }
            }

            from = to;

            if let PathEvent::Close = event {
                output.push(PathEvent::Close);
                path_start = None;
            }
        }

        fn add_line(to: &Point2DF32, output: &mut Vec<PathEvent>, first_point: &mut bool) {
            let to = to.as_euclid();
            if *first_point {
                output.push(PathEvent::MoveTo(to));
                *first_point = false;
            } else {
                output.push(PathEvent::LineTo(to));
            }
        }
    }
}

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
        let t = segment.intersection_t(&self.0);
        if t >= 0.0 && t <= 1.0 {
            results.push(t);
        }
        results
    }
}

impl Edge {
    #[inline]
    fn left(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.bottom_left()),
                                 &Point2DF32::from_euclid(rect.origin)))
    }

    #[inline]
    fn top(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.origin),
                                 &Point2DF32::from_euclid(rect.top_right())))
    }

    #[inline]
    fn right(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.top_right()),
                                 &Point2DF32::from_euclid(rect.bottom_right())))
    }

    #[inline]
    fn bottom(rect: &Rect<f32>) -> Edge {
        Edge(LineSegmentF32::new(&Point2DF32::from_euclid(rect.bottom_right()),
                                 &Point2DF32::from_euclid(rect.bottom_left())))
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
                contour.push_point(segment.baseline.from(), PointFlags::empty());
            }
        }

        contour.push_segment(*segment);
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
    clip_rect: Rect<f32>,
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
    pub(crate) fn new(clip_rect: &Rect<f32>, contour: Contour) -> ContourRectClipper {
        ContourRectClipper { clip_rect: *clip_rect, contour }
    }

    pub(crate) fn clip(mut self) -> Contour {
        if self.clip_rect.contains_rect(&self.contour.bounds()) {
            return self.contour
        }

        self.clip_against(AxisAlignedEdge::Left(self.clip_rect.origin.x));
        self.clip_against(AxisAlignedEdge::Top(self.clip_rect.origin.y));
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
