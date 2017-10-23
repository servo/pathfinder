// pathfinder/font-renderer/src/directwrite/mod.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_snake_case, non_upper_case_globals)]

use dwrite;
use euclid::{Point2D, Size2D};
use kernel32;
use pathfinder_path_utils::PathCommand;
use pathfinder_path_utils::cubic::{CubicPathCommand, CubicPathCommandApproxStream};
use std::collections::BTreeMap;
use std::mem;
use std::os::raw::c_void;
use std::ptr;
use std::slice;
use std::sync::Arc;
use uuid::IID_ID2D1SimplifiedGeometrySink;
use winapi::winerror::{self, S_OK};
use winapi::{self, BOOL, D2D1_BEZIER_SEGMENT, D2D1_FIGURE_BEGIN, D2D1_FIGURE_END};
use winapi::{D2D1_FIGURE_END_CLOSED, D2D1_FILL_MODE, D2D1_PATH_SEGMENT, D2D1_POINT_2F};
use winapi::{DWRITE_FONT_METRICS, DWRITE_GLYPH_METRICS, E_BOUNDS, E_INVALIDARG, FALSE, FILETIME};
use winapi::{FLOAT, GUID, HRESULT, ID2D1SimplifiedGeometrySinkVtbl, IDWriteFactory};
use winapi::{IDWriteFontCollectionLoader, IDWriteFontCollectionLoaderVtbl, IDWriteFontFace};
use winapi::{IDWriteFontFile, IDWriteFontFileEnumerator, IDWriteFontFileEnumeratorVtbl};
use winapi::{IDWriteFontFileLoader, IDWriteFontFileLoaderVtbl, IDWriteFontFileStream};
use winapi::{IDWriteFontFileStreamVtbl, IDWriteGdiInterop, IDWriteGeometrySink, IUnknown};
use winapi::{IUnknownVtbl, TRUE, UINT16, UINT32, UINT64, UINT};

use self::com::{PathfinderCoclass, PathfinderComObject, PathfinderComPtr};
use {FontInstanceKey, FontKey, GlyphDimensions, GlyphKey};

mod com;

DEFINE_GUID! {
    IID_IDWriteFactory, 0xb859ee5a, 0xd838, 0x4b5b, 0xa2, 0xe8, 0x1a, 0xdc, 0x7d, 0x93, 0xdb, 0x48
}
DEFINE_GUID! {
    IID_IDWriteFontCollectionLoader,
    0xcca920e4, 0x52f0, 0x492b, 0xbf, 0xa8, 0x29, 0xc7, 0x2e, 0xe0, 0xa4, 0x68
}
DEFINE_GUID! {
    IID_IDWriteFontFileEnumerator,
    0x72755049, 0x5ff7, 0x435d, 0x83, 0x48, 0x4b, 0xe9, 0x7c, 0xfa, 0x6c, 0x7c
}
DEFINE_GUID! {
    IID_IDWriteFontFileLoader,
    0x727cad4e, 0xd6af, 0x4c9e, 0x8a, 0x08, 0xd6, 0x95, 0xb1, 0x1c, 0xaa, 0x49
}
DEFINE_GUID! {
    IID_IDWriteFontFileStream,
    0x6d4865fe, 0x0ab8, 0x4d91, 0x8f, 0x62, 0x5d, 0xd6, 0xbe, 0x34, 0xa3, 0xe0
}

pub type GlyphOutline = Vec<PathCommand>;

const CURVE_APPROX_ERROR_BOUND: f32 = 0.1;

static PATHFINDER_FONT_COLLECTION_KEY: [u8; 17] = *b"MEMORY_COLLECTION";
static PATHFINDER_FONT_FILE_KEY: [u8; 11] = *b"MEMORY_FILE";

pub struct FontContext {
    dwrite_factory: PathfinderComPtr<IDWriteFactory>,
    #[allow(unused)]
    dwrite_gdi_interop: PathfinderComPtr<IDWriteGdiInterop>,
    dwrite_font_faces: BTreeMap<FontKey, PathfinderComPtr<IDWriteFontFace>>,
}

