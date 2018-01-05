// pathfinder/path-utils/src/svg.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for converting paths to SVG representations.

use std::io::{self, Write};

use PathCommand;

/// Writes a textual representation of the path `stream` to the given `Writer` in SVG `path` form.
pub fn to_svg_description<W, S>(output: &mut W, stream: S) -> io::Result<()>
                                where W: Write, S: Iterator<Item = PathCommand> {
    for segment in stream {
        match segment {
            PathCommand::MoveTo(point) => try!(write!(output, "M{},{} ", point.x, point.y)),
            PathCommand::LineTo(point) => try!(write!(output, "L{},{} ", point.x, point.y)),
            PathCommand::CurveTo(control_point, endpoint) => {
                try!(write!(output, "Q{},{} {},{} ",
                            control_point.x, control_point.y,
                            endpoint.x, endpoint.y))
            }
            PathCommand::ClosePath => try!(output.write_all(b"z")),
        }
    }
    Ok(())
}
