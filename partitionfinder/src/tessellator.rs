// partitionfinder/tessellator.rs

#![allow(dead_code)]

use euclid::Transform2D;
use half::{f16, self};
use std::cmp;
use std::u32;
use {AntialiasingMode, BQuad, BVertex, EdgeInstance, Vertex};

const TOLERANCE: f32 = 0.25;

pub struct Tessellator<'a> {
    b_quads: &'a [BQuad],
    b_vertices: &'a [BVertex],
    b_indices: &'a [u32],
    antialiasing_mode: AntialiasingMode,

    tess_levels: Vec<QuadTessLevels>,
    vertices: Vec<Vertex>,
    msaa_indices: Vec<u32>,
    edge_instances: Vec<EdgeInstance>,
}

// NB: This must match the layout of `MTLQuadTessellationFactorsHalf` in Metal in order for the
// Pathfinder demo to work.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct QuadTessLevels {
    pub outer: [f16; 4],
    pub inner: [f16; 2],
}

impl QuadTessLevels {
    fn new() -> QuadTessLevels {
        QuadTessLevels {
            outer: [half::consts::ZERO; 4],
            inner: [half::consts::ZERO; 2],
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BQuadVertices {
    upper_left_vertex: u32,
    upper_control_point: u32,
    upper_right_vertex: u32,
    lower_left_vertex: u32,
    lower_control_point: u32,
    lower_right_vertex: u32,
}

impl<'a> Tessellator<'a> {
    pub fn new<'b>(antialiasing_mode: AntialiasingMode) -> Tessellator<'b> {
        Tessellator {
            b_quads: &[],
            b_vertices: &[],
            b_indices: &[],
            antialiasing_mode: antialiasing_mode,

            tess_levels: vec![],
            vertices: vec![],
            msaa_indices: vec![],
            edge_instances: vec![],
        }
    }

    pub fn init(&mut self, b_quads: &'a [BQuad], b_vertices: &'a [BVertex], b_indices: &'a [u32]) {
        self.b_quads = b_quads;
        self.b_vertices = b_vertices;
        self.b_indices = b_indices;
        self.tess_levels = vec![QuadTessLevels::new(); b_quads.len()];
    }

    fn b_quad_vertices(&self, b_quad_index: u32) -> BQuadVertices {
        let b_quad = &self.b_quads[b_quad_index as usize];
        BQuadVertices {
            upper_left_vertex: self.b_indices[b_quad.upper_left_vertex_index() as usize],
            upper_right_vertex: self.b_indices[b_quad.upper_right_vertex_index() as usize],
            lower_left_vertex: self.b_indices[b_quad.lower_left_vertex_index() as usize],
            lower_right_vertex: self.b_indices[b_quad.lower_right_vertex_index() as usize],
            upper_control_point: match b_quad.upper_control_point_vertex_index() {
                u32::MAX => u32::MAX,
                control_point_index => self.b_indices[control_point_index as usize],
            },
            lower_control_point: match b_quad.lower_control_point_vertex_index() {
                u32::MAX => u32::MAX,
                control_point_index => self.b_indices[control_point_index as usize],
            },
        }
    }

    pub fn compute_hull(&mut self, transform: &Transform2D<f32>) {
        for b_quad_index in 0..self.tess_levels.len() {
            let b_quad_vertices = self.b_quad_vertices(b_quad_index as u32);

            let upper_tess_level = tess_level_for_edge(b_quad_vertices.upper_left_vertex,
                                                       b_quad_vertices.upper_control_point,
                                                       b_quad_vertices.upper_right_vertex,
                                                       transform,
                                                       self.b_vertices);
            let lower_tess_level = tess_level_for_edge(b_quad_vertices.lower_left_vertex,
                                                       b_quad_vertices.lower_control_point,
                                                       b_quad_vertices.lower_right_vertex,
                                                       transform,
                                                       self.b_vertices);

            // TODO(pcwalton): Use fewer thin triangles.
            let mut tess_levels = &mut self.tess_levels[b_quad_index as usize];
            tess_levels.outer[0] = half::consts::ONE;
            tess_levels.outer[1] = f16::from_f32(upper_tess_level as f32);
            tess_levels.outer[2] = half::consts::ONE;
            tess_levels.outer[3] = f16::from_f32(lower_tess_level as f32);
            tess_levels.inner[0] = f16::from_f32(cmp::max(upper_tess_level,
                                                          lower_tess_level) as f32);
            tess_levels.inner[1] = half::consts::ZERO;
        }
    }

    // TODO(pcwalton): Do a better tessellation that doesn't make so many sliver triangles.
    pub fn compute_domain(&mut self) {
        for (b_quad_index, tess_levels) in self.tess_levels.iter().enumerate() {
            let b_quad_vertices = self.b_quad_vertices(b_quad_index as u32);

            let upper_tess_level = f32::from(tess_levels.outer[1]) as u32;
            let lower_tess_level = f32::from(tess_levels.outer[3]) as u32;
            let tess_level = cmp::max(upper_tess_level, lower_tess_level);

            let path_id = self.b_vertices[b_quad_vertices.upper_left_vertex as usize].path_id;

            let first_upper_vertex_index = self.vertices.len() as u32;
            self.vertices.extend((0..(tess_level + 1)).map(|index| {
                Vertex::new(path_id,
                            b_quad_vertices.upper_left_vertex,
                            b_quad_vertices.upper_control_point,
                            b_quad_vertices.upper_right_vertex,
                            index as f32 / tess_level as f32)
            }));

            let first_lower_vertex_index = self.vertices.len() as u32;
            self.vertices.extend((0..(tess_level + 1)).map(|index| {
                Vertex::new(path_id,
                            b_quad_vertices.lower_left_vertex,
                            b_quad_vertices.lower_control_point,
                            b_quad_vertices.lower_right_vertex,
                            index as f32 / tess_level as f32)
            }));

            // Emit a triangle strip.
            self.msaa_indices.reserve(tess_level as usize * 6);
            for index in 0..tess_level {
                self.msaa_indices.extend([
                    first_upper_vertex_index + index + 0,
                    first_upper_vertex_index + index + 1,
                    first_lower_vertex_index + index + 0,
                    first_upper_vertex_index + index + 1,
                    first_lower_vertex_index + index + 1,
                    first_lower_vertex_index + index + 0,
                ].into_iter())
            }

            // If ECAA is in use, then emit edge instances.
            if self.antialiasing_mode == AntialiasingMode::Ecaa {
                for index in 0..tess_level {
                    self.edge_instances.extend([
                        EdgeInstance::new(first_upper_vertex_index + index + 0,
                                          first_upper_vertex_index + index + 1),
                        EdgeInstance::new(first_lower_vertex_index + index + 0,
                                          first_lower_vertex_index + index + 1)
                    ].into_iter())
                }
            }
        }
    }

    #[inline]
    pub fn tess_levels(&self) -> &[QuadTessLevels] {
        &self.tess_levels
    }

    #[inline]
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    #[inline]
    pub fn msaa_indices(&self) -> &[u32] {
        &self.msaa_indices
    }

    #[inline]
    pub fn edge_instances(&self) -> &[EdgeInstance] {
        &self.edge_instances
    }
}

// http://antigrain.com/research/adaptive_bezier/
fn tess_level_for_edge(left_endpoint_index: u32,
                       control_point_index: u32,
                       right_endpoint_index: u32,
                       transform: &Transform2D<f32>,
                       b_vertices: &[BVertex])
                       -> u32 {
    if control_point_index == u32::MAX {
        return 1
    }

    let left_endpoint = &b_vertices[left_endpoint_index as usize].position;
    let right_endpoint = &b_vertices[right_endpoint_index as usize].position;
    let control_point = &b_vertices[control_point_index as usize].position;

    let p0 = transform.transform_point(left_endpoint);
    let p1 = transform.transform_point(control_point);
    let p2 = transform.transform_point(right_endpoint);

    // FIXME(pcwalton): Is this good for quadratics?
    let length = (p1 - p0).length() + (p2 - p1).length();
    1 + (length * TOLERANCE) as u32
}
