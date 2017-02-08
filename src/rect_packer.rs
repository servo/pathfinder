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

pub struct RectPacker {
    free_rects: Vec<Rect<u32>>,
    available_width: u32,
    shelf_height: u32,
    shelf_count: u32,
    /// The amount of horizontal space allocated in the last shelf.
    width_of_last_shelf: u32,
}

impl RectPacker {
    #[inline]
    pub fn new(available_width: u32, shelf_height: u32) -> RectPacker {
        RectPacker {
            free_rects: vec![],
            available_width: available_width,
            shelf_height: shelf_height,
            shelf_count: 0,
            width_of_last_shelf: 0,
        }
    }

    /// Packs a rectangle of the given size.
    ///
    /// Returns the top-left position of the rectangle or an error if there is no space left.
    pub fn pack(&mut self, size: &Size2D<u32>) -> Result<Point2D<u32>, ()> {
        // Add a one-pixel border to prevent bleed.
        let alloc_size = *size + Size2D::new(2, 2);

        // If the allocation size is less than our shelf height, we will always fail.
        if alloc_size.height > self.shelf_height {
            return Err(())
        }

        let chosen_index_and_rect =
            self.free_rects
                .iter()
                .enumerate()
                .filter(|&(_, rect)| {
                    alloc_size.width <= rect.size.width && alloc_size.height <= rect.size.height
                })
                .min_by(|&(_, a), &(_, b)| area(a).cmp(&area(b)))
                .map(|(index, rect)| (index, *rect));

        let chosen_rect;
        match chosen_index_and_rect {
            None => {
                // Make a new shelf.
                chosen_rect = Rect::new(Point2D::new(0, self.shelf_height * self.shelf_count),
                                        Size2D::new(self.available_width, self.shelf_height));
                self.shelf_count += 1;
                self.width_of_last_shelf = 0
            }
            Some((index, rect)) => {
                self.free_rects.swap_remove(index);
                chosen_rect = rect;
            }
        }

        // Guillotine to bottom.
        let free_below =
            Rect::new(Point2D::new(chosen_rect.origin.x, chosen_rect.origin.y + alloc_size.height),
                      Size2D::new(alloc_size.width, chosen_rect.size.height - alloc_size.height));
        if !free_below.is_empty() {
            self.free_rects.push(free_below);
        }

        // Guillotine to right.
        let free_to_right =
            Rect::new(Point2D::new(chosen_rect.origin.x + alloc_size.width, chosen_rect.origin.y),
                      Size2D::new(chosen_rect.size.width - alloc_size.width,
                                  chosen_rect.size.height));
        if !free_to_right.is_empty() {
            self.free_rects.push(free_to_right);
        }

        // Update width of last shelf if necessary.
        let on_last_shelf = chosen_rect.max_y() >= self.shelf_height * (self.shelf_count - 1);
        if on_last_shelf && self.width_of_last_shelf < chosen_rect.max_x() {
            self.width_of_last_shelf = chosen_rect.max_x()
        }

        let object_origin = chosen_rect.origin + Point2D::new(1, 1);
        Ok(object_origin)
    }

    #[inline]
    pub fn available_width(&self) -> u32 {
        self.available_width
    }

    #[inline]
    pub fn shelf_height(&self) -> u32 {
        self.shelf_height
    }

    #[inline]
    pub fn shelf_columns(&self) -> u32 {
        let full_shelf_count = if self.shelf_count == 0 {
            0
        } else {
            self.shelf_count - 1
        };

        full_shelf_count * self.available_width + self.width_of_last_shelf
    }
}

#[inline]
fn area(rect: &Rect<u32>) -> u32 {
    rect.size.width * rect.size.height
}

