// pathfinder/partitioner/src/mesh.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use euclid::approxeq::ApproxEq;
use euclid::{Point2D, Rect, Size2D, Vector2D};
use lyon_path::PathEvent;
use pathfinder_path_utils::normals::PathNormals;
use pathfinder_path_utils::segments::{self, SegmentIter};
use std::f32;
use std::u32;

use {BQuad, BQuadVertexPositions, BVertexLoopBlinnData};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub b_quads: Vec<BQuad>,
    // FIXME(pcwalton): Merge with `b_vertex_positions` below.
    pub b_quad_vertex_positions: Vec<BQuadVertexPositions>,
    pub b_quad_vertex_interior_indices: Vec<u32>,
    pub b_vertex_positions: Vec<Point2D<f32>>,
    pub b_vertex_loop_blinn_data: Vec<BVertexLoopBlinnData>,
    pub b_boxes: Vec<BBox>,
    pub stencil_segments: Vec<StencilSegment>,
    pub stencil_normals: Vec<StencilNormals>,
}

impl Mesh {
    #[inline]
    pub fn new() -> Mesh {
        Mesh {
            b_quads: vec![],
            b_quad_vertex_positions: vec![],
            b_quad_vertex_interior_indices: vec![],
            b_vertex_positions: vec![],
            b_vertex_loop_blinn_data: vec![],
            b_boxes: vec![],
            stencil_segments: vec![],
            stencil_normals: vec![],
        }
    }

    pub fn clear(&mut self) {
        self.b_quads.clear();
        self.b_quad_vertex_positions.clear();
        self.b_quad_vertex_interior_indices.clear();
        self.b_vertex_positions.clear();
        self.b_vertex_loop_blinn_data.clear();
        self.b_boxes.clear();
        self.stencil_segments.clear();
        self.stencil_normals.clear();
    }

    pub(crate) fn add_b_vertex(
        &mut self,
        position: &Point2D<f32>,
        loop_blinn_data: &BVertexLoopBlinnData,
    ) {
        self.b_vertex_positions.push(*position);
        self.b_vertex_loop_blinn_data.push(*loop_blinn_data);
    }

    pub(crate) fn add_b_quad(&mut self, b_quad: &BQuad) {
        let BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_right_vertex_position: lr,
            ..
        } = self.get_b_quad_vertex_positions(b_quad);

        if ul.x.approx_eq(&ur.x) || ll.x.approx_eq(&lr.x) {
            return;
        }

        self.b_quads.push(*b_quad);

