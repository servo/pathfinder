// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::{Point2D, Rect, Size2D};

/// TODO(pcwalton): Track width of last shelf.
pub struct Atlas {
    free_rects: Vec<Rect<u32>>,
    available_width: u32,
    shelf_height: u32,
    shelf_count: u32,
}

impl Atlas {
    #[inline]
    pub fn new(available_width: u32, shelf_height: u32) -> Atlas {
        Atlas {
            free_rects: vec![],
            available_width: available_width,
            shelf_height: shelf_height,
            shelf_count: 0,
        }
    }

    pub fn place(&mut self, size: &Size2D<u32>) -> Result<Point2D<u32>, ()> {
        let chosen_index_and_rect =
            self.free_rects
                .iter()
                .enumerate()
                .filter(|&(_, rect)| {
                    size.width <= rect.size.width && size.height <= rect.size.height
                })
                .min_by(|&(_, a), &(_, b)| area(a).cmp(&area(b)))
                .map(|(index, rect)| (index, *rect));

        let chosen_rect;
        match chosen_index_and_rect {
            None => {
                // Make a new shelf.
                chosen_rect = Rect::new(Point2D::new(0, self.shelf_height * self.shelf_count),
                                        Size2D::new(self.available_width, self.shelf_height));
                self.shelf_count += 1
            }
            Some((index, rect)) => {
                self.free_rects.swap_remove(index);
                chosen_rect = rect;
            }
        }

        // Guillotine to bottom.
        let free_below =
            Rect::new(Point2D::new(chosen_rect.origin.x, chosen_rect.origin.y + size.height),
                      Size2D::new(size.width, chosen_rect.size.height - size.height));
        if !free_below.is_empty() {
            self.free_rects.push(free_below);
        }

        // Guillotine to right.
        let free_to_right =
            Rect::new(Point2D::new(chosen_rect.origin.x + size.width, chosen_rect.origin.y),
                      Size2D::new(chosen_rect.size.width - size.width, chosen_rect.size.height));
        if !free_to_right.is_empty() {
            self.free_rects.push(free_to_right);
        }

        Ok(chosen_rect.origin)
    }

    #[inline]
    pub fn available_width(&self) -> u32 {
        self.available_width
    }

    #[inline]
    pub fn shelf_height(&self) -> u32 {
        self.shelf_height
    }
}

#[inline]
fn area(rect: &Rect<u32>) -> u32 {
    rect.size.width * rect.size.height
}

