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

use byteorder::{LittleEndian, WriteBytesExt};
use clap::{App, Arg};
use euclid::{Point2D, Rect, Size2D, Transform2D, Vector2D};
use fixedbitset::FixedBitSet;
use jemallocator;
use lyon_geom::cubic_bezier::Flattened;
use lyon_geom::{CubicBezierSegment, LineSegment, QuadraticBezierSegment};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIter;
use pathfinder_path_utils::stroke::{StrokeStyle, StrokeToFillIter};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::mem;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;
use std::u32;
use svgtypes::{Color as SvgColor, PathParser, PathSegment as SvgPathSegment, TransformListParser};
use svgtypes::{TransformListToken};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// TODO(pcwalton): Make this configurable.
const SCALE_FACTOR: f32 = 1.0;

// TODO(pcwalton): Make this configurable.
const FLATTENING_TOLERANCE: f32 = 3.0;

fn main() {
    let matches =
        App::new("tile-svg").arg(Arg::with_name("runs").short("r")
                                                       .long("runs")
                                                       .value_name("COUNT")
                                                       .takes_value(true)
                                                       .help("Run a benchmark with COUNT runs"))
                            .arg(Arg::with_name("sequential").short("s")
                                                             .long("sequential")
                                                             .help("Use only one thread"))
                            .arg(Arg::with_name("INPUT").help("Path to the SVG file to render")
                                                        .required(true)
                                                        .index(1))
                            .arg(Arg::with_name("OUTPUT").help("Path to the output PF3 data")
                                                         .required(false)
                                                         .index(2))
                            .get_matches();
    let runs: usize = match matches.value_of("runs") {
        Some(runs) => runs.parse().unwrap(),
        None => 1,
    };
    let sequential = matches.is_present("sequential");
    let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());
    let output_path = matches.value_of("OUTPUT").map(PathBuf::from);

    let scene = Scene::from_path(&input_path);
    println!("Scene bounds: {:?}", scene.bounds);

    let start_time = Instant::now();
    let mut built_scene = BuiltScene::new(&scene.view_box, scene.objects.len() as u32);
    for _ in 0..runs {
        let built_objects = if sequential {
            scene.build_objects_sequentially()
        } else {
            scene.build_objects_in_parallel()
        };
        built_scene = BuiltScene::from_objects(&scene.view_box, &built_objects);
    }
    let elapsed_time = Instant::now() - start_time;

    let elapsed_ms = elapsed_time.as_secs() as f64 * 1000.0 +
        elapsed_time.subsec_micros() as f64 / 1000.0;
    println!("{:.3}ms elapsed", elapsed_ms / runs as f64);
    println!("{} fill primitives generated", built_scene.fills.len());
    println!("{} tiles ({} solid, {} mask) generated",
             built_scene.solid_tiles.len() + built_scene.mask_tiles.len(),
             built_scene.solid_tiles.len(),
             built_scene.mask_tiles.len());

    /*
    println!("solid tiles:");
    for (index, tile) in built_scene.solid_tiles.iter().enumerate() {
        println!("... {}: {:?}", index, tile);
    }

    println!("fills:");
    for (index, fill) in built_scene.fills.iter().enumerate() {
        println!("... {}: {:?}", index, fill);
    }
    */

    if let Some(output_path) = output_path {
        built_scene.write(&mut BufWriter::new(File::create(output_path).unwrap())).unwrap();
    }
}

#[derive(Debug)]
struct Scene {
    objects: Vec<PathObject>,
    styles: Vec<ComputedStyle>,
    bounds: Rect<f32>,
    view_box: Rect<f32>,
}

#[derive(Debug)]
struct PathObject {
    outline: Outline,
    style: StyleId,
    color: ColorU,
    name: String,
}

#[derive(Debug)]
struct ComputedStyle {
    fill_color: Option<SvgColor>,
    stroke_width: f32,
    stroke_color: Option<SvgColor>,
    transform: Transform2D<f32>,
}

#[derive(Default)]
struct GroupStyle {
    fill_color: Option<SvgColor>,
    stroke_width: Option<f32>,
    stroke_color: Option<SvgColor>,
    transform: Option<Transform2D<f32>>,
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

#[derive(Clone, Copy, PartialEq, Debug)]
struct StyleId(u32);

impl Scene {
    fn new() -> Scene {
        Scene { objects: vec![], styles: vec![], bounds: Rect::zero(), view_box: Rect::zero() }
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
                    scene.push_group_style(&mut reader, event, &mut group_styles, &mut style);

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

                    let computed_style = scene.ensure_style(&mut style, &mut group_styles);
                    scene.push_svg_path(&encoded_path, computed_style, name);

                    group_styles.pop();
                    style = None;
                }

                Ok(Event::Start(ref event)) if event.name() == b"g" => {
                    scene.push_group_style(&mut reader, event, &mut group_styles, &mut style);
                }

