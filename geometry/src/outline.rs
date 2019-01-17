// pathfinder/geometry/src/outline.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A compressed in-memory representation of paths.

use crate::clip::ContourRectClipper;
use crate::line_segment::LineSegmentF32;
use crate::monotonic::MonotonicConversionIter;
use crate::point::Point2DF32;
use crate::segment::{Segment, SegmentFlags, SegmentKind};
use crate::transform3d::Perspective;
use crate::transform::Transform2DF32;
use euclid::{Point2D, Rect, Size2D};
use std::fmt::{self, Debug, Formatter};
use std::mem;

#[derive(Clone, Debug)]
pub struct Outline {
    pub contours: Vec<Contour>,
    bounds: Rect<f32>,
}

#[derive(Clone)]
pub struct Contour {
    points: Vec<Point2DF32>,
    flags: Vec<PointFlags>,
    bounds: Rect<f32>,
}

bitflags! {
    pub struct PointFlags: u8 {
        const CONTROL_POINT_0 = 0x01;
        const CONTROL_POINT_1 = 0x02;
    }
}

impl Outline {
    #[inline]
    pub fn new() -> Outline {
        Outline {
            contours: vec![],
            bounds: Rect::zero(),
        }
    }

    #[inline]
    pub fn from_segments<I>(segments: I) -> Outline
    where
        I: Iterator<Item = Segment>,
    {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();

        for segment in segments {
            if segment.flags.contains(SegmentFlags::FIRST_IN_SUBPATH) {
                if !current_contour.is_empty() {
                    outline
                        .contours
                        .push(mem::replace(&mut current_contour, Contour::new()));
                }
                current_contour.push_point(segment.baseline.from(), PointFlags::empty());
            }

            if segment.flags.contains(SegmentFlags::CLOSES_SUBPATH) {
                if !current_contour.is_empty() {
                    outline.push_contour(mem::replace(&mut current_contour, Contour::new()));
                }
                continue;
            }

            if segment.is_none() {
                continue;
            }

            if !segment.is_line() {
                current_contour.push_point(segment.ctrl.from(), PointFlags::CONTROL_POINT_0);
                if !segment.is_quadratic() {
                    current_contour.push_point(segment.ctrl.to(), PointFlags::CONTROL_POINT_1);
                }
            }

            current_contour.push_point(segment.baseline.to(), PointFlags::empty());
        }

        if !current_contour.is_empty() {
            outline.push_contour(current_contour);
        }

        outline
    }

    #[inline]
    pub fn bounds(&self) -> &Rect<f32> {
        &self.bounds
    }

    #[inline]
    pub fn transform(&mut self, transform: &Transform2DF32) {
        self.contours.iter_mut().for_each(|contour| contour.transform(transform));
        self.bounds = transform.transform_rect(&self.bounds);
    }

    #[inline]
    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        self.contours.iter_mut().for_each(|contour| contour.apply_perspective(perspective));
        self.bounds = perspective.transform_rect(&self.bounds);
    }

    #[inline]
    fn push_contour(&mut self, contour: Contour) {
        if self.contours.is_empty() {
            self.bounds = contour.bounds;
        } else {
            self.bounds = self.bounds.union(&contour.bounds);
        }
        self.contours.push(contour);
    }

    pub fn clip_against_rect(&mut self, clip_rect: &Rect<f32>) {
        for contour in mem::replace(&mut self.contours, vec![]) {
            let contour = ContourRectClipper::new(clip_rect, contour).clip();
            if !contour.is_empty() {
                self.push_contour(contour);
            }
        }
    }
}

impl Contour {
    #[inline]
    pub fn new() -> Contour {
        Contour { points: vec![], flags: vec![], bounds: Rect::zero() }
    }

    #[inline]
    pub fn iter(&self) -> ContourIter {
        ContourIter { contour: self, index: 1 }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.points.len() as u32
    }

    #[inline]
    pub fn bounds(&self) -> &Rect<f32> {
        &self.bounds
    }

    #[inline]
    pub fn position_of(&self, index: u32) -> Point2DF32 {
        self.points[index as usize]
    }

    // TODO(pcwalton): SIMD.
    #[inline]
    pub(crate) fn push_point(&mut self, point: Point2DF32, flags: PointFlags) {
        let first = self.is_empty();
        union_rect(&mut self.bounds, point, first);

        self.points.push(point);
        self.flags.push(flags);
    }

    pub(crate) fn push_segment(&mut self, segment: Segment) {
        if segment.is_none() {
            return
        }

        if self.is_empty() {
            self.push_point(segment.baseline.from(), PointFlags::empty());
        }

        if !segment.is_line() {
            self.push_point(segment.ctrl.from(), PointFlags::CONTROL_POINT_0);
            if !segment.is_quadratic() {
                self.push_point(segment.ctrl.to(), PointFlags::CONTROL_POINT_1);
            }
        }

        self.push_point(segment.baseline.to(), PointFlags::empty());
    }

