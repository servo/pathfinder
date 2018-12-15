// pathfinder/utils/tile-svg/main.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use]
extern crate bitflags;

#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
extern crate rand;

use euclid::{Point2D, Rect, Size2D, Transform2D, Vector2D};
use jemallocator;
use lyon_geom::{CubicBezierSegment, LineSegment, QuadraticBezierSegment};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIter;
use pathfinder_path_utils::stroke::{StrokeStyle, StrokeToFillIter};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::cmp::Ordering;
use std::env;
use std::fmt::{self, Debug, Formatter};
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;
use svgtypes::{Color as SvgColor, PathParser, PathSegment as SvgPathSegment, TransformListParser};
use svgtypes::{TransformListToken};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// TODO(pcwalton): Make this configurable.
const SCALE_FACTOR: f32 = 8.0;

#[derive(Default)]
struct GroupStyle {
    fill_color: Option<SvgColor>,
    stroke_width: Option<f32>,
    stroke_color: Option<SvgColor>,
    transform: Option<Transform2D<f32>>,
}

#[derive(Debug)]
struct ComputedStyle {
    fill_color: Option<SvgColor>,
    stroke_width: f32,
    stroke_color: Option<SvgColor>,
    transform: Transform2D<f32>,
}

impl ComputedStyle {
    fn new() -> ComputedStyle {
        ComputedStyle {
            fill_color: None,
            stroke_width: 1.0,
            stroke_color: None,
            transform: Transform2D::identity(),
        }
    }
}

fn main() {
    let path = PathBuf::from(env::args().skip(1).next().unwrap());
    let scene = Scene::from_path(&path);
    println!("bounds: {:?}", scene.bounds);

    const RUNS: u32 = 1000;
    let start_time = Instant::now();
    let mut primitives = vec![];
    for _ in 0..RUNS {
        primitives = scene.generate_tiles();
    }
    let elapsed_time = Instant::now() - start_time;
    let elapsed_ms = elapsed_time.as_secs() as f64 * 1000.0 +
        elapsed_time.subsec_micros() as f64 / 1000.0;
    println!("{}ms elapsed", elapsed_ms / RUNS as f64);
    println!("{} primitives generated", primitives.len());
}

#[derive(Debug)]
struct Scene {
    objects: Vec<PathObject>,
    styles: Vec<ComputedStyle>,
    bounds: Rect<f32>,
    view_box: Option<Rect<f32>>,
}

#[derive(Debug)]
struct PathObject {
    outline: Outline,
    style: StyleId,
    name: String,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct StyleId(u32);

impl Scene {
    fn new() -> Scene {
        Scene { objects: vec![], styles: vec![], bounds: Rect::zero(), view_box: None }
    }

