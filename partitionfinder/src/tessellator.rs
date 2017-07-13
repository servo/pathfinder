// partitionfinder/tessellator.rs

#![allow(dead_code)]

use euclid::{Point2D, Transform2D};
use half::{f16, self};
use std::cmp;
use std::u32;
use {AntialiasingMode, BQuad, EdgeInstance, Vertex};

const TOLERANCE: f32 = 0.25;

pub struct Tessellator<'a> {
    b_quads: &'a [BQuad],
    b_vertices: &'a [Point2D<f32>],
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

impl<'a> Tessellator<'a> {
    pub fn new<'b>(b_quads: &'b [BQuad],
                   b_vertices: &'b [Point2D<f32>],
                   antialiasing_mode: AntialiasingMode)
                   -> Tessellator<'b> {
        Tessellator {
            b_quads: b_quads,
            b_vertices: b_vertices,
            antialiasing_mode: antialiasing_mode,

            tess_levels: vec![QuadTessLevels::new(); b_quads.len()],
            vertices: vec![],
            msaa_indices: vec![],
            edge_instances: vec![],
        }
    }

    pub fn compute_hull(&mut self, transform: &Transform2D<f32>) {
        for (tess_levels, bquad) in (self.tess_levels.iter_mut()).zip(self.b_quads.iter()) {
            let upper_tess_level = tess_level_for_edge(bquad.upper_left_vertex,
                                                       bquad.upper_control_point,
                                                       bquad.upper_right_vertex,
                                                       transform,
                                                       self.b_vertices);
            let lower_tess_level = tess_level_for_edge(bquad.lower_left_vertex,
                                                       bquad.lower_control_point,
                                                       bquad.lower_right_vertex,
                                                       transform,
                                                       self.b_vertices);

            // TODO(pcwalton): Use fewer thin triangles.
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
        for (b_quad, tess_levels) in self.b_quads.iter().zip(self.tess_levels.iter()) {
            let upper_tess_level = f32::from(tess_levels.outer[1]) as u32;
            let lower_tess_level = f32::from(tess_levels.outer[3]) as u32;
            let tess_level = cmp::max(upper_tess_level, lower_tess_level);

            let first_upper_vertex_index = self.vertices.len() as u32;
            self.vertices.extend((0..(tess_level + 1)).map(|index| {
                Vertex::new(b_quad.upper_left_vertex,
                            b_quad.upper_control_point,
                            b_quad.upper_right_vertex,
                            index as f32 / tess_level as f32)
            }));

            let first_lower_vertex_index = self.vertices.len() as u32;
            self.vertices.extend((0..(tess_level + 1)).map(|index| {
                Vertex::new(b_quad.lower_left_vertex,
                            b_quad.lower_control_point,
                            b_quad.lower_right_vertex,
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

            // If Levien-style antialiasing is in use, then emit edge instances.
            if self.antialiasing_mode == AntialiasingMode::Levien {
                for index in 0..tess_level {
                    let left_tess_coord = index as f32 / tess_level as f32;
                    let right_tess_coord = (index + 1) as f32 / tess_level as f32;

                    self.edge_instances.extend([
                        EdgeInstance::new(b_quad.upper_left_vertex,
                                          b_quad.upper_control_point,
                                          b_quad.upper_right_vertex,
                                          left_tess_coord,
                                          right_tess_coord),
                        EdgeInstance::new(b_quad.lower_left_vertex,
                                          b_quad.lower_control_point,
                                          b_quad.lower_right_vertex,
                                          left_tess_coord,
                                          right_tess_coord),
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
                       b_vertices: &[Point2D<f32>])
                       -> u32 {
    if control_point_index == u32::MAX {
        return 1
    }

    let left_endpoint = &b_vertices[left_endpoint_index as usize];
    let right_endpoint = &b_vertices[right_endpoint_index as usize];
    let control_point = &b_vertices[control_point_index as usize];

    let p0 = transform.transform_point(left_endpoint);
    let p1 = transform.transform_point(control_point);
    let p2 = transform.transform_point(right_endpoint);

    // FIXME(pcwalton): Is this good for quadratics?
    let length = (p1 - p0).length() + (p2 - p1).length();
    1 + (length * TOLERANCE) as u32
}