                Ok(Event::End(ref event)) if event.name() == b"g" => {
                    group_styles.pop();
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
                            scene.view_box = global_transform.transform_rect(&view_box);
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

    fn push_group_style(&mut self,
                        reader: &mut Reader<BufReader<File>>,
                        event: &BytesStart,
                        group_styles: &mut Vec<GroupStyle>,
                        style: &mut Option<StyleId>) {
        let mut group_style = GroupStyle::default();
        let attributes = event.attributes();
        for attribute in attributes {
            let attribute = attribute.unwrap();
            match attribute.key {
                b"fill" => {
                    let value = reader.decode(&attribute.value);
                    if let Ok(color) = SvgColor::from_str(&value) {
                        group_style.fill_color = Some(color);
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
        *style = None;
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

    fn build_objects_sequentially(&self) -> Vec<BuiltObject> {
        self.objects.iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline, object_index as u32, &self.view_box);
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }

    fn build_objects_in_parallel(&self) -> Vec<BuiltObject> {
        self.objects.par_iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline, object_index as u32, &self.view_box);
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }

    fn push_svg_path(&mut self, value: &str, style: StyleId, name: String) {
        if self.get_style(style).stroke_color.is_some() {
            let computed_style = self.get_style(style);
            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let path = PathIter::new(path);
            let path = StrokeToFillIter::new(path, StrokeStyle::new(computed_style.stroke_width));
            let path = MonotonicConversionIter::new(path);
            let outline = Outline::from_path_events(path, computed_style);

            let color = match computed_style.stroke_color {
                None => ColorU::black(),
                Some(color) => ColorU::from_svg_color(color),
            };

            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, color, style, name.clone()));
        }

        if self.get_style(style).fill_color.is_some() {
            let computed_style = self.get_style(style);
            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let path = MonotonicConversionIter::new(path);
            let outline = Outline::from_path_events(path, computed_style);

            let color = match computed_style.fill_color {
                None => ColorU::black(),
                Some(color) => ColorU::from_svg_color(color),
            };

            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, color, style, name));
        }
    }
}

impl PathObject {
    fn new(outline: Outline, color: ColorU, style: StyleId, name: String) -> PathObject {
        PathObject { outline, color, style, name }
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

    fn len(&self) -> u32 {
        self.points.len() as u32
    }

    fn position_of(&self, index: u32) -> Point2D<f32> {
        self.points[index as usize]
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

    fn segment_after(&self, point_index: u32) -> Segment {
        debug_assert!(self.point_is_endpoint(point_index));

        let mut segment = Segment::new();
        segment.from = self.position_of(point_index);
        segment.flags |= SegmentFlags::HAS_ENDPOINTS;

        let point1_index = self.add_to_point_index(point_index, 1);
        if self.point_is_endpoint(point1_index) {
            segment.to = self.position_of(point1_index);
        } else {
            segment.ctrl0 = self.position_of(point1_index);
            segment.flags |= SegmentFlags::HAS_CONTROL_POINT_0;

            let point2_index = self.add_to_point_index(point_index, 2);
            if self.point_is_endpoint(point2_index) {
                segment.to = self.position_of(point2_index);
            } else {
                segment.ctrl1 = self.position_of(point2_index);
                segment.flags |= SegmentFlags::HAS_CONTROL_POINT_1;

                let point3_index = self.add_to_point_index(point_index, 3);
                segment.to = self.position_of(point3_index);
            }
        }

        segment
    }

    fn point_is_endpoint(&self, point_index: u32) -> bool {
        !self.flags[point_index as usize].intersects(PointFlags::CONTROL_POINT_0 |
                                                     PointFlags::CONTROL_POINT_1)
    }

    fn add_to_point_index(&self, point_index: u32, addend: u32) -> u32 {
        let (index, limit) = (point_index + addend, self.len());
        if index >= limit {
            index - limit
        } else {
            index
        }
    }

    fn point_is_logically_above(&self, a: u32, b: u32) -> bool {
        let (a_y, b_y) = (self.points[a as usize].y, self.points[b as usize].y);
        a_y < b_y || (a_y == b_y && a < b)
    }

    fn prev_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.prev_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index
            }
        }
    }