    fn from_path(path: &Path) -> Scene {
        let mut reader = Reader::from_file(&path).unwrap();

        let global_transform = Transform2D::create_scale(SCALE_FACTOR, SCALE_FACTOR);

        let mut xml_buffer = vec![];
        let mut group_styles = vec![];
        let mut style = None;

        let mut scene = Scene::new();

        loop {
            match reader.read_event(&mut xml_buffer) {
                Ok(Event::Start(ref event)) |
                Ok(Event::Empty(ref event)) if event.name() == b"path" => {
                    let attributes = event.attributes();
                    let (mut encoded_path, mut name) = (String::new(), String::new());
                    for attribute in attributes {
                        let attribute = attribute.unwrap();
                        if attribute.key == b"d" {
                            encoded_path = reader.decode(&attribute.value).to_string();
                        } else if attribute.key == b"id" {
                            name = reader.decode(&attribute.value).to_string();
                        }
                    }
                    let style = scene.ensure_style(&mut style, &mut group_styles);
                    scene.push_svg_path(&encoded_path, style, name);
                }

                Ok(Event::Start(ref event)) if event.name() == b"g" => {
                    let mut group_style = GroupStyle::default();
                    let attributes = event.attributes();
                    for attribute in attributes {
                        let attribute = attribute.unwrap();
                        match attribute.key {
                            b"fill" => {
                                let value = reader.decode(&attribute.value);
                                if let Ok(color) = SvgColor::from_str(&value) {
                                    group_style.fill_color = Some(color)
                                }
                            }
                            b"stroke" => {
                                let value = reader.decode(&attribute.value);
                                if let Ok(color) = SvgColor::from_str(&value) {
                                    group_style.stroke_color = Some(color)
                                }
                            }
                            b"transform" => {
                                let value = reader.decode(&attribute.value);
                                let mut current_transform = Transform2D::identity();
                                let transform_list_parser = TransformListParser::from(&*value);
                                for transform in transform_list_parser {
                                    match transform {
                                        Ok(TransformListToken::Matrix { a, b, c, d, e, f }) => {
                                            let transform: Transform2D<f32> =
                                                Transform2D::row_major(a, b, c, d, e, f).cast();
                                            current_transform = current_transform.pre_mul(&transform)
                                        }
                                        _ => {}
                                    }
                                }
                                group_style.transform = Some(current_transform);
                            }
                            b"stroke-width" => {
                                if let Ok(width) = reader.decode(&attribute.value).parse() {
                                    group_style.stroke_width = Some(width)
                                }
                            }
                            _ => {}
                        }
                    }

                    group_styles.push(group_style);
                    style = None;
                }

                Ok(Event::Start(ref event)) if event.name() == b"svg" => {
                    let attributes = event.attributes();
                    for attribute in attributes {
                        let attribute = attribute.unwrap();
                        if attribute.key == b"viewBox" {
                            let view_box = reader.decode(&attribute.value);
                            let mut elements = view_box.split_whitespace()
                                                       .map(|value| f32::from_str(value).unwrap());
                            let view_box = Rect::new(Point2D::new(elements.next().unwrap(),
                                                                  elements.next().unwrap()),
                                                     Size2D::new(elements.next().unwrap(),
                                                                 elements.next().unwrap()));
                            scene.view_box = Some(global_transform.transform_rect(&view_box));
                        }
                    }
                }

                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
            xml_buffer.clear();
        }

        return scene;

    }

    fn ensure_style(&mut self, current_style: &mut Option<StyleId>, group_styles: &[GroupStyle])
                    -> StyleId {
        if let Some(current_style) = *current_style {
            return current_style
        }

        let mut computed_style = ComputedStyle::new();
        for group_style in group_styles {
            if let Some(fill_color) = group_style.fill_color {
                computed_style.fill_color = Some(fill_color)
            }
            if let Some(stroke_width) = group_style.stroke_width {
                computed_style.stroke_width = stroke_width
            }
            if let Some(stroke_color) = group_style.stroke_color {
                computed_style.stroke_color = Some(stroke_color)
            }
            if let Some(transform) = group_style.transform {
                computed_style.transform = computed_style.transform.pre_mul(&transform)
            }
        }

        let id = StyleId(self.styles.len() as u32);
        self.styles.push(computed_style);
        id
    }

    fn get_style(&self, style: StyleId) -> &ComputedStyle {
        &self.styles[style.0 as usize]
    }

    fn generate_tiles(&self) -> Vec<Primitive> {
        let mut primitives = vec![];
        for (index, object) in self.objects.iter().enumerate() {
            //println!("{} ({}): {:?}", index, object.name, object.outline.bounds);
            let mut tiler = Tiler::from_outline(&object.outline, &self.view_box, &mut primitives);
            tiler.generate_tiles();
            // TODO(pcwalton)
        }
        primitives
    }

    fn push_svg_path(&mut self, value: &str, style: StyleId, name: String) {
        if self.get_style(style).stroke_width > 0.0 {
            let computed_style = self.get_style(style);
            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let path = PathIter::new(path);
            let path = StrokeToFillIter::new(path, StrokeStyle::new(computed_style.stroke_width));
            let outline = Outline::from_path_events(path, computed_style);
            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, style, name.clone()));
        }

        if self.get_style(style).fill_color.is_some() {
            let computed_style = self.get_style(style);
            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let outline = Outline::from_path_events(path, computed_style);
            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, style, name));
        }
    }
}

impl PathObject {
    fn new(outline: Outline, style: StyleId, name: String) -> PathObject {
        PathObject { outline, style, name }
    }
}

// Outlines

#[derive(Debug)]
struct Outline {
    contours: Vec<Contour>,
    bounds: Rect<f32>,
}

