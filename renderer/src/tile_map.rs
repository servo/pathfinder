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
use pathfinder_geometry::vector::Vector2I;

#[derive(Debug)]
pub struct DenseTileMap<T> {
    pub data: Vec<T>,
    pub rect: RectI,
}

impl<T> DenseTileMap<T> {
    #[inline]
    pub fn new(rect: RectI) -> DenseTileMap<T>
    where
        T: Copy + Clone + Default,
    {
        let length = rect.size().x() as usize * rect.size().y() as usize;
        DenseTileMap {
            data: vec![T::default(); length],
            rect,
        }
    }

    #[inline]
    pub fn from_builder<F>(build: F, rect: RectI) -> DenseTileMap<T>
    where
        F: FnMut(usize) -> T,
    {
        let length = rect.size().x() as usize * rect.size().y() as usize;
        DenseTileMap {
            data: (0..length).map(build).collect(),
            rect,
        }
    }

    #[inline]
    pub fn coords_to_index(&self, coords: Vector2I) -> Option<usize> {
        coords_to_index(self.rect, coords)
    }

    #[inline]
    pub fn coords_to_index_unchecked(&self, coords: Vector2I) -> usize {
        coords_to_index_unchecked(self.rect, coords)
    }

    #[inline]
    pub fn index_to_coords(&self, index: usize) -> Vector2I {
        index_to_coords(self.rect, index)
    }
}

#[inline]
fn coords_to_index(rect: RectI, coords: Vector2I) -> Option<usize> {
    if rect.contains_point(coords) {
        Some(coords_to_index_unchecked(rect, coords))
    } else {
        None
    }
}

#[inline]
fn coords_to_index_unchecked(rect: RectI, coords: Vector2I) -> usize {
    (coords.y() - rect.min_y()) as usize * rect.size().x() as usize
        + (coords.x() - rect.min_x()) as usize
}

#[inline]
fn index_to_coords(rect: RectI, index: usize) -> Vector2I {
    let (width, index) = (rect.size().x(), index as i32);
    rect.origin() + Vector2I::new(index % width, index / width)
}
