// pathfinder/path-utils/src/stroke.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use freetype_sys::{FT_Init_FreeType, FT_Library, FT_Outline, FT_STROKER_LINECAP_BUTT, FT_Stroker};
use freetype_sys::{FT_STROKER_LINEJOIN_ROUND, FT_Stroker_BeginSubPath, FT_Stroker_ConicTo};
use freetype_sys::{FT_Stroker_Done, FT_Stroker_EndSubPath, FT_Stroker_Export};
use freetype_sys::{FT_Stroker_GetCounts, FT_Stroker_LineTo, FT_Stroker_New, FT_Stroker_Set};
use freetype_sys::{FT_UInt, FT_Vector};
use std::i16;

use freetype::{self, OutlineStream};
use {PathBuffer, PathSegment};

const EPSILON_POSITION_OFFSET: i64 = 8;

thread_local! {
    pub static FREETYPE_LIBRARY: FT_Library = unsafe {
        let mut library = 0 as FT_Library;
        assert!(FT_Init_FreeType(&mut library) == 0);
        library
    };
}

pub fn stroke<I>(output: &mut PathBuffer, stream: I, stroke_width: f32)
                 where I: Iterator<Item = PathSegment> {
    unsafe {
        let mut stroker = 0 as FT_Stroker;
        FREETYPE_LIBRARY.with(|&library| {
            assert!(FT_Stroker_New(library, &mut stroker) == 0);
        });

        // TODO(pcwalton): Make line caps and line join customizable.
        let stroke_width = freetype::f32_to_26_6_ft_fixed(stroke_width);
        FT_Stroker_Set(stroker,
                       stroke_width,
                       FT_STROKER_LINECAP_BUTT,
                       FT_STROKER_LINEJOIN_ROUND,
                       0);

        let mut first_position_in_subpath = None;
        for segment in stream {
            match segment {
                PathSegment::MoveTo(position) => {
                    if first_position_in_subpath.is_some() {
                        assert!(FT_Stroker_EndSubPath(stroker) == 0);
                    }
                    let mut position = freetype::f32_to_ft_vector(&position);
                    first_position_in_subpath = Some(position);
                    assert!(FT_Stroker_BeginSubPath(stroker, &mut position, 1) == 0);

                    // FIXME(pcwalton): This is a really bad hack to guard against segfaults in
                    // FreeType when paths are empty (e.g. moveto plus closepath).
                    let mut epsilon_position = FT_Vector {
                        x: position.x + EPSILON_POSITION_OFFSET,
                        y: position.y,
                    };
                    assert!(FT_Stroker_LineTo(stroker, &mut epsilon_position) == 0);
                }
                PathSegment::LineTo(position) => {
                    let mut position = freetype::f32_to_ft_vector(&position);
                    assert!(FT_Stroker_LineTo(stroker, &mut position) == 0);
                }
                PathSegment::CurveTo(control_point_position, endpoint_position) => {
                    let mut control_point_position =
                        freetype::f32_to_ft_vector(&control_point_position);
                    let mut endpoint_position = freetype::f32_to_ft_vector(&endpoint_position);
                    assert!(FT_Stroker_ConicTo(stroker,
                                               &mut control_point_position,
                                               &mut endpoint_position) == 0);
                }
                PathSegment::ClosePath => {
                    if let Some(mut first_position_in_subpath) = first_position_in_subpath {
                        assert!(FT_Stroker_LineTo(stroker, &mut first_position_in_subpath) == 0);
                        assert!(FT_Stroker_EndSubPath(stroker) == 0);
                    }
                    first_position_in_subpath = None;
                }
            }
        }

        if first_position_in_subpath.is_some() {
            assert!(FT_Stroker_EndSubPath(stroker) == 0)
        }

        let (mut anum_points, mut anum_contours) = (0, 0);
        assert!(FT_Stroker_GetCounts(stroker, &mut anum_points, &mut anum_contours) == 0);
        assert!(anum_points <= i16::MAX as FT_UInt && anum_contours <= i16::MAX as FT_UInt);

        let mut outline_points = vec![FT_Vector { x: 0, y: 0 }; anum_points as usize];
        let mut outline_tags = vec![0; anum_points as usize];
        let mut outline_contours = vec![0; anum_contours as usize];

        let mut outline = FT_Outline {
            n_contours: 0,
            n_points: 0,

            points: outline_points.as_mut_ptr(),
            tags: outline_tags.as_mut_ptr(),
            contours: outline_contours.as_mut_ptr(),

            flags: 0,
        };

        FT_Stroker_Export(stroker, &mut outline);

        FT_Stroker_Done(stroker);

        output.add_stream(OutlineStream::new(&outline, 1.0));
    }
}
