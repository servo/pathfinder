// pathfinder/svg/src/lib.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Converts a subset of SVG to a Pathfinder scene.

use lyon_path::iterator::PathIter;
use pathfinder_geometry::basic::line_segment::LineSegmentF32;
use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectF32;
use pathfinder_geometry::basic::transform2d::{Transform2DF32, Transform2DF32PathIter};
use pathfinder_geometry::outline::Outline;
use pathfinder_geometry::segment::{PathEventsToSegments, Segment};
use pathfinder_geometry::segment::{SegmentFlags, SegmentsToPathEvents};
use pathfinder_geometry::stroke::OutlineStrokeToFill;
use pathfinder_renderer::paint::{ColorU, Paint};
use pathfinder_renderer::scene::{PathObject, PathObjectKind, Scene};
use std::mem;
use usvg::{Color as SvgColor, Node, NodeExt, NodeKind, Paint as UsvgPaint};
use usvg::{PathSegment as UsvgPathSegment, Rect as UsvgRect, Transform as UsvgTransform, Tree};

const HAIRLINE_STROKE_WIDTH: f32 = 0.1;

pub trait SceneExt {
    fn from_tree(tree: Tree) -> Self;
}

impl SceneExt for Scene {
    // TODO(pcwalton): Allow a global transform to be set.
    fn from_tree(tree: Tree) -> Scene {
        let global_transform = Transform2DF32::default();

        let mut scene = Scene::new();

        let root = &tree.root();
        match *root.borrow() {
            NodeKind::Svg(ref svg) => {
                println!("view_box={:?}", svg.view_box);
                scene.view_box = usvg_rect_to_euclid_rect(&svg.view_box.rect);
                for kid in root.children() {
                    process_node(&mut scene, &kid, &global_transform);
                }
            }
            _ => unreachable!(),
        };

        // FIXME(pcwalton): This is needed to avoid stack exhaustion in debug builds when
        // recursively dropping reference counts on very large SVGs. :(
        mem::forget(tree);

        scene
    }
}

fn process_node(scene: &mut Scene, node: &Node, transform: &Transform2DF32) {
    let node_transform = usvg_transform_to_transform_2d(&node.transform());
    let transform = transform.pre_mul(&node_transform);

    match *node.borrow() {
        NodeKind::Group(_) => {
            for kid in node.children() {
                process_node(scene, &kid, &transform)
            }
        }
        NodeKind::Path(ref path) => {
            if let Some(ref fill) = path.fill {
                let style = scene.push_paint(&Paint::from_svg_paint(&fill.paint));

                let path = UsvgPathToSegments::new(path.segments.iter().cloned());
                let path = Transform2DF32PathIter::new(path, &transform);
                let outline = Outline::from_segments(path);

                scene.bounds = scene.bounds.union_rect(outline.bounds());
                scene.objects.push(PathObject::new(
                    outline,
                    style,
                    node.id().to_string(),
                    PathObjectKind::Fill,
                ));
            }

            if let Some(ref stroke) = path.stroke {
                let style = scene.push_paint(&Paint::from_svg_paint(&stroke.paint));
                let stroke_width =
                    f32::max(stroke.width.value() as f32, HAIRLINE_STROKE_WIDTH);

                let path = UsvgPathToSegments::new(path.segments.iter().cloned());
                /*let path = SegmentsToPathEvents::new(path);
                let path = PathIter::new(path);
                let path = StrokeToFillIter::new(path, StrokeStyle::new(stroke_width));
                let path = PathEventsToSegments::new(path);*/
                let path = Transform2DF32PathIter::new(path, &transform);
                let outline = Outline::from_segments(path);

                let mut stroke_to_fill = OutlineStrokeToFill::new(outline, stroke_width);
                stroke_to_fill.offset();
                let outline = stroke_to_fill.outline;

                scene.bounds = scene.bounds.union_rect(outline.bounds());
                scene.objects.push(PathObject::new(
                    outline,
                    style,
                    node.id().to_string(),
                    PathObjectKind::Stroke,
                ));
            }
        }
        _ => {
            // TODO(pcwalton): Handle these by punting to WebRender.
        }
    }
}