struct Contour {
    points: Vec<Point2D<f32>>,
    flags: Vec<PointFlags>,
}

bitflags! {
    struct PointFlags: u8 {
        const CONTROL_POINT_0 = 0x01;
        const CONTROL_POINT_1 = 0x02;
    }
}

impl Outline {
    fn new() -> Outline {
        Outline {
            contours: vec![],
            bounds: Rect::zero(),
        }
    }

    fn from_path_events<I>(path_events: I, style: &ComputedStyle) -> Outline
                           where I: Iterator<Item = PathEvent> {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();
        let mut bounding_points = None;

        let global_transform = Transform2D::create_scale(SCALE_FACTOR, SCALE_FACTOR);
        let transform = global_transform.pre_mul(&style.transform);

        for path_event in path_events {
            match path_event {
                PathEvent::MoveTo(to) => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()))
                    }
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &transform,
                                                           &mut bounding_points);
                }
                PathEvent::LineTo(to) => {
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &transform,
                                                           &mut bounding_points);
                }
                PathEvent::QuadraticTo(ctrl, to) => {
                    current_contour.push_transformed_point(&ctrl,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &transform,
                                                           &mut bounding_points);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &transform,
                                                           &mut bounding_points);
                }
                PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                    current_contour.push_transformed_point(&ctrl0,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &transform,
                                                           &mut bounding_points);
                    current_contour.push_transformed_point(&ctrl1,
                                                           PointFlags::CONTROL_POINT_1,
                                                           &transform,
                                                           &mut bounding_points);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &transform,
                                                           &mut bounding_points);
                }
                PathEvent::Close => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()));
                    }
                }
                PathEvent::Arc(..) => unimplemented!("arcs"),
            }
        }
        if !current_contour.is_empty() {
            outline.contours.push(current_contour)
        }

        if let Some((upper_left, lower_right)) = bounding_points {
            outline.bounds = Rect::from_points([upper_left, lower_right].into_iter())
        }

        outline
    }

    #[inline]
    fn iter(&self) -> OutlineIter {
        OutlineIter { outline: self, contour_iter: None, contour_index: 0 }
    }

    fn segment_after(&self, endpoint_index: PointIndex) -> Segment {
        self.contours[endpoint_index.contour_index].segment_after(endpoint_index.point_index)
    }
}

impl Contour {
    fn new() -> Contour {
        Contour { points: vec![], flags: vec![] }
    }

    fn iter(&self) -> ContourIter {
        ContourIter { contour: self, index: 0 }
    }

    fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    fn push_transformed_point(&mut self,
                              point: &Point2D<f32>,
                              flags: PointFlags,
                              transform: &Transform2D<f32>,
                              bounding_points: &mut Option<(Point2D<f32>, Point2D<f32>)>) {
        let point = transform.transform_point(point);
        self.points.push(point);
        self.flags.push(flags);

        match *bounding_points {
            Some((ref mut upper_left, ref mut lower_right)) => {
                *upper_left = upper_left.min(point);
                *lower_right = lower_right.max(point);
            }
            None => *bounding_points = Some((point, point)),
        }
    }

    fn segment_after(&self, point_index: usize) -> Segment {
        debug_assert!(self.point_is_endpoint(point_index));

        let mut segment = Segment::new();
        segment.from = self.points[point_index];
        segment.flags |= SegmentFlags::HAS_ENDPOINTS;

        let point1_index = self.add_to_point_index(point_index, 1);
        if self.point_is_endpoint(point1_index) {
            segment.to = self.points[point1_index];
        } else {
            segment.ctrl0 = self.points[point1_index];
            segment.flags |= SegmentFlags::HAS_CONTROL_POINT_0;

            let point2_index = self.add_to_point_index(point_index, 2);
            if self.point_is_endpoint(point2_index) {
                segment.to = self.points[point2_index];
            } else {
                segment.ctrl1 = self.points[point2_index];
                segment.flags |= SegmentFlags::HAS_CONTROL_POINT_1;

                let point3_index = self.add_to_point_index(point_index, 3);
                segment.to = self.points[point3_index];
            }
        }

        segment
    }

