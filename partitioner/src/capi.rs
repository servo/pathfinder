// partitionfinder/capi.rs

use env_logger;
use euclid::Point2D;
use legalizer::Legalizer;
use partitioner::Partitioner;
use std::mem;
use std::slice;
use {BQuad, BVertexLoopBlinnData, CurveIndices, Endpoint, LineIndices, Subpath};

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

#[derive(Clone, Copy)]
#[repr(C)]
pub struct EdgeIndices {
    pub upper_line_indices: *const LineIndices,
    pub upper_line_indices_len: u32,
    pub upper_curve_indices: *const CurveIndices,
    pub upper_curve_indices_len: u32,
    pub lower_line_indices: *const LineIndices,
    pub lower_line_indices_len: u32,
    pub lower_curve_indices: *const CurveIndices,
    pub lower_curve_indices_len: u32,
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_new() -> *mut Legalizer {
    let mut legalizer = Box::new(Legalizer::new());
    let legalizer_ptr: *mut Legalizer = &mut *legalizer;
    mem::forget(legalizer);
    legalizer_ptr
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_destroy(legalizer: *mut Legalizer) {
    drop(mem::transmute::<*mut Legalizer, Box<Legalizer>>(legalizer))
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_endpoints(legalizer: *const Legalizer,
                                            out_endpoint_count: *mut u32)
                                            -> *const Endpoint {
    let endpoints = (*legalizer).endpoints();
    if !out_endpoint_count.is_null() {
        *out_endpoint_count = endpoints.len() as u32
    }
    endpoints.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_control_points(legalizer: *const Legalizer,
                                                 out_control_points_count: *mut u32)
                                                 -> *const Point2DF32 {
    let control_points = (*legalizer).control_points();
    if !out_control_points_count.is_null() {
        *out_control_points_count = control_points.len() as u32
    }
    // FIXME(pcwalton): This is unsafe! `Point2D<f32>` and `Point2DF32` may have different layouts!
    control_points.as_ptr() as *const Point2DF32
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_subpaths(legalizer: *const Legalizer,
                                           out_subpaths_count: *mut u32)
                                           -> *const Subpath {
    let subpaths = (*legalizer).subpaths();
    if !out_subpaths_count.is_null() {
        *out_subpaths_count = subpaths.len() as u32
    }
    subpaths.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_move_to(legalizer: *mut Legalizer,
                                          position: *const Point2DF32) {
    (*legalizer).move_to(&(*position).to_point2d())
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_close_path(legalizer: *mut Legalizer) {
    (*legalizer).close_path()
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_line_to(legalizer: *mut Legalizer,
                                          endpoint: *const Point2DF32) {
    (*legalizer).line_to(&(*endpoint).to_point2d())
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_quadratic_curve_to(legalizer: *mut Legalizer,
                                                     control_point: *const Point2DF32,
                                                     endpoint: *const Point2DF32) {
    (*legalizer).quadratic_curve_to(&(*control_point).to_point2d(), &(*endpoint).to_point2d())
}

#[no_mangle]
pub unsafe extern fn pf_legalizer_bezier_curve_to(legalizer: *mut Legalizer,
                                                  point1: *const Point2DF32,
                                                  point2: *const Point2DF32,
                                                  endpoint: *const Point2DF32) {
    (*legalizer).bezier_curve_to(&(*point1).to_point2d(),
                                 &(*point2).to_point2d(),
                                 &(*endpoint).to_point2d())
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
                                             control_points: *const Point2DF32,
                                             control_point_count: u32,
                                             subpaths: *const Subpath,
                                             subpath_count: u32) {
    // FIXME(pcwalton): This is unsafe! `Point2D<f32>` and `Point2DF32` may have different layouts!
    (*partitioner).init(slice::from_raw_parts(endpoints, endpoint_count as usize),
                        slice::from_raw_parts(control_points as *const Point2D<f32>,
                                              control_point_count as usize),
                        slice::from_raw_parts(subpaths, subpath_count as usize))
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_partition<'a>(partitioner: *mut Partitioner<'a>,
                                                  path_id: u16,
                                                  first_subpath_index: u32,
                                                  last_subpath_index: u32) {
    (*partitioner).partition(path_id, first_subpath_index, last_subpath_index)
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_quads<'a>(partitioner: *const Partitioner<'a>,
                                                out_b_quad_count: *mut u32)
                                                -> *const BQuad {
    let b_quads = (*partitioner).b_quads();
    if !out_b_quad_count.is_null() {
        *out_b_quad_count = b_quads.len() as u32
    }
    b_quads.as_ptr()
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_vertex_positions<'a>(partitioner: *const Partitioner<'a>,
                                                           out_b_vertex_count: *mut u32)
                                                           -> *const Point2D<f32> {
    let b_vertex_positions = (*partitioner).b_vertex_positions();
    if !out_b_vertex_count.is_null() {
        *out_b_vertex_count = b_vertex_positions.len() as u32
    }
    b_vertex_positions.as_ptr() as *const Point2D<f32>
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_vertex_path_ids<'a>(partitioner: *const Partitioner<'a>,
                                                          out_b_vertex_count: *mut u32)
                                                          -> *const u16 {
    let b_vertex_path_ids = (*partitioner).b_vertex_path_ids();
    if !out_b_vertex_count.is_null() {
        *out_b_vertex_count = b_vertex_path_ids.len() as u32
    }
    b_vertex_path_ids.as_ptr() as *const u16
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_b_vertex_loop_blinn_data<'a>(
        partitioner: *const Partitioner<'a>,
        out_b_vertex_count: *mut u32)
        -> *const BVertexLoopBlinnData {
    let b_vertex_loop_blinn_data = (*partitioner).b_vertex_loop_blinn_data();
    if !out_b_vertex_count.is_null() {
        *out_b_vertex_count = b_vertex_loop_blinn_data.len() as u32
    }
    b_vertex_loop_blinn_data.as_ptr() as *const BVertexLoopBlinnData
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_cover_indices<'a>(partitioner: *const Partitioner<'a>,
                                                      out_cover_indices: *mut CoverIndices) {
    let cover_indices = (*partitioner).cover_indices();
    (*out_cover_indices).interior_indices = cover_indices.interior_indices.as_ptr();
    (*out_cover_indices).interior_indices_len = cover_indices.interior_indices.len() as u32;
    (*out_cover_indices).curve_indices = cover_indices.curve_indices.as_ptr();
    (*out_cover_indices).curve_indices_len = cover_indices.curve_indices.len() as u32;
}

#[no_mangle]
pub unsafe extern fn pf_partitioner_edge_indices<'a>(partitioner: *const Partitioner<'a>,
                                                     out_edge_indices: *mut EdgeIndices) {
    let edge_indices = (*partitioner).edge_indices();
    (*out_edge_indices).upper_line_indices = edge_indices.upper_line_indices.as_ptr();
    (*out_edge_indices).upper_line_indices_len = edge_indices.upper_line_indices.len() as u32;
    (*out_edge_indices).upper_curve_indices = edge_indices.upper_curve_indices.as_ptr();
    (*out_edge_indices).upper_curve_indices_len = edge_indices.upper_curve_indices.len() as u32;
    (*out_edge_indices).lower_line_indices = edge_indices.lower_line_indices.as_ptr();
    (*out_edge_indices).lower_line_indices_len = edge_indices.lower_line_indices.len() as u32;
    (*out_edge_indices).lower_curve_indices = edge_indices.lower_curve_indices.as_ptr();
    (*out_edge_indices).lower_curve_indices_len = edge_indices.lower_curve_indices.len() as u32;
}

#[no_mangle]
pub unsafe extern fn pf_init_env_logger() -> u32 {
    env_logger::init().is_ok() as u32
}