impl FontContext {
    pub fn new() -> Result<FontContext, ()> {
        unsafe {
            let mut factory: *mut IDWriteFactory = ptr::null_mut();
            if !winerror::SUCCEEDED(dwrite::DWriteCreateFactory(winapi::DWRITE_FACTORY_TYPE_SHARED,
                                                                &IID_IDWriteFactory,
                                                                &mut factory as *mut *mut _ as
                                                                *mut *mut IUnknown)) {
                return Err(())
            }
            let factory = PathfinderComPtr::new(factory);

            let mut gdi_interop: *mut IDWriteGdiInterop = ptr::null_mut();
            if !winerror::SUCCEEDED((**factory).GetGdiInterop(&mut gdi_interop)) {
                return Err(())
            }
            let gdi_interop = PathfinderComPtr::new(gdi_interop);

            Ok(FontContext {
                dwrite_factory: factory,
                dwrite_gdi_interop: gdi_interop,
                dwrite_font_faces: BTreeMap::new(),
            })
        }
    }

    pub fn add_font_from_memory(&mut self, font_key: &FontKey, bytes: Arc<Vec<u8>>, _: u32)
                                -> Result<(), ()> {
        unsafe {
            let font_file_loader = PathfinderFontFileLoader::new(bytes.clone());

            let result = (**self.dwrite_factory).RegisterFontFileLoader(
                font_file_loader.clone().into_raw() as *mut IDWriteFontFileLoader);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }

            let mut font_file = ptr::null_mut();
            let result = (**self.dwrite_factory).CreateCustomFontFileReference(
                PATHFINDER_FONT_FILE_KEY.as_ptr() as *const c_void,
                PATHFINDER_FONT_FILE_KEY.len() as UINT,
                font_file_loader.clone().into_raw() as *mut IDWriteFontFileLoader,
                &mut font_file);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_file = PathfinderComPtr::new(font_file);

            let font_collection_loader = PathfinderFontCollectionLoader::new(font_file);

            let result = (**self.dwrite_factory).RegisterFontCollectionLoader(
                font_collection_loader.clone().into_raw() as *mut IDWriteFontCollectionLoader);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }

            let mut font_collection = ptr::null_mut();
            let result = (**self.dwrite_factory).CreateCustomFontCollection(
                font_collection_loader.clone().into_raw() as *mut IDWriteFontCollectionLoader,
                PATHFINDER_FONT_COLLECTION_KEY.as_ptr() as *const c_void,
                PATHFINDER_FONT_COLLECTION_KEY.len() as UINT32,
                &mut font_collection);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_collection = PathfinderComPtr::new(font_collection);

            let mut font_family = ptr::null_mut();
            let result = (**font_collection).GetFontFamily(0, &mut font_family);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_family = PathfinderComPtr::new(font_family);

            let mut font = ptr::null_mut();
            let result = (**font_family).GetFont(0, &mut font);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font = PathfinderComPtr::new(font);

