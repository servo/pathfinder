// partitionfinder/capi.rs

use env_logger;
use euclid::Transform2D;
use partitioner::Partitioner;
use tessellator::{QuadTessLevels, Tessellator};
use std::mem;
use std::slice;
use {AntialiasingMode, Bezieroid, ControlPoints, Endpoint, Subpath, Vertex};

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

#[no_mangle]
pub unsafe extern fn pf_partitioner_new() -> *mut Partitioner<'static> {
    let mut partitioner = Box::new(Partitioner::new());
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
                                             control_points: *const ControlPoints,
                                             control_points_count: u32,
                                             subpaths: *const Subpath,
                                             subpath_count: u32) {
    (*partitioner).init(slice::from_raw_parts(endpoints, endpoint_count as usize),
                        slice::from_raw_parts(control_points, control_points_count as usize),
                        slice::from_raw_parts(subpaths, subpath_count as usize))
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_partition<'a>(partitioner: *mut Partitioner<'a>,
                                                  first_subpath_index: u32,
                                                  last_subpath_index: u32) {
    (*partitioner).partition(first_subpath_index, last_subpath_index)
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_bezieroids<'a>(partitioner: *mut Partitioner<'a>,
                                                   out_bezieroid_count: *mut u32)
                                                   -> *const Bezieroid {
    let bezieroids = (*partitioner).bezieroids();
    if !out_bezieroid_count.is_null() {
        *out_bezieroid_count = bezieroids.len() as u32
    }
    bezieroids.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_new(endpoints: *const Endpoint,
                                        endpoint_count: u32,
                                        control_points: *const ControlPoints,
                                        control_points_count: u32,
                                        b_quads: *const Bezieroid,
                                        b_quad_count: u32,
                                        antialiasing_mode: AntialiasingMode)
                                        -> *mut Tessellator<'static> {
    let mut tessellator =
        Box::new(Tessellator::new(slice::from_raw_parts(endpoints, endpoint_count as usize),
                                  slice::from_raw_parts(control_points,
                                                        control_points_count as usize),
                                  slice::from_raw_parts(b_quads, b_quad_count as usize),
                                  antialiasing_mode));
    let tessellator_ptr: *mut Tessellator<'static> = &mut *tessellator;
    mem::forget(tessellator);
    tessellator_ptr
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_destroy<'a>(tessellator: *mut Tessellator<'a>) {
    drop(mem::transmute::<*mut Tessellator<'a>, Box<Tessellator>>(tessellator))
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_compute_hull<'a>(tessellator: *mut Tessellator<'a>,
                                                     transform: *const Matrix2DF32) {
    (*tessellator).compute_hull(&Transform2D::column_major((*transform).m00,
                                                           (*transform).m01,
                                                           (*transform).m02,
                                                           (*transform).m10,
                                                           (*transform).m11,
                                                           (*transform).m12))
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_compute_domain<'a>(tessellator: *mut Tessellator<'a>) {
    (*tessellator).compute_domain()
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_tess_levels<'a>(tessellator: *mut Tessellator<'a>,
                                                    out_tess_levels_count: *mut u32)
                                                    -> *const QuadTessLevels {
    let tess_levels = (*tessellator).tess_levels();
    if !out_tess_levels_count.is_null() {
        *out_tess_levels_count = tess_levels.len() as u32
    }
    tess_levels.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_vertices<'a>(tessellator: *mut Tessellator<'a>,
                                                 out_vertex_count: *mut u32)
                                                 -> *const Vertex {
    let vertices = (*tessellator).vertices();
    if !out_vertex_count.is_null() {
        *out_vertex_count = vertices.len() as u32
    }
    vertices.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_msaa_indices<'a>(tessellator: *mut Tessellator<'a>,
                                                     out_msaa_index_count: *mut u32)
                                                     -> *const u32 {
    let msaa_indices = (*tessellator).msaa_indices();
    if !out_msaa_index_count.is_null() {
        *out_msaa_index_count = msaa_indices.len() as u32
    }
    msaa_indices.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_tessellator_levien_indices<'a>(tessellator: *mut Tessellator<'a>,
                                                       out_levien_index_count: *mut u32)
                                                       -> *const u32 {
    let levien_indices = (*tessellator).levien_indices();
    if !out_levien_index_count.is_null() {
        *out_levien_index_count = levien_indices.len() as u32
    }
    levien_indices.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_init_env_logger() -> u32 {
    env_logger::init().is_ok() as u32
}