    fn next_endpoint_index_of(&self, mut point_index: u32) -> u32 {
        loop {
            point_index = self.next_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index
            }
        }
    }

    fn prev_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == 0 {
            self.len() - 1
        } else {
            point_index - 1
        }
    }

    fn next_point_index_of(&self, point_index: u32) -> u32 {
        if point_index == self.len() - 1 {
            0
        } else {
            point_index + 1
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct PointIndex(u32);

impl PointIndex {
    fn new(contour: u32, point: u32) -> PointIndex {
        PointIndex((contour << 20) | point)
    }

    fn contour(self) -> u32 {
        self.0 >> 20
    }

    fn point(self) -> u32 {
        self.0 & 0x000fffff
    }
}

struct ContourIter<'a> {
    contour: &'a Contour,
    index: u32,
}

impl<'a> Iterator for ContourIter<'a> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        let contour = self.contour;
        if self.index == contour.len() + 1 {
            return None
        }
        if self.index == contour.len() {
            self.index += 1;
            return Some(PathEvent::Close)
        }

        let point0_index = self.index;
        let point0 = contour.position_of(point0_index);
        self.index += 1;
        if point0_index == 0 {
            return Some(PathEvent::MoveTo(point0))
        }
        if contour.point_is_endpoint(point0_index) {
            return Some(PathEvent::LineTo(point0))
        }

        let point1_index = self.index;
        let point1 = contour.position_of(point1_index);
        self.index += 1;
        if contour.point_is_endpoint(point1_index) {
            return Some(PathEvent::QuadraticTo(point0, point1))
        }

        let point2_index = self.index;
        let point2 = contour.position_of(point2_index);
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

    fn as_line_segment(&self) -> Option<LineSegment<f32>> {
        if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            Some(LineSegment { from: self.from, to: self.to })
        } else {
            None
        }
    }

    // FIXME(pcwalton): We should basically never use this function.
    fn as_cubic_segment(&self) -> Option<CubicBezierSegment<f32>> {
        if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            None
        } else if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_1) {
            Some((QuadraticBezierSegment {
                from: self.from,
                ctrl: self.ctrl0,
                to: self.to,
            }).to_cubic())
        } else {
            Some(CubicBezierSegment {
                from: self.from,
                ctrl1: self.ctrl0,
                ctrl2: self.ctrl1,
                to: self.to,
            })
        }
    }

    fn is_degenerate(&self) -> bool {
        return f32::abs(self.to.x - self.from.x) < EPSILON ||
            f32::abs(self.to.y - self.from.y) < EPSILON;

        const EPSILON: f32 = 0.0001;
    }

    fn clip_x(&self, range: Range<f32>) -> Option<Segment> {
        // Trivial cases.
        if (self.from.x <= range.start && self.to.x <= range.start) ||
                (self.from.x >= range.end && self.to.x >= range.end) {
            return None
        }
        let (start, end) = (f32::min(self.from.x, self.to.x), f32::max(self.from.x, self.to.x));
        if start >= range.start && end <= range.end {
            return Some(*self)
        }

        // FIXME(pcwalton): Reduce code duplication!
        if let Some(mut line_segment) = self.as_line_segment() {
            if let Some(t) = LineAxis::from_x(&line_segment).solve_for_t(range.start) {
                let (prev, next) = line_segment.split(t);
                if line_segment.from.x < line_segment.to.x {
                    line_segment = next
                } else {
                    line_segment = prev
                }
            }

            if let Some(t) = LineAxis::from_x(&line_segment).solve_for_t(range.end) {
                let (prev, next) = line_segment.split(t);
                if line_segment.from.x < line_segment.to.x {
                    line_segment = prev
                } else {
                    line_segment = next
                }
            }

            let clipped = Segment::from_line(&line_segment);
            return Some(clipped);
        }

        // TODO(pcwalton): Don't degree elevate!
        let mut cubic_segment = self.as_cubic_segment().unwrap();

        if let Some(t) = CubicAxis::from_x(&cubic_segment).solve_for_t(range.start) {
            let (prev, next) = cubic_segment.split(t);
            if cubic_segment.from.x < cubic_segment.to.x {
                cubic_segment = next
            } else {
                cubic_segment = prev
            }
        }

        if let Some(t) = CubicAxis::from_x(&cubic_segment).solve_for_t(range.end) {
            let (prev, next) = cubic_segment.split(t);
            if cubic_segment.from.x < cubic_segment.to.x {
                cubic_segment = prev
            } else {
                cubic_segment = next
            }
        }

        let clipped = Segment::from_cubic(&cubic_segment);
        return Some(clipped);
    }

    fn split_y(&self, y: f32) -> (Option<Segment>, Option<Segment>) {
        // Trivial cases.
        if self.from.y <= y && self.to.y <= y {
            return (Some(*self), None)
        }
        if self.from.y >= y && self.to.y >= y {
            return (None, Some(*self))
        }

        // TODO(pcwalton): Reduce code duplication?
        let (prev, next) = match self.as_line_segment() {
            Some(line_segment) => {
                let t = LineAxis::from_y(&line_segment).solve_for_t(y).unwrap();
                let (prev, next) = line_segment.split(t);
                (Segment::from_line(&prev), Segment::from_line(&next))
            }
            None => {
                // TODO(pcwalton): Don't degree elevate!
                let cubic_segment = self.as_cubic_segment().unwrap();
                let t = CubicAxis::from_y(&cubic_segment).solve_for_t(y);
                let t = t.expect("Failed to solve cubic for Y!");
                let (prev, next) = cubic_segment.split(t);
                (Segment::from_cubic(&prev), Segment::from_cubic(&next))
            }
        };

        if self.from.y < self.to.y {
            (Some(prev), Some(next))
        } else {
            (Some(next), Some(prev))
        }
    }

    #[inline(never)]
    fn generate_fill_primitives(&self, built_object: &mut BuiltObject, tile_y: i16) {
        if let Some(line_segment) = self.as_line_segment() {
            generate_fill_primitives_for_line(line_segment, built_object, tile_y);
            return;
        }

        // TODO(pcwalton): Don't degree elevate!
        let segment = self.as_cubic_segment().unwrap();
        let flattener = Flattened::new(segment, FLATTENING_TOLERANCE);
        let mut from = self.from;
        for to in flattener {
            generate_fill_primitives_for_line(LineSegment { from, to }, built_object, tile_y);
            from = to;
        }

        fn generate_fill_primitives_for_line(mut segment: LineSegment<f32>,
                                             built_object: &mut BuiltObject,
                                             tile_y: i16) {
            let winding = segment.from.x > segment.to.x;
            let (segment_left, segment_right) = if !winding {
                (segment.from.x, segment.to.x)
            } else {
                (segment.to.x, segment.from.x)
            };

            let segment_tile_left = f32::floor(segment_left / TILE_WIDTH) as i16;
            let segment_tile_right = f32::ceil(segment_right / TILE_WIDTH) as i16;

            for subsegment_tile_x in segment_tile_left..segment_tile_right {
                let (mut fill_from, mut fill_to) = (segment.from, segment.to);
                let subsegment_tile_right = (subsegment_tile_x + 1) as f32 * TILE_WIDTH;
                if subsegment_tile_right < segment_right {
                    let x = subsegment_tile_right;
                    let point = Point2D::new(x, segment.solve_y_for_x(x));
                    if !winding {
                        fill_to = point;
                        segment.from = point;
                    } else {
                        fill_from = point;
                        segment.to = point;
                    }
                }

                built_object.add_fill(&fill_from, &fill_to, subsegment_tile_x, tile_y);
            }
        }
    }

    fn is_none(&self) -> bool {
        !self.flags.contains(SegmentFlags::HAS_ENDPOINTS)
    }

    fn min_x(&self) -> f32 { f32::min(self.from.x, self.to.x) }
    fn max_x(&self) -> f32 { f32::max(self.from.x, self.to.x) }

    fn winding(&self) -> i32 {
        match self.from.x.partial_cmp(&self.to.x) {
            Some(Ordering::Less) => -1,
            Some(Ordering::Greater) => 1,
            Some(Ordering::Equal) | None => 0,
        }
    }
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

struct Tiler<'o> {
    outline: &'o Outline,
    object_index: u32,
    built_object: BuiltObject,

    view_box: Rect<f32>,
    bounds: Rect<f32>,

    point_queue: SortedVector<QueuedEndpoint>,
    active_edges: SortedVector<ActiveEdge>,
    old_active_edges: Vec<ActiveEdge>,
}

