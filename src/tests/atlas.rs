/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

use atlas::Atlas;
use euclid::{Rect, Size2D};
use std::cmp;

fn place_objects(available_width: u32, objects: Vec<(u32, u32)>) -> (Atlas, Vec<Rect<u32>>) {
    let objects: Vec<_> = objects.iter()
                                 .map(|&(width, height)| Size2D::new(width, height))
                                 .collect();

    let available_width = cmp::max(available_width,
                                   objects.iter().map(|object| object.width).max().unwrap_or(0));
    let shelf_height = objects.iter().map(|object| object.height).max().unwrap_or(0);

    let mut atlas = Atlas::new(available_width, shelf_height);
    let rects = objects.iter()
                       .map(|object| Rect::new(atlas.place(object).unwrap(), *object))
                       .collect();
    (atlas, rects)
}

quickcheck! {
    fn objects_dont_overlap(available_width: u32, objects: Vec<(u32, u32)>) -> bool {
        let (_, rects) = place_objects(available_width, objects);
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[(i + 1)..] {
                assert!(!a.intersects(b))
            }
        }
        true
    }

    fn objects_dont_exceed_available_width(available_width: u32, objects: Vec<(u32, u32)>) -> bool {
        let (atlas, rects) = place_objects(available_width, objects);
        rects.iter().all(|rect| rect.max_x() <= atlas.available_width())
    }

    fn objects_dont_cross_shelves(available_width: u32, objects: Vec<(u32, u32)>) -> bool {
        let (atlas, rects) = place_objects(available_width, objects);
        rects.iter().all(|rect| {
            rect.is_empty() ||
                rect.origin.y / atlas.shelf_height() == (rect.max_y() - 1) / atlas.shelf_height()
        })
    }
}

