// pathfinder/content/src/outline.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A compressed in-memory representation of a vector path.

use crate::clip::{self, ContourPolygonClipper};
use crate::dilation::ContourDilator;
use crate::orientation::Orientation;
use crate::segment::{Segment, SegmentFlags, SegmentKind};
use crate::util::safe_sqrt;
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::{Transform2F, Matrix2x2F};
use pathfinder_geometry::transform3d::Perspective;
use pathfinder_geometry::unit_vector::UnitVector;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use std::f32::consts::PI;
use std::fmt::{self, Debug, Formatter};
use std::mem;

/// A vector path to be filled. Outlines (a.k.a. paths) consist of *contours* (a.k.a. subpaths),
/// which can be filled according to a fill rule.
///
/// The names "outline" and "contour" come from the TrueType specification. They were chosen to
/// avoid conflicting with the Rust use of "path" for filesystem paths.
#[derive(Clone)]
pub struct Outline {
    pub(crate) contours: Vec<Contour>,
    pub(crate) bounds: RectF,
}

/// An individual subpath, consisting of a series of endpoints and/or control points. Contours can
/// be either open (first and last points disconnected) or closed (first point implicitly joined to
/// last point with a line).
#[derive(Clone)]
pub struct Contour {
    pub(crate) points: Vec<Vector2F>,
    pub(crate) flags: Vec<PointFlags>,
    pub(crate) bounds: RectF,
    pub(crate) closed: bool,
}

bitflags! {
    /// Flags that each point can have, indicating whether it is on-curve or whether it's a control
    /// point.
    pub struct PointFlags: u8 {
        /// This point is the first control point of a cubic Bézier curve or the only control point
        /// of a quadratic Bézier curve.
        const CONTROL_POINT_0 = 0x01;
        /// This point is the second point of a quadratic Bézier curve.
        const CONTROL_POINT_1 = 0x02;
    }
}

bitflags! {
    // Flags specifying what actions to take when pushing a segment onto a contour.
    pub(crate) struct PushSegmentFlags: u8 {
        /// The bounds should be updated.
        const UPDATE_BOUNDS = 0x01;
        /// The "from" point of the segme
        const INCLUDE_FROM_POINT = 0x02;
    }
}

impl Outline {
    /// Creates a new empty outline with no contours.
    #[inline]
    pub fn new() -> Outline {
        Outline {
            contours: vec![],
            bounds: RectF::default(),
        }
    }