impl<'o> Tiler<'o> {
    fn new(outline: &'o Outline, object_index: u32, view_box: &Rect<f32>) -> Tiler<'o> {
        let bounds = outline.bounds.intersection(&view_box).unwrap_or(Rect::zero());
        let built_object = BuiltObject::new(&bounds);

        Tiler {
            outline,
            object_index,
            built_object,

            view_box: *view_box,
            bounds,

            point_queue: SortedVector::new(),
            active_edges: SortedVector::new(),
            old_active_edges: vec![],
        }
    }

    #[inline(never)]
    fn generate_tiles(&mut self) {
        // Initialize the point queue.
        self.init_point_queue();

        // Reset active edges.
        self.active_edges.clear();
        self.old_active_edges.clear();

        // Generate strips.
        let tile_rect = self.built_object.tile_rect;
        for strip_origin_y in tile_rect.origin.y..tile_rect.max_y() {
            self.generate_strip(strip_origin_y);
        }
    }

    #[inline(never)]
    fn generate_strip(&mut self, strip_origin_y: i16) {
        // Process old active edges.
        self.process_old_active_edges(strip_origin_y);

        // Add new active edges.
        let strip_max_y = (strip_origin_y + 1) as f32 * TILE_HEIGHT;
        while let Some(queued_endpoint) = self.point_queue.peek() {
            if queued_endpoint.y >= strip_max_y {
                break
            }
            self.add_new_active_edge(strip_origin_y);
        }
    }

    #[inline(never)]
    fn process_old_active_edges(&mut self, tile_y: i16) {
        let mut current_tile_x = self.built_object.tile_rect.origin.x;
        let mut current_subtile_x = 0.0;
        let mut current_winding = 0;

        debug_assert!(self.old_active_edges.is_empty());
        mem::swap(&mut self.old_active_edges, &mut self.active_edges.array);

        for mut active_edge in self.old_active_edges.drain(..) {
            // Determine x-intercept and winding.
            let (segment_x, edge_winding) =
                if active_edge.segment.from.y < active_edge.segment.to.y {
                    (active_edge.segment.from.x, 1)
                } else {
                    (active_edge.segment.to.x, -1)
                };

            // Move over to the correct tile, filling in as we go.
            let mut segment_tile_x = f32::floor(segment_x / TILE_WIDTH) as i16;
            while current_tile_x < segment_tile_x {
                //println!("... filling!");
                self.built_object.get_tile_mut(current_tile_x, tile_y).backdrop = current_winding;
                current_tile_x += 1;
                current_subtile_x = 0.0;
            }

            // Do subtile fill, if necessary.
            debug_assert!(current_tile_x < self.built_object.tile_rect.max_x());
            let current_x = (current_tile_x as f32) * TILE_WIDTH + current_subtile_x;
            if segment_x >= current_x {
                let (left, right) = (Point2D::new(current_x, 0.0), Point2D::new(segment_x, 0.0));
                self.built_object.add_fill(if edge_winding < 0 { &left  } else { &right },
                                           if edge_winding < 0 { &right } else { &left  },
                                           current_tile_x,
                                           tile_y);
            }

            // Update winding.
            current_winding += edge_winding;

            // Process the edge.
            process_active_edge(&mut active_edge.segment, &mut self.built_object, tile_y);
            if !active_edge.segment.is_none() {
                self.active_edges.push(active_edge);
            }
        }
    }

    #[inline(never)]
    fn add_new_active_edge(&mut self, tile_y: i16) {
        let outline = &self.outline;
        let point_index = self.point_queue.pop().unwrap().point_index;

        let contour = &outline.contours[point_index.contour() as usize];

        // TODO(pcwalton): Could use a bitset of processed edges…
        let prev_endpoint_index = contour.prev_endpoint_index_of(point_index.point());
        let next_endpoint_index = contour.next_endpoint_index_of(point_index.point());
        if contour.point_is_logically_above(point_index.point(), prev_endpoint_index) {
            process_active_segment(contour,
                                   prev_endpoint_index,
                                   &mut self.active_edges,
                                   &mut self.built_object,
                                   tile_y);

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), prev_endpoint_index),
                y: contour.position_of(prev_endpoint_index).y,
            });
        }

        if contour.point_is_logically_above(point_index.point(), next_endpoint_index) {
            process_active_segment(contour,
                                   point_index.point(),
                                   &mut self.active_edges,
                                   &mut self.built_object,
                                   tile_y);

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), next_endpoint_index),
                y: contour.position_of(next_endpoint_index).y,
            });
        }
    }

    #[inline(never)]
    fn init_point_queue(&mut self) {
        // Find MIN points.
        self.point_queue.clear();
        for (contour_index, contour) in self.outline.contours.iter().enumerate() {
            let contour_index = contour_index as u32;
            let mut cur_endpoint_index = 0;
            let mut prev_endpoint_index = contour.prev_endpoint_index_of(cur_endpoint_index);
            let mut next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            while cur_endpoint_index < next_endpoint_index {
                if contour.point_is_logically_above(cur_endpoint_index, prev_endpoint_index) &&
                        contour.point_is_logically_above(cur_endpoint_index, next_endpoint_index) {
                    self.point_queue.push(QueuedEndpoint {
                        point_index: PointIndex::new(contour_index, cur_endpoint_index),
                        y: contour.position_of(cur_endpoint_index).y,
                    });
                }

                prev_endpoint_index = cur_endpoint_index;
                cur_endpoint_index = next_endpoint_index;
                next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            }
        }
    }
}

fn process_active_segment(contour: &Contour,
                          from_endpoint_index: u32,
                          active_edges: &mut SortedVector<ActiveEdge>,
                          built_object: &mut BuiltObject,
                          tile_y: i16) {
    let mut segment = contour.segment_after(from_endpoint_index);
    if segment.is_degenerate() {
        return
    }

    process_active_edge(&mut segment, built_object, tile_y);

    if !segment.is_none() {
        active_edges.push(ActiveEdge::new(segment));
    }
}

