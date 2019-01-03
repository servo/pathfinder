// pathfinder/utils/tile-svg/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
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
use lyon_geom::math::Transform;
use lyon_geom::{CubicBezierSegment, LineSegment, QuadraticBezierSegment};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIter;
use pathfinder_path_utils::stroke::{StrokeStyle, StrokeToFillIter};
use rayon::ThreadPoolBuilder;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::mem;
use std::path::PathBuf;
use std::time::Instant;
use std::u16;
use svgtypes::Color as SvgColor;
use usvg::{Node, NodeExt, NodeKind, Options as UsvgOptions, Paint, PathSegment as UsvgPathSegment};
use usvg::{Rect as UsvgRect, Transform as UsvgTransform, Tree};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

// TODO(pcwalton): Make this configurable.
const SCALE_FACTOR: f32 = 1.0;

// TODO(pcwalton): Make this configurable.
const FLATTENING_TOLERANCE: f32 = 0.333;

const HAIRLINE_STROKE_WIDTH: f32 = 0.5;

fn main() {
    let matches =
        App::new("tile-svg").arg(Arg::with_name("runs").short("r")
                                                       .long("runs")
                                                       .value_name("COUNT")
                                                       .takes_value(true)
                                                       .help("Run a benchmark with COUNT runs"))
                            .arg(Arg::with_name("jobs").short("j")
                                                       .long("jobs")
                                                       .value_name("THREADS")
                                                       .takes_value(true)
                                                       .help("Number of threads to use"))
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
    let jobs: Option<usize> = matches.value_of("jobs").map(|string| string.parse().unwrap());
    let input_path = PathBuf::from(matches.value_of("INPUT").unwrap());
    let output_path = matches.value_of("OUTPUT").map(PathBuf::from);

    // Set up Rayon.
    let mut thread_pool_builder = ThreadPoolBuilder::new();
    if let Some(jobs) = jobs {
        thread_pool_builder = thread_pool_builder.num_threads(jobs);
    }
    thread_pool_builder.build_global().unwrap();

    // Build scene.
    let usvg = Tree::from_file(&input_path, &UsvgOptions::default()).unwrap();
    let scene = Scene::from_tree(usvg);

    println!("Scene bounds: {:?} View box: {:?} Object count: {}",
             scene.bounds,
             scene.view_box,
             scene.objects.len());
    //println!("{:#?}", scene.objects[0]);

    let start_time = Instant::now();
    let mut built_scene = BuiltScene::new(&scene.view_box, scene.objects.len() as u32);
    for _ in 0..runs {
        let built_objects = match jobs {
            Some(1) => scene.build_objects_sequentially(),
            _ => scene.build_objects(),
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
    name: String,
    kind: PathObjectKind,
}

#[derive(Clone, Copy, Debug)]
pub enum PathObjectKind {
    Fill,
    Stroke,
}

#[derive(Debug)]
struct ComputedStyle {
    color: Option<SvgColor>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct StyleId(u32);

impl Scene {
    fn new() -> Scene {
        Scene { objects: vec![], styles: vec![], bounds: Rect::zero(), view_box: Rect::zero() }
    }

    fn from_tree(tree: Tree) -> Scene {
        let global_transform = Transform2D::create_scale(SCALE_FACTOR, SCALE_FACTOR);

        let mut scene = Scene::new();

        let root = &tree.root();
        match *root.borrow() {
            NodeKind::Svg(ref svg) => {
                scene.view_box = usvg_rect_to_euclid_rect(&svg.view_box.rect);
                for kid in root.children() {
                    process_node(&mut scene, &kid, &global_transform);
                }
            }
            _ => unreachable!(),
        };

        return scene;

        fn process_node(scene: &mut Scene, node: &Node, transform: &Transform2D<f32>) {
            let node_transform = usvg_transform_to_euclid_transform_2d(&node.transform());
            let transform = transform.pre_mul(&node_transform);

            match *node.borrow() {
                NodeKind::Group(_) => {
                    for kid in node.children() {
                        process_node(scene, &kid, &transform)
                    }
                }
                NodeKind::Path(ref path) => {
                    if let Some(ref fill) = path.fill {
                        let style = scene.push_paint(&fill.paint);

                        let path = UsvgPathToPathEvents::new(path.segments.iter().cloned());
                        let path = PathTransformingIter::new(path, &transform);
                        let path = MonotonicConversionIter::new(path);
                        let outline = Outline::from_path_events(path);

                        scene.bounds = scene.bounds.union(&outline.bounds);
                        scene.objects.push(PathObject::new(outline,
                                                           style,
                                                           node.id().to_string(),
                                                           PathObjectKind::Fill));
                    }

                    if let Some(ref stroke) = path.stroke {
                        let style = scene.push_paint(&stroke.paint);
                        let stroke_width = f32::max(stroke.width.value() as f32,
                                                    HAIRLINE_STROKE_WIDTH);

                        let path = UsvgPathToPathEvents::new(path.segments.iter().cloned());
                        let path = PathIter::new(path);
                        let path = StrokeToFillIter::new(path, StrokeStyle::new(stroke_width));
                        let path = PathTransformingIter::new(path, &transform);
                        let path = MonotonicConversionIter::new(path);
                        let outline = Outline::from_path_events(path);

                        scene.bounds = scene.bounds.union(&outline.bounds);
                        scene.objects.push(PathObject::new(outline,
                                                           style,
                                                           node.id().to_string(),
                                                           PathObjectKind::Stroke));
                    }
                }
                _ => {
                    // TODO(pcwalton): Handle these by punting to WebRender.
                }
            }
        }

        /*
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

        */
    }

    fn push_paint(&mut self, paint: &Paint) -> StyleId {
        let id = StyleId(self.styles.len() as u32);
        self.styles.push(ComputedStyle {
            color: match *paint {
                Paint::Color(color) => Some(color),
                Paint::Link(..) => None,
            }
        });
        id
    }

    fn get_style(&self, style: StyleId) -> &ComputedStyle {
        &self.styles[style.0 as usize]
    }

    fn build_shader(&self, object_index: u16) -> ObjectShader {
        let object = &self.objects[object_index as usize];
        let style = self.get_style(object.style);
        let color = style.color.map(ColorU::from_svg_color).unwrap_or(ColorU::black());
        ObjectShader { fill_color: color }
    }

    // This function exists to make profiling easier.
    fn build_objects_sequentially(&self) -> Vec<BuiltObject> {
        self.objects.iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline,
                                       object_index as u16,
                                       &self.view_box,
                                       &self.build_shader(object_index as u16));
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }

    fn build_objects(&self) -> Vec<BuiltObject> {
        self.objects.par_iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline,
                                       object_index as u16,
                                       &self.view_box,
                                       &self.build_shader(object_index as u16));
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }

    /*
    fn push_svg_path(&mut self, value: &str, style: StyleId, name: String) {
        let global_transform = Transform2D::create_scale(SCALE_FACTOR, SCALE_FACTOR);
        let transform = global_transform.pre_mul(&self.get_style(style).transform);

        if self.get_style(style).fill_color.is_some() {
            let computed_style = self.get_style(style);
            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let path = PathTransformingIter::new(path, &transform);
            let path = MonotonicConversionIter::new(path);
            let outline = Outline::from_path_events(path);

            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, style, name.clone(), PathObjectKind::Fill));
        }

        if self.get_style(style).stroke_color.is_some() {
            let computed_style = self.get_style(style);
            let stroke_width = f32::max(computed_style.stroke_width, HAIRLINE_STROKE_WIDTH);

            let mut path_parser = PathParser::from(&*value);
            let path = SvgPathToPathEvents::new(&mut path_parser);
            let path = PathIter::new(path);
            let path = StrokeToFillIter::new(path, StrokeStyle::new(stroke_width));
            let path = PathTransformingIter::new(path, &transform);
            let path = MonotonicConversionIter::new(path);
            let outline = Outline::from_path_events(path);

            self.bounds = self.bounds.union(&outline.bounds);
            self.objects.push(PathObject::new(outline, style, name, PathObjectKind::Stroke));
        }
    }
    */
}

impl PathObject {
    fn new(outline: Outline, style: StyleId, name: String, kind: PathObjectKind) -> PathObject {
        PathObject { outline, style, name, kind }
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

    // NB: Assumes the path has already been transformed.
    fn from_path_events<I>(path_events: I) -> Outline where I: Iterator<Item = PathEvent> {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();
        let mut bounding_points = None;

        for path_event in path_events {
            match path_event {
                PathEvent::MoveTo(to) => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()))
                    }
                    current_contour.push_point(&to, PointFlags::empty(), &mut bounding_points);
                }
                PathEvent::LineTo(to) => {
                    current_contour.push_point(&to, PointFlags::empty(), &mut bounding_points);
                }
                PathEvent::QuadraticTo(ctrl, to) => {
                    current_contour.push_point(&ctrl,
                                               PointFlags::CONTROL_POINT_0,
                                               &mut bounding_points);
                    current_contour.push_point(&to, PointFlags::empty(), &mut bounding_points);
                }
                PathEvent::CubicTo(ctrl0, ctrl1, to) => {
                    current_contour.push_point(&ctrl0,
                                               PointFlags::CONTROL_POINT_0,
                                               &mut bounding_points);
                    current_contour.push_point(&ctrl1,
                                               PointFlags::CONTROL_POINT_1,
                                               &mut bounding_points);
                    current_contour.push_point(&to, PointFlags::empty(), &mut bounding_points);
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

    fn push_point(&mut self,
                  point: &Point2D<f32>,
                  flags: PointFlags,
                  bounding_points: &mut Option<(Point2D<f32>, Point2D<f32>)>) {
        self.points.push(*point);
        self.flags.push(flags);

        match *bounding_points {
            Some((ref mut upper_left, ref mut lower_right)) => {
                *upper_left = upper_left.min(*point);
                *lower_right = lower_right.max(*point);
            }
            None => *bounding_points = Some((*point, *point)),
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
                formatter.write_str(" ")?;
            }
            if formatter.alternate() {
                formatter.write_str("\n    ")?;
            }
            write_path_event(formatter, &segment)?;
        }
        if formatter.alternate() {
            formatter.write_str("\n")?
        }
        formatter.write_str("]")?;

        return Ok(());

        fn write_path_event(formatter: &mut Formatter, path_event: &PathEvent) -> fmt::Result {
            match *path_event {
                PathEvent::Arc(..) => {
                    // TODO(pcwalton)
                    formatter.write_str("TODO: arcs")?;
                }
                PathEvent::Close => formatter.write_str("z")?,
                PathEvent::MoveTo(ref to) => {
                    formatter.write_str("M")?;
                    write_point(formatter, to)?;
                }
                PathEvent::LineTo(ref to) => {
                    formatter.write_str("L")?;
                    write_point(formatter, to)?;
                }
                PathEvent::QuadraticTo(ref ctrl, ref to) => {
                    formatter.write_str("Q")?;
                    write_point(formatter, ctrl)?;
                    write_point(formatter, to)?;
                }
                PathEvent::CubicTo(ref ctrl0, ref ctrl1, ref to) => {
                    formatter.write_str("C")?;
                    write_point(formatter, ctrl0)?;
                    write_point(formatter, ctrl1)?;
                    write_point(formatter, to)?;
                }
            }
            return Ok(());
        }

        fn write_point(formatter: &mut Formatter, point: &Point2D<f32>) -> fmt::Result {
            write!(formatter, " {},{}", point.x, point.y)
        }
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
                //println!("split_y({}): cubic_segment={:?}", y, cubic_segment);
                let t = CubicAxis::from_y(&cubic_segment).solve_for_t(y);
                let t = t.expect("Failed to solve cubic for Y!");
                let (prev, next) = cubic_segment.split(t);
                //println!("... split at {} = {:?} / {:?}", t, prev, next);
                (Segment::from_cubic(&prev), Segment::from_cubic(&next))
            }
        };

        if self.from.y < self.to.y {
            (Some(prev), Some(next))
        } else {
            (Some(next), Some(prev))
        }
    }

    fn generate_fill_primitives(&self, built_object: &mut BuiltObject, tile_y: i16) {
        if let Some(line_segment) = self.as_line_segment() {
            generate_fill_primitives_for_line(line_segment, built_object, tile_y);
            return;
        }

        // TODO(pcwalton): Don't degree elevate!
        let segment = self.as_cubic_segment().unwrap();
        //println!("generate_fill_primitives(segment={:?})", segment);
        let flattener = Flattened::new(segment, FLATTENING_TOLERANCE);
        let mut from = self.from;
        for to in flattener {
            generate_fill_primitives_for_line(LineSegment { from, to }, built_object, tile_y);
            from = to;
        }

        fn generate_fill_primitives_for_line(mut segment: LineSegment<f32>,
                                             built_object: &mut BuiltObject,
                                             tile_y: i16) {
            /*
            println!("segment={:?} tile_y={} ({}-{})",
                     segment,
                     tile_y,
                     tile_y as f32 * TILE_HEIGHT,
                     (tile_y + 1) as f32 * TILE_HEIGHT);
            */

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
    object_index: u16,
    built_object: BuiltObject,

    view_box: Rect<f32>,
    bounds: Rect<f32>,

    point_queue: SortedVector<QueuedEndpoint>,
    active_edges: SortedVector<ActiveEdge>,
    old_active_edges: Vec<ActiveEdge>,
}

impl<'o> Tiler<'o> {
    fn new(outline: &'o Outline, object_index: u16, view_box: &Rect<f32>, shader: &ObjectShader)
           -> Tiler<'o> {
        let bounds = outline.bounds.intersection(&view_box).unwrap_or(Rect::zero());
        let built_object = BuiltObject::new(&bounds, shader);

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

        //println!("{:#?}", self.built_object);
    }

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

    fn process_old_active_edges(&mut self, tile_y: i16) {
        let mut current_tile_x = self.built_object.tile_rect.origin.x;
        let mut current_subtile_x = 0.0;
        let mut current_winding = 0;

        debug_assert!(self.old_active_edges.is_empty());
        mem::swap(&mut self.old_active_edges, &mut self.active_edges.array);

        let mut last_segment_x = -9999.0;

        for mut active_edge in self.old_active_edges.drain(..) {
            // Determine x-intercept and winding.
            let (segment_x, edge_winding) =
                if active_edge.segment.from.y < active_edge.segment.to.y {
                    (active_edge.segment.from.x, 1)
                } else {
                    (active_edge.segment.to.x, -1)
                };

            /*
            println!("tile Y {}: segment_x={} edge_winding={} current_tile_x={} current_subtile_x={} current_winding={}",
                     tile_y,
                     segment_x,
                     edge_winding,
                     current_tile_x,
                     current_subtile_x,
                     current_winding);
            println!("... segment={:#?}", active_edge.segment);
            */

            // FIXME(pcwalton): Remove this debug code!
            debug_assert!(segment_x >= last_segment_x);
            last_segment_x = segment_x;

            // Do initial subtile fill, if necessary.
            let segment_tile_x = f32::floor(segment_x / TILE_WIDTH) as i16;
            if current_tile_x < segment_tile_x && current_subtile_x > 0.0 {
                let current_x = (current_tile_x as f32) * TILE_WIDTH + current_subtile_x;
                self.built_object.add_active_fill(current_x,
                                                  (current_tile_x + 1) as f32 * TILE_WIDTH,
                                                  current_winding,
                                                  current_tile_x,
                                                  tile_y);
                current_tile_x += 1;
                current_subtile_x = 0.0;
            }

            // Move over to the correct tile, filling in as we go.
            while current_tile_x < segment_tile_x {
                //println!("... emitting backdrop {} @ tile {}", current_winding, current_tile_x);
                self.built_object.get_tile_mut(current_tile_x, tile_y).backdrop = current_winding;
                current_tile_x += 1;
                current_subtile_x = 0.0;
            }

            // Do final subtile fill, if necessary.
            debug_assert!(current_tile_x == segment_tile_x);
            debug_assert!(current_tile_x < self.built_object.tile_rect.max_x());
            let segment_subtile_x = segment_x - (current_tile_x as f32) * TILE_WIDTH;
            if segment_subtile_x > current_subtile_x {
                let current_x = (current_tile_x as f32) * TILE_WIDTH + current_subtile_x;
                self.built_object.add_active_fill(current_x,
                                                  segment_x,
                                                  current_winding,
                                                  current_tile_x,
                                                  tile_y);
                current_subtile_x = segment_subtile_x;
            }

            // Update winding.
            current_winding += edge_winding;

            // Process the edge.
            process_active_edge(&mut active_edge.segment, &mut self.built_object, tile_y);
            if !active_edge.segment.is_none() {
                self.active_edges.push(active_edge);
            }
        }

        debug_assert_eq!(current_winding, 0);
    }

    fn add_new_active_edge(&mut self, tile_y: i16) {
        let outline = &self.outline;
        let point_index = self.point_queue.pop().unwrap().point_index;

        let contour = &outline.contours[point_index.contour() as usize];

        // TODO(pcwalton): Could use a bitset of processed edges…
        let prev_endpoint_index = contour.prev_endpoint_index_of(point_index.point());
        let next_endpoint_index = contour.next_endpoint_index_of(point_index.point());
        /*
        println!("adding new active edge, tile_y={} point_index={} prev={} next={} pos={:?} prevpos={:?} nextpos={:?}",
                 tile_y,
                 point_index.point(),
                 prev_endpoint_index,
                 next_endpoint_index,
                 contour.position_of(point_index.point()),
                 contour.position_of(prev_endpoint_index),
                 contour.position_of(next_endpoint_index));
        */
        if contour.point_is_logically_above(point_index.point(), prev_endpoint_index) {
            //println!("... adding prev endpoint");
            process_active_segment(contour,
                                   prev_endpoint_index,
                                   &mut self.active_edges,
                                   &mut self.built_object,
                                   tile_y);

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), prev_endpoint_index),
                y: contour.position_of(prev_endpoint_index).y,
            });
            //println!("... done adding prev endpoint");
        }

        if contour.point_is_logically_above(point_index.point(), next_endpoint_index) {
            /*
            println!("... adding next endpoint {} -> {}",
                     point_index.point(),
                     next_endpoint_index);
            */
            process_active_segment(contour,
                                   point_index.point(),
                                   &mut self.active_edges,
                                   &mut self.built_object,
                                   tile_y);

            self.point_queue.push(QueuedEndpoint {
                point_index: PointIndex::new(point_index.contour(), next_endpoint_index),
                y: contour.position_of(next_endpoint_index).y,
            });
            //println!("... done adding next endpoint");
        }
    }

    fn init_point_queue(&mut self) {
        // Find MIN points.
        self.point_queue.clear();
        for (contour_index, contour) in self.outline.contours.iter().enumerate() {
            let contour_index = contour_index as u32;
            let mut cur_endpoint_index = 0;
            let mut prev_endpoint_index = contour.prev_endpoint_index_of(cur_endpoint_index);
            let mut next_endpoint_index = contour.next_endpoint_index_of(cur_endpoint_index);
            loop {
                if contour.point_is_logically_above(cur_endpoint_index, prev_endpoint_index) &&
                        contour.point_is_logically_above(cur_endpoint_index, next_endpoint_index) {
                    self.point_queue.push(QueuedEndpoint {
                        point_index: PointIndex::new(contour_index, cur_endpoint_index),
                        y: contour.position_of(cur_endpoint_index).y,
                    });
                }

                if cur_endpoint_index >= next_endpoint_index {
                    break
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
    /*if segment.is_degenerate() {
        return
    }*/

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
            fills: vec![],
            solid_tiles: vec![],
            mask_tiles: vec![],
            shaders: vec![ObjectShader::default(); object_count as usize],

            tile_rect: round_rect_out_to_tile_bounds(view_box),
        }
    }

    fn from_objects(view_box: &Rect<f32>, objects: &[BuiltObject]) -> BuiltScene {
        let mut scene = BuiltScene::new(view_box, objects.len() as u32);

        let tile_area = scene.tile_rect.size.width as usize * scene.tile_rect.size.height as usize;
        let mut z_buffer = vec![0; tile_area];

        // Initialize z-buffer, and fill solid tiles.
        for (object_index, object) in objects.iter().enumerate().rev() {
            for solid_tile_index in object.solid_tiles.ones() {
                let tile = &object.tiles[solid_tile_index];
                if tile.backdrop == 0 {
                    // Tile is transparent and can't be solid.
                    continue
                }

                let scene_tile_index = scene.scene_tile_index(tile.tile_x, tile.tile_y);
                if z_buffer[scene_tile_index as usize] > object_index as u16 {
                    // Occluded.
                    continue
                }
                z_buffer[scene_tile_index as usize] = object_index as u16;

                scene.solid_tiles.push(SolidTileScenePrimitive {
                    tile_x: tile.tile_x,
                    tile_y: tile.tile_y,
                    object_index: object_index as u16,
                });
            }
        }

        let mut object_tile_index_to_scene_mask_tile_index = vec![];
        for (object_index, object) in objects.iter().enumerate() {
            object_tile_index_to_scene_mask_tile_index.clear();
            object_tile_index_to_scene_mask_tile_index.reserve(object.tiles.len());

            // Copy mask tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                // Skip solid tiles, since we handled them above already.
                if object.solid_tiles[tile_index] {
                    object_tile_index_to_scene_mask_tile_index.push(u16::MAX);
                    continue;
                }

                // Cull occluded tiles.
                let scene_tile_index = scene.scene_tile_index(tile.tile_x, tile.tile_y);
                if z_buffer[scene_tile_index as usize] as usize > object_index {
                    object_tile_index_to_scene_mask_tile_index.push(u16::MAX);
                    continue;
                }

                // Visible mask tile.
                let scene_mask_tile_index = scene.mask_tiles.len() as u16;
                object_tile_index_to_scene_mask_tile_index.push(scene_mask_tile_index);
                scene.mask_tiles.push(MaskTileScenePrimitive {
                    tile: *tile,
                    object_index: object_index as u16,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index = object.tile_coords_to_index(fill.tile_x, fill.tile_y);
                match object_tile_index_to_scene_mask_tile_index[object_tile_index as usize] {
                    u16::MAX => {}
                    scene_mask_tile_index => {
                        scene.fills.push(FillScenePrimitive {
                            from_px: fill.from_px,
                            to_px: fill.to_px,
                            from_subpx: fill.from_subpx,
                            to_subpx: fill.to_subpx,
                            mask_tile_index: scene_mask_tile_index,
                        })
                    }
                }
            }

            // Copy shader.
            scene.shaders[object_index as usize] = object.shader;
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
    solid_tiles: FixedBitSet,
    shader: ObjectShader,
}

#[derive(Debug)]
struct BuiltScene {
    view_box: Rect<f32>,
    fills: Vec<FillScenePrimitive>,
    solid_tiles: Vec<SolidTileScenePrimitive>,
    mask_tiles: Vec<MaskTileScenePrimitive>,
    shaders: Vec<ObjectShader>,

    tile_rect: Rect<i16>,
}

#[derive(Clone, Copy, Debug)]
struct FillObjectPrimitive {
    from_px: Point2DU4,
    to_px: Point2DU4,
    from_subpx: Point2D<u8>,
    to_subpx: Point2D<u8>,
    tile_x: i16,
    tile_y: i16,
}

#[derive(Clone, Copy, Debug)]
struct TileObjectPrimitive {
    tile_x: i16,
    tile_y: i16,
    backdrop: i16,
}

#[derive(Clone, Copy, Debug)]
struct FillScenePrimitive {
    from_px: Point2DU4,
    to_px: Point2DU4,
    from_subpx: Point2D<u8>,
    to_subpx: Point2D<u8>,
    mask_tile_index: u16,
}

#[derive(Clone, Copy, Debug)]
struct SolidTileScenePrimitive {
    tile_x: i16,
    tile_y: i16,
    object_index: u16,
}

#[derive(Clone, Copy, Debug)]
struct MaskTileScenePrimitive {
    tile: TileObjectPrimitive,
    object_index: u16,
}

#[derive(Clone, Copy, Debug, Default)]
struct ObjectShader {
    fill_color: ColorU,
}

#[derive(Clone, Copy, Debug, Default)]
struct ColorU {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

// Utilities for built objects

impl BuiltObject {
    fn new(bounds: &Rect<f32>, shader: &ObjectShader) -> BuiltObject {
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

        let mut solid_tiles = FixedBitSet::with_capacity(tile_count);
        solid_tiles.insert_range(..);

        BuiltObject {
            bounds: *bounds,
            tile_rect,
            tiles,
            fills: vec![],
            solid_tiles,
            shader: *shader,
        }
    }

    fn add_fill(&mut self, from: &Point2D<f32>, to: &Point2D<f32>, tile_x: i16, tile_y: i16) {
        let tile_origin = Vector2D::new(tile_x as f32 * TILE_WIDTH, tile_y as f32 * TILE_HEIGHT);
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        let (from, to) = (*from - tile_origin, *to - tile_origin);

        let from = Point2D::new(clamp(from.x, 0.0, MAX_U12), clamp(from.y, 0.0, MAX_U12));
        let to   = Point2D::new(clamp(to.x,   0.0, MAX_U12), clamp(to.y,   0.0, MAX_U12));

        const MAX_U12: f32 = 16.0 - 1.0 / 256.0;

        let from_px = Point2DU4::new(from.x as u8, from.y as u8);
        let to_px = Point2DU4::new(to.x as u8, to.y as u8);
        let from_subpx = Point2D::new((from.x.fract() * 256.0) as u8,
                                      (from.y.fract() * 256.0) as u8);
        let to_subpx = Point2D::new((to.x.fract() * 256.0) as u8, (to.y.fract() * 256.0) as u8);

        self.fills.push(FillObjectPrimitive {
            from_px, to_px,
            from_subpx, to_subpx,
            tile_x, tile_y,
        });

        self.solid_tiles.set(tile_index as usize, false);
    }

    fn add_active_fill(&mut self,
                       left: f32,
                       right: f32,
                       mut winding: i16,
                       tile_x: i16,
                       tile_y: i16) {
        let tile_origin_y = tile_y as f32 * TILE_HEIGHT;
        let mut left = Point2D::new(left, tile_origin_y);
        let mut right = Point2D::new(right, tile_origin_y);

        if winding > 0 {
            mem::swap(&mut left, &mut right);
        }

        /*
        println!("... emitting fill {} -> {} winding {} @ tile {}",
                 left.x,
                 right.x,
                 winding,
                 tile_x);
        */

        while winding != 0 {
            self.add_fill(&left, &right, tile_x, tile_y);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
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
        let shaders_size = self.shaders.len() * mem::size_of::<ObjectShader>();
        writer.write_u32::<LittleEndian>((4 +
                                          8 + header_size +
                                          8 + fill_size +
                                          8 + solid_tiles_size +
                                          8 + mask_tiles_size +
                                          8 + shaders_size) as u32)?;

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
            writer.write_u8(fill_primitive.from_px.0)?;
            writer.write_u8(fill_primitive.to_px.0)?;
            write_point2d_u8(writer, fill_primitive.from_subpx)?;
            write_point2d_u8(writer, fill_primitive.to_subpx)?;
            writer.write_u16::<LittleEndian>(fill_primitive.mask_tile_index)?;
        }

        writer.write_all(b"soli")?;
        writer.write_u32::<LittleEndian>(solid_tiles_size as u32)?;
        for &tile_primitive in &self.solid_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile_y)?;
            writer.write_u16::<LittleEndian>(tile_primitive.object_index)?;
        }

        writer.write_all(b"mask")?;
        writer.write_u32::<LittleEndian>(mask_tiles_size as u32)?;
        for &tile_primitive in &self.mask_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_y)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile.backdrop)?;
            writer.write_u16::<LittleEndian>(tile_primitive.object_index)?;
        }

        writer.write_all(b"shad")?;
        writer.write_u32::<LittleEndian>(shaders_size as u32)?;
        for &shader in &self.shaders {
            let fill_color = shader.fill_color;
            writer.write_all(&[fill_color.r, fill_color.g, fill_color.b, fill_color.a])?;
        }

        return Ok(());

        fn write_point2d_u8<W>(writer: &mut W, point: Point2D<u8>)
                               -> io::Result<()> where W: Write {
            writer.write_u8(point.x)?;
            writer.write_u8(point.y)?;
            Ok(())
        }
    }
}