    /// Returns a new `Outline` with storage for `capacity` contours preallocated.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Outline {
        Outline {
            contours: Vec::with_capacity(capacity),
            bounds: RectF::default(),
        }
    }

    /// Creates a new outline from a list of segments.
    #[inline]
    pub fn from_segments<I>(segments: I) -> Outline where I: Iterator<Item = Segment> {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();

        for segment in segments {
            if segment.flags.contains(SegmentFlags::FIRST_IN_SUBPATH) {
                if !current_contour.is_empty() {
                    outline
                        .contours
                        .push(mem::replace(&mut current_contour, Contour::new()));
                }
                current_contour.push_point(segment.baseline.from(), PointFlags::empty(), true);
            }

            if segment.flags.contains(SegmentFlags::CLOSES_SUBPATH) {
                if !current_contour.is_empty() {
                    current_contour.close();
                    let contour = mem::replace(&mut current_contour, Contour::new());
                    outline.push_contour(contour);
                }
                continue;
            }

            if segment.is_none() {
                continue;
            }

            if !segment.is_line() {
                current_contour.push_point(segment.ctrl.from(), PointFlags::CONTROL_POINT_0, true);
                if !segment.is_quadratic() {
                    current_contour.push_point(
                        segment.ctrl.to(),
                        PointFlags::CONTROL_POINT_1,
                        true,
                    );
                }
            }

            current_contour.push_point(segment.baseline.to(), PointFlags::empty(), true);
        }

        outline.push_contour(current_contour);
        outline
    }

    /// Creates a new outline that represents a single axis-aligned rectangle.
    #[inline]
    pub fn from_rect(rect: RectF) -> Outline {
        let mut outline = Outline::new();
        outline.push_contour(Contour::from_rect(rect));
        outline
    }

    /// Creates a new outline that represents a rounded rectangle.
    #[inline]
    pub fn from_rect_rounded(rect: RectF, radius: Vector2F) -> Outline {
        let mut outline = Outline::new();
        outline.push_contour(Contour::from_rect_rounded(rect, radius));
        outline
    }

    /// Returns the dimensions of an axis-aligned box that encloses the entire outline.
    #[inline]
    pub fn bounds(&self) -> RectF {
        self.bounds
    }

    /// Returns a list of the subpaths in this path.
    #[inline]
    pub fn contours(&self) -> &[Contour] {
        &self.contours
    }

    /// Destroys this outline and returns a list of its subpaths.
    #[inline]
    pub fn into_contours(self) -> Vec<Contour> {
        self.contours
    }

    /// Removes all contours from this outline.
    #[inline]
    pub fn clear(&mut self) {
        self.contours.clear();
        self.bounds = RectF::default();
    }

    /// Adds a new subpath to this outline.
    pub fn push_contour(&mut self, contour: Contour) {
        if contour.is_empty() {
            return;
        }

        if self.contours.is_empty() {
            self.bounds = contour.bounds;
        } else {
            self.bounds = self.bounds.union_rect(contour.bounds);
        }

        self.contours.push(contour);
    }

    /// Removes the last subpath from this outline and returns it.
    pub fn pop_contour(&mut self) -> Option<Contour> {
        let last_contour = self.contours.pop();

        let mut new_bounds = None;
        for contour in &mut self.contours {
            contour.update_bounds(&mut new_bounds);
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF::default());

        last_contour
    }

    /// Applies an affine transform to this outline and all its subpaths.
    pub fn transform(&mut self, transform: &Transform2F) {
        if transform.is_identity() {
            return;
        }

        let mut new_bounds = None;
        for contour in &mut self.contours {
            contour.transform(transform);
            contour.update_bounds(&mut new_bounds);
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF::default());
    }

    /// Applies an affine transform to this outline and all its subpaths, consuming this outline
    /// instead of mutating it.
    pub fn transformed(mut self, transform: &Transform2F) -> Outline {
        self.transform(transform);
        self
    }

    /// Applies a perspective transform to this outline.
    #[deprecated]
    #[allow(deprecated)]
    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        let mut new_bounds = None;
        for contour in &mut self.contours {
            contour.apply_perspective(perspective);
            contour.update_bounds(&mut new_bounds);
        }
        self.bounds = new_bounds.unwrap_or_else(|| RectF::default());
    }

    /// Thickens the outline by the given amount.
    ///
    /// This is implemented by pushing vectors out along their normals.
    pub fn dilate(&mut self, amount: Vector2F) {
        let orientation = Orientation::from_outline(self);
        self.contours
            .iter_mut()
            .for_each(|contour| contour.dilate(amount, orientation));
        self.bounds = self.bounds.dilate(amount);
    }

    /// Returns true if this outline is obviously completely outside the closed polygon with the
    /// given vertices, via a quick check.
    ///
    /// Even if the outline is outside the polygon, this might return false.
    pub fn is_outside_polygon(&self, clip_polygon: &[Vector2F]) -> bool {
        clip::rect_is_outside_polygon(self.bounds, clip_polygon)
    }

    fn is_inside_polygon(&self, clip_polygon: &[Vector2F]) -> bool {
        clip::rect_is_inside_polygon(self.bounds, clip_polygon)
    }

    /// Clips this outline against the given closed polygon with the given vertices.
    ///
    /// This is implemented with Sutherland-Hodgman clipping.
    pub fn clip_against_polygon(&mut self, clip_polygon: &[Vector2F]) {
        // Quick check.
        if self.is_inside_polygon(clip_polygon) {
            return;
        }

        for contour in mem::replace(&mut self.contours, vec![]) {
            self.push_contour(ContourPolygonClipper::new(clip_polygon, contour).clip());
        }
    }

    /// Marks all contours as closed.
    #[inline]
    pub fn close_all_contours(&mut self) {
        self.contours.iter_mut().for_each(|contour| contour.close());
    }

    /// Returns true if this outline has no points.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.contours.iter().all(Contour::is_empty)
    }

    /// Returns the number of contours in this outline.
    #[inline]
    pub fn len(&self) -> usize {
        self.contours.len()
    }

    /// Appends the contours of another outline to this one.
    pub fn push_outline(&mut self, other: Outline) {
        if other.is_empty() {
            return;
        }

        if self.is_empty() {
            self.bounds = other.bounds;
        } else {
            self.bounds = self.bounds.union_rect(other.bounds);
        }

        self.contours.extend(other.contours);
    }
}

