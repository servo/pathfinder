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

use crate::point::Point2DF32;
use crate::segment::{Segment, SegmentFlags, SegmentKind};
use crate::transform::Transform2DF32;
use euclid::{Point2D, Rect};
use lyon_path::PathEvent;
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
        let mut bounding_points = None;

        for segment in segments {
            if segment.flags.contains(SegmentFlags::FIRST_IN_SUBPATH) {
                if !current_contour.is_empty() {
                    outline
                        .contours
                        .push(mem::replace(&mut current_contour, Contour::new()));
                }
                current_contour.push_point(
                    segment.baseline.from(),
                    PointFlags::empty(),
                    &mut bounding_points,
                );
            }

            if segment.flags.contains(SegmentFlags::CLOSES_SUBPATH) {
                if !current_contour.is_empty() {
                    outline
                        .contours
                        .push(mem::replace(&mut current_contour, Contour::new()));
                }
                continue;
            }

            if segment.is_none() {
                continue;
            }

            if !segment.is_line() {
                current_contour.push_point(
                    segment.ctrl.from(),
                    PointFlags::CONTROL_POINT_0,
                    &mut bounding_points,
                );
                if !segment.is_quadratic() {
                    current_contour.push_point(
                        segment.ctrl.to(),
                        PointFlags::CONTROL_POINT_1,
                        &mut bounding_points,
                    );
                }
            }

            current_contour.push_point(
                segment.baseline.to(),
                PointFlags::empty(),
                &mut bounding_points,
            );
        }

        if !current_contour.is_empty() {
            outline.contours.push(current_contour)
        }

        if let Some((upper_left, lower_right)) = bounding_points {
            outline.bounds =
                Rect::from_points([upper_left.as_euclid(), lower_right.as_euclid()].iter())
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
    }
}

impl Contour {
    #[inline]
    pub fn new() -> Contour {
        Contour {
            points: vec![],
            flags: vec![],
        }
    }

    #[inline]
    pub fn iter(&self) -> ContourIter {
        ContourIter {
            contour: self,
            index: 0,
        }
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
    pub fn position_of(&self, index: u32) -> Point2DF32 {
        self.points[index as usize]
    }

    // TODO(pcwalton): Pack both min and max into a single SIMD register?
    #[inline]
    fn push_point(
        &mut self,
        point: Point2DF32,
        flags: PointFlags,
        bounding_points: &mut Option<(Point2DF32, Point2DF32)>,
    ) {
        self.points.push(point);
        self.flags.push(flags);

        match *bounding_points {
            Some((ref mut upper_left, ref mut lower_right)) => {
                *upper_left = upper_left.min(point);
                *lower_right = lower_right.max(point);
            }
            None => *bounding_points = Some((point, point)),
        }
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
        for point in &mut self.points {
            *point = transform.transform_point(point)
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
            write_path_event(formatter, &segment)?;
        }
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        formatter.write_str("]")?;

        return Ok(());

        fn write_path_event(formatter: &mut Formatter, path_event: &PathEvent) -> fmt::Result {
            match *path_event {
                PathEvent::Arc(..) => {
                    // TODO(pcwalton)
                    formatter.write_str("TODO: arcs")?;
                }
                PathEvent::Close => formatter.write_str("z")?,
                PathEvent::MoveTo(to) => {
                    formatter.write_str("M")?;
                    write_point(formatter, to)?;
                }
                PathEvent::LineTo(to) => {
                    formatter.write_str("L")?;
                    write_point(formatter, to)?;
                }
                PathEvent::QuadraticTo(ctrl, to) => {
                    formatter.write_str("Q")?;
                    write_point(formatter, ctrl)?;
                    write_point(formatter, to)?;
                }
                PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                    formatter.write_str("C")?;
                    write_point(formatter, ctrl0)?;
                    write_point(formatter, ctrl1)?;
                    write_point(formatter, to)?;
                }
            }
            Ok(())
        }

        fn write_point(formatter: &mut Formatter, point: Point2D<f32>) -> fmt::Result {
            write!(formatter, " {},{}", point.x, point.y)
        }
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
    type Item = PathEvent;

    #[inline]
    fn next(&mut self) -> Option<PathEvent> {
        let contour = self.contour;
        if self.index == contour.len() + 1 {
            return None;
        }
        if self.index == contour.len() {
            self.index += 1;
            return Some(PathEvent::Close);
        }

        let point0_index = self.index;
        let point0 = contour.position_of(point0_index);
        self.index += 1;
        if point0_index == 0 {
            return Some(PathEvent::MoveTo(point0.as_euclid()));
        }
        if contour.point_is_endpoint(point0_index) {
            return Some(PathEvent::LineTo(point0.as_euclid()));
        }

        let point1_index = self.index;
        let point1 = contour.position_of(point1_index);
        self.index += 1;
        if contour.point_is_endpoint(point1_index) {
            return Some(PathEvent::QuadraticTo(
                point0.as_euclid(),
                point1.as_euclid(),
            ));
        }

        let point2_index = self.index;
        let point2 = contour.position_of(point2_index);
        self.index += 1;
        debug_assert!(contour.point_is_endpoint(point2_index));
        Some(PathEvent::CubicTo(
            point0.as_euclid(),
            point1.as_euclid(),
            point2.as_euclid(),
        ))
    }
}
