// pathfinder/font-renderer/src/freetype/outline.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Point2D;
use freetype_sys::{FT_Outline, FT_Vector};
use lyon_path::iterator::PathIterator;
use lyon_path::{PathEvent, PathState};

const FREETYPE_POINT_ON_CURVE: i8 = 0x01;

pub struct Outline<'a> {
    outline: &'a FT_Outline,
}

impl<'a> Outline<'a> {
    #[inline]
    pub unsafe fn new(outline: &FT_Outline) -> Outline {
        Outline {
            outline: outline,
        }
    }

    #[inline]
    pub fn iter(&self) -> OutlineStream {
        unsafe {
            OutlineStream::new(self.outline)
        }
    }
}

pub struct OutlineStream<'a> {
    outline: &'a FT_Outline,
    point_index: u16,
    contour_index: u16,
    state: PathState,
    first_point_index_of_contour: bool,
}

impl<'a> OutlineStream<'a> {
    #[inline]
    pub unsafe fn new(outline: &FT_Outline) -> OutlineStream {
        OutlineStream {
            outline: outline,
            point_index: 0,
            contour_index: 0,
            state: PathState::new(),
            first_point_index_of_contour: true,
        }
    }

    #[inline]
    fn current_position_and_tag(&self) -> (Point2D<f32>, i8) {
        unsafe {
            let point_offset = self.point_index as isize;
            let position = ft_vector_to_f32(*self.outline.points.offset(point_offset));
            let tag = *self.outline.tags.offset(point_offset);
            (position, tag)
        }
    }
}

impl<'a> Iterator for OutlineStream<'a> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
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
                        let event = PathEvent::QuadraticTo(control_point_position,
                                                           self.state.first);
                        self.state.path_event(event);
                        return Some(event)
                    }

                    self.contour_index += 1;
                    self.first_point_index_of_contour = true;
                    self.state.close();
                    return Some(PathEvent::Close)
                }

                // FIXME(pcwalton): Approximate cubic curves with quadratics.
                let (position, tag) = self.current_position_and_tag();
                let point_on_curve = (tag & FREETYPE_POINT_ON_CURVE) != 0;

                if self.first_point_index_of_contour {
                    self.first_point_index_of_contour = false;
                    self.point_index += 1;
                    self.state.move_to(position);
                    return Some(PathEvent::MoveTo(position));
                }

                match (control_point_position, point_on_curve) {
                    (Some(control_point_position), false) => {
                        let on_curve_position = control_point_position.lerp(position, 0.5);
                        let event = PathEvent::QuadraticTo(control_point_position,
                                                           on_curve_position);
                        self.state.path_event(event);
                        return Some(event)
                    }
                    (Some(control_point_position), true) => {
                        self.point_index += 1;
                        let event = PathEvent::QuadraticTo(control_point_position, position);
                        self.state.path_event(event);
                        return Some(event)
                    }
                    (None, false) => {
                        self.point_index += 1;
                        control_point_position = Some(position);
                    }
                    (None, true) => {
                        self.point_index += 1;
                        self.state.line_to(position);
                        return Some(PathEvent::LineTo(position))
                    }
                }
            }
        }
    }
}

impl<'a> PathIterator for OutlineStream<'a> {
    #[inline]
    fn get_state(&self) -> &PathState {
        &self.state
    }
}

#[inline]
fn ft_vector_to_f32(ft_vector: FT_Vector) -> Point2D<f32> {
    Point2D::new(ft_vector.x as f32 / 64.0, ft_vector.y as f32 / 64.0)
}