    fn point_is_endpoint(&self, point_index: usize) -> bool {
        !self.flags[point_index].intersects(PointFlags::CONTROL_POINT_0 |
                                            PointFlags::CONTROL_POINT_1)
    }

    fn add_to_point_index(&self, point_index: usize, addend: usize) -> usize {
        let (index, limit) = (point_index + addend, self.points.len());
        if index >= limit {
            index - limit
        } else {
            index
        }
    }
}

impl Debug for Contour {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("[")?;
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        for (index, segment) in self.iter().enumerate() {
            if index > 0 {
                formatter.write_str(",")?;
            }
            if formatter.alternate() {
                formatter.write_str("\n    ")?;
            } else {
                formatter.write_str(" ")?;
            }
            segment.fmt(formatter)?;
        }
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        formatter.write_str("]")
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PointIndex {
    contour_index: usize,
    point_index: usize,
}

struct OutlineIter<'a> {
    outline: &'a Outline,
    contour_iter: Option<ContourIter<'a>>,
    contour_index: usize,
}

struct ContourIter<'a> {
    contour: &'a Contour,
    index: usize,
}

impl<'a> Iterator for OutlineIter<'a> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        if let Some(ref mut contour_iter) = self.contour_iter {
            match contour_iter.next() {
                Some(event) => return Some(event),
                None => {
                    self.contour_iter = None;
                    self.contour_index += 1;
                }
            }
        }

        if self.contour_index == self.outline.contours.len() {
            return None
        }

        self.contour_iter = Some(self.outline.contours[self.contour_index].iter());
        self.next()
    }
}

impl<'a> Iterator for ContourIter<'a> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        let contour = self.contour;
        if self.index == contour.points.len() + 1 {
            return None
        }
        if self.index == contour.points.len() {
            self.index += 1;
            return Some(PathEvent::Close)
        }

        let point0_index = self.index;
        let point0 = contour.points[point0_index];
        self.index += 1;
        if point0_index == 0 {
            return Some(PathEvent::MoveTo(point0))
        }
        if contour.point_is_endpoint(point0_index) {
            return Some(PathEvent::LineTo(point0))
        }

        let point1_index = self.index;
        let point1 = contour.points[point1_index];
        self.index += 1;
        if contour.point_is_endpoint(point1_index) {
            return Some(PathEvent::QuadraticTo(point0, point1))
        }

        let point2_index = self.index;
        let point2 = contour.points[point2_index];
        self.index += 1;
        debug_assert!(contour.point_is_endpoint(point2_index));
        Some(PathEvent::CubicTo(point0, point1, point2))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    from: Point2D<f32>,
    ctrl0: Point2D<f32>,
    ctrl1: Point2D<f32>,
    to: Point2D<f32>,
    flags: SegmentFlags,
}

impl Segment {
    fn new() -> Segment {
        Segment {
            from: Point2D::zero(),
            ctrl0: Point2D::zero(),
            ctrl1: Point2D::zero(),
            to: Point2D::zero(),
            flags: SegmentFlags::empty(),
        }
    }

    fn from_line(line: &LineSegment<f32>) -> Segment {
        Segment {
            from: line.from,
            ctrl0: Point2D::zero(),
            ctrl1: Point2D::zero(),
            to: line.to,
            flags: SegmentFlags::HAS_ENDPOINTS,
        }
    }

    fn from_quadratic(curve: &QuadraticBezierSegment<f32>) -> Segment {
        Segment {
            from: curve.from,
            ctrl0: curve.ctrl,
            ctrl1: Point2D::zero(),
            to: curve.to,
            flags: SegmentFlags::HAS_ENDPOINTS | SegmentFlags::HAS_CONTROL_POINT_0
        }
    }

    fn from_cubic(curve: &CubicBezierSegment<f32>) -> Segment {
        Segment {
            from: curve.from,
            ctrl0: curve.ctrl1,
            ctrl1: curve.ctrl2,
            to: curve.to,
            flags: SegmentFlags::HAS_ENDPOINTS | SegmentFlags::HAS_CONTROL_POINT_0 |
                SegmentFlags::HAS_CONTROL_POINT_1,
        }
    }