        self.add_b_quad_vertex_positions(b_quad);
        self.add_b_box(b_quad);
    }

    fn add_b_quad_vertex_positions(&mut self, b_quad: &BQuad) {
        let b_quad_vertex_positions = self.get_b_quad_vertex_positions(b_quad);
        let first_b_quad_vertex_position_index = (self.b_quad_vertex_positions.len() as u32) * 6;
        self.push_b_quad_vertex_position_interior_indices(
            first_b_quad_vertex_position_index,
            &b_quad_vertex_positions,
        );
        self.b_quad_vertex_positions.push(b_quad_vertex_positions);
    }

    fn add_b_box(&mut self, b_quad: &BQuad) {
        let BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_control_point_position: uc,
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_control_point_position: lc,
            lower_right_vertex_position: lr,
        } = self.get_b_quad_vertex_positions(b_quad);

        let rect = Rect::from_points([ul, uc, ur, ll, lc, lr].into_iter());

        let (edge_ucl, edge_urc, edge_ulr) = (uc - ul, ur - uc, ul - ur);
        let (edge_lcl, edge_lrc, edge_llr) = (lc - ll, lr - lc, ll - lr);

        let (edge_len_ucl, edge_len_urc) = (edge_ucl.length(), edge_urc.length());
        let (edge_len_lcl, edge_len_lrc) = (edge_lcl.length(), edge_lrc.length());
        let (edge_len_ulr, edge_len_llr) = (edge_ulr.length(), edge_llr.length());

        let (uv_upper, uv_lower, sign_upper, sign_lower, mode_upper, mode_lower);

        if edge_len_ucl < 0.01
            || edge_len_urc < 0.01
            || edge_len_ulr < 0.01
            || edge_ucl.dot(-edge_ulr) > 0.9999 * edge_len_ucl * edge_len_ulr
        {
            uv_upper = Uv::line(&rect, &ul, &ur);
            sign_upper = -1.0;
            mode_upper = -1.0;
        } else {
            uv_upper = Uv::curve(&rect, &ul, &uc, &ur);
            sign_upper = (edge_ucl.cross(-edge_ulr)).signum();
            mode_upper = 1.0;
        }

        if edge_len_lcl < 0.01
            || edge_len_lrc < 0.01
            || edge_len_llr < 0.01
            || edge_lcl.dot(-edge_llr) > 0.9999 * edge_len_lcl * edge_len_llr
        {
            uv_lower = Uv::line(&rect, &ll, &lr);
            sign_lower = 1.0;
            mode_lower = -1.0;
        } else {
            uv_lower = Uv::curve(&rect, &ll, &lc, &lr);
            sign_lower = -(edge_lcl.cross(-edge_llr)).signum();
            mode_lower = 1.0;
        }

        let b_box = BBox {
            upper_left_position: rect.origin,
            lower_right_position: rect.bottom_right(),
            upper_left_uv_upper: uv_upper.origin,
            upper_left_uv_lower: uv_lower.origin,
            d_upper_uv_dx: uv_upper.d_uv_dx,
            d_lower_uv_dx: uv_lower.d_uv_dx,
            d_upper_uv_dy: uv_upper.d_uv_dy,
            d_lower_uv_dy: uv_lower.d_uv_dy,
            upper_sign: sign_upper,
            lower_sign: sign_lower,
            upper_mode: mode_upper,
            lower_mode: mode_lower,
        };

        self.b_boxes.push(b_box);
    }

    fn get_b_quad_vertex_positions(&self, b_quad: &BQuad) -> BQuadVertexPositions {
        let ul = self.b_vertex_positions[b_quad.upper_left_vertex_index as usize];
        let ur = self.b_vertex_positions[b_quad.upper_right_vertex_index as usize];
        let ll = self.b_vertex_positions[b_quad.lower_left_vertex_index as usize];
        let lr = self.b_vertex_positions[b_quad.lower_right_vertex_index as usize];

        let mut b_quad_vertex_positions = BQuadVertexPositions {
            upper_left_vertex_position: ul,
            upper_control_point_position: ul.lerp(ur, 0.5),
            upper_right_vertex_position: ur,
            lower_left_vertex_position: ll,
            lower_control_point_position: ll.lerp(lr, 0.5),
            lower_right_vertex_position: lr,
        };

        if b_quad.upper_control_point_vertex_index != u32::MAX {
            let uc = &self.b_vertex_positions[b_quad.upper_control_point_vertex_index as usize];
            b_quad_vertex_positions.upper_control_point_position = *uc;
        }

        if b_quad.lower_control_point_vertex_index != u32::MAX {
            let lc = &self.b_vertex_positions[b_quad.lower_control_point_vertex_index as usize];
            b_quad_vertex_positions.lower_control_point_position = *lc;
        }

        b_quad_vertex_positions
    }

    fn push_b_quad_vertex_position_interior_indices(
        &mut self,
        first_vertex_index: u32,
        b_quad: &BQuadVertexPositions,
    ) {
        let upper_curve_is_concave = (b_quad.upper_right_vertex_position
            - b_quad.upper_left_vertex_position)
            .cross(b_quad.upper_control_point_position - b_quad.upper_left_vertex_position)
            > 0.0;
        let lower_curve_is_concave = (b_quad.lower_left_vertex_position
            - b_quad.lower_right_vertex_position)
            .cross(b_quad.lower_control_point_position - b_quad.lower_right_vertex_position)
            > 0.0;

        let indices: &'static [u32] = match (upper_curve_is_concave, lower_curve_is_concave) {
            (false, false) => &[UL, UR, LL, UR, LR, LL],
            (true, false) => &[UL, UC, LL, UC, LR, LL, UR, LR, UC],
            (false, true) => &[UL, LC, LL, UL, UR, LC, UR, LR, LC],
            (true, true) => &[UL, UC, LL, UC, LC, LL, UR, LC, UC, UR, LR, LC],
        };

        self.b_quad_vertex_interior_indices
            .extend(indices.into_iter().map(|index| index + first_vertex_index));

        const UL: u32 = 0;
        const UC: u32 = 1;
        const UR: u32 = 2;
        const LR: u32 = 3;
        const LC: u32 = 4;
        const LL: u32 = 5;
    }

    pub fn push_stencil_segments<I>(&mut self, stream: I)
    where
        I: Iterator<Item = PathEvent>,
    {
        let segment_iter = SegmentIter::new(stream);
        for segment in segment_iter {
            match segment {
                segments::Segment::Line(line_segment) => {
                    self.stencil_segments.push(StencilSegment {
                        from: line_segment.from,
                        ctrl: line_segment.from.lerp(line_segment.to, 0.5),
                        to: line_segment.to,
                    })
                }
                segments::Segment::Quadratic(quadratic_segment) => {
                    self.stencil_segments.push(StencilSegment {
                        from: quadratic_segment.from,
                        ctrl: quadratic_segment.ctrl,
                        to: quadratic_segment.to,
                    })
                }
                segments::Segment::Cubic(..) => {
                    panic!("push_stencil_segments(): Convert cubics to quadratics first!")
                }
                segments::Segment::EndSubpath(..) => {}
            }
        }
    }

    /// Computes vertex normals necessary for emboldening and/or stem darkening. This is intended
    /// for stencil-and-cover.
    pub fn push_stencil_normals<I>(&mut self, stream: I)
    where
        I: Iterator<Item = PathEvent>,
    {
        let mut normals = PathNormals::new();
        normals.add_path(stream);
        self.stencil_normals
            .extend(normals.normals().iter().map(|normals| StencilNormals {
                from: normals.from,
                ctrl: normals.ctrl,
                to: normals.to,
            }))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BBox {
    pub upper_left_position: Point2D<f32>,
    pub lower_right_position: Point2D<f32>,
    pub upper_left_uv_upper: Point2D<f32>,
    pub upper_left_uv_lower: Point2D<f32>,
    pub d_upper_uv_dx: Vector2D<f32>,
    pub d_lower_uv_dx: Vector2D<f32>,
    pub d_upper_uv_dy: Vector2D<f32>,
    pub d_lower_uv_dy: Vector2D<f32>,
    pub upper_sign: f32,
    pub lower_sign: f32,
    pub upper_mode: f32,
    pub lower_mode: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StencilSegment {
    pub from: Point2D<f32>,
    pub ctrl: Point2D<f32>,
    pub to: Point2D<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StencilNormals {
    pub from: Vector2D<f32>,
    pub ctrl: Vector2D<f32>,
    pub to: Vector2D<f32>,
}

#[derive(Clone, Copy, Debug)]
struct CornerValues {
    upper_left: Point2D<f32>,
    upper_right: Point2D<f32>,
    lower_left: Point2D<f32>,
    lower_right: Point2D<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Uv {
    origin: Point2D<f32>,
    d_uv_dx: Vector2D<f32>,
    d_uv_dy: Vector2D<f32>,
}

impl Uv {
    fn from_values(
        origin: &Point2D<f32>,
        origin_right: &Point2D<f32>,
        origin_down: &Point2D<f32>,
    ) -> Uv {
        Uv {
            origin: *origin,
            d_uv_dx: *origin_right - *origin,
            d_uv_dy: *origin_down - *origin,
        }
    }

    fn curve(
        rect: &Rect<f32>,
        left: &Point2D<f32>,
        ctrl: &Point2D<f32>,
        right: &Point2D<f32>,
    ) -> Uv {
        let origin_right = rect.top_right();
        let origin_down = rect.bottom_left();

        let (lambda_origin, denom) = to_barycentric(left, ctrl, right, &rect.origin);
        let (lambda_origin_right, _) = to_barycentric(left, ctrl, right, &origin_right);
        let (lambda_origin_down, _) = to_barycentric(left, ctrl, right, &origin_down);

        let uv_origin = lambda_to_uv(&lambda_origin, denom);
        let uv_origin_right = lambda_to_uv(&lambda_origin_right, denom);
        let uv_origin_down = lambda_to_uv(&lambda_origin_down, denom);

        return Uv::from_values(&uv_origin, &uv_origin_right, &uv_origin_down);

        // https://gamedev.stackexchange.com/a/23745
        fn to_barycentric(
            a: &Point2D<f32>,
            b: &Point2D<f32>,
            c: &Point2D<f32>,
            p: &Point2D<f32>,
        ) -> ([f64; 2], f64) {
            let (a, b, c, p) = (a.to_f64(), b.to_f64(), c.to_f64(), p.to_f64());
            let (v0, v1, v2) = (b - a, c - a, p - a);
            let (d00, d01) = (v0.dot(v0), v0.dot(v1));
            let d11 = v1.dot(v1);
            let (d20, d21) = (v2.dot(v0), v2.dot(v1));
            let denom = d00 * d11 - d01 * d01;
            ([(d11 * d20 - d01 * d21), (d00 * d21 - d01 * d20)], denom)
        }

        fn lambda_to_uv(lambda: &[f64; 2], denom: f64) -> Point2D<f32> {
            (Point2D::new(lambda[0] * 0.5 + lambda[1], lambda[1]) / denom).to_f32()
        }
    }

    fn line(rect: &Rect<f32>, left: &Point2D<f32>, right: &Point2D<f32>) -> Uv {
        let (values, line_bounds);
        if f32::abs(left.y - right.y) < 0.01 {
            values = CornerValues {
                upper_left: Point2D::new(0.0, 0.5),
                upper_right: Point2D::new(0.5, 1.0),
                lower_right: Point2D::new(1.0, 0.5),
                lower_left: Point2D::new(0.5, 0.0),
            };
            line_bounds = Rect::new(
                *left + Vector2D::new(0.0, -1.0),
                Size2D::new(right.x - left.x, 2.0),
            );
        } else {
            if left.y < right.y {
                values = CornerValues {
                    upper_left: Point2D::new(1.0, 1.0),
                    upper_right: Point2D::new(0.0, 1.0),
                    lower_left: Point2D::new(1.0, 0.0),
                    lower_right: Point2D::new(0.0, 0.0),
                };
            } else {
                values = CornerValues {
                    upper_left: Point2D::new(0.0, 1.0),
                    upper_right: Point2D::new(1.0, 1.0),
                    lower_left: Point2D::new(0.0, 0.0),
                    lower_right: Point2D::new(1.0, 0.0),
                };
            }
            line_bounds = Rect::from_points([*left, *right].into_iter());
        }

        let origin_right = rect.top_right();
        let origin_down = rect.bottom_left();

        let uv_origin = bilerp(&line_bounds, &values, &rect.origin);
        let uv_origin_right = bilerp(&line_bounds, &values, &origin_right);
        let uv_origin_down = bilerp(&line_bounds, &values, &origin_down);

        return Uv::from_values(&uv_origin, &uv_origin_right, &uv_origin_down);

        fn bilerp(
            rect: &Rect<f32>,
            values: &CornerValues,
            position: &Point2D<f32>,
        ) -> Point2D<f32> {
            let upper = values.upper_left.lerp(
                values.upper_right,
                (position.x - rect.min_x()) / rect.size.width,
            );
            let lower = values.lower_left.lerp(
                values.lower_right,
                (position.x - rect.min_x()) / rect.size.width,
            );
            upper.lerp(lower, (position.y - rect.min_y()) / rect.size.height)
        }
    }
}