fn process_active_edge(active_edge: &mut Segment, built_object: &mut BuiltObject, tile_y: i16) {
    // Chop the segment.
    // TODO(pcwalton): Maybe these shouldn't be Options?
    let (upper_segment, lower_segment) = active_edge.split_y((tile_y + 1) as f32 * TILE_HEIGHT);

    // Add fill primitives for upper part.
    if let Some(segment) = upper_segment {
        segment.generate_fill_primitives(built_object, tile_y);
    }

    // Queue lower part.
    *active_edge = lower_segment.unwrap_or(Segment::new());
}

// Scene construction

impl BuiltScene {
    fn new(view_box: &Rect<f32>, object_count: u32) -> BuiltScene {
        BuiltScene {
            view_box: *view_box,
            object_count,
            fills: vec![],
            solid_tiles: vec![],
            mask_tiles: vec![],

            tile_rect: round_rect_out_to_tile_bounds(view_box),
        }
    }

    #[inline(never)]
    fn from_objects(view_box: &Rect<f32>, objects: &[BuiltObject]) -> BuiltScene {
        let mut scene = BuiltScene::new(view_box, objects.len() as u32);

        let mut z_buffer = FixedBitSet::with_capacity(scene.tile_rect.size.width as usize *
                                                      scene.tile_rect.size.height as usize);

        let mut object_tile_index_to_scene_mask_tile_index = vec![];

        for (object_index, object) in objects.iter().enumerate().rev() {
            object_tile_index_to_scene_mask_tile_index.clear();
            object_tile_index_to_scene_mask_tile_index.reserve(object.tiles.len());

            // Copy tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                let scene_tile_index = scene.scene_tile_index(tile.tile_x, tile.tile_y);
                if z_buffer[scene_tile_index as usize] {
                    // Occluded.
                    object_tile_index_to_scene_mask_tile_index.push(u32::MAX);
                } else if object.mask_tiles[tile_index] {
                    // Visible mask tile.
                    let scene_mask_tile_index = scene.mask_tiles.len() as u32;
                    object_tile_index_to_scene_mask_tile_index.push(scene_mask_tile_index);
                    scene.mask_tiles.push(MaskTileScenePrimitive {
                        tile: *tile,
                        object_index: object_index as u32,
                    });
                } else {
                    // Visible transparent or solid tile.
                    object_tile_index_to_scene_mask_tile_index.push(u32::MAX);
                    if tile.backdrop != 0 {
                        scene.solid_tiles.push(SolidTileScenePrimitive {
                            tile_x: tile.tile_x,
                            tile_y: tile.tile_y,
                            object_index: object_index as u32,
                        });
                        z_buffer.insert(scene_tile_index as usize);
                    }
                }
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index = object.tile_coords_to_index(fill.tile_x, fill.tile_y);
                match object_tile_index_to_scene_mask_tile_index[object_tile_index as usize] {
                    u32::MAX => {}
                    scene_mask_tile_index => {
                        scene.fills.push(FillScenePrimitive {
                            from: fill.from,
                            to: fill.to,
                            mask_tile_index: scene_mask_tile_index,
                        })
                    }
                }
            }
        }

        scene
    }

    fn scene_tile_index(&self, tile_x: i16, tile_y: i16) -> u32 {
        (tile_y - self.tile_rect.origin.y) as u32 * self.tile_rect.size.width as u32 +
            (tile_x - self.tile_rect.origin.x) as u32
    }
}

// Primitives

#[derive(Debug)]
struct BuiltObject {
    bounds: Rect<f32>,
    tile_rect: Rect<i16>,
    tiles: Vec<TileObjectPrimitive>,
    fills: Vec<FillObjectPrimitive>,
    mask_tiles: FixedBitSet,
}

#[derive(Debug)]
struct BuiltScene {
    view_box: Rect<f32>,
    fills: Vec<FillScenePrimitive>,
    solid_tiles: Vec<SolidTileScenePrimitive>,
    mask_tiles: Vec<MaskTileScenePrimitive>,
    object_count: u32,

    tile_rect: Rect<i16>,
}

#[derive(Clone, Copy, Debug)]
struct FillObjectPrimitive {
    from: Point2D<f32>,
    to: Point2D<f32>,
    tile_x: i16,
    tile_y: i16,
}

#[derive(Clone, Copy, Debug)]
struct TileObjectPrimitive {
    tile_x: i16,
    tile_y: i16,
    backdrop: i32,
}

#[derive(Clone, Copy, Debug)]
struct FillScenePrimitive {
    from: Point2D<f32>,
    to: Point2D<f32>,
    mask_tile_index: u32,
}

#[derive(Clone, Copy, Debug)]
struct SolidTileScenePrimitive {
    tile_x: i16,
    tile_y: i16,
    object_index: u32,
}

#[derive(Clone, Copy, Debug)]
struct MaskTileScenePrimitive {
    tile: TileObjectPrimitive,
    object_index: u32,
}

#[derive(Clone, Copy, Debug)]
struct ColorU {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

// Utilities for built objects

impl BuiltObject {
    fn new(bounds: &Rect<f32>) -> BuiltObject {
        // Compute the tile rect.
        let tile_rect = round_rect_out_to_tile_bounds(&bounds);

        // Allocate tiles.
        let tile_count = tile_rect.size.width as usize * tile_rect.size.height as usize;
        let mut tiles = Vec::with_capacity(tile_count);
        for y in tile_rect.origin.y..tile_rect.max_y() {
            for x in tile_rect.origin.x..tile_rect.max_x() {
                tiles.push(TileObjectPrimitive::new(x, y));
            }
        }

        BuiltObject {
            bounds: *bounds,
            tile_rect,
            tiles,
            fills: vec![],
            mask_tiles: FixedBitSet::with_capacity(tile_count),
        }
    }

    fn add_fill(&mut self, from: &Point2D<f32>, to: &Point2D<f32>, tile_x: i16, tile_y: i16) {
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        self.fills.push(FillObjectPrimitive { from: *from, to: *to, tile_x, tile_y });
        self.mask_tiles.insert(tile_index as usize);
    }

