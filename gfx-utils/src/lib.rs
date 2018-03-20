// pathfinder/gfx-utils/lib.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate euclid;

use euclid::{Point2D, Size2D, Vector2D};
use std::cmp;

pub struct ShelfBinPacker {
    next: Point2D<i32>,
    max_size: Size2D<i32>,
    padding: Vector2D<i32>,
    shelf_height: i32,
}

impl ShelfBinPacker {
    pub fn new(max_size: &Size2D<i32>, padding: &Vector2D<i32>) -> ShelfBinPacker {
        ShelfBinPacker {
            next: padding.to_point(),
            max_size: *max_size,
            padding: *padding,
            shelf_height: 0,
        }
    }

    pub fn add(&mut self, size: &Size2D<i32>) -> Result<Point2D<i32>, ()> {
        let mut next = self.next;
        let mut lower_right = Point2D::new(next.x + size.width, next.y + size.height) +
            self.padding;
        if lower_right.x > self.max_size.width {
            next = Point2D::new(0, next.y + self.shelf_height);
            self.shelf_height = 0;
            lower_right = Point2D::new(size.width, next.y + size.height) + self.padding;
        }
        if lower_right.x > self.max_size.width || lower_right.y > self.max_size.height {
            return Err(())
        }
        self.shelf_height = cmp::max(self.shelf_height, size.height);
        self.next = next + Vector2D::new(size.width + self.padding.x * 2, 0);
        Ok(next)
    }
}


