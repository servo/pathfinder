// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use byteorder::{BigEndian, ReadBytesExt};
use euclid::Point2D;
use otf::head::HeadTable;
use otf::loca::LocaTable;
use otf::{Error, FontTable};
use outline::GlyphBounds;
use std::mem;
use std::ops::Mul;
use util::Jump;

const F2DOT14_ZERO: F2Dot14 = F2Dot14(0);
const F2DOT14_ONE:  F2Dot14 = F2Dot14(0b0100_0000_0000_0000);

bitflags! {
    flags SimpleFlags: u8 {
        const ON_CURVE = 1 << 0,
        const X_SHORT_VECTOR = 1 << 1,
        const Y_SHORT_VECTOR = 1 << 2,
        const REPEAT = 1 << 3,
        const THIS_X_IS_SAME = 1 << 4,
        const THIS_Y_IS_SAME = 1 << 5,
    }
}

bitflags! {
    flags CompositeFlags: u16 {
        const ARG_1_AND_2_ARE_WORDS = 1 << 0,
        const ARGS_ARE_XY_VALUES = 1 << 1,
        const ROUND_XY_TO_GRID = 1 << 2,
        const WE_HAVE_A_SCALE = 1 << 3,
        const MORE_COMPONENTS = 1 << 5,
        const WE_HAVE_AN_X_AND_Y_SCALE = 1 << 6,
        const WE_HAVE_A_TWO_BY_TWO = 1 << 7,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    /// Where the point is located in glyph space.
    pub position: Point2D<i16>,

    /// The index of the point in this contour.
    ///
    /// When iterating over points via `for_each_point`, a value of 0 here indicates that a new
    /// contour begins.
    pub index_in_contour: u16,

    /// The kind of point this is.
    pub kind: PointKind,
}

/// The type of point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PointKind {
    /// The point is on the curve.
    OnCurve,
    /// The point is a quadratic control point.
    QuadControl,
    /// The point is the first cubic control point.
    FirstCubicControl,
    /// The point is the second cubic control point.
    SecondCubicControl,
}

/// TODO(pcwalton): Add some caching so we don't keep going to the `loca` table all the time.
#[derive(Clone, Copy, Debug)]
pub struct GlyfTable<'a> {
    pub table: FontTable<'a>,
}

impl<'a> GlyfTable<'a> {
    #[inline]
    pub fn new(table: FontTable) -> GlyfTable {
        GlyfTable {
            table: table,
        }
    }

    pub fn for_each_point<F>(&self,
                             head_table: &HeadTable,
                             loca_table: &LocaTable,
                             glyph_id: u16,
                             mut callback: F)
                             -> Result<(), Error> where F: FnMut(&Point) {
        let mut reader = self.table.bytes;

        match try!(loca_table.location_of(head_table, glyph_id)) {
            None => {
                // No points.
                return Ok(())
            }
            Some(offset) => try!(reader.jump(offset as usize).map_err(Error::eof)),
        }

        let glyph_start = reader;
        let number_of_contours = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        if number_of_contours >= 0 {
            self.for_each_point_in_simple_glyph(glyph_start, callback)
        } else {
            self.for_each_point_in_composite_glyph(glyph_start, head_table, loca_table, callback)
        }
    }