impl SolidTileScenePrimitive {
    fn new(tile_x: i16, tile_y: i16, object_index: u16) -> SolidTileScenePrimitive {
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

// USVG stuff

fn usvg_rect_to_euclid_rect(rect: &UsvgRect) -> Rect<f32> {
    Rect::new(Point2D::new(rect.x, rect.y), Size2D::new(rect.width, rect.height)).to_f32()
}

fn usvg_transform_to_euclid_transform_2d(transform: &UsvgTransform) -> Transform2D<f32> {
    Transform2D::row_major(transform.a as f32, transform.b as f32,
                           transform.c as f32, transform.d as f32,
                           transform.e as f32, transform.f as f32)
}

struct UsvgPathToPathEvents<I> where I: Iterator<Item = UsvgPathSegment> {
    iter: I,
}

impl<I> UsvgPathToPathEvents<I> where I: Iterator<Item = UsvgPathSegment> {
    fn new(iter: I) -> UsvgPathToPathEvents<I> {
        UsvgPathToPathEvents { iter }
    }
}

impl<I> Iterator for UsvgPathToPathEvents<I> where I: Iterator<Item = UsvgPathSegment> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        match self.iter.next()? {
            UsvgPathSegment::MoveTo { x, y } => {
                Some(PathEvent::MoveTo(Point2D::new(x, y).to_f32()))
            }
            UsvgPathSegment::LineTo { x, y } => {
                Some(PathEvent::LineTo(Point2D::new(x, y).to_f32()))
            }
            UsvgPathSegment::CurveTo { x1, y1, x2, y2, x, y } => {
                Some(PathEvent::CubicTo(Point2D::new(x1, y1).to_f32(),
                                        Point2D::new(x2, y2).to_f32(),
                                        Point2D::new(x,  y) .to_f32()))
            }
            UsvgPathSegment::ClosePath => Some(PathEvent::Close),
        }
    }
}

