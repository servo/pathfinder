// pathfinder/partitioner/src/indexed_path.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::Point2D;
use lyon_path::PathEvent;
use lyon_path::iterator::PathIterator;
use std::num::Wrapping;
use std::ops::Range;

pub struct IndexedPath {
    pub endpoints: Vec<IndexedEndpoint>,
    pub subpath_ranges: Vec<Range<u32>>,
}

impl IndexedPath {
    #[inline]
    pub fn new() -> IndexedPath {
        IndexedPath {
            endpoints: vec![],
            subpath_ranges: vec![],
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.endpoints.clear();
        self.subpath_ranges.clear();
    }

    pub fn add_monotonic_path<I>(&mut self, mut path: I) where I: PathIterator {
        let mut subpath_index = Wrapping(self.subpath_ranges.len() as u32);
        let mut first_endpoint_index_of_subpath = self.endpoints.len() as u32;

        while let Some(event) = path.next() {
            match event {
                PathEvent::MoveTo(_) => {
                    let last_endpoint_index = self.endpoints.len() as u32;
                    if first_endpoint_index_of_subpath < last_endpoint_index {
                        subpath_index += Wrapping(1);

                        self.subpath_ranges
                            .push(first_endpoint_index_of_subpath..last_endpoint_index);
                        first_endpoint_index_of_subpath = last_endpoint_index;
                    }
                }
                PathEvent::LineTo(to) => {
                    self.endpoints.push(IndexedEndpoint {
                        to: to,
                        ctrl: None,
                        subpath_index: subpath_index.0,
                    })
                }
                PathEvent::QuadraticTo(ctrl, to) => {
                    self.endpoints.push(IndexedEndpoint {
                        to: to,
                        ctrl: Some(ctrl),
                        subpath_index: subpath_index.0,
                    })
                }
                PathEvent::CubicTo(..) | PathEvent::Arc(..) => {
                    panic!("Convert cubics and arcs to quadratics first!")
                }
                PathEvent::Close => {
                    let state = path.get_state();
                    if state.first != state.current {
                        self.endpoints.push(IndexedEndpoint {
                            to: state.first,
                            ctrl: None,
                            subpath_index: subpath_index.0,
                        })
                    }
                }
            }
        }

        let last_endpoint_index = self.endpoints.len() as u32;
        self.subpath_ranges.push(first_endpoint_index_of_subpath..last_endpoint_index);

        println!("{:#?}", self.endpoints);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct IndexedEndpoint {
    pub to: Point2D<f32>,
    pub ctrl: Option<Point2D<f32>>,
    pub subpath_index: u32,
}
