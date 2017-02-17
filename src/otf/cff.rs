// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use euclid::Point2D;
use otf::glyf::{Point, PointKind};
use otf::head::HeadTable;
use otf::{Error, FontTable};
use outline::GlyphBounds;
use std::cmp;
use std::u16;
use util::Jump;

#[derive(Clone, Copy, Debug)]
pub struct CffTable<'a> {
    // The offset of the char strings INDEX.
    char_strings: u32,
    table: FontTable<'a>,
}

impl<'a> CffTable<'a> {
    #[inline]
    pub fn new(table: FontTable) -> Result<CffTable, Error> {
        let mut reader = table.bytes;

        // Check version.
        let major = try!(reader.read_u8().map_err(Error::eof));
        let minor = try!(reader.read_u8().map_err(Error::eof));
        if major != 1 || minor != 0 {
            return Err(Error::UnsupportedCffVersion)
        }

        // Skip the header.
        let hdr_size = try!(reader.read_u8().map_err(Error::eof));
        try!(reader.jump(hdr_size as usize - 3).map_err(Error::eof));

        // Skip the name INDEX.
        //
        // TODO(pcwalton): What to do if there are multiple fonts here?
        try!(skip_index(&mut reader));

        // Get the top DICT for our font.
        if try!(find_in_index(&mut reader, 0)).is_none() {
            return Err(Error::CffTopDictNotFound)
        }

        // Find the CharStrings offset within the top DICT.
        let char_strings = try!(get_integer_in_dict(&mut reader, 17));

        // Skip the string INDEX.
        try!(skip_index(&mut reader));

        // Ignore the global subr INDEX for now.
        //
        // TODO(pcwalton): Support global subroutines.

        Ok(CffTable {
            char_strings: char_strings as u32,
            table: table,
        })
    }

