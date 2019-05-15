// pathfinder/renderer/src/tiles.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::builder::SceneBuilder;
use crate::gpu_data::{AlphaTileBatchPrimitive, BuiltObject, TileObjectPrimitive};
use crate::paint::{self, BuiltPalette, PaintId};
use crate::sorted_vector::SortedVector;
use pathfinder_geometry::basic::line_segment::LineSegmentF32;
use pathfinder_geometry::basic::point::{Point2DF32, Point2DI32};
use pathfinder_geometry::basic::rect::{RectF32, RectI32};
use pathfinder_geometry::outline::{Contour, Outline, PointIndex};
use pathfinder_geometry::segment::Segment;
use std::cmp::Ordering;
use std::mem;

// TODO(pcwalton): Make this configurable.
const FLATTENING_TOLERANCE: f32 = 0.1;

pub const TILE_WIDTH: u32 = 16;
pub const TILE_HEIGHT: u32 = 16;

pub(crate) struct Tiler<'a> {
    builder: &'a SceneBuilder<'a>,
    outline: &'a Outline,
    pub built_object: BuiltObject,
    paint_id: PaintId,
    object_index: u16,
    object_is_opaque: bool,

    point_queue: SortedVector<QueuedEndpoint>,
    active_edges: SortedVector<ActiveEdge>,
    old_active_edges: Vec<ActiveEdge>,
}

impl<'a> Tiler<'a> {
    #[allow(clippy::or_fun_call)]
    pub(crate) fn new(
        builder: &'a SceneBuilder<'a>,
        outline: &'a Outline,
        view_box: RectF32,
        object_index: u16,
        paint_id: PaintId,
        object_is_opaque: bool,
    ) -> Tiler<'a> {
        let bounds = outline
            .bounds()
            .intersection(view_box)
            .unwrap_or(RectF32::default());
        let built_object = BuiltObject::new(bounds);

