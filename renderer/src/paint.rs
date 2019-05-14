// pathfinder/renderer/src/paint.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::gpu_data::PaintData;
use crate::scene::Scene;
use pathfinder_geometry::basic::point::Point2DI32;

const PAINT_TEXTURE_WIDTH: i32 = 256;
const PAINT_TEXTURE_HEIGHT: i32 = 256;

impl Scene {
    pub fn build_paint_data(&self) -> PaintData {
        let size = Point2DI32::new(PAINT_TEXTURE_WIDTH, PAINT_TEXTURE_HEIGHT);
        let mut texels = vec![0; size.x() as usize * size.y() as usize * 4];
        for (path_object_index, path_object) in self.paths.iter().enumerate() {
            let paint = &self.paints[path_object.paint().0 as usize];
            texels[path_object_index * 4 + 0] = paint.color.r;
            texels[path_object_index * 4 + 1] = paint.color.g;
            texels[path_object_index * 4 + 2] = paint.color.b;
            texels[path_object_index * 4 + 3] = paint.color.a;
        }
        PaintData { size, texels }
    }
}

pub(crate) fn object_index_to_paint_coords(object_index: u16) -> Point2DI32 {
    let tex_coords = Point2DI32::new(object_index as i32 % PAINT_TEXTURE_WIDTH,
                                     object_index as i32 / PAINT_TEXTURE_WIDTH);
    tex_coords.scale(256) + Point2DI32::new(128, 128)
}
