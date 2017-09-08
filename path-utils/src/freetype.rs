// pathfinder/path-utils/src/freetype.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Point2D;
use freetype_sys::{FT_Fixed, FT_Outline, FT_Pos, FT_Vector};

use PathSegment;

const FREETYPE_POINT_ON_CURVE: i8 = 0x01;

pub struct OutlineStream<'a> {
    outline: &'a FT_Outline,
    point_index: u16,
    contour_index: u16,
    first_position_of_subpath: Point2D<f32>,
    first_point_index_of_contour: bool,
    dpi: f32,
}

impl<'a> OutlineStream<'a> {
    #[inline]
    pub unsafe fn new(outline: &FT_Outline, dpi: f32) -> OutlineStream {
        OutlineStream {
            outline: outline,
            point_index: 0,
            contour_index: 0,
            first_position_of_subpath: Point2D::zero(),
            first_point_index_of_contour: true,
            dpi: dpi,
        }
    }

    #[inline]
    fn current_position_and_tag(&self) -> (Point2D<f32>, i8) {
        unsafe {
            let point_offset = self.point_index as isize;
            let position = ft_vector_to_f32(*self.outline.points.offset(point_offset));
            let tag = *self.outline.tags.offset(point_offset);
            (position * self.dpi, tag)
        }
    }
}

impl<'a> Iterator for OutlineStream<'a> {
    type Item = PathSegment;

    fn next(&mut self) -> Option<PathSegment> {
        unsafe {
            let mut control_point_position: Option<Point2D<f32>> = None;
            loop {
                if self.contour_index == self.outline.n_contours as u16 {
                    return None
                }

                let last_point_index_in_current_contour =
                    *self.outline.contours.offset(self.contour_index as isize) as u16; 
                if self.point_index == last_point_index_in_current_contour + 1 {
                    if let Some(control_point_position) = control_point_position {
                        return Some(PathSegment::CurveTo(control_point_position,
                                                         self.first_position_of_subpath))
                    }

                    self.contour_index += 1;
                    self.first_point_index_of_contour = true;
                    return Some(PathSegment::ClosePath)
                }

                // FIXME(pcwalton): Approximate cubic curves with quadratics.
                let (position, tag) = self.current_position_and_tag();
                let point_on_curve = (tag & FREETYPE_POINT_ON_CURVE) != 0;

                if self.first_point_index_of_contour {
                    self.first_point_index_of_contour = false;
                    self.first_position_of_subpath = position;
                    self.point_index += 1;
                    return Some(PathSegment::MoveTo(position));
                }

                match (control_point_position, point_on_curve) {
                    (Some(control_point_position), false) => {
                        let on_curve_position = control_point_position.lerp(position, 0.5);
                        return Some(PathSegment::CurveTo(control_point_position,
                                                         on_curve_position))
                    }
                    (Some(control_point_position), true) => {
                        self.point_index += 1;
                        return Some(PathSegment::CurveTo(control_point_position, position))
                    }
                    (None, false) => {
                        self.point_index += 1;
                        control_point_position = Some(position);
                    }
                    (None, true) => {
                        self.point_index += 1;
                        return Some(PathSegment::LineTo(position))
                    }
                }
            }
        }
    }
}

#[inline]
fn ft_vector_to_f32(ft_vector: FT_Vector) -> Point2D<f32> {
    Point2D::new(ft_vector.x as f32 / 64.0, ft_vector.y as f32 / 64.0)
}

#[inline]
pub fn f32_to_ft_vector(point: &Point2D<f32>) -> FT_Vector {
    FT_Vector {
        x: (point.x * 64.0).round() as FT_Pos,
        y: (point.y * 64.0).round() as FT_Pos,
    }
}

#[inline]
pub fn f32_to_26_6_ft_fixed(length: f32) -> FT_Fixed {
    (length * 64.0).round() as FT_Fixed
}