    fn for_each_point_in_simple_glyph<F>(&self, mut reader: &[u8], mut callback: F)
                                         -> Result<(), Error> where F: FnMut(&Point) {
        // Determine how many contours we have.
        let number_of_contours = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        if number_of_contours == 0 {
            return Ok(())
        }

        // Skip over the rest of the header.
        try!(reader.jump(mem::size_of::<i16>() * 4).map_err(Error::eof));

        // Find out how many points we have.
        let mut endpoints_reader = reader;
        try!(reader.jump(mem::size_of::<u16>() as usize * (number_of_contours as usize - 1))
                   .map_err(Error::eof));
        let number_of_points = try!(reader.read_u16::<BigEndian>().map_err(Error::eof)) + 1;

        // Skip over hinting instructions.
        let instruction_length = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
        try!(reader.jump(instruction_length as usize).map_err(Error::eof));

        // Find the offsets of the X and Y coordinates.
        let flags_reader = reader;
        let x_coordinate_length = try!(calculate_size_of_x_coordinates(&mut reader,
                                                                       number_of_points));

        // Set up the streams.
        let mut flag_parser = try!(FlagParser::new(flags_reader));
        let mut x_coordinate_reader = reader;
        try!(reader.jump(x_coordinate_length as usize).map_err(Error::eof));
        let mut y_coordinate_reader = reader;

        // Now parse the contours.
        let (mut position, mut point_index) = (Point2D::new(0, 0), 0);
        for _ in 0..number_of_contours {
            let contour_point_count = try!(endpoints_reader.read_u16::<BigEndian>()
                                                           .map_err(Error::eof)) - point_index + 1;

            let mut first_on_curve_point = None;
            let mut initial_off_curve_point = None;
            let mut last_point_was_off_curve = false;
            let mut point_index_in_contour = 0;

            for contour_point_index in 0..contour_point_count {
                let flags = SimpleFlags::from_bits_truncate(*flag_parser.current);
                try!(flag_parser.next());

                let mut delta = Point2D::new(0, 0);
                if flags.contains(X_SHORT_VECTOR) {
                    delta.x = try!(x_coordinate_reader.read_u8().map_err(Error::eof)) as i16;
                    if !flags.contains(THIS_X_IS_SAME) {
                        delta.x = -delta.x
                    }
                } else if !flags.contains(THIS_X_IS_SAME) {
                    delta.x = try!(x_coordinate_reader.read_i16::<BigEndian>().map_err(Error::eof))
                }
                if flags.contains(Y_SHORT_VECTOR) {
                    delta.y = try!(y_coordinate_reader.read_u8().map_err(Error::eof)) as i16;
                    if !flags.contains(THIS_Y_IS_SAME) {
                        delta.y = -delta.y
                    }
                } else if !flags.contains(THIS_Y_IS_SAME) {
                    delta.y = try!(y_coordinate_reader.read_i16::<BigEndian>().map_err(Error::eof))
                }

                if last_point_was_off_curve && !flags.contains(ON_CURVE) {
                    let position = position + delta / 2;

                    // An important edge case!
                    if first_on_curve_point.is_none() {
                        first_on_curve_point = Some(position)
                    }

                    callback(&Point {
                        position: position,
                        index_in_contour: point_index_in_contour,
                        kind: PointKind::OnCurve,
                    });
                    point_index_in_contour += 1
                }

                position = position + delta;

                if flags.contains(ON_CURVE) && first_on_curve_point.is_none() {
                    first_on_curve_point = Some(position)
                }

                // Sometimes the initial point is an off curve point. In that case, save it so we
                // can emit it later when closing the path.
                if !flags.contains(ON_CURVE) && first_on_curve_point.is_none() {
                    debug_assert!(initial_off_curve_point.is_none());
                    initial_off_curve_point = Some(position)
                } else {
                    callback(&Point {
                        position: position,
                        kind: if flags.contains(ON_CURVE) {
                            PointKind::OnCurve
                        } else {
                            PointKind::QuadControl
                        },
                        index_in_contour: point_index_in_contour,
                    });
                    point_index_in_contour += 1
                }

                last_point_was_off_curve = !flags.contains(ON_CURVE);
                point_index += 1;
            }

            // We're about to close the path. Emit the initial off curve point if there was one.
            if let Some(initial_off_curve_point) = initial_off_curve_point {
                if last_point_was_off_curve {
                    // Another important edge case!
                    let position = position + (initial_off_curve_point - position) / 2;
                    callback(&Point {
                        position: position,
                        index_in_contour: point_index_in_contour,
                        kind: PointKind::OnCurve,
                    });
                    point_index_in_contour += 1
                }

                callback(&Point {
                    position: initial_off_curve_point,
                    kind: PointKind::QuadControl,
                    index_in_contour: point_index_in_contour,
                });
                point_index_in_contour += 1
            }

            // Close the path.
            if let Some(first_on_curve_point) = first_on_curve_point {
                callback(&Point {
                    position: first_on_curve_point,
                    kind: PointKind::OnCurve,
                    index_in_contour: point_index_in_contour,
                })
            }
        }

        Ok(())
    }

    // TODO(pcwalton): Consider rasterizing pieces of composite glyphs independently and
    // compositing them together.
    fn for_each_point_in_composite_glyph<F>(&self,
                                            mut reader: &[u8],
                                            head_table: &HeadTable,
                                            loca_table: &LocaTable,
                                            mut callback: F)
                                            -> Result<(), Error> where F: FnMut(&Point) {
        try!(reader.jump(mem::size_of::<i16>() * 5).map_err(Error::eof));

        loop {
            let flags = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
            let flags = CompositeFlags::from_bits_truncate(flags);
            let glyph_index = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));

            let (arg0, arg1);
            if flags.contains(ARG_1_AND_2_ARE_WORDS) {
                arg0 = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
                arg1 = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
            } else {
                arg0 = try!(reader.read_i8().map_err(Error::eof)) as i16;
                arg1 = try!(reader.read_i8().map_err(Error::eof)) as i16;
            }

            let mut transform = Mat3x2::identity();
            if flags.contains(ARGS_ARE_XY_VALUES) {
                transform.m02 = arg0;
                transform.m12 = arg1;
            }