trait PaintExt {
    fn from_svg_paint(svg_paint: &UsvgPaint) -> Self;
}

impl PaintExt for Paint {
    #[inline]
    fn from_svg_paint(svg_paint: &UsvgPaint) -> Paint {
        Paint {
            color: match *svg_paint {
                UsvgPaint::Color(color) => ColorU::from_svg_color(color),
                UsvgPaint::Link(_) => {
                    // TODO(pcwalton)
                    ColorU::black()
                }
            }
        }
    }
}

fn usvg_rect_to_euclid_rect(rect: &UsvgRect) -> RectF32 {
    RectF32::new(
        Point2DF32::new(rect.x as f32, rect.y as f32),
        Point2DF32::new(rect.width as f32, rect.height as f32),
    )
}

fn usvg_transform_to_transform_2d(transform: &UsvgTransform) -> Transform2DF32 {
    Transform2DF32::row_major(
        transform.a as f32,
        transform.b as f32,
        transform.c as f32,
        transform.d as f32,
        transform.e as f32,
        transform.f as f32,
    )
}

struct UsvgPathToSegments<I>
where
    I: Iterator<Item = UsvgPathSegment>,
{
    iter: I,
    first_subpath_point: Point2DF32,
    last_subpath_point: Point2DF32,
    just_moved: bool,
}

impl<I> UsvgPathToSegments<I>
where
    I: Iterator<Item = UsvgPathSegment>,
{
    fn new(iter: I) -> UsvgPathToSegments<I> {
        UsvgPathToSegments {
            iter,
            first_subpath_point: Point2DF32::default(),
            last_subpath_point: Point2DF32::default(),
            just_moved: false,
        }
    }
}

impl<I> Iterator for UsvgPathToSegments<I>
where
    I: Iterator<Item = UsvgPathSegment>,
{
    type Item = Segment;

    fn next(&mut self) -> Option<Segment> {
        match self.iter.next()? {
            UsvgPathSegment::MoveTo { x, y } => {
                let to = Point2DF32::new(x as f32, y as f32);
                self.first_subpath_point = to;
                self.last_subpath_point = to;
                self.just_moved = true;
                self.next()
            }
            UsvgPathSegment::LineTo { x, y } => {
                let to = Point2DF32::new(x as f32, y as f32);
                let mut segment =
                    Segment::line(&LineSegmentF32::new(&self.last_subpath_point, &to));
                if self.just_moved {
                    segment.flags.insert(SegmentFlags::FIRST_IN_SUBPATH);
                }
                self.last_subpath_point = to;
                self.just_moved = false;
                Some(segment)
            }
            UsvgPathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                let ctrl0 = Point2DF32::new(x1 as f32, y1 as f32);
                let ctrl1 = Point2DF32::new(x2 as f32, y2 as f32);
                let to = Point2DF32::new(x as f32, y as f32);
                let mut segment = Segment::cubic(
                    &LineSegmentF32::new(&self.last_subpath_point, &to),
                    &LineSegmentF32::new(&ctrl0, &ctrl1),
                );
                if self.just_moved {
                    segment.flags.insert(SegmentFlags::FIRST_IN_SUBPATH);
                }
                self.last_subpath_point = to;
                self.just_moved = false;
                Some(segment)
            }
            UsvgPathSegment::ClosePath => {
                let mut segment = Segment::line(&LineSegmentF32::new(
                    &self.last_subpath_point,
                    &self.first_subpath_point,
                ));
                segment.flags.insert(SegmentFlags::CLOSES_SUBPATH);
                self.just_moved = false;
                self.last_subpath_point = self.first_subpath_point;
                Some(segment)
            }
        }
    }
}

trait ColorUExt {
    fn from_svg_color(svg_color: SvgColor) -> Self;
}

impl ColorUExt for ColorU {
    #[inline]
    fn from_svg_color(svg_color: SvgColor) -> ColorU {
        ColorU {
            r: svg_color.red,
            g: svg_color.green,
            b: svg_color.blue,
            a: 255,
        }
    }
}
