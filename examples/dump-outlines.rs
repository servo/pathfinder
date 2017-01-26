/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

extern crate euclid;
extern crate memmap;
extern crate pathfinder;

use euclid::Point2D;
use memmap::{Mmap, Protection};
use pathfinder::batch::GlyphRange;
use pathfinder::charmap::CodepointRange;
use pathfinder::otf::Font;
use std::char;
use std::env;

fn main() {
    let file = Mmap::open_path(env::args().nth(1).unwrap(), Protection::Read).unwrap();
    unsafe {
        let font = Font::new(file.as_slice()).unwrap();
        let codepoint_ranges = [CodepointRange::new('!' as u32, '~' as u32)];
        let glyph_ranges = font.cmap.glyph_ranges_for_codepoint_ranges(&codepoint_ranges).unwrap();
        for (codepoint, glyph_id) in
                codepoint_ranges.iter()
                                .flat_map(CodepointRange::iter)
                                .zip(glyph_ranges.iter().flat_map(GlyphRange::iter)) {
            println!("Glyph {}: codepoint {} '{}':",
                     glyph_id,
                     codepoint,
                     char::from_u32(codepoint).unwrap_or('?'));

            let mut last_point: Option<Point2D<i16>> = None;
            let mut last_point_was_off_curve = false;
            font.glyf.as_ref().unwrap().for_each_point(&font.head,
                                                       font.loca.as_ref().unwrap(),
                                                       glyph_id as u32,
                                                       |point| {
                if point.first_point_in_contour {
                    println!("M {},{}", point.position.x, point.position.y);
                } else {
                    let last = last_point.unwrap();
                    if point.on_curve {
                        if last_point_was_off_curve {
                            println!("Q {},{} {},{}",
                                     last.x,
                                     last.y,
                                     point.position.x,
                                     point.position.y);
                        } else {
                            println!("L {},{}", point.position.x, point.position.y);
                        }
                    }
                }

                last_point_was_off_curve = !point.on_curve;
                last_point = Some(point.position);
            }).unwrap()
        }
    }
}

