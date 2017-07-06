// partitionfinder/tessellator.rs

#![allow(dead_code)]

use euclid::{Length, Transform2D};
use half::{f16, self};
use std::cmp;
use std::u32;
use {AntialiasingMode, BQuad, ControlPoints, Endpoint, Vertex};

const TOLERANCE: f32 = 0.25;

pub struct Tessellator<'a> {
    endpoints: &'a [Endpoint],
    control_points: &'a [ControlPoints],
    b_quads: &'a [BQuad],
    antialiasing_mode: AntialiasingMode,

    tess_levels: Vec<QuadTessLevels>,
    vertices: Vec<Vertex>,
    msaa_indices: Vec<u32>,
    levien_indices: Vec<u32>,
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
    pub fn new<'b>(endpoints: &'b [Endpoint],
                   control_points: &'b [ControlPoints],
                   b_quads: &'b [BQuad],
                   antialiasing_mode: AntialiasingMode)
                   -> Tessellator<'b> {
        Tessellator {
            endpoints: endpoints,
            control_points: control_points,
            b_quads: b_quads,
            antialiasing_mode: antialiasing_mode,

            tess_levels: vec![QuadTessLevels::new(); b_quads.len()],
            vertices: vec![],
            msaa_indices: vec![],
            levien_indices: vec![],
        }
    }

    pub fn compute_hull(&mut self, transform: &Transform2D<f32>) {
        for (tess_levels, bquad) in (self.tess_levels.iter_mut()).zip(self.b_quads.iter()) {
            let upper_tess_level = tess_level_for_edge(bquad.upper_prev_endpoint,
                                                       bquad.upper_next_endpoint,
                                                       bquad.upper_left_time,
                                                       bquad.upper_right_time,
                                                       transform,
                                                       self.endpoints,
                                                       self.control_points);
            let lower_tess_level = tess_level_for_edge(bquad.lower_prev_endpoint,
                                                       bquad.lower_prev_endpoint,
                                                       bquad.lower_left_time,
                                                       bquad.lower_right_time,
                                                       transform,
                                                       self.endpoints,
                                                       self.control_points);

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
                let left_time: Length<f32, ()> = Length::new(b_quad.upper_left_time);
                let right_time: Length<f32, ()> = Length::new(b_quad.upper_right_time);
                Vertex::new(b_quad.upper_prev_endpoint,
                            b_quad.upper_next_endpoint,
                            left_time.lerp(right_time, index as f32 / tess_level as f32).get())
            }));

            let first_lower_vertex_index = self.vertices.len() as u32;
            self.vertices.extend((0..(tess_level + 1)).map(|index| {
                let left_time: Length<f32, ()> = Length::new(b_quad.lower_left_time);
                let right_time: Length<f32, ()> = Length::new(b_quad.lower_right_time);
                Vertex::new(b_quad.lower_prev_endpoint,
                            b_quad.lower_next_endpoint,
                            left_time.lerp(right_time, index as f32 / tess_level as f32).get())
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
    pub fn levien_indices(&self) -> &[u32] {
        &self.levien_indices
    }

}

// http://antigrain.com/research/adaptive_bezier/
fn tess_level_for_edge(prev_endpoint_index: u32,
                       next_endpoint_index: u32,
                       left_time: f32,
                       right_time: f32,
                       transform: &Transform2D<f32>,
                       endpoints: &[Endpoint],
                       control_points: &[ControlPoints])
                       -> u32 {
    let control_points_index = endpoints[next_endpoint_index as usize].control_points_index;
    if control_points_index == u32::MAX {
        return 1
    }

    let (prev_time, next_time) = (left_time.min(right_time), left_time.max(right_time));

    let prev_endpoint = &endpoints[prev_endpoint_index as usize];
    let next_endpoint = &endpoints[next_endpoint_index as usize];
    let control_points = &control_points[control_points_index as usize];

    let p0 = transform.transform_point(&prev_endpoint.position);
    let p1 = transform.transform_point(&control_points.point1);
    let p2 = transform.transform_point(&control_points.point2);
    let p3 = transform.transform_point(&next_endpoint.position);

    let length = (p1 - p0).length() + (p2 - p1).length() + (p3 - p2).length();
    1 + (length * TOLERANCE * (next_time - prev_time)) as u32
}
