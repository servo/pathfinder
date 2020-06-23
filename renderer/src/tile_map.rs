// pathfinder/renderer/src/tile_map.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::{Vector2I, vec2i};

#[derive(Clone, Debug)]
pub struct DenseTileMap<T> where T: Clone + Copy {
    pub data: Vec<T>,
    pub rect: RectI,
}

impl<T> DenseTileMap<T> where T: Clone + Copy {
    #[inline]
    pub fn from_builder<F>(mut build: F, rect: RectI) -> DenseTileMap<T>
                           where F: FnMut(Vector2I) -> T {
        let mut data = Vec::with_capacity(rect.size().x() as usize * rect.size().y() as usize);
        for y in rect.min_y()..rect.max_y() {
            for x in rect.min_x()..rect.max_x() {
                data.push(build(vec2i(x, y)));
            }
        }
        DenseTileMap { data, rect }
    }

    #[inline]
    pub fn get(&self, coords: Vector2I) -> Option<&T> {
        self.coords_to_index(coords).and_then(|index| self.data.get(index))
    }

    #[inline]
    pub fn get_mut(&mut self, coords: Vector2I) -> Option<&mut T> {
        match self.coords_to_index(coords) {
            None => None,
            Some(index) => self.data.get_mut(index),
        }
    }

    #[inline]
    pub fn coords_to_index(&self, coords: Vector2I) -> Option<usize> {
        if self.rect.contains_point(coords) {
            Some(self.coords_to_index_unchecked(coords))
        } else {
            None
        }
    }

    #[inline]
    pub fn coords_to_index_unchecked(&self, coords: Vector2I) -> usize {
        (coords.y() - self.rect.min_y()) as usize * self.rect.size().x() as usize
            + (coords.x() - self.rect.min_x()) as usize
    }

    #[inline]
    pub fn index_to_coords(&self, index: usize) -> Vector2I {
        let (width, index) = (self.rect.size().x(), index as i32);
        self.rect.origin() + vec2i(index % width, index / width)
    }
}