    #[inline]
    pub fn segment_after(&self, point_index: u32) -> Segment {
        debug_assert!(self.point_is_endpoint(point_index));

        let mut segment = Segment::none();
        segment.baseline.set_from(&self.position_of(point_index));

        let point1_index = self.add_to_point_index(point_index, 1);
        if self.point_is_endpoint(point1_index) {
            segment.baseline.set_to(&self.position_of(point1_index));
            segment.kind = SegmentKind::Line;
        } else {
            segment.ctrl.set_from(&self.position_of(point1_index));

            let point2_index = self.add_to_point_index(point_index, 2);
            if self.point_is_endpoint(point2_index) {
                segment.baseline.set_to(&self.position_of(point2_index));
                segment.kind = SegmentKind::Quadratic;
            } else {
                segment.ctrl.set_to(&self.position_of(point2_index));
                segment.kind = SegmentKind::Cubic;

                let point3_index = self.add_to_point_index(point_index, 3);
                segment.baseline.set_to(&self.position_of(point3_index));
            }
        }

        segment
    }

    #[inline]
    pub fn point_is_endpoint(&self, point_index: u32) -> bool {
        !self.flags[point_index as usize]
            .intersects(PointFlags::CONTROL_POINT_0 | PointFlags::CONTROL_POINT_1)
    }

    #[inline]
    pub fn add_to_point_index(&self, point_index: u32, addend: u32) -> u32 {
        let (index, limit) = (point_index + addend, self.len());
        if index >= limit {
            index - limit
        } else {
            index
        }
    }

    #[inline]
    pub fn point_is_logically_above(&self, a: u32, b: u32) -> bool {
        let (a_y, b_y) = (self.points[a as usize].y(), self.points[b as usize].y());
        a_y < b_y || (a_y == b_y && a < b)
    }

    #[inline]
    pub fn prev_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.prev_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index;
            }
        }
    }

    #[inline]
    pub fn next_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.next_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index;
            }
        }
    }

    #[inline]
    pub fn prev_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == 0 {
            self.len() - 1
        } else {
            point_index - 1
        }
    }

    #[inline]
    pub fn next_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == self.len() - 1 {
            0
        } else {
            point_index + 1
        }
    }

    #[inline]
    pub fn transform(&mut self, transform: &Transform2DF32) {
        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = transform.transform_point(point);
            union_rect(&mut self.bounds, *point, point_index == 0);
        }

        // TODO(pcwalton): Skip this step if the transform is rectilinear.
        self.make_monotonic();
    }

    #[inline]
    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = perspective.transform_point(point);
            union_rect(&mut self.bounds, *point, point_index == 0);
        }

        self.make_monotonic();
    }

    #[inline]
    pub fn make_monotonic(&mut self) {
        let contour = mem::replace(self, Contour::new());
        for segment in MonotonicConversionIter::new(contour.iter()) {
            self.push_segment(segment);
        }
    }
}

impl Debug for Contour {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("[")?;
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        for (index, segment) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(" ")?;
            }
            if formatter.alternate() {
                formatter.write_str("\n    ")?;
            }
            segment.fmt(formatter)?;
        }
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        formatter.write_str("]")?;

        return Ok(());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointIndex(u32);

impl PointIndex {
    #[inline]
    pub fn new(contour: u32, point: u32) -> PointIndex {
        debug_assert!(contour <= 0xfff);
        debug_assert!(point <= 0x000f_ffff);
        PointIndex((contour << 20) | point)
    }

    #[inline]
    pub fn contour(self) -> u32 {
        self.0 >> 20
    }

    #[inline]
    pub fn point(self) -> u32 {
        self.0 & 0x000f_ffff
    }
}

pub struct ContourIter<'a> {
    contour: &'a Contour,
    index: u32,
}

impl<'a> Iterator for ContourIter<'a> {
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        let contour = self.contour;
        if self.index == contour.len() + 1 {
            return None;
        }

        let point0_index = self.index - 1;
        let point0 = contour.position_of(point0_index);
        if self.index == contour.len() {
            let point1 = contour.position_of(0);
            self.index += 1;
            return Some(Segment::line(&LineSegmentF32::new(&point0, &point1)));
        }

        let point1_index = self.index;
        self.index += 1;
        let point1 = contour.position_of(point1_index);
        if contour.point_is_endpoint(point1_index) {
            return Some(Segment::line(&LineSegmentF32::new(&point0, &point1)));
        }

        let point2_index = self.index;
        let point2 = contour.position_of(point2_index);
        self.index += 1;
        if contour.point_is_endpoint(point2_index) {
            return Some(Segment::quadratic(&LineSegmentF32::new(&point0, &point2), &point1));
        }

        let point3_index = self.index;
        let point3 = contour.position_of(point3_index);
        self.index += 1;
        debug_assert!(contour.point_is_endpoint(point3_index));
        return Some(Segment::cubic(&LineSegmentF32::new(&point0, &point3),
                                   &LineSegmentF32::new(&point1, &point2)));
    }
}

#[inline]
fn union_rect(bounds: &mut Rect<f32>, new_point: Point2DF32, first: bool) {
    if first {
        *bounds = Rect::new(new_point.as_euclid(), Size2D::zero());
        return;
    }

    let (mut min_x, mut min_y) = (bounds.origin.x, bounds.origin.y);
    let (mut max_x, mut max_y) = (bounds.max_x(), bounds.max_y());
    min_x = min_x.min(new_point.x());
    min_y = min_y.min(new_point.y());
    max_x = max_x.max(new_point.x());
    max_y = max_y.max(new_point.y());
    *bounds = Rect::new(Point2D::new(min_x, min_y), Size2D::new(max_x - min_x, max_y - min_y));
}
