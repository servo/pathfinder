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
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::mem;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;
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
    let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());
    let output_path = matches.value_of("OUTPUT").map(PathBuf::from);

    let scene = Scene::from_path(&input_path);
    println!("Scene bounds: {:?}", scene.bounds);

    let start_time = Instant::now();
    let mut built_scene = BuiltScene::new();
    for _ in 0..runs {
        built_scene = scene.build();
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
    view_box: Option<Rect<f32>>,
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

    fn build(&self) -> BuiltScene {
        let mut built_scene = BuiltScene::new();
        for (index, object) in self.objects.iter().enumerate() {
            let mut tiler = Tiler::from_outline(&object.outline,
                                                object.color,
                                                &self.view_box,
                                                &mut built_scene);
            tiler.generate_tiles();
            // TODO(pcwalton)
        }
        built_scene
    }

    fn push_svg_path(&mut self, value: &str, style: StyleId, name: String) {
        if self.get_style(style).stroke_width > 0.0 {
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

    fn segment_after(&self, endpoint_index: PointIndex) -> Segment {
        self.contours[endpoint_index.contour_index].segment_after(endpoint_index.point_index)
    }

    fn point_is_logically_above(&self, a: &PointIndex, b: &PointIndex) -> bool {
        let a_y = self.contours[a.contour_index].points[a.point_index].y;
        let b_y = self.contours[b.contour_index].points[b.point_index].y;
        match a_y.partial_cmp(&b_y) {
            Some(Ordering::Less) => true,
            Some(Ordering::Greater) => false,
            None | Some(Ordering::Equal) => {
                match a.contour_index.cmp(&b.contour_index) {
                    Ordering::Less => true,
                    Ordering::Greater => false,
                    Ordering::Equal => a.point_index < b.point_index,
                }
            }
        }
    }

    fn get(&self, point_index: &PointIndex) -> Point2D<f32> {
        self.contours[point_index.contour_index].points[point_index.point_index]
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

    fn point_is_logically_above(&self, a: usize, b: usize) -> bool {
        let (a_y, b_y) = (self.points[a].y, self.points[b].y);
        match a_y.partial_cmp(&b_y) {
            Some(Ordering::Less) => true,
            Some(Ordering::Greater) => false,
            None | Some(Ordering::Equal) => a < b,
        }
    }

    fn prev_endpoint_index_of(&self, mut point_index: usize) -> usize {
        loop {
            point_index = self.prev_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index
            }
        }
    }

    fn next_endpoint_index_of(&self, mut point_index: usize) -> usize {
        loop {
            point_index = self.next_point_index_of(point_index);
            if self.point_is_endpoint(point_index) {
                return point_index
            }
        }
    }

    fn prev_point_index_of(&self, point_index: usize) -> usize {
        if point_index == 0 {
            self.points.len() - 1
        } else {
            point_index - 1
        }
    }

    fn next_point_index_of(&self, point_index: usize) -> usize {
        if point_index == self.points.len() - 1 {
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

#[derive(Clone, Copy, Debug, Default)]
struct PointIndex {
    contour_index: usize,
    point_index: usize,
}

struct ContourIter<'a> {
    contour: &'a Contour,
    index: usize,
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
        //println!("split_y({:?}, {:?})", self, y);

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
                //println!("... split line at {}: {:?}, {:?}", t, prev, next);
                (Segment::from_line(&prev), Segment::from_line(&next))
            }
            None => {
                // TODO(pcwalton): Don't degree elevate!
                let mut cubic_segment = self.as_cubic_segment().unwrap();
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
    fn generate_fill_primitives(&self,
                                strip_origin: &Point2D<f32>,
                                primitives: &mut Vec<FillPrimitive>) {
        if let Some(ref line_segment) = self.as_line_segment() {
            //println!("generate_fill_primitives({:?}, {:?})", strip_origin, line_segment);
            generate_fill_primitives_for_line(line_segment, strip_origin, primitives);
            return;
        }

        // TODO(pcwalton): Don't degree elevate!
        let segment = self.as_cubic_segment().unwrap();
        let flattener = Flattened::new(segment, FLATTENING_TOLERANCE);
        let mut from = self.from;
        for to in flattener {
            generate_fill_primitives_for_line(&LineSegment { from, to }, strip_origin, primitives);
            from = to;
        }

        fn generate_fill_primitives_for_line(segment: &LineSegment<f32>,
                                             strip_origin: &Point2D<f32>,
                                             primitives: &mut Vec<FillPrimitive>) {
            let mut segment = *segment;

            // TODO(pcwalton): Factor this point-to-tile logic out. It keeps getting repeated…
            let mut from_tile_index =
                f32::max(0.0, f32::floor((segment.from.x - strip_origin.x) / TILE_WIDTH)) as u32;
            loop {
                let tile_offset =
                    Vector2D::new(from_tile_index as f32 * TILE_WIDTH + strip_origin.x,
                                  strip_origin.y);

                let to_tile_index =
                    f32::max(0.0, f32::floor((segment.to.x - strip_origin.x) / TILE_WIDTH)) as u32;

                if from_tile_index == to_tile_index {
                    /*println!("... ... pushing LAST fill primitive {}: {:?} @ {:?}",
                             primitives.len(),
                             segment,
                             tile_offset);*/
                    primitives.push(FillPrimitive {
                        from: segment.from - tile_offset,
                        to: segment.to - tile_offset,
                        tile_index: from_tile_index,
                    });
                    break;
                }

                // Split line at tile boundary.
                let (next_tile_index, split_x) = if segment.from.x < segment.to.x {
                    (from_tile_index + 1, tile_offset.x + TILE_WIDTH)
                } else {
                    (from_tile_index - 1, tile_offset.x)
                };
                let (prev_segment, next_segment) = segment.split_at_x(split_x);
                /*println!("... ... pushing fill primitive {}: {:?} @ {:?}",
                         primitives.len(),
                         prev_segment,
                         tile_offset);*/
                primitives.push(FillPrimitive {
                    from: prev_segment.from - tile_offset,
                    to: prev_segment.to - tile_offset,
                    tile_index: from_tile_index,
                });

                from_tile_index = next_tile_index;
                segment = next_segment;
            }
        }
    }

    fn is_none(&self) -> bool {
        !self.flags.contains(SegmentFlags::HAS_ENDPOINTS)
    }

    fn min_y(&self) -> f32 {
        f32::min(self.from.y, self.to.y)
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
    fill_color: ColorU,
    built_scene: &'p mut BuiltScene,

    view_box: Option<Rect<f32>>,

    point_queue: Heap<PointIndex>,
    active_intervals: Intervals,
    active_edges: Vec<Segment>,
}

impl<'o, 'p> Tiler<'o, 'p> {
    fn from_outline(outline: &'o Outline,
                    fill_color: ColorU,
                    view_box: &Option<Rect<f32>>,
                    built_scene: &'p mut BuiltScene)
                    -> Tiler<'o, 'p> {
        Tiler {
            outline,
            fill_color,
            built_scene,

            view_box: *view_box,

            point_queue: Heap::new(),
            active_intervals: Intervals::new(0.0..0.0),
            active_edges: vec![],
        }
    }

    fn generate_tiles(&mut self) {
        // Find MIN points.
        self.point_queue.clear();
        for (contour_index, contour) in self.outline.contours.iter().enumerate() {
            let mut cur_endpoint_index = 0;
            let mut prev_endpoint_index = contour.prev_endpoint_index_of(cur_endpoint_index);
            let mut next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            while cur_endpoint_index < next_endpoint_index {
                if contour.point_is_logically_above(cur_endpoint_index, prev_endpoint_index) &&
                        contour.point_is_logically_above(cur_endpoint_index, next_endpoint_index) {
                    let point_index = PointIndex {
                        contour_index,
                        point_index: cur_endpoint_index,
                    };
                    let outline = &self.outline;
                    self.point_queue.push(point_index, |a_index, b_index| {
                        if outline.point_is_logically_above(a_index, b_index) {
                            Ordering::Less
                        } else {
                            Ordering::Greater
                        }
                    });
                }

                prev_endpoint_index = cur_endpoint_index;
                cur_endpoint_index = next_endpoint_index;
                next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            }
        }
        // Sort all edge indices.
        // TODO(pcwalton): Only find MIN points.
        /*
        self.sorted_edge_indices.clear();
        for contour_index in 0..self.outline.contours.len() {
            let contour = &self.outline.contours[contour_index];
            for point_index in 0..contour.points.len() {
                if contour.is_min_point(point_index) {
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
        */

        // Clip to the view box.
        let mut bounds = self.outline.bounds;
        if let Some(view_box) = self.view_box {
            let max_x = f32::min(view_box.max_x(), bounds.max_x());
            let max_y = f32::min(view_box.max_y(), bounds.max_y());
            bounds.origin.x = f32::max(view_box.origin.x, bounds.origin.x);
            bounds.size.width = f32::max(0.0, max_x - bounds.origin.x);
            bounds.size.height = f32::max(0.0, max_y - bounds.origin.y);
        }

        self.active_intervals.reset(bounds.origin.x, bounds.max_x());
        self.active_edges.clear();

        let mut next_edge_index_index = 0;

        let mut strip_origin =
            Point2D::new(f32::floor(bounds.origin.x / TILE_WIDTH) * TILE_WIDTH,
                         f32::floor(bounds.origin.y / TILE_HEIGHT) * TILE_HEIGHT);
        let strip_right_extent = f32::ceil(bounds.max_x() / TILE_WIDTH) * TILE_WIDTH;

        let tiles_across = ((strip_right_extent - strip_origin.x) / TILE_WIDTH) as usize;

        let mut strip_fills = vec![];
        let mut strip_tiles = Vec::with_capacity(tiles_across);
        let mut used_strip_tiles = FixedBitSet::with_capacity(tiles_across);

        // Generate strips.
        while strip_origin.y < bounds.max_y() {
            // Determine strip bounds.
            let strip_extent = Point2D::new(strip_right_extent, strip_origin.y + TILE_HEIGHT);
            let strip_bounds = Rect::new(strip_origin,
                                         Size2D::new(strip_right_extent - strip_origin.x,
                                                     strip_extent.y - strip_origin.y));

            // We can skip a bunch of steps if we're above the viewport.
            let above_view_box = match self.view_box {
                Some(ref view_box) => strip_extent.y <= view_box.origin.y,
                None => false,
            };

            // Reset strip info.
            strip_fills.clear();
            strip_tiles.clear();
            used_strip_tiles.clear();

            // Allocate tiles.
            let mut tile_left = strip_origin.x;
            while tile_left < strip_right_extent {
                let strip_origin = Point2D::new(tile_left, strip_origin.y);
                strip_tiles.push(MaskTilePrimitive::new(&strip_origin, self.fill_color));
                tile_left += TILE_WIDTH;
            }

            // Populate tile strip with active intervals.
            // TODO(pcwalton): Use only the active edge list!
            let mut strip_tile_index = 0;
            for interval in &self.active_intervals.ranges {
                while strip_tile_index < strip_tiles.len() {
                    let tile_left = strip_tiles[strip_tile_index].position.x;
                    let tile_right = tile_left + TILE_WIDTH;

                    let tile_interval = intersect_ranges(tile_left..tile_right,
                                                         interval.start..interval.end);
                    if interval.winding != 0.0 {
                        if tile_interval == (tile_left..tile_right) {
                            strip_tiles[strip_tile_index].backdrop = interval.winding
                        } else if tile_interval.start < tile_interval.end {
                            let left = Point2D::new(tile_interval.start - tile_left, 0.0);
                            let right = Point2D::new(tile_interval.end - tile_left, 0.0);
                            strip_fills.push(FillPrimitive {
                                from:       if interval.winding < 0.0 { left } else { right },
                                to:         if interval.winding < 0.0 { right } else { left },
                                tile_index: strip_tile_index as u32,
                            });
                            used_strip_tiles.insert(strip_tile_index);
                        }
                    }

                    if tile_right > interval.end {
                        break
                    }

                    strip_tile_index += 1;
                }
            }

            // Process old active edges.
            for active_edge in &mut self.active_edges {
                let fills = if above_view_box { None } else { Some(&mut strip_fills) };
                //println!("processing old active edge: {:?}", active_edge);
                process_active_edge(active_edge,
                                    &strip_bounds,
                                    fills,
                                    &mut self.active_intervals,
                                    &mut used_strip_tiles)
            }
            self.active_edges.retain(|edge| !edge.is_none());

            // Add new active edges.
            loop {
                match self.point_queue.peek_min() {
                    Some(point_index) if self.outline.get(point_index).y < strip_extent.y => {}
                    Some(_) | None => break,
                }

                let outline = &self.outline;
                let point_index = self.point_queue.shift_min(|a_index, b_index| {
                    if outline.point_is_logically_above(a_index, b_index) {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                }).unwrap();

                let contour = &outline.contours[point_index.contour_index];

                let prev_endpoint_index = contour.prev_endpoint_index_of(point_index.point_index);
                let next_endpoint_index = contour.next_endpoint_index_of(point_index.point_index);
                if contour.point_is_logically_above(point_index.point_index, prev_endpoint_index) {
                    let fills = if above_view_box { None } else { Some(&mut strip_fills) };
                    process_active_segment(contour,
                                           prev_endpoint_index,
                                           &mut self.active_edges,
                                           &strip_bounds,
                                           fills,
                                           &mut self.active_intervals,
                                           &mut used_strip_tiles);
                }
                if contour.point_is_logically_above(point_index.point_index, next_endpoint_index) {
                    let fills = if above_view_box { None } else { Some(&mut strip_fills) };
                    process_active_segment(contour,
                                           point_index.point_index,
                                           &mut self.active_edges,
                                           &strip_bounds,
                                           fills,
                                           &mut self.active_intervals,
                                           &mut used_strip_tiles);
                }
            }

            // Finalize tiles.
            if !above_view_box {
                // Flush tiles.
                let first_tile_index = self.built_scene.mask_tiles.len() as u32;
                //println!("--- first tile index {} ---", first_tile_index);
                for (tile_index, tile) in strip_tiles.iter().enumerate() {
                    if used_strip_tiles.contains(tile_index) {
                        /*println!("mask index {} -> {}",
                                 tile_index,
                                 self.built_scene.mask_tiles.len());*/
                        self.built_scene.mask_tiles.push(*tile);
                    } else if tile.backdrop != 0.0 {
                        self.built_scene.solid_tiles.push(SolidTilePrimitive {
                            position: tile.position,
                            color: tile.color,
                        });
                    }
                }

                // Flush fills.
                //
                // TODO(pcwalton): Don't use a temporary vector to hold these.
                for fill in &strip_fills {
                    let real_tile_index = first_tile_index +
                        used_strip_tiles.count_ones(0..(fill.tile_index as usize)) as u32;
                    /*println!("flush fill, mask index {} -> {}",
                             fill.tile_index,
                             real_tile_index);*/
                    self.built_scene.fills.push(FillPrimitive {
                        from: fill.from,
                        to: fill.to,
                        tile_index: real_tile_index,
                    });
                }
            }

            strip_origin.y = strip_extent.y;
        }
    }
}

fn process_active_segment(contour: &Contour,
                          from_endpoint_index: usize,
                          active_edges: &mut Vec<Segment>,
                          strip_bounds: &Rect<f32>,
                          mut fills: Option<&mut Vec<FillPrimitive>>,
                          active_intervals: &mut Intervals,
                          used_tiles: &mut FixedBitSet) {
    let segment = contour.segment_after(from_endpoint_index);
    if segment.is_degenerate() {
        return
    }

    //println!("processing new active edge: {:?}", segment);
    //println!("... is not degenerate ...");
    let strip_range = (strip_bounds.origin.x)..(strip_bounds.max_x());
    let mut segment = match segment.clip_x(strip_range.clone()) {
        Some(segment) => segment,
        None => return,
    };

    //println!("... clipped to {:?}: {:?}", strip_range, segment);
    process_active_edge(&mut segment, &strip_bounds, fills, active_intervals, used_tiles);

    if !segment.is_none() {
        active_edges.push(segment);
    }
}

fn process_active_edge(active_edge: &mut Segment,
                       strip_bounds: &Rect<f32>,
                       mut fills: Option<&mut Vec<FillPrimitive>>,
                       active_intervals: &mut Intervals,
                       used_tiles: &mut FixedBitSet) {
    let strip_extent = strip_bounds.bottom_right();

    // TODO(pcwalton): Maybe these shouldn't be Options?
    let (upper_segment, lower_segment) = active_edge.split_y(strip_extent.y);
    *active_edge = Segment::new();

    if let Some(segment) = upper_segment {
        if let Some(ref mut fills) = fills {
            //println!("process_active_edge: generating fill primitives for {:?}", segment);
            segment.generate_fill_primitives(&strip_bounds.origin, *fills);
        }

        // FIXME(pcwalton): Assumes x-monotonicity!
        let mut from_x = clamp(segment.from.x, 0.0, active_intervals.extent());
        let mut to_x = clamp(segment.to.x, 0.0, active_intervals.extent());
        from_x = clamp(from_x, 0.0, strip_extent.x);
        to_x = clamp(to_x, 0.0, strip_extent.x);
        if from_x < to_x {
            active_intervals.add(IntervalRange::new(from_x, to_x, -1.0))
        } else {
            active_intervals.add(IntervalRange::new(to_x, from_x, 1.0))
        }

        // FIXME(pcwalton): Assumes x-monotonicity!
        // FIXME(pcwalton): Don't hardcode a view box left of 0!
        let mut min_x = f32::min(segment.from.x, segment.to.x);
        let mut max_x = f32::max(segment.from.x, segment.to.x);
        min_x = clamp(min_x, 0.0, strip_extent.x);
        max_x = clamp(max_x, 0.0, strip_extent.x);
        let tile_left = f32::floor(min_x / TILE_WIDTH) * TILE_WIDTH;
        let tile_right = f32::ceil(max_x / TILE_WIDTH) * TILE_WIDTH;
        let left_tile_index = (tile_left - strip_bounds.origin.x) as u32 / TILE_WIDTH as u32;
        let right_tile_index = (tile_right - strip_bounds.origin.x) as u32 / TILE_WIDTH as u32;

        // Set used bits.
        for tile_index in left_tile_index..right_tile_index {
            used_tiles.insert(tile_index as usize);
        }
    }

    match lower_segment {
        Some(segment) => *active_edge = segment,
        None => *active_edge = Segment::new(),
    }
}

// Primitives

#[derive(Debug)]
struct BuiltScene {
    fills: Vec<FillPrimitive>,
    solid_tiles: Vec<SolidTilePrimitive>,
    mask_tiles: Vec<MaskTilePrimitive>,
}

#[derive(Clone, Copy, Debug)]
struct FillPrimitive {
    from: Point2D<f32>,
    to: Point2D<f32>,
    tile_index: u32,
}

#[derive(Clone, Copy, Debug)]
struct SolidTilePrimitive {
    position: Point2D<f32>,
    color: ColorU,
}

#[derive(Clone, Copy, Debug)]
struct MaskTilePrimitive {
    position: Point2D<f32>,
    color: ColorU,
    backdrop: f32,
}

#[derive(Clone, Copy, Debug)]
struct ColorU {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl BuiltScene {
    fn new() -> BuiltScene {
        BuiltScene { fills: vec![], solid_tiles: vec![], mask_tiles: vec![] }
    }

    fn write<W>(&self, writer: &mut W) -> io::Result<()> where W: Write {
        writer.write_all(b"RIFF")?;

        let fill_size = self.fills.len() * mem::size_of::<FillPrimitive>();
        let solid_tiles_size = self.solid_tiles.len() * mem::size_of::<SolidTilePrimitive>();
        let mask_tiles_size = self.mask_tiles.len() * mem::size_of::<MaskTilePrimitive>();
        writer.write_u32::<LittleEndian>((4 +
                                          8 + fill_size +
                                          8 + solid_tiles_size +
                                          8 + mask_tiles_size) as u32)?;

        writer.write_all(b"PF3S")?;

        writer.write_all(b"fill")?;
        writer.write_u32::<LittleEndian>(fill_size as u32)?;
        for fill_primitive in &self.fills {
            write_point(writer, &fill_primitive.from)?;
            write_point(writer, &fill_primitive.to)?;
            writer.write_u32::<LittleEndian>(fill_primitive.tile_index)?;
        }

        writer.write_all(b"soli")?;
        writer.write_u32::<LittleEndian>(solid_tiles_size as u32)?;
        for &tile_primitive in &self.solid_tiles {
            let color = tile_primitive.color;
            write_point(writer, &tile_primitive.position)?;
            writer.write_all(&[color.r, color.g, color.b, color.a]).unwrap();
        }

        writer.write_all(b"mask")?;
        writer.write_u32::<LittleEndian>(mask_tiles_size as u32)?;
        for &tile_primitive in &self.mask_tiles {
            let color = tile_primitive.color;
            write_point(writer, &tile_primitive.position)?;
            writer.write_f32::<LittleEndian>(tile_primitive.backdrop)?;
            writer.write_all(&[color.r, color.g, color.b, color.a]).unwrap();
        }

        return Ok(());

        fn write_point<W>(writer: &mut W, point: &Point2D<f32>) -> io::Result<()> where W: Write {
            writer.write_f32::<LittleEndian>(point.x)?;
            writer.write_f32::<LittleEndian>(point.y)?;
            Ok(())
        }
    }
}

impl SolidTilePrimitive {
    fn new(position: &Point2D<f32>, color: ColorU) -> SolidTilePrimitive {
        SolidTilePrimitive { position: *position, color }
    }
}

impl MaskTilePrimitive {
    fn new(position: &Point2D<f32>, color: ColorU) -> MaskTilePrimitive {
        MaskTilePrimitive { position: *position, backdrop: 0.0, color }
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
    fn new(bounds: Range<f32>) -> Intervals {
        Intervals {
            ranges: vec![IntervalRange::new(bounds.start, bounds.end, 0.0)],
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

    fn reset(&mut self, start: f32, end: f32) {
        self.ranges.truncate(1);
        self.ranges[0] = IntervalRange::new(start, end, 0.0);
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

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
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
    fn sample_deriv(&self, t: f32) -> f32 {
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
        2.0 * (self.to - 2.0 * self.ctrl + self.from)
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
        // FIXME(pcwalton): Use Horner's method or something.
        let p01 = lerp(self.from, self.ctrl0, t);
        let p12 = lerp(self.ctrl0, self.ctrl1, t);
        let p23 = lerp(self.ctrl1, self.to, t);
        let (p012, p123) = (lerp(p01, p12, t), lerp(p12, p23, t));
        lerp(p012, p123, t)
    }
    fn sample_deriv(&self, t: f32) -> f32 {
        let inv_t = 1.0 - t;
        3.0 * inv_t * inv_t * (self.ctrl0 - self.from) +
            6.0 * inv_t * t * (self.ctrl1 - self.ctrl0) +
            3.0 * t * t * (self.to - self.ctrl1)
    }
}

// Heap

#[derive(Clone, Debug)]
pub struct Heap<T> {
    array: Vec<T>,
}

impl<T> Heap<T> {
    fn new() -> Heap<T> {
        Heap { array: vec![] }
    }

    fn sift_up<C>(&mut self, mut index: usize, mut compare: C) where C: FnMut(&T, &T) -> Ordering {
        while index != 0 {
            let parent_index = self.parent_index(index);
            if compare(&self.array[index], &self.array[parent_index]) == Ordering::Less {
                self.array.swap(index, parent_index)
            }
            index = parent_index;
        }
    }

    fn sift_down<C>(&mut self, mut index: usize, mut compare: C)
                    where C: FnMut(&T, &T) -> Ordering {
        while self.first_child_index(index) < self.array.len() {
            let min_child = self.min_child(index, |a, b| compare(a, b));
            if compare(&self.array[index], &self.array[min_child]) == Ordering::Greater {
                self.array.swap(index, min_child)
            }
            index = min_child;
        }
    }

    fn min_child<C>(&mut self, index: usize, mut compare: C) -> usize
                    where C: FnMut(&T, &T) -> Ordering {
        let first_child_index = self.first_child_index(index);
        let last_child_index = self.last_child_index(index);
        if last_child_index >= self.array.len() ||
                compare(&self.array[first_child_index],
                        &self.array[last_child_index]) == Ordering::Less {
            first_child_index
        } else {
            last_child_index
        }
    }

    fn parent_index(&self, index: usize)      -> usize { (index - 1) / 2 }
    fn first_child_index(&self, index: usize) -> usize { index * 2 + 1   }
    fn last_child_index(&self, index: usize)  -> usize { index * 2 + 2   }

    fn push<C>(&mut self, value: T, mut compare: C) where C: FnMut(&T, &T) -> Ordering {
        let index = self.array.len();
        self.array.push(value);
        self.sift_up(index, compare);
    }

    fn peek_min(&self) -> Option<&T> {
        self.array.get(0)
    }

    fn shift_min<C>(&mut self, mut compare: C) -> Option<T> where C: FnMut(&T, &T) -> Ordering {
        if self.array.is_empty() {
            None
        } else {
            let min = self.array.swap_remove(0);
            self.sift_down(0, compare);
            Some(min)
        }
    }

    fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    fn clear(&mut self) {
        self.array.clear()
    }
}

// Trivial utilities

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn clamp(x: f32, min: f32, max: f32) -> f32 {
    f32::max(f32::min(x, max), min)
}

fn intersect_ranges(a: Range<f32>, b: Range<f32>) -> Range<f32> {
    let (start, end) = (f32::max(a.start, b.start), f32::min(a.end, b.end));
    if start < end {
        start..end
    } else {
        start..start
    }
}

fn t_is_too_close_to_zero_or_one(t: f32) -> bool {
    const EPSILON: f32 = 0.001;

    t < EPSILON || t > 1.0 - EPSILON
}

// Testing

#[cfg(test)]
mod test {
    use crate::{Heap, IntervalRange, Intervals};
    use quickcheck::{self, Arbitrary, Gen};
    use rand::Rng;
    use std::ops::Range;

    #[test]
    fn test_heap() {
        quickcheck::quickcheck(prop_heap as fn(Vec<i32>) -> bool);

        fn prop_heap(mut values: Vec<i32>) -> bool {
            let mut heap = Heap::new();
            for &value in &values {
                heap.push(value, |a, b| a.cmp(&b))
            }

            values.sort();
            let mut results = Vec::with_capacity(values.len());
            while !heap.is_empty() {
                results.push(heap.shift_min(|a, b| a.cmp(&b)).unwrap());
            }
            assert_eq!(&values, &results);

            true
        }
    }

    #[test]
    fn test_intervals() {
        quickcheck::quickcheck(prop_intervals as fn(Spec) -> bool);

        fn prop_intervals(spec: Spec) -> bool {
            let mut intervals = Intervals::new(spec.bounds.clone());
            for range in spec.ranges {
                intervals.add(range);
            }

            assert!(intervals.ranges.len() > 0);
            assert_eq!(intervals.ranges[0].start, spec.bounds.start);
            assert_eq!(intervals.ranges.last().unwrap().end, spec.bounds.end);
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
            bounds: Range<f32>,
            ranges: Vec<IntervalRange>,
        }

        impl Arbitrary for Spec {
            fn arbitrary<G>(g: &mut G) -> Spec where G: Gen {
                const EPSILON: f32 = 0.0001;

                let size = g.size();
                let start = g.gen_range(EPSILON, size as f32);
                let end = g.gen_range(start + EPSILON, size as f32);

                let mut ranges = vec![];
                let range_count = g.gen_range(0, size);
                for _ in 0..range_count {
                    let (a, b) = (g.gen_range(start, end), g.gen_range(start, end));
                    let winding = g.gen_range(-(size as i32), size as i32) as f32;
                    ranges.push(IntervalRange::new(f32::min(a, b), f32::max(a, b), winding));
                }

                Spec {
                    bounds: start..end,
                    ranges,
                }
            }
        }
    }
}
