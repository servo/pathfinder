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
use pathfinder_geometry::basic::point::Point2DI;
use pathfinder_geometry::color::ColorU;

const PAINT_TEXTURE_WIDTH: i32 = 256;
const PAINT_TEXTURE_HEIGHT: i32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Paint {
    pub color: ColorU,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PaintId(pub u16);

impl Paint {
    pub(crate) fn is_opaque(&self) -> bool {
        self.color.a == 255
    }
}

impl Scene {
    pub fn build_paint_data(&self) -> PaintData {
        let size = Point2DI::new(PAINT_TEXTURE_WIDTH, PAINT_TEXTURE_HEIGHT);
        let mut texels = vec![0; size.x() as usize * size.y() as usize * 4];
        for (paint_index, paint) in self.paints.iter().enumerate() {
            texels[paint_index * 4 + 0] = paint.color.r;
            texels[paint_index * 4 + 1] = paint.color.g;
            texels[paint_index * 4 + 2] = paint.color.b;
            texels[paint_index * 4 + 3] = paint.color.a;
        }
        PaintData { size, texels }
    }
}

pub(crate) fn paint_id_to_tex_coords(paint_id: PaintId) -> Point2DI {
    let tex_coords = Point2DI::new(paint_id.0 as i32 % PAINT_TEXTURE_WIDTH,
                                     paint_id.0 as i32 / PAINT_TEXTURE_WIDTH);
    tex_coords.scale(256) + Point2DI::new(128, 128)
}
