// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A faster version of `Seek` that supports only forward motion from the current position.
pub trait Jump {
    /// Moves the pointer forward `n` bytes from the *current* position.
    fn jump(&mut self, n: usize) -> Result<(), ()>;
}

impl<'a> Jump for &'a [u8] {
    #[inline]
    fn jump(&mut self, n: usize) -> Result<(), ()> {
        if n <= self.len() {
            *self = &(*self)[n..];
            Ok(())
        } else {
            Err(())
        }
    }
}