impl Debug for Outline {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        for (contour_index, contour) in self.contours.iter().enumerate() {
            if contour_index > 0 {
                write!(formatter, " ")?;
            }
            contour.fmt(formatter)?;
        }
        Ok(())
    }
}

impl Contour {
    /// Creates a new empty unclosed subpath.
    #[inline]
    pub fn new() -> Contour {
        Contour {
            points: vec![],
            flags: vec![],
            bounds: RectF::default(),
            closed: false,
        }
    }

    /// Creates a new empty unclosed subpath with space preallocated for the given number of
    /// points.
    #[inline]
    pub fn with_capacity(length: usize) -> Contour {
        Contour {
            points: Vec::with_capacity(length),
            flags: Vec::with_capacity(length),
            bounds: RectF::default(),
            closed: false,
        }
    }

    /// Creates a closed subpath representing the given axis-aligned rectangle.
    #[inline]
    pub fn from_rect(rect: RectF) -> Contour {
        let mut contour = Contour::with_capacity(4);
        contour.push_point(rect.origin(), PointFlags::empty(), false);
        contour.push_point(rect.upper_right(), PointFlags::empty(), false);
        contour.push_point(rect.lower_right(), PointFlags::empty(), false);
        contour.push_point(rect.lower_left(), PointFlags::empty(), false);
        contour.close();
        contour.bounds = rect;
        contour
    }

    /// Creates a closed subpath representing the given axis-aligned rounded rectangle.
    #[inline]
    pub fn from_rect_rounded(rect: RectF, radius: Vector2F) -> Contour {
        use std::f32::consts::SQRT_2;
        const QUARTER_ARC_CP_FROM_OUTSIDE: f32 = (3.0 - 4.0 * (SQRT_2 - 1.0)) / 3.0;

        if radius.is_zero() {
            return Contour::from_rect(rect);
        }
        let radius = radius.min(rect.size() * 0.5);
        let contol_point_offset = radius * QUARTER_ARC_CP_FROM_OUTSIDE;

        let mut contour = Contour::with_capacity(8);

        // upper left corner
        {
            let p0 = rect.origin();
            let p1 = p0 + contol_point_offset;
            let p2 = p0 + radius;
            contour.push_endpoint(vec2f(p0.x(), p2.y()));
            contour.push_cubic(
                vec2f(p0.x(), p1.y()),
                vec2f(p1.x(), p0.y()),
                vec2f(p2.x(), p0.y())
            );
        }

        // upper right
        {
            let p0 = rect.upper_right();
            let p1 = p0 + contol_point_offset * vec2f(-1.0, 1.0);
            let p2 = p0 + radius * vec2f(-1.0, 1.0);
            contour.push_endpoint(vec2f(p2.x(), p0.y()));
            contour.push_cubic(
                vec2f(p1.x(), p0.y()),
                vec2f(p0.x(), p1.y()),
                vec2f(p0.x(), p2.y())
            );
        }

        // lower right
        {
            let p0 = rect.lower_right();
            let p1 = p0 + contol_point_offset * vec2f(-1.0, -1.0);
            let p2 = p0 + radius * vec2f(-1.0, -1.0);
            contour.push_endpoint(vec2f(p0.x(), p2.y()));
            contour.push_cubic(
                vec2f(p0.x(), p1.y()),
                vec2f(p1.x(), p0.y()),
                vec2f(p2.x(), p0.y())
            );
        }

        // lower left
        {
            let p0 = rect.lower_left();
            let p1 = p0 + contol_point_offset * vec2f(1.0, -1.0);
            let p2 = p0 + radius * vec2f(1.0, -1.0);
            contour.push_endpoint(vec2f(p2.x(), p0.y()));
            contour.push_cubic(
                vec2f(p1.x(), p0.y()),
                vec2f(p0.x(), p1.y()),
                vec2f(p0.x(), p2.y())
            );
        }

        contour.close();
        contour
    }

