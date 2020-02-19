// pathfinder/renderer/src/allocator.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A simple quadtree-based texture allocator.

use pathfinder_geometry::rect::RectI;
use pathfinder_geometry::vector::Vector2I;
use std::mem;

const MAX_TEXTURE_LENGTH: u32 = 4096;

#[derive(Debug)]
pub struct TextureAllocator {
    root: TreeNode,
    size: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextureLocation {
    pub rect: RectI,
}

#[derive(Debug)]
enum TreeNode {
    EmptyLeaf,
    FullLeaf,
    // Top left, top right, bottom left, and bottom right, in that order.
    Parent([Box<TreeNode>; 4]),
}

impl TextureAllocator {
    #[inline]
    pub fn new(size: u32) -> TextureAllocator {
        // Make sure that the size is a power of two.
        debug_assert_eq!(size & (size - 1), 0);
        TextureAllocator { root: TreeNode::EmptyLeaf, size }
    }

    #[inline]
    pub fn allocate(&mut self, requested_size: Vector2I) -> Option<TextureLocation> {
        let requested_length =
            (requested_size.x().max(requested_size.y()) as u32).next_power_of_two();
        loop {
            if let Some(location) = self.root.allocate(Vector2I::default(),
                                                       self.size,
                                                       requested_length) {
                return Some(location);
            }
            if !self.grow() {
                return None;
            }
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn free(&mut self, location: TextureLocation) {
        let requested_length = location.rect.width() as u32;
        self.root.free(Vector2I::default(), self.size, location.rect.origin(), requested_length)
    }

    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        match self.root {
            TreeNode::EmptyLeaf => true,
            _ => false,
        }
    }

    // TODO(pcwalton): Make this more flexible.
    pub fn grow(&mut self) -> bool {
        if self.size >= MAX_TEXTURE_LENGTH {
            return false;
        }

        let old_root = mem::replace(&mut self.root, TreeNode::EmptyLeaf);
        self.size *= 2;

        // NB: Don't change the order of the children, or else texture coordinates of
        // already-allocated objects will become invalid.
        self.root = TreeNode::Parent([
            Box::new(old_root),
            Box::new(TreeNode::EmptyLeaf),
            Box::new(TreeNode::EmptyLeaf),
            Box::new(TreeNode::EmptyLeaf),
        ]);

        true
    }

    #[inline]
    pub fn size(&self) -> u32 {
        self.size
    }

    #[inline]
    pub fn scale(&self) -> f32 {
        1.0 / self.size as f32
    }
}

impl TreeNode {
    // Invariant: `requested_size` must be a power of two.
    fn allocate(&mut self, this_origin: Vector2I, this_size: u32, requested_size: u32)
                -> Option<TextureLocation> {
        if let TreeNode::FullLeaf = *self {
            // No room here.
            return None;
        }
        if this_size < requested_size {
            // Doesn't fit.
            return None;
        }

        // Allocate here or split, as necessary.
        if let TreeNode::EmptyLeaf = *self {
            // Do we have a perfect fit?
            if this_size == requested_size {
                *self = TreeNode::FullLeaf;
                return Some(TextureLocation {
                    rect: RectI::new(this_origin, Vector2I::splat(this_size as i32)),
                });
            }

            // Split.
            *self = TreeNode::Parent([
                Box::new(TreeNode::EmptyLeaf),
                Box::new(TreeNode::EmptyLeaf),
                Box::new(TreeNode::EmptyLeaf),
                Box::new(TreeNode::EmptyLeaf),
            ]);
        }

        // Recurse into children.
        match *self {
            TreeNode::Parent(ref mut kids) => {
                let kid_size = this_size / 2;
                if let Some(origin) = kids[0].allocate(this_origin, kid_size, requested_size) {
                    return Some(origin);
                }
                if let Some(origin) =
                        kids[1].allocate(this_origin + Vector2I::new(kid_size as i32, 0),
                                         kid_size,
                                         requested_size) {
                    return Some(origin);
                }
                if let Some(origin) =
                        kids[2].allocate(this_origin + Vector2I::new(0, kid_size as i32),
                                         kid_size,
                                         requested_size) {
                    return Some(origin);
                }
                if let Some(origin) =
                        kids[3].allocate(this_origin + Vector2I::splat(kid_size as i32),
                                         kid_size,
                                         requested_size) {
                    return Some(origin);
                }

                self.merge_if_necessary();
                return None;
            }
            TreeNode::EmptyLeaf | TreeNode::FullLeaf => unreachable!(),
        }
    }

    #[allow(dead_code)]
    fn free(&mut self,
            this_origin: Vector2I,
            this_size: u32,
            requested_origin: Vector2I,
            requested_size: u32) {
        if this_size <= requested_size {
            if this_size == requested_size && this_origin == requested_origin {
                *self = TreeNode::EmptyLeaf;
            }
            return;
        }

        let child_size = this_size / 2;
        let this_center = this_origin + Vector2I::splat(child_size as i32);

        let child_index;
        let mut child_origin = this_origin;

        if requested_origin.y() < this_center.y() {
            if requested_origin.x() < this_center.x() {
                child_index = 0;
            } else {
                child_index = 1;
                child_origin = child_origin + Vector2I::new(child_size as i32, 0);
            }
        } else {
            if requested_origin.x() < this_center.x() {
                child_index = 2;
                child_origin = child_origin + Vector2I::new(0, child_size as i32);
            } else {
                child_index = 3;
                child_origin = this_center;
            }
        }

        match *self {
            TreeNode::Parent(ref mut kids) => {
                kids[child_index].free(child_origin, child_size, requested_origin, requested_size);
                self.merge_if_necessary();
            }
            TreeNode::EmptyLeaf | TreeNode::FullLeaf => unreachable!(),
        }
    }

    fn merge_if_necessary(&mut self) {
        match *self {
            TreeNode::Parent(ref mut kids) => {
                if kids.iter().all(|kid| {
                    match **kid {
                        TreeNode::EmptyLeaf => true,
                        _ => false,
                    }
                }) {
                    *self = TreeNode::EmptyLeaf;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod test {
    use pathfinder_geometry::vector::Vector2I;
    use quickcheck;
    use std::u32;

    use super::TextureAllocator;

    #[test]
    fn test_allocation_and_freeing() {
        quickcheck::quickcheck(prop_allocation_and_freeing_work as
                               fn(u32, Vec<(u32, u32)>) -> bool);

        fn prop_allocation_and_freeing_work(mut length: u32, mut sizes: Vec<(u32, u32)>) -> bool {
            length = u32::next_power_of_two(length).max(1);

            for &mut (ref mut width, ref mut height) in &mut sizes {
                *width = (*width).min(length).max(1);
                *height = (*height).min(length).max(1);
            }

            let mut allocator = TextureAllocator::new(length);
            let mut locations = vec![];
            for &(width, height) in &sizes {
                let size = Vector2I::new(width as i32, height as i32);
                if let Some(location) = allocator.allocate(size) {
                    locations.push(location);
                }
            }

            for location in locations {
                allocator.free(location);
            }

            assert!(allocator.is_empty());

            true
        }
    }
}
