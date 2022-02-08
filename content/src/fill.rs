// pathfinder/content/src/fill.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The fill rule, which determines how self-intersecting paths are filled.

/// The fill rule, which determines how self-intersecting paths are filled.
///
/// Paths that don't intersect themselves (and have no holes) are unaffected by the choice of fill
/// rule.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FillRule {
    /// The nonzero rule: <https://en.wikipedia.org/wiki/Nonzero-rule>
    Winding,
    /// The even-odd rule: <https://en.wikipedia.org/wiki/Even%E2%80%93odd_rule>
    EvenOdd,
}