        Tiler {
            builder,
            outline,
            built_object,
            object_index,
            paint_id,
            object_is_opaque,

            point_queue: SortedVector::new(),
            active_edges: SortedVector::new(),
            old_active_edges: vec![],
        }
    }

    pub(crate) fn generate_tiles(&mut self) {
        // Initialize the point queue.
        self.init_point_queue();

        // Reset active edges.
        self.active_edges.clear();
        self.old_active_edges.clear();

        // Generate strips.
        let tile_rect = self.built_object.tile_rect();
        for strip_origin_y in tile_rect.min_y()..tile_rect.max_y() {
            self.generate_strip(strip_origin_y);
        }

        // Pack and cull.
        self.pack_and_cull();

        // Done!
        debug!("{:#?}", self.built_object);
    }

    fn generate_strip(&mut self, strip_origin_y: i32) {
        // Process old active edges.
        self.process_old_active_edges(strip_origin_y);

        // Add new active edges.
        let strip_max_y = ((i32::from(strip_origin_y) + 1) * TILE_HEIGHT as i32) as f32;
        while let Some(queued_endpoint) = self.point_queue.peek() {
            // We're done when we see an endpoint that belongs to the next tile strip.
            //
            // Note that this test must be `>`, not `>=`, in order to make sure we don't miss
            // active edges that lie precisely on the tile strip boundary.
            if queued_endpoint.y > strip_max_y {
                break;
            }

            self.add_new_active_edge(strip_origin_y);
        }
    }

    fn pack_and_cull(&mut self) {
        for (tile_index, tile) in self.built_object.tiles.data.iter().enumerate() {
            let tile_coords = self
                .built_object
                .local_tile_index_to_coords(tile_index as u32);

            if tile.is_solid() {
                // Blank tiles are always skipped.
                if tile.backdrop == 0 {
                    continue;
                }

                // If this is a solid tile, poke it into the Z-buffer and stop here.
                if self.object_is_opaque {
                    self.builder.z_buffer.update(tile_coords, self.object_index);
                    continue;
                }
            }

            let origin_uv = self.builder.built_palette.norm_tex_coords(self.paint_id) +
                BuiltPalette::half_texel();

            let alpha_tile = AlphaTileBatchPrimitive::new(
                tile_coords,
                tile.backdrop,
                self.object_index,
                tile.alpha_tile_index as u16,
                origin_uv,
            );

            self.built_object.alpha_tiles.push(alpha_tile);
        }
    }

    fn process_old_active_edges(&mut self, tile_y: i32) {
        let mut current_tile_x = self.built_object.tile_rect().min_x();
        let mut current_subtile_x = 0.0;
        let mut current_winding = 0;

        debug_assert!(self.old_active_edges.is_empty());
        mem::swap(&mut self.old_active_edges, &mut self.active_edges.array);

        // FIXME(pcwalton): Yuck.
        let mut last_segment_x = -9999.0;

        let tile_top = (i32::from(tile_y) * TILE_HEIGHT as i32) as f32;

        debug!("---------- tile y {}({}) ----------", tile_y, tile_top);
        debug!("old active edges: {:#?}", self.old_active_edges);

        for mut active_edge in self.old_active_edges.drain(..) {
            // Determine x-intercept and winding.
            let segment_x = active_edge.crossing.x();
            let edge_winding =
                if active_edge.segment.baseline.from_y() < active_edge.segment.baseline.to_y() {
                    1
                } else {
                    -1
                };

            debug!(
                "tile Y {}({}): segment_x={} edge_winding={} current_tile_x={} \
                 current_subtile_x={} current_winding={}",
                tile_y,
                tile_top,
                segment_x,
                edge_winding,
                current_tile_x,
                current_subtile_x,
                current_winding
            );
            debug!(
                "... segment={:#?} crossing={:?}",
                active_edge.segment, active_edge.crossing
            );

            // FIXME(pcwalton): Remove this debug code!
            debug_assert!(segment_x >= last_segment_x);
            last_segment_x = segment_x;

            // Do initial subtile fill, if necessary.
            let segment_tile_x = f32::floor(segment_x) as i32 / TILE_WIDTH as i32;
            if current_tile_x < segment_tile_x && current_subtile_x > 0.0 {
                let current_x =
                    (i32::from(current_tile_x) * TILE_WIDTH as i32) as f32 + current_subtile_x;
                let tile_right_x = ((i32::from(current_tile_x) + 1) * TILE_WIDTH as i32) as f32;
                let current_tile_coords = Point2DI32::new(current_tile_x, tile_y);
                self.built_object.add_active_fill(
                    self.builder,
                    current_x,
                    tile_right_x,
                    current_winding,
                    current_tile_coords,
                );
                current_tile_x += 1;
                current_subtile_x = 0.0;
            }

            // Move over to the correct tile, filling in as we go.
            while current_tile_x < segment_tile_x {
                debug!(
                    "... emitting backdrop {} @ tile {}",
                    current_winding, current_tile_x
                );
                let current_tile_coords = Point2DI32::new(current_tile_x, tile_y);
                if let Some(tile_index) = self
                    .built_object
                    .tile_coords_to_local_index(current_tile_coords)
                {
                    // FIXME(pcwalton): Handle winding overflow.
                    self.built_object.tiles.data[tile_index as usize].backdrop =
                        current_winding as i8;
                }

                current_tile_x += 1;
                current_subtile_x = 0.0;
            }

            // Do final subtile fill, if necessary.
            debug_assert_eq!(current_tile_x, segment_tile_x);
            let segment_subtile_x =
                segment_x - (i32::from(current_tile_x) * TILE_WIDTH as i32) as f32;
            if segment_subtile_x > current_subtile_x {
                let current_x =
                    (i32::from(current_tile_x) * TILE_WIDTH as i32) as f32 + current_subtile_x;
                let current_tile_coords = Point2DI32::new(current_tile_x, tile_y);
                self.built_object.add_active_fill(
                    self.builder,
                    current_x,
                    segment_x,
                    current_winding,
                    current_tile_coords,
                );
                current_subtile_x = segment_subtile_x;
            }

            // Update winding.
            current_winding += edge_winding;

            // Process the edge.
            debug!("about to process existing active edge {:#?}", active_edge);
            debug_assert!(f32::abs(active_edge.crossing.y() - tile_top) < 0.1);
            active_edge.process(self.builder, &mut self.built_object, tile_y);
            if !active_edge.segment.is_none() {
                self.active_edges.push(active_edge);
            }
        }
    }

    fn add_new_active_edge(&mut self, tile_y: i32) {
        let outline = &self.outline;
        let point_index = self.point_queue.pop().unwrap().point_index;

        let contour = &outline.contours()[point_index.contour() as usize];

        // TODO(pcwalton): Could use a bitset of processed edges…
        let prev_endpoint_index = contour.prev_endpoint_index_of(point_index.point());
        let next_endpoint_index = contour.next_endpoint_index_of(point_index.point());

        debug!(
            "adding new active edge, tile_y={} point_index={} prev={} next={} pos={:?} \
             prevpos={:?} nextpos={:?}",
            tile_y,
            point_index.point(),
            prev_endpoint_index,
            next_endpoint_index,
            contour.position_of(point_index.point()),
            contour.position_of(prev_endpoint_index),
            contour.position_of(next_endpoint_index)
        );

        if contour.point_is_logically_above(point_index.point(), prev_endpoint_index) {
            debug!("... adding prev endpoint");

            process_active_segment(
                contour,
                prev_endpoint_index,
                &mut self.active_edges,
                self.builder,
                &mut self.built_object,
                tile_y,
            );

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), prev_endpoint_index),
                y: contour.position_of(prev_endpoint_index).y(),
            });

            debug!("... done adding prev endpoint");
        }

        if contour.point_is_logically_above(point_index.point(), next_endpoint_index) {
            debug!(
                "... adding next endpoint {} -> {}",
                point_index.point(),
                next_endpoint_index
            );

            process_active_segment(
                contour,
                point_index.point(),
                &mut self.active_edges,
                self.builder,
                &mut self.built_object,
                tile_y,
            );

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), next_endpoint_index),
                y: contour.position_of(next_endpoint_index).y(),
            });

            debug!("... done adding next endpoint");
        }
    }

    fn init_point_queue(&mut self) {
        // Find MIN points.
        self.point_queue.clear();
        for (contour_index, contour) in self.outline.contours().iter().enumerate() {
            let contour_index = contour_index as u32;
            let mut cur_endpoint_index = 0;
            let mut prev_endpoint_index = contour.prev_endpoint_index_of(cur_endpoint_index);
            let mut next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            loop {
                if contour.point_is_logically_above(cur_endpoint_index, prev_endpoint_index)
                    && contour.point_is_logically_above(cur_endpoint_index, next_endpoint_index)
                {
                    self.point_queue.push(QueuedEndpoint {
                        point_index: PointIndex::new(contour_index, cur_endpoint_index),
                        y: contour.position_of(cur_endpoint_index).y(),
                    });
                }

                if cur_endpoint_index >= next_endpoint_index {
                    break;
                }

                prev_endpoint_index = cur_endpoint_index;
                cur_endpoint_index = next_endpoint_index;
                next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            }
        }
    }
}