    // Replaces this contour with a new one, with arrays preallocated to match `self`.
    #[inline]
    pub(crate) fn take(&mut self) -> Contour {
        let length = self.len() as usize;
        mem::replace(
            self,
            Contour {
                points: Vec::with_capacity(length),
                flags: Vec::with_capacity(length),
                bounds: RectF::default(),
                closed: false,
            },
        )
    }

    /// Restores this contour to the state of `Contour::new()` but keeps the points buffer
    /// allocated.
    #[inline]
    pub fn clear(&mut self) {
        self.points.clear();
        self.flags.clear();
        self.bounds = RectF::default();
        self.closed = false;
    }

    /// Returns an iterator over the segments in this contour.
    #[inline]
    pub fn iter(&self, flags: ContourIterFlags) -> ContourIter {
        ContourIter {
            contour: self,
            index: 1,
            flags,
        }
    }

    /// Returns true if this contour has no points.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns the number of points (including on-curve and control points) in this contour.
    #[inline]
    pub fn len(&self) -> u32 {
        self.points.len() as u32
    }

    /// Returns the dimensions of an axis-aligned rectangle that encloses this contour.
    #[inline]
    pub fn bounds(&self) -> RectF {
        self.bounds
    }

    /// Returns true if this contour is closed.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Returns the position of the point (which can be an on-curve point or a control point) with
    /// the given index.
    ///
    /// Panics if the index is out of bounds.
    #[inline]
    pub fn position_of(&self, index: u32) -> Vector2F {
        self.points[index as usize]
    }

    /// Returns the position of the first point in this subpath.
    #[inline]
    pub fn first_position(&self) -> Option<Vector2F> {
        self.points.first().cloned()
    }

    /// Returns the position of the last point in this subpath.
    #[inline]
    pub fn last_position(&self) -> Option<Vector2F> {
        self.points.last().cloned()
    }

    #[inline]
    pub(crate) fn position_of_last(&self, index: u32) -> Vector2F {
        self.points[self.points.len() - index as usize]
    }

    /// Returns a set of flags that describes the type of the point with the given index.
    ///
    /// Panics if the index is out of range.
    #[inline]
    pub fn flags_of(&self, index: u32) -> PointFlags {
        self.flags[index as usize]
    }

    /// Adds a new on-curve point at the given position to this contour.
    #[inline]
    pub fn push_endpoint(&mut self, to: Vector2F) {
        self.push_point(to, PointFlags::empty(), true);
    }

    /// Adds a new quadratic Bézier curve to the given on-curve position and control point to this
    /// contour.
    #[inline]
    pub fn push_quadratic(&mut self, ctrl: Vector2F, to: Vector2F) {
        self.push_point(ctrl, PointFlags::CONTROL_POINT_0, true);
        self.push_point(to, PointFlags::empty(), true);
    }

    /// Adds a new cubic Bézier curve to the given on-curve position and control points to this
    /// contour.
    #[inline]
    pub fn push_cubic(&mut self, ctrl0: Vector2F, ctrl1: Vector2F, to: Vector2F) {
        self.push_point(ctrl0, PointFlags::CONTROL_POINT_0, true);
        self.push_point(ctrl1, PointFlags::CONTROL_POINT_1, true);
        self.push_point(to, PointFlags::empty(), true);
    }

    /// Marks this contour as closed, which results in an implicit line from the end back to the
    /// starting point.
    #[inline]
    pub fn close(&mut self) {
        self.closed = true;
    }

    #[inline]
    pub(crate) fn push_point(&mut self,
                             point: Vector2F,
                             flags: PointFlags,
                             update_bounds: bool) {
        debug_assert!(!point.x().is_nan() && !point.y().is_nan());

        if update_bounds {
            let first = self.is_empty();
            union_rect(&mut self.bounds, point, first);
        }

        self.points.push(point);
        self.flags.push(flags);
    }