            let mut font_face = ptr::null_mut();
            let result = (**font).CreateFontFace(&mut font_face);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }
            let font_face = PathfinderComPtr::new(font_face);

            let result = (**self.dwrite_factory).UnregisterFontCollectionLoader(
                font_collection_loader.into_raw() as *mut IDWriteFontCollectionLoader);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }

            let result = (**self.dwrite_factory).UnregisterFontFileLoader(
                font_file_loader.into_raw() as *mut IDWriteFontFileLoader);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }

            self.dwrite_font_faces.insert(*font_key, font_face);
            Ok(())
        }
    }

    #[inline]
    pub fn delete_font(&mut self, font_key: &FontKey) {
        self.dwrite_font_faces.remove(font_key);
    }

    pub fn glyph_dimensions(&self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                            -> Option<GlyphDimensions> {
        unsafe {
            let font_face = match self.dwrite_font_faces.get(&font_instance.font_key) {
                None => return None,
                Some(font_face) => (*font_face).clone(),
            };

            let glyph_index = glyph_key.glyph_index as UINT16;
            let mut metrics: DWRITE_GLYPH_METRICS = mem::zeroed();

            let result = (**font_face).GetDesignGlyphMetrics(&glyph_index, 1, &mut metrics, FALSE);
            if !winerror::SUCCEEDED(result) {
                return None
            }

            Some(GlyphDimensions {
                advance: metrics.advanceWidth as f32,
                origin: Point2D::new(metrics.leftSideBearing, metrics.bottomSideBearing),
                size: Size2D::new((metrics.rightSideBearing - metrics.leftSideBearing) as u32,
                                  (metrics.topSideBearing - metrics.bottomSideBearing) as u32),
            })
        }
    }

    pub fn glyph_outline(&mut self, font_instance: &FontInstanceKey, glyph_key: &GlyphKey)
                         -> Result<GlyphOutline, ()> {
        unsafe {
            let font_face = match self.dwrite_font_faces.get(&font_instance.font_key) {
                None => return Err(()),
                Some(font_face) => (*font_face).clone(),
            };

            let mut metrics: DWRITE_FONT_METRICS = mem::zeroed();
            (**font_face).GetMetrics(&mut metrics);

            let geometry_sink = PathfinderGeometrySink::new();
            let glyph_index = glyph_key.glyph_index as UINT16;

            let result =
                (**font_face).GetGlyphRunOutline(metrics.designUnitsPerEm as FLOAT,    
                                                 &glyph_index,
                                                 ptr::null(),
                                                 ptr::null(),
                                                 1,
                                                 FALSE,
                                                 FALSE,
                                                 *geometry_sink as *mut IDWriteGeometrySink);
            if !winerror::SUCCEEDED(result) {
                return Err(())
            }

            let approx_stream =
                CubicPathCommandApproxStream::new((**geometry_sink).commands.iter().cloned(),
                                                  CURVE_APPROX_ERROR_BOUND);

            let approx_commands: Vec<_> = approx_stream.collect();
            Ok(approx_commands)
        }
    }
}

#[repr(C)]
struct PathfinderFontCollectionLoader {
    object: PathfinderComObject<PathfinderFontCollectionLoader>,
    font_file: PathfinderComPtr<IDWriteFontFile>,
}

static PATHFINDER_FONT_COLLECTION_LOADER_VTABLE:
       IDWriteFontCollectionLoaderVtbl = IDWriteFontCollectionLoaderVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontCollectionLoader>::AddRef,
        Release: PathfinderComObject::<PathfinderFontCollectionLoader>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontCollectionLoader>::QueryInterface,
    },
    CreateEnumeratorFromKey: PathfinderFontCollectionLoader::CreateEnumeratorFromKey,
};

impl PathfinderCoclass for PathfinderFontCollectionLoader {
    type InterfaceVtable = IDWriteFontCollectionLoaderVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontCollectionLoader }
    fn vtable() -> &'static IDWriteFontCollectionLoaderVtbl {
        &PATHFINDER_FONT_COLLECTION_LOADER_VTABLE
    }
}

impl PathfinderFontCollectionLoader {
    #[inline]
    fn new(font_file: PathfinderComPtr<IDWriteFontFile>)
           -> PathfinderComPtr<PathfinderFontCollectionLoader> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontCollectionLoader {
                object: PathfinderComObject::construct(),
                font_file: font_file,
            })))
        }
    }

    unsafe extern "system" fn CreateEnumeratorFromKey(
            this: *mut IDWriteFontCollectionLoader,
            factory: *mut IDWriteFactory,
            _: *const c_void,
            _: UINT32,
            font_file_enumerator: *mut *mut IDWriteFontFileEnumerator)
            -> HRESULT {
        let this = this as *mut PathfinderFontCollectionLoader;

        let factory = PathfinderComPtr::new(factory);
        let font_file = (*this).font_file.clone();
        let new_font_file_enumerator = PathfinderFontFileEnumerator::new(factory, font_file);

        *font_file_enumerator = new_font_file_enumerator.into_raw() as
            *mut IDWriteFontFileEnumerator;
        S_OK
    }
}

#[repr(C)]
struct PathfinderFontFileEnumerator {
    object: PathfinderComObject<PathfinderFontFileEnumerator>,
    factory: PathfinderComPtr<IDWriteFactory>,
    font_file: PathfinderComPtr<IDWriteFontFile>,
    state: PathfinderFontFileEnumeratorState,
}

