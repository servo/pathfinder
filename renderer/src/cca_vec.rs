// pathfinder/renderer/src/cca_vec.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Concurrent Copyable Array Vectors: fixed-capacity buffers of POD data that
//! can be mutably accessed from multiple threads at a time.
//!
//! It is the user's responsibility to ensure proper synchronization. However,
//! it should be impossible to get memory safety violations from use of this
//! type.

use std::cell::UnsafeCell;
use std::mem;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {
    data: Vec<UnsafeCell<T>>,
    len: AtomicUsize,
}

impl<T> ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {
    pub fn new(capacity: u32) -> ConcurrentCopyableArrayVec<T> {
        unsafe {
            ConcurrentCopyableArrayVec {
                data: (0..capacity).map(|_| UnsafeCell::new(mem::uninitialized())).collect(),
                len: AtomicUsize::new(0),
            }
        }
    }

    #[inline]
    pub fn get(&self, index: u32) -> T {
        unsafe {
            *(self.data[index as usize].get())
        }
    }

    #[inline]
    pub fn set(&self, index: u32, element: T) {
        unsafe {
            let ptr = self.data[index as usize].get();
            *ptr = element;
        }
    }

    #[inline]
    pub fn push(&self, element: T) -> u32 {
        let index = self.len.fetch_add(1, Ordering::SeqCst) as u32;
        self.set(index, element);
        index
    }

    #[inline]
    pub fn clear(&self) {
        self.len.store(0, Ordering::SeqCst);
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.len.load(Ordering::SeqCst) as u32
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    unsafe fn as_slice(&self) -> &[T] {
        mem::transmute::<&[UnsafeCell<T>], &[T]>(&self.data[0..(self.len() as usize)])
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<T> {
        unsafe {
            self.as_slice().to_vec()
        }
    }

    #[inline]
    pub fn range_to_vec(&self, range: Range<u32>) -> Vec<T> {
        unsafe {
            self.as_slice()[(range.start as usize)..(range.end as usize)].to_vec()
        }
    }
}

unsafe impl<T> Sync for ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {}