pub fn round_rect_out_to_tile_bounds(rect: RectF32) -> RectI32 {
    rect.scale_xy(Point2DF32::new(
        1.0 / TILE_WIDTH as f32,
        1.0 / TILE_HEIGHT as f32,
    ))
    .round_out()
    .to_i32()
}

fn process_active_segment(
    contour: &Contour,
    from_endpoint_index: u32,
    active_edges: &mut SortedVector<ActiveEdge>,
    builder: &SceneBuilder,
    built_object: &mut BuiltObject,
    tile_y: i32,
) {
    let mut active_edge = ActiveEdge::from_segment(&contour.segment_after(from_endpoint_index));
    debug!("... process_active_segment({:#?})", active_edge);
    active_edge.process(builder, built_object, tile_y);
    if !active_edge.segment.is_none() {
        debug!("... ... pushing resulting active edge: {:#?}", active_edge);
        active_edges.push(active_edge);
    }
}

// Queued endpoints

#[derive(PartialEq)]
struct QueuedEndpoint {
    point_index: PointIndex,
    y: f32,
}

impl Eq for QueuedEndpoint {}

impl PartialOrd<QueuedEndpoint> for QueuedEndpoint {
    fn partial_cmp(&self, other: &QueuedEndpoint) -> Option<Ordering> {
        // NB: Reversed!
        (other.y, other.point_index).partial_cmp(&(self.y, self.point_index))
    }
}

// Active edges

#[derive(Clone, PartialEq, Debug)]
struct ActiveEdge {
    segment: Segment,
    // TODO(pcwalton): Shrink `crossing` down to just one f32?
    crossing: Point2DF32,
}

impl ActiveEdge {
    fn from_segment(segment: &Segment) -> ActiveEdge {
        let crossing = if segment.baseline.from_y() < segment.baseline.to_y() {
            segment.baseline.from()
        } else {
            segment.baseline.to()
        };
        ActiveEdge::from_segment_and_crossing(segment, &crossing)
    }

    fn from_segment_and_crossing(segment: &Segment, crossing: &Point2DF32) -> ActiveEdge {
        ActiveEdge {
            segment: *segment,
            crossing: *crossing,
        }
    }