static PATHFINDER_FONT_FILE_ENUMERATOR_VTABLE:
       IDWriteFontFileEnumeratorVtbl = IDWriteFontFileEnumeratorVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileEnumerator>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileEnumerator>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileEnumerator>::QueryInterface,
    },
    GetCurrentFontFile: PathfinderFontFileEnumerator::GetCurrentFontFile,
    MoveNext: PathfinderFontFileEnumerator::MoveNext,
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum PathfinderFontFileEnumeratorState {
    Start,
    AtFontFile,
    End,
}

impl PathfinderCoclass for PathfinderFontFileEnumerator {
    type InterfaceVtable = IDWriteFontFileEnumeratorVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileEnumerator }
    fn vtable() -> &'static IDWriteFontFileEnumeratorVtbl {
        &PATHFINDER_FONT_FILE_ENUMERATOR_VTABLE
    }
}

impl PathfinderFontFileEnumerator {
    #[inline]
    fn new(factory: PathfinderComPtr<IDWriteFactory>, font_file: PathfinderComPtr<IDWriteFontFile>)
           -> PathfinderComPtr<PathfinderFontFileEnumerator> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileEnumerator {
                object: PathfinderComObject::construct(),
                factory: factory,
                font_file: font_file,
                state: PathfinderFontFileEnumeratorState::Start,
            })))
        }
    }

    unsafe extern "system" fn GetCurrentFontFile(this: *mut IDWriteFontFileEnumerator,
                                                 font_file: *mut *mut IDWriteFontFile)
                                                 -> HRESULT {
        let this = this as *mut PathfinderFontFileEnumerator;
        if (*this).state != PathfinderFontFileEnumeratorState::AtFontFile {
            *font_file = ptr::null_mut();
            return E_BOUNDS
        }

        *font_file = (*this).font_file.clone().into_raw();
        S_OK
    }

    unsafe extern "system" fn MoveNext(this: *mut IDWriteFontFileEnumerator,
                                       has_current_file: *mut BOOL)
                                       -> HRESULT {
        let this = this as *mut PathfinderFontFileEnumerator;
        match (*this).state {
            PathfinderFontFileEnumeratorState::Start => {
                (*this).state = PathfinderFontFileEnumeratorState::AtFontFile;
                *has_current_file = TRUE;
            }
            PathfinderFontFileEnumeratorState::AtFontFile => {
                (*this).state = PathfinderFontFileEnumeratorState::End;
                *has_current_file = FALSE;
            }
            PathfinderFontFileEnumeratorState::End => *has_current_file = FALSE,
        }
        S_OK
    }
}

#[repr(C)]
struct PathfinderFontFileLoader {
    object: PathfinderComObject<PathfinderFontFileLoader>,
    buffer: Arc<Vec<u8>>,
}

static PATHFINDER_FONT_FILE_LOADER_VTABLE: IDWriteFontFileLoaderVtbl = IDWriteFontFileLoaderVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileLoader>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileLoader>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileLoader>::QueryInterface,
    },
    CreateStreamFromKey: PathfinderFontFileLoader::CreateStreamFromKey,
};

impl PathfinderCoclass for PathfinderFontFileLoader {
    type InterfaceVtable = IDWriteFontFileLoaderVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileLoader }
    fn vtable() -> &'static IDWriteFontFileLoaderVtbl { &PATHFINDER_FONT_FILE_LOADER_VTABLE }
}

impl PathfinderFontFileLoader {
    #[inline]
    fn new(buffer: Arc<Vec<u8>>) -> PathfinderComPtr<PathfinderFontFileLoader> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileLoader {
                object: PathfinderComObject::construct(),
                buffer: buffer,
            })))
        }
    }

    unsafe extern "system" fn CreateStreamFromKey(
            this: *mut IDWriteFontFileLoader,
            font_file_reference_key: *const c_void,
            font_file_reference_key_size: UINT32,
            font_file_stream: *mut *mut IDWriteFontFileStream)
            -> HRESULT {
        let this = this as *mut PathfinderFontFileLoader;
        let font_file_reference = slice::from_raw_parts(font_file_reference_key as *const u8,
                                                        font_file_reference_key_size as usize);
        if font_file_reference != PATHFINDER_FONT_FILE_KEY {
            *font_file_stream = ptr::null_mut();
            return E_INVALIDARG
        }

        *font_file_stream = PathfinderFontFileStream::new((*this).buffer.clone()).into_raw() as
            *mut IDWriteFontFileStream;
        S_OK
    }
}