            if flags.contains(WE_HAVE_A_SCALE) {
                let scale = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
                transform.m00 = scale;
                transform.m11 = scale;
            } else if flags.contains(WE_HAVE_AN_X_AND_Y_SCALE) {
                transform.m00 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
                transform.m11 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
            } else if flags.contains(WE_HAVE_A_TWO_BY_TWO) {
                transform.m00 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
                transform.m01 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
                transform.m10 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
                transform.m11 = F2Dot14(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)));
            }

            if let Some(offset) = try!(loca_table.location_of(head_table, glyph_index)) {
                let mut reader = self.table.bytes;
                try!(reader.jump(offset as usize).map_err(Error::eof));
                self.for_each_point_in_simple_glyph(reader, |point| {
                    callback(&transform.transform(&point))
                });
            }

            if !flags.contains(MORE_COMPONENTS) {
                break
            }
        }

        Ok(())
    }

    pub fn glyph_bounds(&self, head_table: &HeadTable, loca_table: &LocaTable, glyph_id: u16)
                        -> Result<GlyphBounds, Error> {
        let mut reader = self.table.bytes;

        match try!(loca_table.location_of(head_table, glyph_id)) {
            None => {
                // No outlines.
                return Ok(GlyphBounds {
                    left: 0,
                    bottom: 0,
                    right: 0,
                    top: 0,
                })
            }
            Some(offset) => try!(reader.jump(offset as usize).map_err(Error::eof)),
        }

        // Skip over the number of contours.
        try!(reader.read_i16::<BigEndian>().map_err(Error::eof));

        let x_min = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        let y_min = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        let x_max = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        let y_max = try!(reader.read_i16::<BigEndian>().map_err(Error::eof));
        Ok(GlyphBounds {
            left: x_min as i32,
            bottom: y_min as i32,
            right: x_max as i32,
            top: y_max as i32,
        })
    }
}

// Given a reader pointing to the start of the list of flags, returns the size in bytes of the list
// of X coordinates and positions the reader at the start of that list.
#[inline]
fn calculate_size_of_x_coordinates<'a, 'b>(reader: &'a mut &'b [u8], number_of_points: u16)
                                           -> Result<u16, Error> {
    let (mut x_coordinate_length, mut points_left) = (0, number_of_points);
    while points_left > 0 {
        let flags = SimpleFlags::from_bits_truncate(try!(reader.read_u8().map_err(Error::eof)));
        let repeat_count = if !flags.contains(REPEAT) {
            1
        } else {
            try!(reader.read_u8().map_err(Error::eof)) as u16 + 1
        };

        if flags.contains(X_SHORT_VECTOR) {
            x_coordinate_length += repeat_count
        } else if !flags.contains(THIS_X_IS_SAME) {
            x_coordinate_length += repeat_count * 2
        }

        points_left -= repeat_count
    }

    Ok(x_coordinate_length)
}

struct FlagParser<'a> {
    next: &'a [u8],
    current: &'a u8,
    repeats_left: u8,
}

impl<'a> FlagParser<'a> {
    #[inline]
    fn new(buffer: &[u8]) -> Result<FlagParser, Error> {
        let mut parser = FlagParser {
            next: buffer,
            current: &buffer[0],
            repeats_left: 0,
        };
        try!(parser.next());
        Ok(parser)
    }

    #[inline]
    fn next(&mut self) -> Result<(), Error> {
        if self.repeats_left > 0 {
            self.repeats_left -= 1;
            return Ok(())
        }

        self.current = match self.next.get(0) {
            Some(value) => value,
            None => return Err(Error::UnexpectedEof),
        };

        let flags = SimpleFlags::from_bits_truncate(*self.current);
        self.next = &self.next[1..];

        if flags.contains(REPEAT) {
            self.repeats_left = match self.next.get(0) {
                Some(&value) => value,
                None => return Err(Error::UnexpectedEof),
            };

            self.next = &self.next[1..];
        } else {
            self.repeats_left = 0
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
struct Mat3x2 {
    m00: F2Dot14,
    m01: F2Dot14,
    m02: i16,
    m10: F2Dot14,
    m11: F2Dot14,
    m12: i16,
}

impl Mat3x2 {
    fn identity() -> Mat3x2 {
        Mat3x2 {
            m00: F2DOT14_ONE,  m01: F2DOT14_ZERO, m02: 0,
            m10: F2DOT14_ZERO, m11: F2DOT14_ONE,  m12: 0,
        }
    }

    // TODO(pcwalton): SIMD/FMA.
    fn transform(&self, point: &Point) -> Point {
        let p = point.position;
        Point {
            position: Point2D::new(self.m00 * p.x + self.m01 * p.y + self.m02,
                                   self.m10 * p.x + self.m11 * p.y + self.m12),
            ..*point
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct F2Dot14(i16);

impl Mul<i16> for F2Dot14 {
    type Output = i16;

    #[inline]
    fn mul(self, other: i16) -> i16 {
        ((self.0 as i32 * other as i32) >> 14) as i16
    }
}