    fn add_to_list(&self, tile_left: f32, primitives: &mut Vec<Primitive>) {
        let vector = Vector2D::new(-tile_left, 0.0);
        primitives.push(Primitive {
            from: self.from + vector,
            ctrl0: self.ctrl0 + vector,
            ctrl1: self.ctrl1 + vector,
            to: self.to + vector,
        })
    }

    fn is_none(&self) -> bool {
        !self.flags.contains(SegmentFlags::HAS_ENDPOINTS)
    }

    // Note: If we convert these to monotonic then we can optimize this method.
    // TODO(pcwalton): Consider changing the representation of `Segment` to remove the code
    // duplication in the branches here?
    fn min_y(&self) -> f32 {
        let mut min_y = f32::min(self.from.y, self.to.y);
        if self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            min_y = f32::min(min_y, self.ctrl0.y)
        }
        if self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_1) {
            min_y = f32::min(min_y, self.ctrl1.y)
        }
        min_y
    }

    fn clip_y(&self, y: f32) -> ClippedSegments {
        if self.from.y < y && self.to.y < y {
            return ClippedSegments { min: Some(*self), max: None }
        }
        if self.from.y > y && self.to.y > y {
            return ClippedSegments { min: None, max: Some(*self) }
        }

        let (prev, next) = if self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_1) {
            let curve = CubicBezierSegment {
                from: self.from,
                ctrl1: self.ctrl0,
                ctrl2: self.ctrl1,
                to: self.to,
            };
            let swapped_curve = CubicBezierSegment {
                from: curve.from.yx(),
                ctrl1: curve.ctrl1.yx(),
                ctrl2: curve.ctrl2.yx(),
                to: curve.to.yx(),
            };
            let (prev, next) = curve.split(
                    swapped_curve.assume_monotonic().solve_t_for_x(y, 0.0..1.0, TOLERANCE));
            (Segment::from_cubic(&prev), Segment::from_cubic(&next))
        } else if self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            let curve = QuadraticBezierSegment { from: self.from, ctrl: self.ctrl0, to: self.to };
            let (prev, next) = curve.split(curve.assume_monotonic().solve_t_for_y(y));
            (Segment::from_quadratic(&prev), Segment::from_quadratic(&next))
        } else {
            let line = LineSegment { from: self.from, to: self.to };
            let (prev, next) = line.split(line.solve_t_for_y(y));
            (Segment::from_line(&prev), Segment::from_line(&next))
        };

        if self.from.y <= self.to.y {
            return ClippedSegments { min: Some(prev), max: Some(next) };
        } else {
            return ClippedSegments { min: Some(next), max: Some(prev) };
        }

        const TOLERANCE: f32 = 0.01;
    }

    fn translate(&self, by: &Vector2D<f32>) -> Segment {
        let flags = self.flags;
        let (from, to) = if flags.contains(SegmentFlags::HAS_ENDPOINTS) {
            (self.from + *by, self.to + *by)
        } else {
            (Point2D::zero(), Point2D::zero())
        };
        let ctrl0 = if flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            self.ctrl0 + *by
        } else {
            Point2D::zero()
        };
        let ctrl1 = if flags.contains(SegmentFlags::HAS_CONTROL_POINT_1) {
            self.ctrl1 + *by
        } else {
            Point2D::zero()
        };
        Segment { from, ctrl0, ctrl1, to, flags }
    }
}

struct ClippedSegments {
    min: Option<Segment>,
    max: Option<Segment>,
}

bitflags! {
    struct SegmentFlags: u8 {
        const HAS_ENDPOINTS       = 0x01;
        const HAS_CONTROL_POINT_0 = 0x02;
        const HAS_CONTROL_POINT_1 = 0x04;
    }
}

// Tiling

const TILE_WIDTH: f32 = 16.0;
const TILE_HEIGHT: f32 = 16.0;

struct Tiler<'o, 'p> {
    outline: &'o Outline,
    primitives: &'p mut Vec<Primitive>,

    view_box: Option<Rect<f32>>,

    sorted_edge_indices: Vec<PointIndex>,
    active_intervals: Intervals,
    active_edges: Vec<Segment>,
}

