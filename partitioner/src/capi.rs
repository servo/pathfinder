// partitionfinder/capi.rs

use env_logger;
use euclid::Point2D;
use std::mem;
use std::slice;

use mesh_library::MeshLibrary;
use partitioner::Partitioner;
use {BQuad, BVertexLoopBlinnData, Endpoint, Subpath};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Point2DF32 {
    pub x: f32,
    pub y: f32,
}

impl Point2DF32 {
    #[inline]
    pub fn to_point2d(&self) -> Point2D<f32> {
        Point2D::new(self.x, self.y)
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Matrix2DF32 {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct CoverIndices {
    pub interior_indices: *const u32,
    pub interior_indices_len: u32,
    pub curve_indices: *const u32,
    pub curve_indices_len: u32,
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_new() -> *mut Partitioner<'static> {
    let mut partitioner = Box::new(Partitioner::new(MeshLibrary::new()));
    let partitioner_ptr: *mut Partitioner<'static> = &mut *partitioner;
    mem::forget(partitioner);
    partitioner_ptr
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_destroy<'a>(partitioner: *mut Partitioner<'a>) {
    drop(mem::transmute::<*mut Partitioner<'a>, Box<Partitioner>>(partitioner))
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_init<'a>(partitioner: *mut Partitioner<'a>,
                                             endpoints: *const Endpoint,
                                             endpoint_count: u32,
                                             control_points: *const Point2DF32,
                                             control_point_count: u32,
                                             subpaths: *const Subpath,
                                             subpath_count: u32) {
    (*partitioner).init_with_raw_data(slice::from_raw_parts(endpoints, endpoint_count as usize),
                                      slice::from_raw_parts(control_points as *const Point2D<f32>,
                                                            control_point_count as usize),
                                      slice::from_raw_parts(subpaths, subpath_count as usize))
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_partition<'a>(partitioner: *mut Partitioner<'a>,
                                                  path_id: u16,
                                                  first_subpath_index: u32,
                                                  last_subpath_index: u32) {
    (*partitioner).partition(path_id, first_subpath_index, last_subpath_index);
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_quads<'a>(partitioner: *const Partitioner<'a>,
                                                out_b_quad_count: *mut u32)
                                                -> *const BQuad {
    let b_quads = &(*partitioner).library().b_quads;
    if !out_b_quad_count.is_null() {
        *out_b_quad_count = b_quads.len() as u32
    }
    b_quads.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_vertex_positions<'a>(partitioner: *const Partitioner<'a>,
                                                           out_b_vertex_count: *mut u32)
                                                           -> *const Point2D<f32> {
    let b_vertex_positions = &(*partitioner).library().b_vertex_positions;
    if !out_b_vertex_count.is_null() {
        *out_b_vertex_count = b_vertex_positions.len() as u32
    }
    b_vertex_positions.as_ptr() as *const Point2D<f32>
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_vertex_loop_blinn_data<'a>(
        partitioner: *const Partitioner<'a>,
        out_b_vertex_count: *mut u32)
        -> *const BVertexLoopBlinnData {
    let b_vertex_loop_blinn_data = &(*partitioner).library().b_vertex_loop_blinn_data;
    if !out_b_vertex_count.is_null() {
        *out_b_vertex_count = b_vertex_loop_blinn_data.len() as u32
    }
    b_vertex_loop_blinn_data.as_ptr() as *const BVertexLoopBlinnData
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_cover_indices<'a>(partitioner: *const Partitioner<'a>,
                                                      out_cover_indices: *mut CoverIndices) {
    let cover_indices = &(*partitioner).library().cover_indices;
    (*out_cover_indices).interior_indices = cover_indices.interior_indices.as_ptr();
    (*out_cover_indices).interior_indices_len = cover_indices.interior_indices.len() as u32;
    (*out_cover_indices).curve_indices = cover_indices.curve_indices.as_ptr();
    (*out_cover_indices).curve_indices_len = cover_indices.curve_indices.len() as u32;
}

#[no_mangle]
pub unsafe extern fn pf_init_env_logger() -> u32 {
    env_logger::init().is_ok() as u32
}
