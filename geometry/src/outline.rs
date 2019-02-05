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

use crate::basic::line_segment::LineSegmentF32;
use crate::basic::point::Point2DF32;
use crate::basic::rect::RectF32;
use crate::basic::transform2d::Transform2DF32;
use crate::basic::transform3d::Perspective;
use crate::clip::{ContourPolygonClipper, ContourRectClipper};
use crate::monotonic::MonotonicConversionIter;
use crate::segment::{Segment, SegmentFlags, SegmentKind};
use std::fmt::{self, Debug, Formatter};
use std::mem;

#[derive(Clone)]
pub struct Outline {
    pub contours: Vec<Contour>,
    bounds: RectF32,
}

#[derive(Clone)]
pub struct Contour {
    pub(crate) points: Vec<Point2DF32>,
    pub(crate) flags: Vec<PointFlags>,
    pub(crate) bounds: RectF32,
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
        Outline { contours: vec![], bounds: RectF32::default() }
    }

    #[inline]
    pub fn from_segments<I>(segments: I) -> Outline
    where
        I: Iterator<Item = Segment>,
    {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();
        let mut bounds = None;

        for segment in segments {
            if segment.flags.contains(SegmentFlags::FIRST_IN_SUBPATH) {
                if !current_contour.is_empty() {
                    outline
                        .contours
                        .push(mem::replace(&mut current_contour, Contour::new()));
                }
                current_contour.push_point(segment.baseline.from(), PointFlags::empty(), true);
            }

            if segment.flags.contains(SegmentFlags::CLOSES_SUBPATH) {
                if !current_contour.is_empty() {
                    let contour = mem::replace(&mut current_contour, Contour::new());
                    contour.update_bounds(&mut bounds);
                    outline.contours.push(contour);
                }
                continue;
            }

            if segment.is_none() {
                continue;
            }

            if !segment.is_line() {
                current_contour.push_point(segment.ctrl.from(), PointFlags::CONTROL_POINT_0, true);
                if !segment.is_quadratic() {
                    current_contour.push_point(segment.ctrl.to(),
                                               PointFlags::CONTROL_POINT_1,
                                               true);
                }
            }

            current_contour.push_point(segment.baseline.to(), PointFlags::empty(), true);
        }

        if !current_contour.is_empty() {
            current_contour.update_bounds(&mut bounds);
            outline.contours.push(current_contour);
        }

        if let Some(bounds) = bounds {
            outline.bounds = bounds;
        }

        outline
    }

    #[inline]
    pub fn bounds(&self) -> RectF32 {
        self.bounds
    }

    pub fn transform(&mut self, transform: &Transform2DF32) {
        let mut new_bounds = None;
        for contour in &mut self.contours {
            contour.transform(transform);
            contour.update_bounds(&mut new_bounds);
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF32::default());
    }

    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        let mut new_bounds = None;
        for contour in &mut self.contours {
            contour.apply_perspective(perspective);
            contour.update_bounds(&mut new_bounds);
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF32::default());
    }

    pub fn prepare_for_tiling(&mut self, view_box: RectF32) {
        for contour in &mut self.contours {
            contour.prepare_for_tiling(view_box);
        }
        self.bounds = self.bounds.intersection(view_box).unwrap_or_else(|| RectF32::default());
    }

    pub fn clip_against_polygon(&mut self, clip_polygon: &[Point2DF32]) {
        let mut new_bounds = None;
        for contour in mem::replace(&mut self.contours, vec![]) {
            let contour = ContourPolygonClipper::new(clip_polygon, contour).clip();
            if !contour.is_empty() {
                contour.update_bounds(&mut new_bounds);
                self.contours.push(contour);
            }
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF32::default());
    }

    pub fn clip_against_rect(&mut self, clip_rect: RectF32) {
        if clip_rect.contains_rect(self.bounds) {
            return;
        }

        let mut new_bounds = None;
        for contour in mem::replace(&mut self.contours, vec![]) {
            let contour = ContourRectClipper::new(clip_rect, contour).clip();
            if !contour.is_empty() {
                contour.update_bounds(&mut new_bounds);
                self.contours.push(contour);
            }
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF32::default());
    }
}

impl Debug for Outline {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        for (contour_index, contour) in self.contours.iter().enumerate() {
            if contour_index > 0 {
                write!(formatter, " ")?;
            }
            contour.fmt(formatter)?;
        }
        Ok(())
    }
}

impl Contour {
    #[inline]
    pub fn new() -> Contour {
        Contour { points: vec![], flags: vec![], bounds: RectF32::default() }
    }

