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
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {
    data: Vec<UnsafeCell<T>>,
    allocated_len: AtomicUsize,
    committed_len: AtomicUsize,
}

impl<T> ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {
    pub fn new(capacity: u32) -> ConcurrentCopyableArrayVec<T> {
        unsafe {
            ConcurrentCopyableArrayVec {
                data: (0..capacity).map(|_| UnsafeCell::new(mem::uninitialized())).collect(),
                allocated_len: AtomicUsize::new(0),
                committed_len: AtomicUsize::new(0),
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

    /// Once this method returns, it is guaranteed that all elements prior to
    /// the pushed element are visible to this thread.
    #[inline]
    pub fn push(&self, element: T) -> u32 {
        let index = self.allocated_len.fetch_add(1, Ordering::SeqCst);
        self.set(index as u32, element);
        while self.committed_len.compare_exchange(index,
                                                  index + 1,
                                                  Ordering::Release,
                                                  Ordering::Relaxed).is_err() {}
        index as u32
    }

    #[inline]
    pub fn clear(&self) {
        self.committed_len.store(0, Ordering::Relaxed);
        self.allocated_len.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn allocated_len(&self) -> u32 {
        self.allocated_len.load(Ordering::Relaxed) as u32
    }

    #[inline]
    pub fn committed_len(&self) -> u32 {
        self.committed_len.load(Ordering::Relaxed) as u32
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.allocated_len() == 0
    }

    #[inline]
    pub fn range_to_vec(&self, range: Range<u32>) -> Vec<T> {
        unsafe {
            assert!(range.start <= range.end);
            assert!((range.end as usize) <= self.data.len());
            let count = (range.end - range.start) as usize;
            let mut result: Vec<T> = Vec::with_capacity(count);
            let src = &self.data[range.start as usize] as *const UnsafeCell<T> as *const T;
            ptr::copy_nonoverlapping(src, result.as_mut_ptr(), count);
            result.set_len(count);
            result
        }
    }

    #[inline]
    pub fn to_vec(&self) -> Vec<T> {
        self.range_to_vec(0..(self.committed_len()))
    }
}

unsafe impl<T> Sync for ConcurrentCopyableArrayVec<T> where T: Copy + Default + Sync {}