    // FIXME(pcwalton): Use a `Point2D<i16>` instead?
    fn tile_coords_to_index(&self, tile_x: i16, tile_y: i16) -> u32 {
        /*println!("tile_coords_to_index(x={}, y={}, tile_rect={:?})",
                 tile_x,
                 tile_y,
                 self.tile_rect);*/
        (tile_y - self.tile_rect.origin.y) as u32 * self.tile_rect.size.width as u32 +
            (tile_x - self.tile_rect.origin.x) as u32
    }

    fn get_tile_mut(&mut self, tile_x: i16, tile_y: i16) -> &mut TileObjectPrimitive {
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        &mut self.tiles[tile_index as usize]
    }
}

// Scene serialization

impl BuiltScene {
    fn write<W>(&self, writer: &mut W) -> io::Result<()> where W: Write {
        writer.write_all(b"RIFF")?;

        let header_size = 4 * 4;
        let fill_size = self.fills.len() * mem::size_of::<FillScenePrimitive>();
        let solid_tiles_size = self.solid_tiles.len() * mem::size_of::<SolidTileScenePrimitive>();
        let mask_tiles_size = self.mask_tiles.len() * mem::size_of::<MaskTileScenePrimitive>();
        writer.write_u32::<LittleEndian>((4 +
                                          8 + header_size +
                                          8 + fill_size +
                                          8 + solid_tiles_size +
                                          8 + mask_tiles_size) as u32)?;

        writer.write_all(b"PF3S")?;

        writer.write_all(b"head")?;
        writer.write_u32::<LittleEndian>(header_size as u32)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.x)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.y)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.width)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.height)?;

        writer.write_all(b"fill")?;
        writer.write_u32::<LittleEndian>(fill_size as u32)?;
        for fill_primitive in &self.fills {
            write_point(writer, &fill_primitive.from)?;
            write_point(writer, &fill_primitive.to)?;
            writer.write_u32::<LittleEndian>(fill_primitive.mask_tile_index)?;
        }

        writer.write_all(b"soli")?;
        writer.write_u32::<LittleEndian>(solid_tiles_size as u32)?;
        for &tile_primitive in &self.solid_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile_y)?;
            writer.write_u32::<LittleEndian>(tile_primitive.object_index)?;
        }

        writer.write_all(b"mask")?;
        writer.write_u32::<LittleEndian>(mask_tiles_size as u32)?;
        for &tile_primitive in &self.mask_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_y)?;
            writer.write_i32::<LittleEndian>(tile_primitive.tile.backdrop)?;
            writer.write_u32::<LittleEndian>(tile_primitive.object_index)?;
        }

        return Ok(());

        fn write_point<W>(writer: &mut W, point: &Point2D<f32>) -> io::Result<()> where W: Write {
            writer.write_f32::<LittleEndian>(point.x)?;
            writer.write_f32::<LittleEndian>(point.y)?;
            Ok(())
        }
    }
}

impl SolidTileScenePrimitive {
    fn new(tile_x: i16, tile_y: i16, object_index: u32) -> SolidTileScenePrimitive {
        SolidTileScenePrimitive { tile_x, tile_y, object_index }
    }
}

impl TileObjectPrimitive {
    fn new(tile_x: i16, tile_y: i16) -> TileObjectPrimitive {
        TileObjectPrimitive { tile_x, tile_y, backdrop: 0 }
    }
}

impl ColorU {
    fn black() -> ColorU {
        ColorU { r: 0, g: 0, b: 0, a: 255 }
    }

    fn from_svg_color(svg_color: SvgColor) -> ColorU {
        ColorU { r: svg_color.red, g: svg_color.green, b: svg_color.blue, a: 255 }
    }
}

// Tile geometry utilities

fn round_rect_out_to_tile_bounds(rect: &Rect<f32>) -> Rect<i16> {
    let tile_origin = Point2D::new(f32::floor(rect.origin.x / TILE_WIDTH) as i16,
                                   f32::floor(rect.origin.y / TILE_HEIGHT) as i16);
    let tile_extent = Point2D::new(f32::ceil(rect.max_x() / TILE_WIDTH) as i16,
                                   f32::ceil(rect.max_y() / TILE_HEIGHT) as i16);
    let tile_size = Size2D::new(tile_extent.x - tile_origin.x, tile_extent.y - tile_origin.y);
    Rect::new(tile_origin, tile_size)
}

// SVG stuff

struct SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    iter: &'a mut I,
    last_endpoint: Point2D<f32>,
    last_ctrl_point: Option<Point2D<f32>>,
}

impl<'a, I> SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    fn new(iter: &'a mut I) -> SvgPathToPathEvents<'a, I> {
        SvgPathToPathEvents { iter, last_endpoint: Point2D::zero(), last_ctrl_point: None }
    }
}