    pub fn for_each_point<F>(&self, glyph_id: u16, mut callback: F)
                             -> Result<(), Error> where F: FnMut(&Point) {
        let mut reader = self.table.bytes;
        try!(reader.jump(self.char_strings as usize).map_err(Error::eof));

        let char_string_length = match try!(find_in_index(&mut reader, glyph_id)) {
            Some(char_string_length) => char_string_length,
            None => return Err(Error::UnexpectedEof),
        };

        let mut reader = &reader[0..char_string_length as usize];
        let mut stack = EvaluationStack::new();
        let (mut start, mut pos) = (Point2D::new(0, 0), Point2D::new(0, 0));
        let mut index_in_contour = 0;
        let mut hint_count = 0;

        // FIXME(pcwalton): This shouldn't panic on stack bounds check failures.
        while let Ok(b0) = reader.read_u8() {
            match b0 {
                32...246 => try!(stack.push(b0 as i32 - 139)),
                247...250 => {
                    let b1 = try!(reader.read_u8().map_err(Error::eof));
                    try!(stack.push((b0 as i32 - 247) * 256 + b1 as i32 + 108))
                }
                251...254 => {
                    let b1 = try!(reader.read_u8().map_err(Error::eof));
                    try!(stack.push((b0 as i32 - 251) * -256 - b1 as i32 - 108))
                }
                255 => {
                    // FIXME(pcwalton): Don't truncate the lower 16 bits.
                    try!(stack.push(try!(reader.read_i32::<BigEndian>().map_err(Error::eof)) >>
                         16))
                }
                28 => {
                    let number = try!(reader.read_i16::<BigEndian>().map_err(Error::eof)) as i32;
                    try!(stack.push(number))
                }

                4 => {
                    // |- dy1 vmoveto
                    close_path_if_necessary(&mut pos, &start, index_in_contour, &mut callback);
                    pos.y += stack.array[0] as i16;
                    callback(&Point {
                        position: pos,
                        index_in_contour: 0,
                        kind: PointKind::OnCurve,
                    });
                    start = pos;
                    index_in_contour = 1;
                    stack.clear()
                }
                5 => {
                    // |- {dxa dya}+ rlineto
                    for points in stack.array[0..stack.size as usize].chunks(2) {
                        pos = pos + Point2D::new(points[0] as i16, points[1] as i16);
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour,
                            kind: PointKind::OnCurve,
                        });
                        index_in_contour += 1
                    }
                    stack.clear()
                }
                6 => {
                    // |- dx1 {dya dxb}* hlineto
                    // |- {dxa dyb}* hlineto
                    for (i, length) in stack.array[0..stack.size as usize].iter().enumerate() {
                        if i % 2 == 0 {
                            pos.x += *length as i16
                        } else {
                            pos.y += *length as i16
                        }
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour,
                            kind: PointKind::OnCurve,
                        });
                        index_in_contour += 1
                    }
                    stack.clear()
                }
                7 => {
                    // |- dy1 {dxa dyb}* vlineto
                    // |- {dya dxb}* vlineto
                    for (i, length) in stack.array[0..stack.size as usize].iter().enumerate() {
                        if i % 2 == 0 {
                            pos.y += *length as i16
                        } else {
                            pos.x += *length as i16
                        }
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour,
                            kind: PointKind::OnCurve,
                        });
                        index_in_contour += 1
                    }
                    stack.clear()
                }
                8 => {
                    // |- {dxa dya dxb dyb dxc dyc}+ rrcurveto (8)
                    for chunk in stack.array[0..stack.size as usize].chunks(6) {
                        pos = pos + Point2D::new(chunk[0] as i16, chunk[1] as i16);
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 0,
                            kind: PointKind::FirstCubicControl,
                        });

                        pos = pos + Point2D::new(chunk[2] as i16, chunk[3] as i16);
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 1,
                            kind: PointKind::SecondCubicControl,
                        });

                        pos = pos + Point2D::new(chunk[4] as i16, chunk[5] as i16);
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 2,
                            kind: PointKind::OnCurve,
                        });

                        index_in_contour += 3
                    }
                    stack.clear()
                }
                30 => {
                    // |- dy1 dx2 dy2 dx3 {dxa dxb dyb dyc dyd dxe dye dxf}* dyf? vhcurveto (30)
                    // |- {dya dxb dyb dxc dxd dxe dye dyf}+ dxf? vhcurveto (30)
                    for (i, chunk) in stack.array[0..stack.size as usize].chunks(4).enumerate() {
                        if chunk.len() != 4 {
                            break
                        }

                        let dxyf = if i * 4 + 5 == stack.size as usize {
                            stack.array[stack.size as usize - 1]
                        } else {
                            0
                        };

                        if i % 2 == 0 {
                            process_hvcurveto_v(chunk,
                                                &mut pos,
                                                dxyf,
                                                &mut index_in_contour,
                                                &mut callback);
                        } else {
                            process_hvcurveto_h(chunk,
                                                &mut pos,
                                                dxyf,
                                                &mut index_in_contour,
                                                &mut callback);
                        }
                    }
                    stack.clear()
                }
                31 => {
                    // |- dx1 dx2 dy2 dy3 {dya dxb dyb dxc dxd dxe dye dyf}* dxf? hvcurveto (31)
                    // |- {dxa dxb dyb dyc dyd dxe dye dxf}+ dyf? hvcurveto (31)
                    for (i, chunk) in stack.array[0..stack.size as usize].chunks(4).enumerate() {
                        if chunk.len() != 4 {
                            break
                        }

                        let dxyf = if i * 4 + 5 == stack.size as usize {
                            stack.array[stack.size as usize - 1]
                        } else {
                            0
                        };

                        if i % 2 == 0 {
                            process_hvcurveto_h(chunk,
                                                &mut pos,
                                                dxyf,
                                                &mut index_in_contour,
                                                &mut callback);
                        } else {
                            process_hvcurveto_v(chunk,
                                                &mut pos,
                                                dxyf,
                                                &mut index_in_contour,
                                                &mut callback);
                        }
                    }
                    stack.clear()
                }
                26 => {
                    // |- dx1? {dya dxb dyb dyc}+ vvcurveto (26)
                    let start;
                    if stack.size % 2 == 0 {
                        start = 0
                    } else {
                        pos.x += stack.array[0] as i16;
                        start = 1
                    }

                    for (i, chunk) in stack.array[start..stack.size as usize]
                                           .chunks(4)
                                           .enumerate() {
                        pos.y += chunk[0] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour,
                            kind: PointKind::FirstCubicControl,
                        });

                        pos.x += chunk[1] as i16;
                        pos.y += chunk[2] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 1,
                            kind: PointKind::SecondCubicControl,
                        });

                        pos.y += chunk[3] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 2,
                            kind: PointKind::OnCurve,
                        });

                        index_in_contour += 3
                    }
                    stack.clear()
                }
                27 => {
                    // |- dy1? {dxa dxb dyb dxc}+ hhcurveto (27)
                    let start;
                    if stack.size % 2 == 0 {
                        start = 0
                    } else {
                        pos.y += stack.array[0] as i16;
                        start = 1
                    }

                    for (i, chunk) in stack.array[start..stack.size as usize]
                                           .chunks(4)
                                           .enumerate() {
                        pos.x += chunk[0] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour,
                            kind: PointKind::FirstCubicControl,
                        });

                        pos.x += chunk[1] as i16;
                        pos.y += chunk[2] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 1,
                            kind: PointKind::SecondCubicControl,
                        });

                        pos.x += chunk[3] as i16;
                        callback(&Point {
                            position: pos,
                            index_in_contour: index_in_contour + 2,
                            kind: PointKind::OnCurve,
                        });

                        index_in_contour += 3
                    }
                    stack.clear()
                }
                14 => {
                    // endchar
                    break
                }
                1 | 18 => {
                    // hstem hint (ignored)
                    hint_count += stack.size as u16 / 2;
                    stack.clear()
                }
                3 | 23 => {
                    // vstem hint (ignored)
                    hint_count += stack.size as u16 / 2;
                    stack.clear()
                }
                19 => {
                    // hintmask (ignored)
                    //
                    // First, process an implicit vstem hint.
                    //
                    // FIXME(pcwalton): Should only do that if we're in the header.
                    hint_count += stack.size as u16 / 2;
                    stack.clear();

                    // Now skip ⌈hint_count / 8⌉ bytes.
                    let hint_byte_count = (hint_count as usize + 7) / 8;
                    try!(reader.jump(hint_byte_count).map_err(Error::eof));
                }
                21 => {
                    // |- dx1 dy1 rmoveto
                    close_path_if_necessary(&mut pos, &start, index_in_contour, &mut callback);
                    pos = pos + Point2D::new(stack.array[0] as i16, stack.array[1] as i16);
                    callback(&Point {
                        position: pos,
                        index_in_contour: 0,
                        kind: PointKind::OnCurve,
                    });
                    start = pos;
                    index_in_contour = 1;
                    stack.clear()
                }
                22 => {
                    // |- dx1 hmoveto
                    close_path_if_necessary(&mut pos, &start, index_in_contour, &mut callback);
                    pos.x += stack.array[0] as i16;
                    callback(&Point {
                        position: pos,
                        index_in_contour: 0,
                        kind: PointKind::OnCurve,
                    });
                    start = pos;
                    index_in_contour = 1;
                    stack.clear()
                }

                12 => {
                    // TODO(pcwalton): Support these extended operators.
                    let _operator = (12 << 8) |
                        (try!(reader.read_u8().map_err(Error::eof)) as u32);
                    stack.clear();
                    return Err(Error::CffUnimplementedOperator)
                }
                _ => {
                    stack.clear();
                    return Err(Error::CffUnimplementedOperator)
                }
            }
        }

        close_path_if_necessary(&mut pos, &start, index_in_contour, &mut callback);
        Ok(())
    }

    // TODO(pcwalton): Do some caching, perhaps?
    // TODO(pcwalton): Compute this at the same time as `for_each_point`, perhaps?
    pub fn glyph_bounds(&self, glyph_id: u16) -> Result<GlyphBounds, Error> {
        let mut bounds = GlyphBounds::default();
        self.for_each_point(glyph_id, |point| {
            bounds.left = cmp::min(bounds.left, point.position.x as i32);
            bounds.bottom = cmp::min(bounds.bottom, point.position.y as i32);
            bounds.right = cmp::max(bounds.right, point.position.x as i32);
            bounds.top = cmp::max(bounds.top, point.position.y as i32);
        });
        Ok(bounds)
    }
}