#[repr(C)]
struct PathfinderFontFileStream {
    object: PathfinderComObject<PathfinderFontFileStream>,
    buffer: Arc<Vec<u8>>,
    creation_time: UINT64,
}

static PATHFINDER_FONT_FILE_STREAM_VTABLE: IDWriteFontFileStreamVtbl = IDWriteFontFileStreamVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderFontFileStream>::AddRef,
        Release: PathfinderComObject::<PathfinderFontFileStream>::Release,
        QueryInterface: PathfinderComObject::<PathfinderFontFileStream>::QueryInterface,
    },
    GetFileSize: PathfinderFontFileStream::GetFileSize,
    GetLastWriteTime: PathfinderFontFileStream::GetLastWriteTime,
    ReadFileFragment: PathfinderFontFileStream::ReadFileFragment,
    ReleaseFileFragment: PathfinderFontFileStream::ReleaseFileFragment,
};

impl PathfinderCoclass for PathfinderFontFileStream {
    type InterfaceVtable = IDWriteFontFileStreamVtbl;
    fn interface_guid() -> &'static GUID { &IID_IDWriteFontFileStream }
    fn vtable() -> &'static IDWriteFontFileStreamVtbl { &PATHFINDER_FONT_FILE_STREAM_VTABLE }
}

impl PathfinderFontFileStream {
    #[inline]
    fn new(buffer: Arc<Vec<u8>>) -> PathfinderComPtr<PathfinderFontFileStream> {
        unsafe {
            let mut now = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            kernel32::GetSystemTimeAsFileTime(&mut now);

            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderFontFileStream {
                object: PathfinderComObject::construct(),
                buffer: buffer,
                creation_time: ((now.dwHighDateTime as UINT64) << 32) |
                    (now.dwLowDateTime as UINT64),
            })))
        }
    }

    unsafe extern "system" fn GetFileSize(this: *mut IDWriteFontFileStream, file_size: *mut UINT64)
                                          -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        *file_size = (*this).buffer.len() as UINT64;
        S_OK
    }

    unsafe extern "system" fn GetLastWriteTime(this: *mut IDWriteFontFileStream,
                                               last_write_time: *mut UINT64)
                                               -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        *last_write_time = (*this).creation_time;
        S_OK
    }

    unsafe extern "system" fn ReadFileFragment(this: *mut IDWriteFontFileStream,
                                               fragment_start: *mut *const c_void,
                                               file_offset: UINT64,
                                               fragment_size: UINT64,
                                               fragment_context: *mut *mut c_void)
                                               -> HRESULT {
        let this = this as *mut PathfinderFontFileStream;
        let buffer_length = (*this).buffer.len() as u64;
        if file_offset > buffer_length || file_offset + fragment_size > buffer_length {
            return E_BOUNDS
        }

        let ptr = (*(*this).buffer).as_ptr().offset(file_offset as isize) as *const c_void;
        *fragment_start = ptr;
        *fragment_context = ptr as *mut c_void;
        (*(this as *mut IUnknown)).AddRef();
        S_OK
    }

    unsafe extern "system" fn ReleaseFileFragment(this: *mut IDWriteFontFileStream,
                                                  _: *mut c_void) {
        let this = this as *mut PathfinderFontFileStream;
        (*(this as *mut IUnknown)).Release();
    }
}

#[repr(C)]
struct PathfinderGeometrySink {
    object: PathfinderComObject<PathfinderGeometrySink>,
    commands: Vec<CubicPathCommand>,
}