    #[inline]
    pub(crate) fn push_segment(&mut self, segment: &Segment, flags: PushSegmentFlags) {
        if segment.is_none() {
            return;
        }

        let update_bounds = flags.contains(PushSegmentFlags::UPDATE_BOUNDS);
        self.push_point(segment.baseline.from(), PointFlags::empty(), update_bounds);

        if !segment.is_line() {
            self.push_point(
                segment.ctrl.from(),
                PointFlags::CONTROL_POINT_0,
                update_bounds,
            );
            if !segment.is_quadratic() {
                self.push_point(
                    segment.ctrl.to(),
                    PointFlags::CONTROL_POINT_1,
                    update_bounds,
                );
            }
        }

        self.push_point(segment.baseline.to(), PointFlags::empty(), update_bounds);
    }

    /// Adds Bézier curves approximating a possibly-transformed unit arc to this contour.
    ///
    /// Arguments:
    ///
    /// * `transform`: An affine transform to apply to the unit arc. This can be used to reposition
    ///   and resize the arc.
    ///
    /// * `start_angle`: The starting angle in radians. 0 represents the +x axis.
    ///
    /// * `end_angle`: The ending angle in radians. 0 represents the +x axis.
    ///
    /// * `direction`: Whether the arc should be drawn clockwise or counterclockwise from the +x
    ///   axis.
    pub fn push_arc(&mut self,
                    transform: &Transform2F,
                    start_angle: f32,
                    end_angle: f32,
                    direction: ArcDirection) {
        if end_angle - start_angle >= PI * 2.0 {
            self.push_ellipse(transform);
        } else {
            let start = vec2f(start_angle.cos(), start_angle.sin());
            let end   = vec2f(end_angle.cos(),   end_angle.sin());
            self.push_arc_from_unit_chord(transform, LineSegment2F::new(start, end), direction);
        }
    }

    /// Given the endpoints of a unit arc, adds Bézier curves to approximate that arc to the
    /// current contour. The given transform is applied to the resulting arc.
    pub fn push_arc_from_unit_chord(&mut self,
                                    transform: &Transform2F,
                                    mut chord: LineSegment2F,
                                    direction: ArcDirection) {
        let mut direction_transform = Transform2F::default();
        if direction == ArcDirection::CCW {
            chord *= vec2f(1.0, -1.0);
            direction_transform = Transform2F::from_scale(vec2f(1.0, -1.0));
        }

        let (mut vector, end_vector) = (UnitVector(chord.from()), UnitVector(chord.to()));
        for segment_index in 0..4 {
            debug!("push_arc_from_unit_chord(): loop segment index {}", segment_index);

            let mut sweep_vector = end_vector.rev_rotate_by(vector);
            let last = sweep_vector.0.x() >= -EPSILON && sweep_vector.0.y() >= -EPSILON;
            debug!("... end_vector={:?} vector={:?} sweep_vector={:?} last={:?}",
                   end_vector,
                   vector,
                   sweep_vector,
                   last);

            let mut segment;
            if !last {
                sweep_vector = UnitVector(vec2f(0.0, 1.0));
                segment = Segment::quarter_circle_arc();
            } else {
                segment = Segment::arc_from_cos(sweep_vector.0.x());
            }

            let half_sweep_vector = sweep_vector.halve_angle();
            let rotation = Transform2F::from_rotation_vector(half_sweep_vector.rotate_by(vector));
            segment = segment.transform(&(*transform * direction_transform * rotation));

            let mut push_segment_flags = PushSegmentFlags::UPDATE_BOUNDS;
            if segment_index == 0 {
                push_segment_flags.insert(PushSegmentFlags::INCLUDE_FROM_POINT);
            }
            self.push_segment(&segment, push_segment_flags);

            if last {
                break;
            }

            vector = vector.rotate_by(sweep_vector);
        }

        const EPSILON: f32 = 0.001;
    }