impl<'a, I> Iterator for SvgPathToPathEvents<'a, I> where I: Iterator<Item = SvgPathSegment> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        return match self.iter.next() {
            None => None,
            Some(SvgPathSegment::MoveTo { abs, x, y }) => {
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                self.last_ctrl_point = None;
                Some(PathEvent::MoveTo(to))
            }
            Some(SvgPathSegment::LineTo { abs, x, y }) => {
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::HorizontalLineTo { abs, x }) => {
                let to = compute_point(x, 0.0, abs, &self.last_endpoint);
                self.last_endpoint = to;
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::VerticalLineTo { abs, y }) => {
                let to = compute_point(0.0, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                self.last_ctrl_point = None;
                Some(PathEvent::LineTo(to))
            }
            Some(SvgPathSegment::Quadratic { abs, x1, y1, x, y }) => {
                let ctrl = compute_point(x1, y1, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                Some(PathEvent::QuadraticTo(ctrl, to))
            }
            Some(SvgPathSegment::SmoothQuadratic { abs, x, y }) => {
                let ctrl = reflect_point(&self.last_endpoint, &self.last_ctrl_point);
                self.last_ctrl_point = Some(ctrl);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                Some(PathEvent::QuadraticTo(ctrl, to))
            }
            Some(SvgPathSegment::CurveTo { abs, x1, y1, x2, y2, x, y }) => {
                let ctrl0 = compute_point(x1, y1, abs, &self.last_endpoint);
                let ctrl1 = compute_point(x2, y2, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl1);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
            }
            Some(SvgPathSegment::SmoothCurveTo { abs, x2, y2, x, y }) => {
                let ctrl0 = reflect_point(&self.last_endpoint, &self.last_ctrl_point);
                let ctrl1 = compute_point(x2, y2, abs, &self.last_endpoint);
                self.last_ctrl_point = Some(ctrl1);
                let to = compute_point(x, y, abs, &self.last_endpoint);
                self.last_endpoint = to;
                Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
            }
            Some(SvgPathSegment::ClosePath { abs: _ }) => {
                // FIXME(pcwalton): Current endpoint becomes path initial point!
                self.last_ctrl_point = None;
                Some(PathEvent::Close)
            }
            Some(SvgPathSegment::EllipticalArc { .. }) => unimplemented!("arcs"),
        };

        fn compute_point(x: f64, y: f64, abs: bool, last_endpoint: &Point2D<f32>)
                         -> Point2D<f32> {
            let point = Point2D::new(x, y).to_f32();
            if !abs {
                *last_endpoint + point.to_vector()
            } else {
                point
            }
        }

        fn reflect_point(last_endpoint: &Point2D<f32>, last_ctrl_point: &Option<Point2D<f32>>)
                         -> Point2D<f32> {
            match *last_ctrl_point {
                Some(ref last_ctrl_point) => {
                    let vector = *last_endpoint - *last_ctrl_point;
                    *last_endpoint + vector
                }
                None => *last_endpoint,
            }
        }
    }
}

// Monotonic conversion utilities

// TODO(pcwalton): I think we only need to be monotonic in Y, maybe?
struct MonotonicConversionIter<I> where I: Iterator<Item = PathEvent> {
    inner: I,
    buffer: Option<PathEvent>,
    last_point: Point2D<f32>,
}

impl<I> Iterator for MonotonicConversionIter<I> where I: Iterator<Item = PathEvent> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        if self.buffer.is_none() {
            match self.inner.next() {
                None => return None,
                Some(event) => self.buffer = Some(event),
            }
        }

        match self.buffer.take().unwrap() {
            PathEvent::MoveTo(to) => {
                self.last_point = to;
                Some(PathEvent::MoveTo(to))
            }
            PathEvent::LineTo(to) => {
                self.last_point = to;
                Some(PathEvent::LineTo(to))
            }
            PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                let segment = CubicBezierSegment {
                    from: self.last_point,
                    ctrl1: ctrl0,
                    ctrl2: ctrl1,
                    to,
                };
                if segment.is_monotonic() {
                    self.last_point = to;
                    return Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
                }
                // FIXME(pcwalton): O(n^2)!
                let mut t = None;
                segment.for_each_monotonic_t(|split_t| {
                    if t.is_none() {
                        t = Some(split_t)
                    }
                });
                let t = t.unwrap();
                if t_is_too_close_to_zero_or_one(t) {
                    self.last_point = to;
                    return Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
                }
                let (prev, next) = segment.split(t);
                self.last_point = next.from;
                self.buffer = Some(PathEvent::CubicTo(next.ctrl1, next.ctrl2, next.to));
                return Some(PathEvent::CubicTo(prev.ctrl1, prev.ctrl2, prev.to));
            }
            PathEvent::QuadraticTo(ctrl, to) => {
                let segment = QuadraticBezierSegment { from: self.last_point, ctrl: ctrl, to };
                if segment.is_monotonic() {
                    self.last_point = to;
                    return Some(PathEvent::QuadraticTo(ctrl, to))
                }
                // FIXME(pcwalton): O(n^2)!
                let mut t = None;
                segment.for_each_monotonic_t(|split_t| {
                    if t.is_none() {
                        t = Some(split_t)
                    }
                });
                let t = t.unwrap();
                if t_is_too_close_to_zero_or_one(t) {
                    self.last_point = to;
                    return Some(PathEvent::QuadraticTo(ctrl, to))
                }
                let (prev, next) = segment.split(t);
                self.last_point = next.from;
                self.buffer = Some(PathEvent::QuadraticTo(next.ctrl, next.to));
                return Some(PathEvent::QuadraticTo(prev.ctrl, prev.to));
            }
            PathEvent::Close => Some(PathEvent::Close),
            PathEvent::Arc(a, b, c, d) => {
                // FIXME(pcwalton): Make these monotonic too.
                return Some(PathEvent::Arc(a, b, c, d))
            }
        }
    }
}

impl<I> MonotonicConversionIter<I> where I: Iterator<Item = PathEvent> {
    fn new(inner: I) -> MonotonicConversionIter<I> {
        MonotonicConversionIter {
            inner,
            buffer: None,
            last_point: Point2D::zero(),
        }
    }
}

// Path utilities

trait SolveT {
    fn sample(&self, t: f32) -> f32;
    fn sample_deriv(&self, t: f32) -> f32;

    // TODO(pcwalton): Use Brent's method.
    fn solve_for_t(&self, x: f32) -> Option<f32> {
        const MAX_ITERATIONS: u32 = 64;
        const TOLERANCE: f32 = 0.001;

        let (mut min, mut max) = (0.0, 1.0);
        let (mut x_min, x_max) = (self.sample(min) - x, self.sample(max) - x);
        if (x_min < 0.0 && x_max < 0.0) || (x_min > 0.0 && x_max > 0.0) {
            return None
        }

        let mut iteration = 0;
        loop {
            let mid = lerp(min, max, 0.5);
            if iteration >= MAX_ITERATIONS || (max - min) * 0.5 < TOLERANCE {
                return Some(mid)
            }

            let x_mid = self.sample(mid) - x;
            if x_mid == 0.0 {
                return Some(mid)
            }

            if (x_min < 0.0 && x_mid < 0.0) || (x_min > 0.0 && x_mid > 0.0) {
                min = mid;
                x_min = x_mid;
            } else {
                max = mid;
            }

            iteration += 1;
        }
    }
}