static PATHFINDER_GEOMETRY_SINK_VTABLE: ID2D1SimplifiedGeometrySinkVtbl =
        ID2D1SimplifiedGeometrySinkVtbl {
    parent: IUnknownVtbl {
        AddRef: PathfinderComObject::<PathfinderGeometrySink>::AddRef,
        Release: PathfinderComObject::<PathfinderGeometrySink>::Release,
        QueryInterface: PathfinderComObject::<PathfinderGeometrySink>::QueryInterface,
    },
    AddBeziers: PathfinderGeometrySink::AddBeziers,
    AddLines: PathfinderGeometrySink::AddLines,
    BeginFigure: PathfinderGeometrySink::BeginFigure,
    Close: PathfinderGeometrySink::Close,
    EndFigure: PathfinderGeometrySink::EndFigure,
    SetFillMode: PathfinderGeometrySink::SetFillMode,
    SetSegmentFlags: PathfinderGeometrySink::SetSegmentFlags,
};

impl PathfinderCoclass for PathfinderGeometrySink {
    type InterfaceVtable = ID2D1SimplifiedGeometrySinkVtbl;
    fn interface_guid() -> &'static GUID { unsafe { &IID_ID2D1SimplifiedGeometrySink } }
    fn vtable() -> &'static ID2D1SimplifiedGeometrySinkVtbl { &PATHFINDER_GEOMETRY_SINK_VTABLE }
}

impl PathfinderGeometrySink {
    #[inline]
    fn new() -> PathfinderComPtr<PathfinderGeometrySink> {
        unsafe {
            PathfinderComPtr::new(Box::into_raw(Box::new(PathfinderGeometrySink {
                object: PathfinderComObject::construct(),
                commands: vec![],
            })))
        }
    }

    unsafe extern "system" fn AddBeziers(this: *mut IDWriteGeometrySink,
                                         beziers: *const D2D1_BEZIER_SEGMENT,
                                         beziers_count: UINT) {
        let this = this as *mut PathfinderGeometrySink;
        let beziers = slice::from_raw_parts(beziers, beziers_count as usize);
        for bezier in beziers {
            let control_point_0 =
                PathfinderGeometrySink::d2d_point_2f_to_flipped_f32_point(&bezier.point1);
            let control_point_1 =
                PathfinderGeometrySink::d2d_point_2f_to_flipped_f32_point(&bezier.point2);
            let endpoint =
                PathfinderGeometrySink::d2d_point_2f_to_flipped_f32_point(&bezier.point3);
            (*this).commands.push(CubicPathCommand::CubicCurveTo(control_point_0,
                                                                 control_point_1,
                                                                 endpoint))
        }
    }

    unsafe extern "system" fn AddLines(this: *mut IDWriteGeometrySink,
                                       points: *const D2D1_POINT_2F,
                                       points_count: UINT) {
        let this = this as *mut PathfinderGeometrySink;
        let points = slice::from_raw_parts(points, points_count as usize);
        for point in points {
            let point = PathfinderGeometrySink::d2d_point_2f_to_flipped_f32_point(&point);
            (*this).commands.push(CubicPathCommand::LineTo(point))
        }
    }

    unsafe extern "system" fn BeginFigure(this: *mut IDWriteGeometrySink,
                                          start_point: D2D1_POINT_2F,
                                          _: D2D1_FIGURE_BEGIN) {
        let this = this as *mut PathfinderGeometrySink;
        let start_point = PathfinderGeometrySink::d2d_point_2f_to_flipped_f32_point(&start_point);
        (*this).commands.push(CubicPathCommand::MoveTo(start_point))
    }

    unsafe extern "system" fn Close(_: *mut IDWriteGeometrySink) -> HRESULT {
        S_OK
    }

    unsafe extern "system" fn EndFigure(this: *mut IDWriteGeometrySink,
                                        figure_end: D2D1_FIGURE_END) {
        let this = this as *mut PathfinderGeometrySink;
        if figure_end == D2D1_FIGURE_END_CLOSED {
            (*this).commands.push(CubicPathCommand::ClosePath)
        }
    }

    unsafe extern "system" fn SetFillMode(_: *mut IDWriteGeometrySink, _: D2D1_FILL_MODE) {
        // TODO(pcwalton)
    }

    unsafe extern "system" fn SetSegmentFlags(_: *mut IDWriteGeometrySink, _: D2D1_PATH_SEGMENT) {
        // Should be unused.
    }

    #[inline]
    fn d2d_point_2f_to_flipped_f32_point(point: &D2D1_POINT_2F) -> Point2D<f32> {
        Point2D::new(point.x, -point.y)
    }
}