    /// Adds an arc specified in SVG form to the current contour.
    ///
    /// Draws an ellipse section with radii given by `radius` rotated by `x_axis_rotation` in
    /// radians to `to` in the given direction. If `large_arc` is true, draws an arc bigger than
    /// π radians, otherwise smaller than π radians.
    pub fn push_svg_arc(&mut self,
                        radius: Vector2F,
                        x_axis_rotation: f32,
                        large_arc: bool,
                        direction: ArcDirection,
                        to: Vector2F) {
        let r = radius;
        let p = to;
        let last = self.last_position().unwrap_or_default();

        if r.x().is_finite() & r.y().is_finite() {
            let r = r.abs();
            let r_inv = r.recip();
            let sign = match (large_arc, direction) {
                (false, ArcDirection::CW) | (true, ArcDirection::CCW) => 1.0,
                (false, ArcDirection::CCW) | (true, ArcDirection::CW) => -1.0
            };
            let rot = Matrix2x2F::from_rotation(x_axis_rotation);
            // x'
            let q = rot.adjugate() * (last - p) * 0.5;
            let q2 = q * q;

            let gamma = q2 * r_inv * r_inv;
            let gamma = gamma.x() + gamma.y();

            let (a, b, c) = if gamma <= 1.0 {
                // normal case
                let r2 = r * r;

                let r2_prod = r2.x() * r2.y(); // r_x^2 r_y^2

                let rq2 = r2 * q2.yx(); // (r_x^2 q_y^2, r_y^2 q_x^2)
                let rq2_sum = rq2.x() + rq2.y(); // r_x^2 q_y^2 + r_y^2 q_x^2
                // c'
                let s = vec2f(1., -1.) * r * (q * r_inv).yx() * safe_sqrt((r2_prod - rq2_sum) / rq2_sum) * sign;
                let c = rot * s + (last + p) * 0.5;
                
                let a = (q - s) * r_inv;
                let b = -(q + s) * r_inv;
                (a, b, c)
            } else {
                let c = (last + p) * 0.5;
                let a = q * r_inv;
                let b = -a;
                (a, b, c)
            };
            
            let transform = Transform2F {
                matrix: rot,
                vector: c
            } * Transform2F::from_scale(r);
            let chord = LineSegment2F::new(a, b);
            self.push_arc_from_unit_chord(&transform, chord, direction);
        } else {
            self.push_endpoint(p);
        }
    }

    /// Adds an unit circle to this contour, transformed with the given transform.
    ///
    /// Non-uniform scales can be used to transform this circle into an ellipse.
    pub fn push_ellipse(&mut self, transform: &Transform2F) {
        let segment = Segment::quarter_circle_arc();
        let mut rotation;
        self.push_segment(&segment.transform(transform),
                          PushSegmentFlags::UPDATE_BOUNDS | PushSegmentFlags::INCLUDE_FROM_POINT);
        rotation = Transform2F::from_rotation_vector(UnitVector(vec2f( 0.0,  1.0)));
        self.push_segment(&segment.transform(&(*transform * rotation)),
                          PushSegmentFlags::UPDATE_BOUNDS);
        rotation = Transform2F::from_rotation_vector(UnitVector(vec2f(-1.0,  0.0)));
        self.push_segment(&segment.transform(&(*transform * rotation)),
                          PushSegmentFlags::UPDATE_BOUNDS);
        rotation = Transform2F::from_rotation_vector(UnitVector(vec2f( 0.0, -1.0)));
        self.push_segment(&segment.transform(&(*transform * rotation)),
                          PushSegmentFlags::UPDATE_BOUNDS);
    }

    /// Returns the segment starting at the point with the given index.
    ///
    /// The index must represent an on-curve point.
    ///
    /// Panics if `point_index` is out of range.
    #[inline]
    pub fn segment_after(&self, point_index: u32) -> Segment {
        debug_assert!(self.point_is_endpoint(point_index));

        let mut segment = Segment::none();
        segment.baseline.set_from(self.position_of(point_index));

        let point1_index = self.add_to_point_index(point_index, 1);
        if self.point_is_endpoint(point1_index) {
            segment.baseline.set_to(self.position_of(point1_index));
            segment.kind = SegmentKind::Line;
        } else {
            segment.ctrl.set_from(self.position_of(point1_index));

            let point2_index = self.add_to_point_index(point_index, 2);
            if self.point_is_endpoint(point2_index) {
                segment.baseline.set_to(self.position_of(point2_index));
                segment.kind = SegmentKind::Quadratic;
            } else {
                segment.ctrl.set_to(self.position_of(point2_index));
                segment.kind = SegmentKind::Cubic;

                let point3_index = self.add_to_point_index(point_index, 3);
                segment.baseline.set_to(self.position_of(point3_index));
            }
        }

        segment
    }

