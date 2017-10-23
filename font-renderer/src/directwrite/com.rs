// pathfinder/font-renderer/src/directwrite/com.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utility types for Microsoft COM.

use std::mem;
use std::ops::Deref;
use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use uuid;
use winapi::{E_NOINTERFACE, E_POINTER, GUID, HRESULT, IUnknown, REFIID, S_OK, ULONG};

pub struct PathfinderComPtr<T> {
    ptr: *mut T,
}

impl<T> PathfinderComPtr<T> {
    #[inline]
    pub unsafe fn new(ptr: *mut T) -> PathfinderComPtr<T> {
        PathfinderComPtr {
            ptr: ptr,
        }
    }

    #[inline]
    pub fn into_raw(self) -> *mut T {
        let ptr = self.ptr;
        mem::forget(self);
        ptr
    }
}

impl<T> Clone for PathfinderComPtr<T> {
    #[inline]
    fn clone(&self) -> PathfinderComPtr<T> {
        unsafe {
            (*(self.ptr as *mut IUnknown)).AddRef();
        }
        PathfinderComPtr {
            ptr: self.ptr,
        }
    }
}

impl<T> Drop for PathfinderComPtr<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            (*(self.ptr as *mut IUnknown)).Release();
        }
    }
}

impl<T> Deref for PathfinderComPtr<T> {
    type Target = *mut T;
    #[inline]
    fn deref(&self) -> &*mut T {
        &self.ptr
    }
}

pub trait PathfinderCoclass {
    type InterfaceVtable: 'static;
    fn interface_guid() -> &'static GUID;
    fn vtable() -> &'static Self::InterfaceVtable;
}

#[repr(C)]
pub struct PathfinderComObject<DerivedClass> where DerivedClass: PathfinderCoclass {
    vtable: &'static DerivedClass::InterfaceVtable,
    ref_count: AtomicUsize,
}

impl<DerivedClass> PathfinderComObject<DerivedClass> where DerivedClass: PathfinderCoclass {
    #[inline]
    pub unsafe fn construct() -> PathfinderComObject<DerivedClass> {
        PathfinderComObject {
            vtable: DerivedClass::vtable(),
            ref_count: AtomicUsize::new(1),
        }
    }

    pub unsafe extern "system" fn AddRef(this: *mut IUnknown) -> ULONG {
        let this = this as *mut PathfinderComObject<DerivedClass>;
        ((*this).ref_count.fetch_add(1, Ordering::SeqCst) + 1) as ULONG
    }

    pub unsafe extern "system" fn Release(this: *mut IUnknown) -> ULONG {
        let this = this as *mut PathfinderComObject<DerivedClass>;
        let new_ref_count = (*this).ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
        if new_ref_count == 0 {
            drop(Box::from_raw(this))
        }
        new_ref_count as ULONG
    }

    pub unsafe extern "system" fn QueryInterface(this: *mut IUnknown,
                                                 riid: REFIID,
                                                 object: *mut *mut c_void)
                                                 -> HRESULT {
        if object.is_null() {
            return E_POINTER
        }
        if guids_are_equal(&*riid, &uuid::IID_IUnknown) ||
                guids_are_equal(&*riid, DerivedClass::interface_guid()) {
            *object = this as *mut c_void;
            return S_OK
        }
        *object = ptr::null_mut();
        E_NOINTERFACE
    }
}

fn guids_are_equal(a: &GUID, b: &GUID) -> bool {
    a.Data1 == b.Data1 && a.Data2 == b.Data2 && a.Data3 == b.Data3 && a.Data4 == b.Data4
}