// FIXME(pcwalton): This is probably dumb and inefficient.
struct LineAxis { from: f32, to: f32 }
impl LineAxis {
    fn from_x(segment: &LineSegment<f32>) -> LineAxis {
        LineAxis { from: segment.from.x, to: segment.to.x }
    }
    fn from_y(segment: &LineSegment<f32>) -> LineAxis {
        LineAxis { from: segment.from.y, to: segment.to.y }
    }
}
impl SolveT for LineAxis {
    fn sample(&self, t: f32) -> f32 {
        lerp(self.from, self.to, t)
    }
    fn sample_deriv(&self, _: f32) -> f32 {
        self.to - self.from
    }
}

struct QuadraticAxis { from: f32, ctrl: f32, to: f32 }
impl QuadraticAxis {
    fn from_x(segment: &QuadraticBezierSegment<f32>) -> QuadraticAxis {
        QuadraticAxis { from: segment.from.x, ctrl: segment.ctrl.x, to: segment.to.x }
    }
    fn from_y(segment: &QuadraticBezierSegment<f32>) -> QuadraticAxis {
        QuadraticAxis { from: segment.from.y, ctrl: segment.ctrl.y, to: segment.to.y }
    }
}
impl SolveT for QuadraticAxis {
    fn sample(&self, t: f32) -> f32 {
        lerp(lerp(self.from, self.ctrl, t), lerp(self.ctrl, self.to, t), t)
    }
    fn sample_deriv(&self, t: f32) -> f32 {
        2.0 * lerp(self.ctrl - self.from, self.to - self.ctrl, t)
    }
}

struct CubicAxis { from: f32, ctrl0: f32, ctrl1: f32, to: f32 }
impl CubicAxis {
    fn from_x(segment: &CubicBezierSegment<f32>) -> CubicAxis {
        CubicAxis {
            from: segment.from.x,
            ctrl0: segment.ctrl1.x,
            ctrl1: segment.ctrl2.x,
            to: segment.to.x,
        }
    }
    fn from_y(segment: &CubicBezierSegment<f32>) -> CubicAxis {
        CubicAxis {
            from: segment.from.y,
            ctrl0: segment.ctrl1.y,
            ctrl1: segment.ctrl2.y,
            to: segment.to.y,
        }
    }
}
impl SolveT for CubicAxis {
    fn sample(&self, t: f32) -> f32 {
        let b3 = self.to + 3.0 * (self.ctrl0 - self.ctrl1) - self.from;
        let b2 = 3.0 * (self.from - 2.0 * self.ctrl0 + self.ctrl1) + b3 * t;
        let b1 = 3.0 * (self.ctrl0 - self.from) + b2 * t;
        let b0 = self.from + b1 * t;
        b0
    }
    fn sample_deriv(&self, t: f32) -> f32 {
        let inv_t = 1.0 - t;
        3.0 * inv_t * inv_t * (self.ctrl0 - self.from) +
            6.0 * inv_t * t * (self.ctrl1 - self.ctrl0) +
            3.0 * t * t * (self.to - self.ctrl1)
    }
}

// SortedVector

#[derive(Clone, Debug)]
pub struct SortedVector<T> where T: PartialOrd {
    array: Vec<T>,
}

impl<T> SortedVector<T> where T: PartialOrd {
    fn new() -> SortedVector<T> {
        SortedVector { array: vec![] }
    }

    fn push(&mut self, value: T) {
        self.array.push(value);
        let mut index = self.array.len() - 1;
        while index > 0 {
            index -= 1;
            if self.array[index] <= self.array[index + 1] {
                break
            }
            self.array.swap(index, index + 1);
        }
    }

    fn peek(&self) -> Option<&T>   { self.array.last()     }
    fn pop(&mut self) -> Option<T> { self.array.pop()      }
    fn clear(&mut self)            { self.array.clear()    }

    #[allow(dead_code)]
    fn is_empty(&self) -> bool     { self.array.is_empty() }
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
}

impl ActiveEdge {
    fn new(segment: Segment) -> ActiveEdge {
        ActiveEdge { segment }
    }
}

impl PartialOrd<ActiveEdge> for ActiveEdge {
    fn partial_cmp(&self, other: &ActiveEdge) -> Option<Ordering> {
        // NB: Reversed!
        let this_x = if self.segment.from.y < self.segment.to.y {
            self.segment.from.x
        } else {
            self.segment.to.x
        };
        let other_x = if other.segment.from.y < other.segment.to.y {
            other.segment.from.x
        } else {
            other.segment.to.x
        };
        this_x.partial_cmp(&other_x)
    }
}

// Trivial utilities

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn clamp(x: f32, min: f32, max: f32) -> f32 {
    f32::max(f32::min(x, max), min)
}

fn t_is_too_close_to_zero_or_one(t: f32) -> bool {
    const EPSILON: f32 = 0.001;

    t < EPSILON || t > 1.0 - EPSILON
}

// Testing

#[cfg(test)]
mod test {
    use crate::SortedVector;
    use quickcheck;

    #[test]
    fn test_sorted_vec() {
        quickcheck::quickcheck(prop_sorted_vec as fn(Vec<i32>) -> bool);

        fn prop_sorted_vec(mut values: Vec<i32>) -> bool {
            let mut sorted_vec = SortedVector::new();
            for &value in &values {
                sorted_vec.push(value)
            }

            values.sort();
            let mut results = Vec::with_capacity(values.len());
            while !sorted_vec.is_empty() {
                results.push(sorted_vec.pop().unwrap());
            }
            results.reverse();
            assert_eq!(&values, &results);

            true
        }
    }
}