// Path transformation utilities

struct PathTransformingIter<I> where I: Iterator<Item = PathEvent> {
    inner: I,
    transform: Transform2D<f32>,
}

impl<I> Iterator for PathTransformingIter<I> where I: Iterator<Item = PathEvent> {
    type Item = PathEvent;

    fn next(&mut self) -> Option<PathEvent> {
        self.inner.next().map(|event| event.transform(&self.transform))
    }
}

impl<I> PathTransformingIter<I> where I: Iterator<Item = PathEvent> {
    fn new(inner: I, transform: &Transform2D<f32>) -> PathTransformingIter<I> {
        PathTransformingIter {
            inner,
            transform: *transform,
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
                //println!("considering segment {:?}...", segment);
                if segment.is_monotonic() {
                    //println!("... is monotonic");
                    self.last_point = to;
                    return Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
                }
                if cubic_segment_is_tiny(&segment) {
                    self.last_point = to;
                    return Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
                }
                // FIXME(pcwalton): O(n^2)!
                let mut t = 1.0;
                segment.for_each_monotonic_t(|split_t| {
                    //println!("... split t={}", split_t);
                    t = f32::min(t, split_t);
                });
                if t_is_too_close_to_zero_or_one(t) {
                    //println!("... segment t={} is too close to bounds, pushing", t);
                    self.last_point = to;
                    return Some(PathEvent::CubicTo(ctrl0, ctrl1, to))
                }
                //println!("... making segment monotonic @ t={}", t);
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
                if quadratic_segment_is_tiny(&segment) {
                    self.last_point = to;
                    return Some(PathEvent::QuadraticTo(ctrl, to))
                }
                // FIXME(pcwalton): O(n^2)!
                let mut t = 1.0;
                segment.for_each_monotonic_t(|split_t| {
                    //println!("... split t={}", split_t);
                    t = f32::min(t, split_t);
                });
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

// Geometry

#[derive(Clone, Copy, Debug)]
struct Point2DU4(pub u8);

impl Point2DU4 {
    fn new(x: u8, y: u8) -> Point2DU4 { Point2DU4(x | (y << 4)) }
}

// Path utilities

/*
fn cubic_segment_is_nearly_monotonic(segment: &CubicBezierSegment<f32>) -> bool {
    let min_x = f32::min(segment.from.x, segment.to.x) - EPSILON;
    let max_x = f32::max(segment.from.x, segment.to.x) + EPSILON;
    let min_y = f32::min(segment.from.y, segment.to.y) - EPSILON;
    let max_y = f32::max(segment.from.y, segment.to.y) + EPSILON;

    return min_x <= segment.ctrl1.x && segment.ctrl1.x <= max_x &&
        min_x <= segment.ctrl2.x && segment.ctrl2.x <= max_x &&
        min_y <= segment.ctrl1.y && segment.ctrl1.y <= max_y &&
        min_y <= segment.ctrl2.y && segment.ctrl2.y <= max_y;

    const EPSILON: f32 = 0.1;
}

fn cubic_segment_is_nearly_monotonic(segment: &CubicBezierSegment<f32>) -> bool {
    let mut t = None;
    segment.for_each_monotonic_t(|split_t| {
        if t.is_none() {
            t = Some(split_t)
        }
    });
    return match t {
        None => true,
        Some(t) => t < EPSILON || t > 1.0 - EPSILON,
    };

    const EPSILON: f32 = 0.01;
}
*/

const TINY_EPSILON: f32 = 0.1;

fn cubic_segment_is_tiny(segment: &CubicBezierSegment<f32>) -> bool {
    let (x0, x1) = segment.fast_bounding_range_x();
    let (y0, y1) = segment.fast_bounding_range_y();
    let (x_delta, y_delta) = (f32::abs(x0 - x1), f32::abs(y0 - y1));
    return x_delta < TINY_EPSILON || y_delta < TINY_EPSILON;
}

fn quadratic_segment_is_tiny(segment: &QuadraticBezierSegment<f32>) -> bool {
    let (x0, x1) = segment.fast_bounding_range_x();
    let (y0, y1) = segment.fast_bounding_range_y();
    let (x_delta, y_delta) = (f32::abs(x0 - x1), f32::abs(y0 - y1));
    return x_delta < TINY_EPSILON || y_delta < TINY_EPSILON;

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