// Moves the reader to the location of the given element in the index. Returns the length of the
// element if the element was found or `None` otherwise.
fn find_in_index(reader: &mut &[u8], index: u16) -> Result<Option<u32>, Error> {
    let count = try!(reader.read_u16::<BigEndian>().map_err(Error::eof));
    if count == 0 {
        return Ok(None)
    }

    let off_size = try!(reader.read_u8().map_err(Error::eof));

    let mut offset_reader = *reader;
    try!(offset_reader.jump(off_size as usize * cmp::min(index, count) as usize)
                      .map_err(Error::eof));
    let offset = try!(read_offset(&mut offset_reader, off_size));

    let next_offset = if index < count {
        Some(try!(read_offset(&mut offset_reader, off_size)) - offset)
    } else {
        None
    };

    try!(reader.jump(off_size as usize * (count as usize + 1) + offset as usize - 1)
               .map_err(Error::eof));
    return Ok(next_offset)
}

// Skips over an INDEX by reading the last element in the offset array and seeking the appropriate
// number of bytes forward.
fn skip_index(reader: &mut &[u8]) -> Result<(), Error> {
    find_in_index(reader, u16::MAX).map(drop)
}

// Returns the integer with the given operator.
fn get_integer_in_dict(reader: &mut &[u8], operator: u16) -> Result<i32, Error> {
    let mut last_integer_operand = None;
    loop {
        let b0 = try!(reader.read_u8().map_err(Error::eof));
        match b0 {
            32...246 => last_integer_operand = Some(b0 as i32 - 139),
            247...250 => {
                let b1 = try!(reader.read_u8().map_err(Error::eof));
                last_integer_operand = Some((b0 as i32 - 247) * 256 + b1 as i32 + 108)
            }
            251...254 => {
                let b1 = try!(reader.read_u8().map_err(Error::eof));
                last_integer_operand = Some(-(b0 as i32 - 251) * 256 - b1 as i32 - 108)
            }
            28 => {
                last_integer_operand =
                    Some(try!(reader.read_i16::<BigEndian>().map_err(Error::eof)) as i32)
            }
            29 => {
                last_integer_operand =
                    Some(try!(reader.read_i32::<BigEndian>().map_err(Error::eof)) as i32)
            }
            30 => {
                // TODO(pcwalton): Real numbers.
                while (try!(reader.read_u8().map_err(Error::eof)) & 0xf) != 0xf {}
            }
            12 => {
                let b1 = try!(reader.read_u8().map_err(Error::eof));
                if operator == (((b1 as u16) << 8) | (b0 as u16)) {
                    match last_integer_operand {
                        Some(last_integer_operand) => return Ok(last_integer_operand),
                        None => return Err(Error::CffIntegerNotFound),
                    }
                }
                last_integer_operand = None
            }
            _ => {
                if operator == b0 as u16 {
                    match last_integer_operand {
                        Some(last_integer_operand) => return Ok(last_integer_operand),
                        None => return Err(Error::CffIntegerNotFound),
                    }
                }
                last_integer_operand = None
            }
        }
    }
}