    /// Returns a line segment from the point with the given index to the next point, whether
    /// on-curve or off-curve.
    ///
    /// Panics if `prev_point_index` is not in range.
    #[inline]
    pub fn hull_segment_after(&self, prev_point_index: u32) -> LineSegment2F {
        let next_point_index = self.next_point_index_of(prev_point_index);
        LineSegment2F::new(
            self.points[prev_point_index as usize],
            self.points[next_point_index as usize],
        )
    }

    /// Returns true if the given point is on-curve or false if it is off-curve.
    #[inline]
    pub fn point_is_endpoint(&self, point_index: u32) -> bool {
        !self.flags[point_index as usize]
             .intersects(PointFlags::CONTROL_POINT_0 | PointFlags::CONTROL_POINT_1)
    }

    /// Returns `point_index + addend` modulo the number of points in this contour.
    #[inline]
    pub fn add_to_point_index(&self, point_index: u32, addend: u32) -> u32 {
        let (index, limit) = (point_index + addend, self.len());
        if index >= limit {
            index - limit
        } else {
            index
        }
    }

    /// Returns the first on-curve point strictly before the point with the given index.
    ///
    /// This takes closed paths into account, so the returned index might be greater than
    /// `point_index`.
    #[inline]
    pub fn prev_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.prev_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index;
            }
        }
    }

    /// Returns the first on-curve point strictly after the point with the given index.
    ///
    /// This takes closed paths into account, so the returned index might be less than
    /// `point_index`.
    #[inline]
    pub fn next_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.next_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index;
            }
        }
    }

    /// Returns the index of the point before the given `point_index`.
    ///
    /// If the index of the first point is passed in, then this returns the index of the last
    /// point.
    #[inline]
    pub fn prev_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == 0 {
            self.len() - 1
        } else {
            point_index - 1
        }
    }

    /// Returns the index of the point after the given `point_index`.
    ///
    /// If the index of the last point is passed in, then this returns the index of the first
    /// point.
    #[inline]
    pub fn next_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == self.len() - 1 {
            0
        } else {
            point_index + 1
        }
    }

    /// Applies the given affine transform to this subpath.
    pub fn transform(&mut self, transform: &Transform2F) {
        if transform.is_identity() {
            return;
        }

        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = *transform * *point;
            union_rect(&mut self.bounds, *point, point_index == 0);
        }
    }

    /// Applies the given affine transform to this contour, returning a new contour instead of
    /// mutating this one.
    #[inline]
    pub fn transformed(mut self, transform: &Transform2F) -> Contour {
        self.transform(transform);
        self
    }

    /// Applies a perspective transform to this subpath.
    #[deprecated]
    pub fn apply_perspective(&mut self, perspective: &Perspective) {
        for (point_index, point) in self.points.iter_mut().enumerate() {
            *point = *perspective * *point;
            union_rect(&mut self.bounds, *point, point_index == 0);
        }
    }

    /// Thickens the outline by the given amount. The `orientation` parameter specifies the winding
    /// of the path (clockwise or counterclockwise) and is necessary to avoid flipped normals.
    pub fn dilate(&mut self, amount: Vector2F, orientation: Orientation) {
        ContourDilator::new(self, amount, orientation).dilate();
        self.bounds = self.bounds.dilate(amount);
    }

    // Use this function to keep bounds up to date when mutating paths. See `Outline::transform()`
    // for an example of use.
    pub(crate) fn update_bounds(&self, bounds: &mut Option<RectF>) {
        *bounds = Some(match *bounds {
            None => self.bounds,
            Some(bounds) => bounds.union_rect(self.bounds),
        })
    }
}