impl<'o, 'p> Tiler<'o, 'p> {
    fn from_outline(outline: &'o Outline,
                    view_box: &Option<Rect<f32>>,
                    primitives: &'p mut Vec<Primitive>)
                    -> Tiler<'o, 'p> {
        Tiler {
            outline,
            primitives,

            view_box: *view_box,

            sorted_edge_indices: vec![],
            active_intervals: Intervals::new(0.0),
            active_edges: vec![],
        }
    }

    fn generate_tiles(&mut self) {
        // Sort all edge indices.
        self.sorted_edge_indices.clear();
        for contour_index in 0..self.outline.contours.len() {
            let contour = &self.outline.contours[contour_index];
            for point_index in 0..contour.points.len() {
                if contour.point_is_endpoint(point_index) {
                    self.sorted_edge_indices.push(PointIndex { contour_index, point_index })
                }
            }
        }
        {
            let outline = &self.outline;
            self.sorted_edge_indices.sort_unstable_by(|edge_index_a, edge_index_b| {
                let segment_a = outline.segment_after(*edge_index_a);
                let segment_b = outline.segment_after(*edge_index_b);
                segment_a.min_y().partial_cmp(&segment_b.min_y()).unwrap_or(Ordering::Equal)
            });
        }

        // Guard band clipping...
        let bounds = self.outline.bounds;
        let (mut max_x, mut max_y) = (bounds.max_x(), bounds.max_y());
        if let Some(view_box) = self.view_box {
            max_x = clamp(max_x, view_box.origin.x, view_box.max_x());
            max_y = clamp(max_y, view_box.origin.y, view_box.max_y());
        }

        self.active_intervals.reset(max_x);
        self.active_edges.clear();
        let mut next_edge_index_index = 0;

        let mut tile_top = f32::floor(bounds.origin.y / TILE_HEIGHT) * TILE_HEIGHT;
        while tile_top < max_y {
            let tile_extent = Point2D::new(max_x, tile_top + TILE_HEIGHT);

            let above_view_box = match self.view_box {
                Some(ref view_box) => tile_extent.y <= view_box.origin.y,
                None => false,
            };

            // TODO(pcwalton): Populate tile strip with active intervals.

            // Process old active edges.
            for active_edge in &mut self.active_edges {
                let primitives = if above_view_box { None } else { Some(&mut *self.primitives) };
                process_active_edge(active_edge,
                                    &tile_extent,
                                    primitives,
                                    &mut self.active_intervals)
            }
            self.active_edges.retain(|edge| !edge.is_none());

            // Add new active edges.
            while next_edge_index_index < self.sorted_edge_indices.len() {
                let mut segment =
                    self.outline.segment_after(self.sorted_edge_indices[next_edge_index_index]);
                if segment.min_y() > tile_extent.y {
                    break
                }

                let primitives = if above_view_box { None } else { Some(&mut *self.primitives) };
                process_active_edge(&mut segment,
                                    &tile_extent,
                                    primitives,
                                    &mut self.active_intervals);
                if !segment.is_none() {
                    self.active_edges.push(segment);
                }

                next_edge_index_index += 1;
            }

            tile_top = tile_extent.y;
        }
    }
}

fn process_active_edge(active_edge: &mut Segment,
                       strip_extent: &Point2D<f32>,
                       primitives: Option<&mut Vec<Primitive>>,
                       active_intervals: &mut Intervals) {
    let clipped = active_edge.clip_y(strip_extent.y);
    if let Some(upper_segment) = clipped.min {
        if let Some(primitives) = primitives {
            // FIXME(pcwalton): Assumes x-monotonicity!
            // FIXME(pcwalton): Don't hardcode a view box left of 0!
            let mut min_x = f32::min(upper_segment.from.x, upper_segment.to.x);
            let mut max_x = f32::max(upper_segment.from.x, upper_segment.to.x);
            min_x = clamp(min_x, 0.0, strip_extent.x);
            max_x = clamp(max_x, 0.0, strip_extent.x);

            let mut tile_left = f32::floor(min_x / TILE_WIDTH) * TILE_WIDTH;
            while tile_left < max_x {
                active_edge.add_to_list(tile_left, primitives);
                tile_left += TILE_WIDTH;
            }
        }

        // FIXME(pcwalton): Assumes x-monotonicity!
        let mut from_x = f32::max(0.0, f32::min(active_intervals.extent(), upper_segment.from.x));
        let mut to_x = f32::max(0.0, f32::min(active_intervals.extent(), upper_segment.to.x));
        from_x = clamp(from_x, 0.0, strip_extent.x);
        to_x = clamp(to_x, 0.0, strip_extent.x);
        if from_x < to_x {
            active_intervals.add(IntervalRange::new(from_x, to_x, -1.0))
        } else {
            active_intervals.add(IntervalRange::new(to_x, from_x, 1.0))
        }
    }

    match clipped.max {
        Some(lower_segment) => *active_edge = lower_segment,
        None => *active_edge = Segment::new(),
    }
}

// Primitives

#[derive(Clone, Copy, Debug)]
struct Primitive {
    from: Point2D<f32>,
    ctrl0: Point2D<f32>,
    ctrl1: Point2D<f32>,
    to: Point2D<f32>,
}

// Intervals

#[derive(Debug)]
struct Intervals {
    ranges: Vec<IntervalRange>,
}

#[derive(Clone, Copy, Debug)]
struct IntervalRange {
    start: f32,
    end: f32,
    winding: f32,
}

impl Intervals {
    fn new(end: f32) -> Intervals {
        Intervals {
            ranges: vec![IntervalRange::new(0.0, end, 0.0)],
        }
    }