    // Replaces this contour with a new one, with arrays preallocated to match `self`.
    #[inline]
    pub(crate) fn take(&mut self) -> Contour {
        let length = self.len() as usize;
        mem::replace(self, Contour {
            points: Vec::with_capacity(length),
            flags: Vec::with_capacity(length),
            bounds: RectF32::default(),
        })
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
    pub fn bounds(&self) -> RectF32 {
        self.bounds
    }

    #[inline]
    pub fn position_of(&self, index: u32) -> Point2DF32 {
        self.points[index as usize]
    }

    #[inline]
    pub(crate) fn last_position(&self) -> Option<Point2DF32> {
        self.points.last().cloned()
    }

    // TODO(pcwalton): SIMD.
    #[inline]
    pub(crate) fn push_point(&mut self,
                             point: Point2DF32,
                             flags: PointFlags,
                             update_bounds: bool) {
        if update_bounds {
            let first = self.is_empty();
            union_rect(&mut self.bounds, point, first);
        }

        self.points.push(point);
        self.flags.push(flags);
    }

    pub(crate) fn push_segment(&mut self, segment: Segment, update_bounds: bool) {
        if segment.is_none() {
            return
        }

        if self.is_empty() {
            self.push_point(segment.baseline.from(), PointFlags::empty(), update_bounds);
        }

        if !segment.is_line() {
            self.push_point(segment.ctrl.from(), PointFlags::CONTROL_POINT_0, update_bounds);
            if !segment.is_quadratic() {
                self.push_point(segment.ctrl.to(), PointFlags::CONTROL_POINT_1, update_bounds);
            }
        }

        self.push_point(segment.baseline.to(), PointFlags::empty(), update_bounds);
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

    pub fn transform(&mut self, transform: &Transform2DF32) {
        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = transform.transform_point(point);
            union_rect(&mut self.bounds, *point, point_index == 0);
        }
    }

    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = perspective.transform_point_2d(point);
            union_rect(&mut self.bounds, *point, point_index == 0);
        }
    }

    fn prepare_for_tiling(&mut self, view_box: RectF32) {
        // Snap points to the view box bounds. This mops up floating point error from the clipping
        // process.
        let (origin_upper_left, origin_lower_right) = (view_box.origin(), view_box.lower_right());
        let (mut last_endpoint_index, mut contour_is_monotonic) = (None, true);
        for point_index in 0..(self.points.len() as u32) {
            let position = &mut self.points[point_index as usize];
            *position = position.clamp(origin_upper_left, origin_lower_right);

            if contour_is_monotonic {
                if self.point_is_endpoint(point_index) {
                    if let Some(last_endpoint_index) = last_endpoint_index {
                        if !self.curve_with_endpoints_is_monotonic(last_endpoint_index,
                                                                   point_index) {
                            contour_is_monotonic = false;
                        }
                    }
                    last_endpoint_index = Some(point_index);
                }
            }
        }

        // Convert to monotonic, if necessary.
        if !contour_is_monotonic {
            let contour = self.take();
            self.bounds = contour.bounds;
            for segment in MonotonicConversionIter::new(contour.iter()) {
                self.push_segment(segment, false);
            }
        }

        // Update bounds.
        self.bounds = self.bounds.intersection(view_box).unwrap_or_else(|| RectF32::default());
    }

    fn curve_with_endpoints_is_monotonic(&self, start_endpoint_index: u32, end_endpoint_index: u32)
                                         -> bool {
        let start_position = self.points[start_endpoint_index as usize];
        let end_position = self.points[end_endpoint_index as usize];

        if start_position.x() <= end_position.x() {
            for point_index in start_endpoint_index..end_endpoint_index {
                if self.points[point_index as usize].x() >
                        self.points[point_index as usize + 1].x() {
                    return false;
                }
            }
        } else {
            for point_index in start_endpoint_index..end_endpoint_index {
                if self.points[point_index as usize].x() <
                        self.points[point_index as usize + 1].x() {
                    return false;
                }
            }
        }

        if start_position.y() <= end_position.y() {
            for point_index in start_endpoint_index..end_endpoint_index {
                if self.points[point_index as usize].y() >
                        self.points[point_index as usize + 1].y() {
                    return false;
                }
            }
        } else {
            for point_index in start_endpoint_index..end_endpoint_index {
                if self.points[point_index as usize].y() <
                        self.points[point_index as usize + 1].y() {
                    return false;
                }
            }
        }

        true
    }

    fn update_bounds(&self, bounds: &mut Option<RectF32>) {
        *bounds = Some(match *bounds {
            None => self.bounds,
            Some(bounds) => bounds.union_rect(self.bounds),
        })
    }
}

impl Debug for Contour {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        for (segment_index, segment) in self.iter().enumerate() {
            if segment_index == 0 {
                write!(formatter,
                       "M {} {}",
                       segment.baseline.from_x(),
                       segment.baseline.from_y())?;
            }

            match segment.kind {
                SegmentKind::None => {}
                SegmentKind::Line => {
                    write!(formatter,
                           " L {} {}",
                           segment.baseline.to_x(),
                           segment.baseline.to_y())?;
                }
                SegmentKind::Quadratic => {
                    write!(formatter,
                           " Q {} {} {} {}",
                           segment.ctrl.from_x(),
                           segment.ctrl.from_y(),
                           segment.baseline.to_x(),
                           segment.baseline.to_y())?;
                }
                SegmentKind::Cubic => {
                    write!(formatter,
                           " C {} {} {} {} {} {}",
                           segment.ctrl.from_x(),
                           segment.ctrl.from_y(),
                           segment.ctrl.to_x(),
                           segment.ctrl.to_y(),
                           segment.baseline.to_x(),
                           segment.baseline.to_y())?;
                }
            }
        }

        write!(formatter, " z")
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
fn union_rect(bounds: &mut RectF32, new_point: Point2DF32, first: bool) {
    if first {
        *bounds = RectF32::from_points(new_point, new_point);
    } else {
        *bounds = bounds.union_point(new_point)
    }
}