impl Debug for Contour {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        for (segment_index, segment) in self.iter(ContourIterFlags::IGNORE_CLOSE_SEGMENT)
                                            .enumerate() {
            if segment_index == 0 {
                write!(
                    formatter,
                    "M {} {}",
                    segment.baseline.from_x(),
                    segment.baseline.from_y()
                )?;
            }

            match segment.kind {
                SegmentKind::None => {}
                SegmentKind::Line => {
                    write!(
                        formatter,
                        " L {} {}",
                        segment.baseline.to_x(),
                        segment.baseline.to_y()
                    )?;
                }
                SegmentKind::Quadratic => {
                    write!(
                        formatter,
                        " Q {} {} {} {}",
                        segment.ctrl.from_x(),
                        segment.ctrl.from_y(),
                        segment.baseline.to_x(),
                        segment.baseline.to_y()
                    )?;
                }
                SegmentKind::Cubic => {
                    write!(
                        formatter,
                        " C {} {} {} {} {} {}",
                        segment.ctrl.from_x(),
                        segment.ctrl.from_y(),
                        segment.ctrl.to_x(),
                        segment.ctrl.to_y(),
                        segment.baseline.to_x(),
                        segment.baseline.to_y()
                    )?;
                }
            }
        }

        if self.closed {
            write!(formatter, " z")?;
        }

        Ok(())
    }
}

/// The index of a point within an outline, either on-curve or off-curve.
///
/// This packs a contour index with a point index into a single 32-bit value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointIndex(u32);

impl PointIndex {
    /// Packs a contour index and the index of a point within that contour into a single value.
    #[inline]
    pub fn new(contour: u32, point: u32) -> PointIndex {
        debug_assert!(contour <= 0xfff);
        debug_assert!(point <= 0x000f_ffff);
        PointIndex((contour << 20) | point)
    }

    /// Extracts the index of the contour and returns it.
    #[inline]
    pub fn contour(self) -> u32 {
        self.0 >> 20
    }

    /// Extracts the index of the point within that contour and returns it.
    #[inline]
    pub fn point(self) -> u32 {
        self.0 & 0x000f_ffff
    }
}

/// Iterates over all Bézier segments within a contour.
pub struct ContourIter<'a> {
    contour: &'a Contour,
    index: u32,
    flags: ContourIterFlags,
}

impl<'a> Iterator for ContourIter<'a> {
    type Item = Segment;

    #[inline]
    fn next(&mut self) -> Option<Segment> {
        let contour = self.contour;

        let include_close_segment = self.contour.closed &&
            !self.flags.contains(ContourIterFlags::IGNORE_CLOSE_SEGMENT);
        if (self.index == contour.len() && !include_close_segment) ||
                self.index == contour.len() + 1 {
            return None;
        }

        let point0_index = self.index - 1;
        let point0 = contour.position_of(point0_index);
        if self.index == contour.len() {
            let point1 = contour.position_of(0);
            self.index += 1;
            return Some(Segment::line(LineSegment2F::new(point0, point1)));
        }

        let point1_index = self.index;
        self.index += 1;
        let point1 = contour.position_of(point1_index);
        if contour.point_is_endpoint(point1_index) {
            return Some(Segment::line(LineSegment2F::new(point0, point1)));
        }

        let point2_index = self.index;
        let point2 = contour.position_of(point2_index);
        self.index += 1;
        if contour.point_is_endpoint(point2_index) {
            return Some(Segment::quadratic(LineSegment2F::new(point0, point2), point1));
        }

        let point3_index = self.index;
        let point3 = contour.position_of(point3_index);
        self.index += 1;
        debug_assert!(contour.point_is_endpoint(point3_index));
        return Some(Segment::cubic(
            LineSegment2F::new(point0, point3),
            LineSegment2F::new(point1, point2),
        ));
    }
}

/// The direction of an arc: clockwise or counterclockwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArcDirection {
    /// Clockwise, starting from the +x axis.
    CW,
    /// Counterclockwise, starting from the +x axis.
    CCW,
}

bitflags! {
    /// Flags that control the behavior of `Contour::iter()`.
    pub struct ContourIterFlags: u8 {
        /// Set to true to avoid iterating over the implicit line segment that joins the last point
        /// to the first point for closed contours.
        const IGNORE_CLOSE_SEGMENT = 1;
    }
}

#[inline]
pub(crate) fn union_rect(bounds: &mut RectF, new_point: Vector2F, first: bool) {
    if first {
        *bounds = RectF::from_points(new_point, new_point);
    } else {
        *bounds = bounds.union_point(new_point)
    }
}