    fn add(&mut self, range: IntervalRange) {
        if range.is_empty() {
            return
        }

        self.split_at(range.start);
        self.split_at(range.end);

        // Adjust winding numbers.
        let mut index = 0;
        while range.start != self.ranges[index].start {
            index += 1
        }
        loop {
            self.ranges[index].winding += range.winding;
            if range.end == self.ranges[index].end {
                break
            }
            index += 1
        }

        self.merge_adjacent();
    }

    fn reset(&mut self, end: f32) {
        self.ranges.truncate(1);
        self.ranges[0] = IntervalRange::new(0.0, end, 0.0);
    }

    fn extent(&self) -> f32 {
        self.ranges.last().unwrap().end
    }

    fn split_at(&mut self, value: f32) {
        let (mut low, mut high) = (0, self.ranges.len());
        loop {
            let mid = low + (high - low) / 2;

            let IntervalRange {
                start: old_start,
                end: old_end,
                winding,
            } = self.ranges[mid];

            if value < old_start {
                high = mid;
                continue
            }
            if value > old_end {
                low = mid + 1;
                continue
            }

            if old_start < value && value < old_end {
                self.ranges[mid] = IntervalRange::new(old_start, value, winding);
                self.ranges.insert(mid + 1, IntervalRange::new(value, old_end, winding));
            }
            return
        }
    }

    fn merge_adjacent(&mut self) {
        let mut dest_range_index = 0;
        let mut current_range = self.ranges[0];
        for src_range_index in 1..self.ranges.len() {
            if self.ranges[src_range_index].winding == current_range.winding {
                current_range.end = self.ranges[src_range_index].end
            } else {
                self.ranges[dest_range_index] = current_range;
                dest_range_index += 1;
                current_range = self.ranges[src_range_index];
            }
        }
        self.ranges[dest_range_index] = current_range;
        dest_range_index += 1;
        self.ranges.truncate(dest_range_index);
    }
}

impl IntervalRange {
    fn new(start: f32, end: f32, winding: f32) -> IntervalRange {
        IntervalRange {
            start,
            end,
            winding,
        }
    }

    fn contains(&self, value: f32) -> bool {
        value >= self.start && value < self.end
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

// SVG stuff

struct SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    iter: &'a mut I,
    last_endpoint: Option<Point2D<f32>>,
    last_ctrl_point: Option<Point2D<f32>>,
}

impl<'a, I> SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    fn new(iter: &'a mut I) -> SvgPathToPathEvents<'a, I> {
        SvgPathToPathEvents { iter, last_endpoint: None, last_ctrl_point: None }
    }
}