    fn process(&mut self, builder: &SceneBuilder, built_object: &mut BuiltObject, tile_y: i32) {
        let tile_bottom = ((i32::from(tile_y) + 1) * TILE_HEIGHT as i32) as f32;
        debug!(
            "process_active_edge({:#?}, tile_y={}({}))",
            self, tile_y, tile_bottom
        );

        let mut segment = self.segment;
        let winding = segment.baseline.y_winding();

        if segment.is_line() {
            let line_segment = segment.as_line_segment();
            self.segment =
                match self.process_line_segment(&line_segment, builder, built_object, tile_y) {
                    Some(lower_part) => Segment::line(&lower_part),
                    None => Segment::none(),
                };
            return;
        }

        // TODO(pcwalton): Don't degree elevate!
        if !segment.is_cubic() {
            segment = segment.to_cubic();
        }

        // If necessary, draw initial line.
        if self.crossing.y() < segment.baseline.min_y() {
            let first_line_segment =
                LineSegmentF32::new(self.crossing, segment.baseline.upper_point()).orient(winding);
            if self
                .process_line_segment(&first_line_segment, builder, built_object, tile_y)
                .is_some()
            {
                return;
            }
        }

        let mut oriented_segment = segment.orient(winding);
        loop {
            let mut split_t = 1.0;
            let mut before_segment = oriented_segment;
            let mut after_segment = None;

            while !before_segment
                .as_cubic_segment()
                .is_flat(FLATTENING_TOLERANCE)
            {
                let next_t = 0.5 * split_t;
                let (before, after) = oriented_segment.as_cubic_segment().split(next_t);
                before_segment = before;
                after_segment = Some(after);
                split_t = next_t;
            }

            debug!(
                "... tile_y={} winding={} segment={:?} t={} before_segment={:?}
                    after_segment={:?}",
                tile_y, winding, segment, split_t, before_segment, after_segment
            );

            let line = before_segment.baseline.orient(winding);
            match self.process_line_segment(&line, builder, built_object, tile_y) {
                Some(ref lower_part) if split_t == 1.0 => {
                    self.segment = Segment::line(&lower_part);
                    return;
                }
                None if split_t == 1.0 => {
                    self.segment = Segment::none();
                    return;
                }
                Some(_) => {
                    self.segment = after_segment.unwrap().orient(winding);
                    return;
                }
                None => oriented_segment = after_segment.unwrap(),
            }
        }
    }

    fn process_line_segment(
        &mut self,
        line_segment: &LineSegmentF32,
        builder: &SceneBuilder,
        built_object: &mut BuiltObject,
        tile_y: i32,
    ) -> Option<LineSegmentF32> {
        let tile_bottom = ((i32::from(tile_y) + 1) * TILE_HEIGHT as i32) as f32;
        debug!(
            "process_line_segment({:?}, tile_y={}) tile_bottom={}",
            line_segment, tile_y, tile_bottom
        );

        if line_segment.max_y() <= tile_bottom {
            built_object.generate_fill_primitives_for_line(builder, *line_segment, tile_y);
            return None;
        }

        let (upper_part, lower_part) = line_segment.split_at_y(tile_bottom);
        built_object.generate_fill_primitives_for_line(builder, upper_part, tile_y);
        self.crossing = lower_part.upper_point();
        Some(lower_part)
    }
}

impl PartialOrd<ActiveEdge> for ActiveEdge {
    fn partial_cmp(&self, other: &ActiveEdge) -> Option<Ordering> {
        self.crossing.x().partial_cmp(&other.crossing.x())
    }
}

impl AlphaTileBatchPrimitive {
    #[inline]
    fn new(tile_coords: Point2DI32,
           backdrop: i8,
           object_index: u16,
           tile_index: u16,
           origin_uv: Point2DI32)
           -> AlphaTileBatchPrimitive {
        AlphaTileBatchPrimitive {
            tile_x_lo: (tile_coords.x() & 0xff) as u8,
            tile_y_lo: (tile_coords.y() & 0xff) as u8,
            tile_hi: (((tile_coords.x() >> 8) & 0x0f) | ((tile_coords.y() >> 4) & 0xf0)) as u8,
            backdrop,
            object_index,
            tile_index,
            origin_u: origin_uv.x() as u16,
            origin_v: origin_uv.y() as u16,
        }
    }

    #[inline]
    pub fn tile_coords(&self) -> Point2DI32 {
        Point2DI32::new(
            (self.tile_x_lo as i32) | (((self.tile_hi & 0xf) as i32) << 8),
            (self.tile_y_lo as i32) | (((self.tile_hi & 0xf0) as i32) << 4),
        )
    }
}

impl Default for TileObjectPrimitive {
    #[inline]
    fn default() -> TileObjectPrimitive {
        TileObjectPrimitive { backdrop: 0, alpha_tile_index: !0 }
    }
}

impl TileObjectPrimitive {
    #[inline]
    pub fn is_solid(&self) -> bool { self.alpha_tile_index == !0 }
}