// Reads an Offset with the given size.
fn read_offset(reader: &mut &[u8], size: u8) -> Result<u32, Error> {
    match size {
        1 => Ok(try!(reader.read_u8().map_err(Error::eof)) as u32),
        2 => Ok(try!(reader.read_u16::<BigEndian>().map_err(Error::eof)) as u32),
        3 => {
            let hi = try!(reader.read_u8().map_err(Error::eof)) as u32;
            let lo = try!(reader.read_u16::<BigEndian>().map_err(Error::eof)) as u32;
            Ok((hi << 16) | lo)
        }
        4 => Ok(try!(reader.read_u32::<BigEndian>().map_err(Error::eof))),
        _ => Err(Error::CffBadOffset),
    }
}

// The CFF evaluation stack used during CharString reading.
struct EvaluationStack {
    array: [i32; 48],
    size: u8,
}

impl EvaluationStack {
    fn new() -> EvaluationStack {
        EvaluationStack {
            array: [0; 48],
            size: 0,
        }
    }

    fn push(&mut self, value: i32) -> Result<(), Error> {
        if (self.size as usize) < self.array.len() {
            self.array[self.size as usize] = value;
            self.size += 1;
            Ok(())
        } else {
            Err(Error::CffStackOverflow)
        }
    }

    fn clear(&mut self) {
        self.size = 0
    }
}

fn close_path_if_necessary<F>(pos: &mut Point2D<i16>,
                              start: &Point2D<i16>,
                              index_in_contour: u16,
                              mut callback: F)
                              where F: FnMut(&Point) {
    if index_in_contour == 0 {
        // No path to close.
        return
    }

    callback(&Point {
        position: *start,
        index_in_contour: index_in_contour,
        kind: PointKind::OnCurve,
    });
}

fn process_hvcurveto_h<F>(chunk: &[i32],
                          pos: &mut Point2D<i16>,
                          dxf: i32,
                          index_in_contour: &mut u16,
                          mut callback: F)
                          where F: FnMut(&Point) {
    pos.x += chunk[0] as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 0,
        kind: PointKind::FirstCubicControl,
    });

    pos.x += chunk[1] as i16;
    pos.y += chunk[2] as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 1,
        kind: PointKind::SecondCubicControl,
    });

    pos.x += dxf as i16;
    pos.y += chunk[3] as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 2,
        kind: PointKind::OnCurve,
    });

    *index_in_contour += 3
}

fn process_hvcurveto_v<F>(chunk: &[i32],
                          pos: &mut Point2D<i16>,
                          dyf: i32,
                          index_in_contour: &mut u16,
                          mut callback: F)
                          where F: FnMut(&Point) {
    pos.y += chunk[0] as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 0,
        kind: PointKind::FirstCubicControl,
    });

    pos.x += chunk[1] as i16;
    pos.y += chunk[2] as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 1,
        kind: PointKind::SecondCubicControl,
    });

    pos.x += chunk[3] as i16;
    pos.y += dyf as i16;
    callback(&Point {
        position: *pos,
        index_in_contour: *index_in_contour + 2,
        kind: PointKind::OnCurve,
    });

    *index_in_contour += 3
}