impl<'a, I> Iterator for SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        return match self.iter.next() {
            None => None,
            Some(SvgPathSegment::MoveTo { abs, x, y }) => {
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                self.last_ctrl_point = None;
                Some(PathEvent::MoveTo(to))
            }
            Some(SvgPathSegment::LineTo { abs, x, y }) => {
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::HorizontalLineTo { abs, x }) => {
                let to = compute_point(x, 0.0, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::VerticalLineTo { abs, y }) => {
                let to = compute_point(0.0, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::Quadratic { abs, x1, y1, x, y }) => {
                let ctrl = compute_point(x1, y1, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                Some(PathEvent::QuadraticTo(ctrl, to))
            }
            Some(SvgPathSegment::SmoothQuadratic { abs, x, y }) => {
                let ctrl = self.last_endpoint.unwrap_or(Point2D::zero()) +
                    (self.last_endpoint.unwrap_or(Point2D::zero()) -
                        self.last_ctrl_point.unwrap_or(Point2D::zero()));
                self.last_ctrl_point = Some(ctrl);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                Some(PathEvent::QuadraticTo(ctrl, to))
            }
            Some(SvgPathSegment::CurveTo { abs, x1, y1, x2, y2, x, y }) => {
                let ctrl0 = compute_point(x1, y1, abs, &self.last_endpoint);
                let ctrl1 = compute_point(x2, y2, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl1);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
            }
            Some(SvgPathSegment::SmoothCurveTo { abs, x2, y2, x, y }) => {
                let ctrl0 = self.last_endpoint.unwrap_or(Point2D::zero()) +
                    (self.last_endpoint.unwrap_or(Point2D::zero()) -
                        self.last_ctrl_point.unwrap_or(Point2D::zero()));
                let ctrl1 = compute_point(x2, y2, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl1);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = Some(to);
                Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
            }
            Some(SvgPathSegment::ClosePath { abs: _ }) => {
                Some(PathEvent::Close)
            }
            Some(SvgPathSegment::EllipticalArc { .. }) => unimplemented!("arcs"),
        };

        fn compute_point(x: f64, y: f64, abs: bool, last_endpoint: &Option<Point2D<f32>>)
                         -> Point2D<f32> {
            let point = Point2D::new(x, y).to_f32();
            match *last_endpoint {
                Some(last_endpoint) if !abs => last_endpoint + point.to_vector(),
                _ => point,
            }
        }
    }
}

// Trivial utilities

fn clamp(x: f32, min: f32, max: f32) -> f32 {
    f32::max(f32::min(x, max), min)
}

// Testing

#[cfg(test)]
mod test {
    use crate::{IntervalRange, Intervals};
    use quickcheck::{self, Arbitrary, Gen};
    use rand::Rng;

    #[test]
    fn test_intervals() {
        quickcheck::quickcheck(prop_intervals as fn(Spec) -> bool);

        fn prop_intervals(spec: Spec) -> bool {
            let mut intervals = Intervals::new(spec.end);
            for range in spec.ranges {
                intervals.add(range);
            }

            assert!(intervals.ranges.len() > 0);
            assert_eq!(intervals.ranges[0].start, 0.0);
            assert_eq!(intervals.ranges.last().unwrap().end, spec.end);
            for prev_index in 0..(intervals.ranges.len() - 1) {
                let next_index = prev_index + 1;
                assert_eq!(intervals.ranges[prev_index].end, intervals.ranges[next_index].start);
                assert_ne!(intervals.ranges[prev_index].winding,
                           intervals.ranges[next_index].winding);
            }

            true
        }

        #[derive(Clone, Debug)]
        struct Spec {
            end: f32,
            ranges: Vec<IntervalRange>,
        }

        impl Arbitrary for Spec {
            fn arbitrary<G>(g: &mut G) -> Spec where G: Gen {
                const EPSILON: f32 = 0.0001;

                let size = g.size();
                let end = g.gen_range(EPSILON, size as f32);

                let mut ranges = vec![];
                let range_count = g.gen_range(0, size);
                for _ in 0..range_count {
                    let (a, b) = (g.gen_range(0.0, end), g.gen_range(0.0, end));
                    let winding = g.gen_range(-(size as i32), size as i32) as f32;
                    ranges.push(IntervalRange::new(f32::min(a, b), f32::max(a, b), winding));
                }

                Spec {
                    end,
                    ranges,
                }
            }
        }
    }
}
