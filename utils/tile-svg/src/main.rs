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
use euclid::{Point2D, Rect, Size2D, Transform2D};
use fixedbitset::FixedBitSet;
use hashbrown::HashMap;
use jemallocator;
use lyon_geom::cubic_bezier::Flattened;
use lyon_geom::math::Transform;
use lyon_geom::{CubicBezierSegment, LineSegment, QuadraticBezierSegment};
use lyon_path::PathEvent;
use lyon_path::iterator::PathIter;
use pathfinder_path_utils::stroke::{StrokeStyle, StrokeToFillIter};
use rayon::ThreadPoolBuilder;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use simdeez::Simd;
use simdeez::overloads::I32x4_41;
use simdeez::sse41::Sse41;
use std::arch::x86_64;
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::mem;
use std::ops::{Add, Mul, Sub};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::time::Instant;
use std::u16;
use svgtypes::Color as SvgColor;
use usvg::{Node, NodeExt, NodeKind, Options as UsvgOptions, Paint as UsvgPaint};
use usvg::{PathSegment as UsvgPathSegment, Rect as UsvgRect, Transform as UsvgTransform, Tree};

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

    println!("Scene bounds: {:?} View box: {:?}", scene.bounds, scene.view_box);
    println!("{} objects, {} paints", scene.objects.len(), scene.paints.len());

    let start_time = Instant::now();
    let mut built_scene = BuiltScene::new(&scene.view_box, vec![]);
    for _ in 0..runs {
        let z_buffer = ZBuffer::new(&scene.view_box);

        let built_objects = match jobs {
            Some(1) => scene.build_objects_sequentially(&z_buffer),
            _ => scene.build_objects(&z_buffer),
        };

        let built_shaders = scene.build_shaders();
        built_scene = BuiltScene::from_objects_and_shaders(&scene.view_box,
                                                           &built_objects,
                                                           built_shaders,
                                                           &z_buffer);
    }
    let elapsed_time = Instant::now() - start_time;

    let elapsed_ms = elapsed_time.as_secs() as f64 * 1000.0 +
        elapsed_time.subsec_micros() as f64 / 1000.0;
    println!("{:.3}ms elapsed", elapsed_ms / runs as f64);

    println!("{} solid tiles", built_scene.solid_tiles.len());
    for (batch_index, batch) in built_scene.batches.iter().enumerate() {
        println!("Batch {}: {} fills, {} mask tiles",
                 batch_index,
                 batch.fills.len(),
                 batch.mask_tiles.len());
    }

    if let Some(output_path) = output_path {
        built_scene.write(&mut BufWriter::new(File::create(output_path).unwrap())).unwrap();
    }
}

#[derive(Debug)]
struct Scene {
    objects: Vec<PathObject>,
    paints: Vec<Paint>,
    paint_cache: HashMap<Paint, PaintId>,
    bounds: Rect<f32>,
    view_box: Rect<f32>,
}

#[derive(Debug)]
struct PathObject {
    outline: Outline,
    paint: PaintId,
    name: String,
    kind: PathObjectKind,
}

#[derive(Clone, Copy, Debug)]
pub enum PathObjectKind {
    Fill,
    Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Paint {
    color: ColorU,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct PaintId(u16);

impl Scene {
    fn new() -> Scene {
        Scene {
            objects: vec![],
            paints: vec![],
            paint_cache: HashMap::new(),
            bounds: Rect::zero(),
            view_box: Rect::zero(),
        }
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

        // FIXME(pcwalton): This is needed to avoid stack exhaustion in debug builds when
        // recursively dropping reference counts on very large SVGs. :(
        mem::forget(tree);

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
                        let style = scene.push_paint(&Paint::from_svg_paint(&fill.paint));

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
                        let style = scene.push_paint(&Paint::from_svg_paint(&stroke.paint));
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
    }

    fn push_paint(&mut self, paint: &Paint) -> PaintId {
        if let Some(paint_id) = self.paint_cache.get(paint) {
            return *paint_id
        }

        let paint_id = PaintId(self.paints.len() as u16);
        self.paint_cache.insert(*paint, paint_id);
        self.paints.push(*paint);
        paint_id
    }

    fn build_shaders(&self) -> Vec<ObjectShader> {
        self.paints.iter().map(|paint| ObjectShader { fill_color: paint.color }).collect()
    }

    fn build_objects_sequentially(&self, z_buffer: &ZBuffer) -> Vec<BuiltObject> {
        self.objects.iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline,
                                       &self.view_box,
                                       object_index as u16,
                                       ShaderId(object.paint.0),
                                       z_buffer);
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }

    fn build_objects(&self, z_buffer: &ZBuffer) -> Vec<BuiltObject> {
        self.objects.par_iter().enumerate().map(|(object_index, object)| {
            let mut tiler = Tiler::new(&object.outline,
                                       &self.view_box,
                                       object_index as u16,
                                       ShaderId(object.paint.0),
                                       z_buffer);
            tiler.generate_tiles();
            tiler.built_object
        }).collect()
    }
}

impl PathObject {
    fn new(outline: Outline, paint: PaintId, name: String, kind: PathObjectKind) -> PathObject {
        PathObject { outline, paint, name, kind }
    }
}

// Outlines

#[derive(Debug)]
struct Outline {
    contours: Vec<Contour>,
    bounds: Rect<f32>,
}

struct Contour {
    points: Vec<Point2DF32>,
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
                PathEvent::MoveTo(ref to) => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()))
                    }
                    current_contour.push_point(&Point2DF32::from_euclid(to),
                                               PointFlags::empty(),
                                               &mut bounding_points);
                }
                PathEvent::LineTo(ref to) => {
                    current_contour.push_point(&Point2DF32::from_euclid(to),
                                               PointFlags::empty(),
                                               &mut bounding_points);
                }
                PathEvent::QuadraticTo(ref ctrl, ref to) => {
                    current_contour.push_point(&Point2DF32::from_euclid(ctrl),
                                               PointFlags::CONTROL_POINT_0,
                                               &mut bounding_points);
                    current_contour.push_point(&Point2DF32::from_euclid(to),
                                               PointFlags::empty(),
                                               &mut bounding_points);
                }
                PathEvent::CubicTo(ref ctrl0, ref ctrl1, ref to) => {
                    current_contour.push_point(&Point2DF32::from_euclid(ctrl0),
                                               PointFlags::CONTROL_POINT_0,
                                               &mut bounding_points);
                    current_contour.push_point(&Point2DF32::from_euclid(ctrl1),
                                               PointFlags::CONTROL_POINT_1,
                                               &mut bounding_points);
                    current_contour.push_point(&Point2DF32::from_euclid(to),
                                               PointFlags::empty(),
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
            outline.bounds = Rect::from_points([
                upper_left.as_euclid(),
                lower_right.as_euclid(),
            ].into_iter())
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

    fn position_of(&self, index: u32) -> Point2DF32 {
        self.points[index as usize]
    }

    // TODO(pcwalton): Pack both min and max into a single SIMD register?
    fn push_point(&mut self,
                  point: &Point2DF32,
                  flags: PointFlags,
                  bounding_points: &mut Option<(Point2DF32, Point2DF32)>) {
        self.points.push(*point);
        self.flags.push(flags);

        match *bounding_points {
            Some((ref mut upper_left, ref mut lower_right)) => {
                *upper_left = upper_left.min(point);
                *lower_right = lower_right.max(point);
            }
            None => *bounding_points = Some((*point, *point)),
        }
    }

    // TODO(pcwalton): Optimize this more with SIMD?
    fn segment_after(&self, point_index: u32) -> Segment {
        debug_assert!(self.point_is_endpoint(point_index));

        let mut flags = SegmentFlags::HAS_ENDPOINTS;
        let from = self.position_of(point_index);
        let mut ctrl0 = Point2DF32::default();
        let mut ctrl1 = Point2DF32::default();
        let mut to = Point2DF32::default();

        let point1_index = self.add_to_point_index(point_index, 1);
        if self.point_is_endpoint(point1_index) {
            to = self.position_of(point1_index);
        } else {
            ctrl0 = self.position_of(point1_index);
            flags |= SegmentFlags::HAS_CONTROL_POINT_0;

            let point2_index = self.add_to_point_index(point_index, 2);
            if self.point_is_endpoint(point2_index) {
                to = self.position_of(point2_index);
            } else {
                ctrl1 = self.position_of(point2_index);
                flags |= SegmentFlags::HAS_CONTROL_POINT_1;

                let point3_index = self.add_to_point_index(point_index, 3);
                to = self.position_of(point3_index);
            }
        }

        let mut segment = Segment::new();
        segment.baseline = LineSegmentF32::new(&from, &to);
        segment.ctrl = LineSegmentF32::new(&ctrl0, &ctrl1);
        segment.flags = flags;
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
        let (a_y, b_y) = (self.points[a as usize].y(), self.points[b as usize].y());
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
        debug_assert!(contour <= 0xfff);
        debug_assert!(point <= 0x000fffff);
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
            return Some(PathEvent::MoveTo(point0.as_euclid()))
        }
        if contour.point_is_endpoint(point0_index) {
            return Some(PathEvent::LineTo(point0.as_euclid()))
        }

        let point1_index = self.index;
        let point1 = contour.position_of(point1_index);
        self.index += 1;
        if contour.point_is_endpoint(point1_index) {
            return Some(PathEvent::QuadraticTo(point0.as_euclid(), point1.as_euclid()))
        }

        let point2_index = self.index;
        let point2 = contour.position_of(point2_index);
        self.index += 1;
        debug_assert!(contour.point_is_endpoint(point2_index));
        Some(PathEvent::CubicTo(point0.as_euclid(), point1.as_euclid(), point2.as_euclid()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    baseline: LineSegmentF32,
    ctrl: LineSegmentF32,
    flags: SegmentFlags,
}

impl Segment {
    fn new() -> Segment {
        Segment {
            baseline: LineSegmentF32::default(),
            ctrl: LineSegmentF32::default(),
            flags: SegmentFlags::empty(),
        }
    }

    fn from_line(line: &LineSegmentF32) -> Segment {
        Segment {
            baseline: *line,
            ctrl: LineSegmentF32::default(),
            flags: SegmentFlags::HAS_ENDPOINTS,
        }
    }

    fn from_quadratic(curve: &QuadraticBezierSegment<f32>) -> Segment {
        Segment {
            baseline: LineSegmentF32::new(&Point2DF32::from_euclid(&curve.from),
                                          &Point2DF32::from_euclid(&curve.to)),
            ctrl: LineSegmentF32::new(&Point2DF32::from_euclid(&curve.ctrl),
                                      &Point2DF32::default()),
            flags: SegmentFlags::HAS_ENDPOINTS | SegmentFlags::HAS_CONTROL_POINT_0
        }
    }

    fn from_cubic(curve: &CubicBezierSegment<f32>) -> Segment {
        Segment {
            baseline: LineSegmentF32::new(&Point2DF32::from_euclid(&curve.from),
                                          &Point2DF32::from_euclid(&curve.to)),
            ctrl: LineSegmentF32::new(&Point2DF32::from_euclid(&curve.ctrl1),
                                      &Point2DF32::from_euclid(&curve.ctrl2)),
            flags: SegmentFlags::HAS_ENDPOINTS | SegmentFlags::HAS_CONTROL_POINT_0 |
                SegmentFlags::HAS_CONTROL_POINT_1,
        }
    }

    fn as_line_segment(&self) -> Option<LineSegmentF32> {
        if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            Some(self.baseline)
        } else {
            None
        }
    }

    // FIXME(pcwalton): We should basically never use this function.
    fn as_lyon_cubic_segment(&self) -> Option<CubicBezierSegment<f32>> {
        if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0) {
            None
        } else if !self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_1) {
            Some((QuadraticBezierSegment {
                from: self.baseline.from().as_euclid(),
                ctrl: self.ctrl.from().as_euclid(),
                to: self.baseline.to().as_euclid(),
            }).to_cubic())
        } else {
            Some(CubicBezierSegment {
                from: self.baseline.from().as_euclid(),
                ctrl1: self.ctrl.from().as_euclid(),
                ctrl2: self.ctrl.to().as_euclid(),
                to: self.baseline.to().as_euclid(),
            })
        }
    }

    fn split_y(&self, y: f32) -> (Option<Segment>, Option<Segment>) {
        // Trivial cases.
        if self.baseline.from_y() <= y && self.baseline.to_y() <= y {
            return (Some(*self), None)
        }
        if self.baseline.from_y() >= y && self.baseline.to_y() >= y {
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
                let cubic_segment = self.as_lyon_cubic_segment().unwrap();
                //println!("split_y({}): cubic_segment={:?}", y, cubic_segment);
                let t = CubicAxis::from_y(&cubic_segment).solve_for_t(y);
                let t = t.expect("Failed to solve cubic for Y!");
                let (prev, next) = self.as_cubic_segment().split(t);
                //println!("... split at {} = {:?} / {:?}", t, prev, next);
                (prev, next)
            }
        };

        if self.baseline.from_y() < self.baseline.to_y() {
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

        if self.is_cubic_segment() {
            let segment = self.as_cubic_segment();
            let flattener = segment.flattener();
            let mut from = self.baseline.from();
            for to in flattener {
                generate_fill_primitives_for_line(LineSegmentF32::new(&from, &to),
                                                  built_object,
                                                  tile_y);
                from = to;
            }
            let to = self.baseline.to();
            generate_fill_primitives_for_line(LineSegmentF32::new(&from, &to),
                                              built_object,
                                              tile_y);
        } else {
            // TODO(pcwalton): Don't degree elevate!
            let segment = self.as_lyon_cubic_segment().unwrap();
            //println!("generate_fill_primitives(segment={:?})", segment);
            let flattener = Flattened::new(segment, FLATTENING_TOLERANCE);
            let mut from = self.baseline.from();
            for to in flattener {
                let to = Point2DF32::from_euclid(&to);
                generate_fill_primitives_for_line(LineSegmentF32::new(&from, &to),
                                                  built_object,
                                                  tile_y);
                from = to;
            }
            let to = self.baseline.to();
            generate_fill_primitives_for_line(LineSegmentF32::new(&from, &to),
                                                built_object,
                                                tile_y);
        }

        // TODO(pcwalton): Optimize this better with SIMD!
        fn generate_fill_primitives_for_line(mut segment: LineSegmentF32,
                                             built_object: &mut BuiltObject,
                                             tile_y: i16) {
            /*
            println!("segment={:?} tile_y={} ({}-{})",
                     segment,
                     tile_y,
                     tile_y as f32 * TILE_HEIGHT,
                     (tile_y + 1) as f32 * TILE_HEIGHT);
            */

            let winding = segment.from_x() > segment.to_x();
            let (segment_left, segment_right) = if !winding {
                (segment.from_x(), segment.to_x())
            } else {
                (segment.to_x(), segment.from_x())
            };

            let segment_tile_left = (f32::floor(segment_left) as i32 / TILE_WIDTH as i32) as i16;
            let segment_tile_right = alignup_i32(f32::ceil(segment_right) as i32,
                                                 TILE_WIDTH as i32) as i16;

            for subsegment_tile_x in segment_tile_left..segment_tile_right {
                let (mut fill_from, mut fill_to) = (segment.from(), segment.to());
                let subsegment_tile_right = ((subsegment_tile_x as i32 + 1) * TILE_HEIGHT as i32)
                    as f32;
                if subsegment_tile_right < segment_right {
                    let x = subsegment_tile_right;
                    let point = Point2DF32::new(x, segment.solve_y_for_x(x));
                    if !winding {
                        fill_to = point;
                        segment = LineSegmentF32::new(&point, &segment.to());
                    } else {
                        fill_from = point;
                        segment = LineSegmentF32::new(&segment.from(), &point);
                    }
                }

                let fill_segment = LineSegmentF32::new(&fill_from, &fill_to);
                built_object.add_fill(&fill_segment, subsegment_tile_x, tile_y);
            }
        }
    }

    fn is_none(&self) -> bool {
        !self.flags.contains(SegmentFlags::HAS_ENDPOINTS)
    }

    fn is_cubic_segment(&self) -> bool {
        self.flags.contains(SegmentFlags::HAS_CONTROL_POINT_0 | SegmentFlags::HAS_CONTROL_POINT_1)
    }

    fn as_cubic_segment(&self) -> CubicSegment {
        debug_assert!(self.is_cubic_segment());
        CubicSegment(self)
    }
}

bitflags! {
    struct SegmentFlags: u8 {
        const HAS_ENDPOINTS       = 0x01;
        const HAS_CONTROL_POINT_0 = 0x02;
        const HAS_CONTROL_POINT_1 = 0x04;
    }
}

#[derive(Clone, Copy, Debug)]
struct CubicSegment<'s>(&'s Segment);

impl<'s> CubicSegment<'s> {
    fn flattener(self) -> CubicCurveFlattener {
        CubicCurveFlattener { curve: *self.0 }
    }

    fn sample(self, t: f32) -> Point2DF32 {
        let (from, to) = (self.0.baseline.from(), self.0.baseline.to());
        let (ctrl0, ctrl1) = (self.0.ctrl.from(), self.0.ctrl.to());

        let b3 = to + (ctrl0 - ctrl1).scale(3.0) - from;
        let b2 = (from - ctrl0 - ctrl0 + ctrl1).scale(3.0) + b3.scale(t);
        let b1 = (ctrl0 - from).scale(3.0) + b2.scale(t);
        let b0 = from + b1.scale(t);
        b0
    }

    fn split(self, t: f32) -> (Segment, Segment) {
        unsafe {
            let tttt = Sse41::set1_ps(t);

            let p0p3 = self.0.baseline.0;
            let p1p2 = self.0.ctrl.0;
            let p0p1 = assemble(&p0p3, &p1p2, 0, 0);

            // p01 = lerp(p0, p1, t), p12 = lerp(p1, p2, t), p23 = lerp(p2, p3, t)
            let p01p12 = Sse41::add_ps(p0p1, Sse41::mul_ps(tttt, Sse41::sub_ps(p1p2, p0p1)));
            let pxxp23 = Sse41::add_ps(p1p2, Sse41::mul_ps(tttt, Sse41::sub_ps(p0p3, p1p2)));

            let p12p23 = assemble(&p01p12, &pxxp23, 1, 1);

            // p012 = lerp(p01, p12, t), p123 = lerp(p12, p23, t)
            let p012p123 = Sse41::add_ps(p01p12, Sse41::mul_ps(tttt,
                                                               Sse41::sub_ps(p12p23, p01p12)));

            let p123 = pluck(&p012p123, 1);

            // p0123 = lerp(p012, p123, t)
            let p0123 = Sse41::add_ps(p012p123,
                                      Sse41::mul_ps(tttt, Sse41::sub_ps(p123, p012p123)));

            let baseline0 = assemble(&p0p3, &p0123, 0, 0);
            let ctrl0 = assemble(&p01p12, &p012p123, 0, 0);
            let baseline1 = assemble(&p0123, &p0p3, 0, 1);
            let ctrl1 = assemble(&p012p123, &p12p23, 1, 1);

            let flags = SegmentFlags::HAS_ENDPOINTS | SegmentFlags::HAS_CONTROL_POINT_0 |
                SegmentFlags::HAS_CONTROL_POINT_1;

            return (Segment {
                baseline: LineSegmentF32(baseline0),
                ctrl: LineSegmentF32(ctrl0),
                flags,
            }, Segment {
                baseline: LineSegmentF32(baseline1),
                ctrl: LineSegmentF32(ctrl1),
                flags,
            })
        }

        // Constructs a new 4-element vector from two pairs of adjacent lanes in two input vectors.
        unsafe fn assemble(a_data: &<Sse41 as Simd>::Vf32,
                           b_data: &<Sse41 as Simd>::Vf32,
                           a_index: usize,
                           b_index: usize)
                           -> <Sse41 as Simd>::Vf32 {
            let (a_data, b_data) = (Sse41::castps_pd(*a_data), Sse41::castps_pd(*b_data));
            let mut result = Sse41::setzero_pd();
            result[0] = a_data[a_index];
            result[1] = b_data[b_index];
            Sse41::castpd_ps(result)
        }

        // Constructs a new 2-element vector from a pair of adjacent lanes in an input vector.
        unsafe fn pluck(data: &<Sse41 as Simd>::Vf32, index: usize) -> <Sse41 as Simd>::Vf32 {
            let data = Sse41::castps_pd(*data);
            let mut result = Sse41::setzero_pd();
            result[0] = data[index];
            Sse41::castpd_ps(result)
        }
    }

    fn split_after(self, t: f32) -> Segment {
        self.split(t).1
    }
}

// Tiling

const TILE_WIDTH: u32 = 16;
const TILE_HEIGHT: u32 = 16;

struct Tiler<'o, 'z> {
    outline: &'o Outline,
    built_object: BuiltObject,
    object_index: u16,
    z_buffer: &'z ZBuffer,

    view_box: Rect<f32>,
    bounds: Rect<f32>,

    point_queue: SortedVector<QueuedEndpoint>,
    active_edges: SortedVector<ActiveEdge>,
    old_active_edges: Vec<ActiveEdge>,
}

impl<'o, 'z> Tiler<'o, 'z> {
    fn new(outline: &'o Outline,
           view_box: &Rect<f32>,
           object_index: u16,
           shader: ShaderId,
           z_buffer: &'z ZBuffer)
           -> Tiler<'o, 'z> {
        let bounds = outline.bounds.intersection(&view_box).unwrap_or(Rect::zero());
        let built_object = BuiltObject::new(&bounds, shader);

        Tiler {
            outline,
            built_object,
            object_index,
            z_buffer,

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

        // Cull.
        self.cull();
        //println!("{:#?}", self.built_object);
    }

    fn generate_strip(&mut self, strip_origin_y: i16) {
        // Process old active edges.
        self.process_old_active_edges(strip_origin_y);

        // Add new active edges.
        let strip_max_y = ((strip_origin_y as i32 + 1) * TILE_HEIGHT as i32) as f32;
        while let Some(queued_endpoint) = self.point_queue.peek() {
            if queued_endpoint.y >= strip_max_y {
                break
            }
            self.add_new_active_edge(strip_origin_y);
        }
    }

    fn cull(&self) {
        for solid_tile_index in self.built_object.solid_tiles.ones() {
            let tile = &self.built_object.tiles[solid_tile_index];
            if tile.backdrop != 0 {
                self.z_buffer.update(tile.tile_x, tile.tile_y, self.object_index);
            }
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
                if active_edge.segment.baseline.from_y() < active_edge.segment.baseline.to_y() {
                    (active_edge.segment.baseline.from_x(), 1)
                } else {
                    (active_edge.segment.baseline.to_x(), -1)
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
            let segment_tile_x = (f32::floor(segment_x) as i32 / TILE_WIDTH as i32) as i16;
            if current_tile_x < segment_tile_x && current_subtile_x > 0.0 {
                let current_x = (current_tile_x as i32 * TILE_WIDTH as i32) as f32 +
                    current_subtile_x;
                let tile_right_x = ((current_tile_x + 1) as i32 * TILE_WIDTH as i32) as f32;
                self.built_object.add_active_fill(current_x,
                                                  tile_right_x,
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
            let segment_subtile_x = segment_x - (current_tile_x as i32 * TILE_WIDTH as i32) as f32;
            if segment_subtile_x > current_subtile_x {
                let current_x = (current_tile_x as i32 * TILE_WIDTH as i32) as f32 +
                    current_subtile_x;
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

        //debug_assert_eq!(current_winding, 0);
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
                y: contour.position_of(prev_endpoint_index).y(),
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
                y: contour.position_of(next_endpoint_index).y(),
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
                        y: contour.position_of(cur_endpoint_index).y(),
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
    process_active_edge(&mut segment, built_object, tile_y);

    if !segment.is_none() {
        active_edges.push(ActiveEdge::new(segment));
    }
}

fn process_active_edge(active_edge: &mut Segment, built_object: &mut BuiltObject, tile_y: i16) {
    // Chop the segment.
    // TODO(pcwalton): Maybe these shouldn't be Options?
    let (upper_segment, lower_segment) =
        active_edge.split_y(((tile_y as i32 + 1) * TILE_HEIGHT as i32) as f32);

    // Add fill primitives for upper part.
    if let Some(segment) = upper_segment {
        segment.generate_fill_primitives(built_object, tile_y);
    }

    // Queue lower part.
    *active_edge = lower_segment.unwrap_or(Segment::new());
}

// Scene construction

impl BuiltScene {
    fn new(view_box: &Rect<f32>, shaders: Vec<ObjectShader>) -> BuiltScene {
        BuiltScene {
            view_box: *view_box,
            batches: vec![],
            solid_tiles: vec![],
            shaders,

            tile_rect: round_rect_out_to_tile_bounds(view_box),
        }
    }

    #[inline(never)]
    fn from_objects_and_shaders(view_box: &Rect<f32>,
                                objects: &[BuiltObject],
                                shaders: Vec<ObjectShader>,
                                z_buffer: &ZBuffer)
                                -> BuiltScene {
        let mut scene = BuiltScene::new(view_box, shaders);
        scene.add_batch();

        // Initialize z-buffer, and fill solid tiles.
        z_buffer.push_solid_tiles(&mut scene, objects);

        // Build batches.
        let mut object_tile_index_to_scene_mask_tile_index = vec![];
        for (object_index, object) in objects.iter().enumerate() {
            object_tile_index_to_scene_mask_tile_index.clear();
            object_tile_index_to_scene_mask_tile_index.reserve(object.tiles.len());

            // Copy mask tiles.
            for (tile_index, tile) in object.tiles.iter().enumerate() {
                // Skip solid tiles, since we handled them above already.
                if object.solid_tiles[tile_index] {
                    object_tile_index_to_scene_mask_tile_index.push(BLANK);
                    continue;
                }

                // Cull occluded tiles.
                let scene_tile_index = scene_tile_index(tile.tile_x,
                                                        tile.tile_y,
                                                        &scene.tile_rect);
                if !z_buffer.test(scene_tile_index, object_index as u16) {
                    object_tile_index_to_scene_mask_tile_index.push(BLANK);
                    continue;
                }

                // Visible mask tile.
                let mut scene_mask_tile_index = scene.batches.last().unwrap().mask_tiles.len() as
                    u16;
                if scene_mask_tile_index == u16::MAX {
                    scene.add_batch();
                    scene_mask_tile_index = 0;
                }

                object_tile_index_to_scene_mask_tile_index.push(SceneMaskTileIndex {
                    batch_index: scene.batches.len() as u16 - 1,
                    mask_tile_index: scene_mask_tile_index,
                });

                scene.batches.last_mut().unwrap().mask_tiles.push(MaskTileBatchPrimitive {
                    tile: *tile,
                    shader: object.shader,
                });
            }

            // Remap and copy fills, culling as necessary.
            for fill in &object.fills {
                let object_tile_index = object.tile_coords_to_index(fill.tile_x, fill.tile_y);
                let SceneMaskTileIndex {
                    batch_index,
                    mask_tile_index,
                } = object_tile_index_to_scene_mask_tile_index[object_tile_index as usize];
                if batch_index < u16::MAX {
                    scene.batches[batch_index as usize].fills.push(FillBatchPrimitive {
                        px: fill.px,
                        subpx: fill.subpx,
                        mask_tile_index,
                    });
                }
            }
        }

        return scene;

        #[derive(Clone, Copy, Debug)]
        struct SceneMaskTileIndex {
            batch_index: u16,
            mask_tile_index: u16,
        }

        const BLANK: SceneMaskTileIndex = SceneMaskTileIndex {
            batch_index: 0,
            mask_tile_index: 0,
        };
    }

    fn add_batch(&mut self) {
        self.batches.push(Batch::new());
    }
}

fn scene_tile_index(tile_x: i16, tile_y: i16, tile_rect: &Rect<i16>) -> u32 {
    (tile_y - tile_rect.origin.y) as u32 * tile_rect.size.width as u32 +
        (tile_x - tile_rect.origin.x) as u32
}

// Culling

struct ZBuffer {
    buffer: Vec<AtomicUsize>,
    tile_rect: Rect<i16>,
}

impl ZBuffer {
    fn new(view_box: &Rect<f32>) -> ZBuffer {
        let tile_rect = round_rect_out_to_tile_bounds(view_box);
        let tile_area = tile_rect.size.width as usize * tile_rect.size.height as usize;
        ZBuffer {
            buffer: (0..tile_area).map(|_| AtomicUsize::new(0)).collect(),
            tile_rect,
        }
    }

    fn test(&self, scene_tile_index: u32, object_index: u16) -> bool {
        let existing_depth = self.buffer[scene_tile_index as usize].load(AtomicOrdering::SeqCst);
        existing_depth < object_index as usize + 1
    }

    fn update(&self, tile_x: i16, tile_y: i16, object_index: u16) {
        let scene_tile_index = scene_tile_index(tile_x, tile_y, &self.tile_rect) as usize;
        let mut old_depth = self.buffer[scene_tile_index].load(AtomicOrdering::SeqCst);
        let new_depth = (object_index + 1) as usize;
        while old_depth < new_depth {
            let prev_depth = self.buffer[scene_tile_index]
                                 .compare_and_swap(old_depth,
                                                   new_depth,
                                                   AtomicOrdering::SeqCst);
            if prev_depth == old_depth {
                // Successfully written.
                return
            }
            old_depth = prev_depth;
        }
    }

    fn push_solid_tiles(&self, scene: &mut BuiltScene, objects: &[BuiltObject]) {
        let tile_rect = scene.tile_rect;
        for scene_tile_y in 0..tile_rect.size.height {
            for scene_tile_x in 0..tile_rect.size.width {
                let scene_tile_index = scene_tile_y as usize * tile_rect.size.width as usize +
                    scene_tile_x as usize;
                let depth = self.buffer[scene_tile_index].load(AtomicOrdering::Relaxed);
                if depth == 0 {
                    continue
                }
                let object_index = (depth - 1) as usize;
                scene.solid_tiles.push(SolidTileScenePrimitive {
                    tile_x: scene_tile_x + tile_rect.origin.x,
                    tile_y: scene_tile_y + tile_rect.origin.y,
                    shader: objects[object_index].shader,
                });
            }
        }
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
    shader: ShaderId,
}

#[derive(Debug)]
struct BuiltScene {
    view_box: Rect<f32>,
    batches: Vec<Batch>,
    solid_tiles: Vec<SolidTileScenePrimitive>,
    shaders: Vec<ObjectShader>,

    tile_rect: Rect<i16>,
}

#[derive(Debug)]
struct Batch {
    fills: Vec<FillBatchPrimitive>,
    mask_tiles: Vec<MaskTileBatchPrimitive>,
}

#[derive(Clone, Copy, Debug)]
struct FillObjectPrimitive {
    px: LineSegmentU4,
    subpx: LineSegmentU8,
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
struct FillBatchPrimitive {
    px: LineSegmentU4,
    subpx: LineSegmentU8,
    mask_tile_index: u16,
}

#[derive(Clone, Copy, Debug)]
struct SolidTileScenePrimitive {
    tile_x: i16,
    tile_y: i16,
    shader: ShaderId,
}

#[derive(Clone, Copy, Debug)]
struct MaskTileBatchPrimitive {
    tile: TileObjectPrimitive,
    shader: ShaderId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ShaderId(pub u16);

#[derive(Clone, Copy, Debug, Default)]
struct ObjectShader {
    fill_color: ColorU,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
struct ColorU {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

// Utilities for built objects

impl BuiltObject {
    fn new(bounds: &Rect<f32>, shader: ShaderId) -> BuiltObject {
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
            shader,
        }
    }

    // TODO(pcwalton): SIMD-ify `tile_x` and `tile_y`.
    fn add_fill(&mut self, segment: &LineSegmentF32, tile_x: i16, tile_y: i16) {
        let tile_origin = Point2DF32::new((tile_x as i32 * TILE_WIDTH as i32) as f32,
                                          (tile_y as i32 * TILE_HEIGHT as i32) as f32);
        let tile_index = self.tile_coords_to_index(tile_x, tile_y);
        let mut segment = *segment - tile_origin;

        let (tile_min, tile_max) = (Point2DF32::default(), Point2DF32::splat(16.0 - 1.0 / 256.0));
        segment = segment.clamp(&tile_min, &tile_max);

        let px = segment.to_line_segment_u4();
        let subpx = segment.fract().scale(256.0).to_line_segment_u8();

        /*
        // TODO(pcwalton): Cull degenerate fills again.
        // Cull degenerate fills.
        let (from_px, to_px) = (from.to_u8(), to.to_u8());
        if from_px.x == to_px.x && from_subpx.x == to_subpx.x {
            return
        }
        */

        self.fills.push(FillObjectPrimitive { px, subpx, tile_x, tile_y });

        self.solid_tiles.set(tile_index as usize, false);
    }

    fn add_active_fill(&mut self,
                       left: f32,
                       right: f32,
                       mut winding: i16,
                       tile_x: i16,
                       tile_y: i16) {
        let tile_origin_y = (tile_y as i32 * TILE_HEIGHT as i32) as f32;
        let left = Point2DF32::new(left, tile_origin_y);
        let right = Point2DF32::new(right, tile_origin_y);

        let segment = if winding < 0 {
            LineSegmentF32::new(&left, &right)
        } else {
            LineSegmentF32::new(&right, &left)
        };

        /*
        println!("... emitting fill {} -> {} winding {} @ tile {}",
                 left.x,
                 right.x,
                 winding,
                 tile_x);
        */

        while winding != 0 {
            self.add_fill(&segment, tile_x, tile_y);
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

impl Paint {
    fn from_svg_paint(svg_paint: &UsvgPaint) -> Paint {
        Paint {
            color: match *svg_paint {
                UsvgPaint::Color(color) => ColorU::from_svg_color(color),
                UsvgPaint::Link(_) => {
                    // TODO(pcwalton)
                    ColorU::black()
                }
            },
        }
    }
}

// Scene serialization

impl BuiltScene {
    fn write<W>(&self, writer: &mut W) -> io::Result<()> where W: Write {
        writer.write_all(b"RIFF")?;

        let header_size = 4 * 6;

        let solid_tiles_size = self.solid_tiles.len() * mem::size_of::<SolidTileScenePrimitive>();

        let batch_sizes: Vec<_> = self.batches.iter().map(|batch| {
            BatchSizes {
                fills: (batch.fills.len() * mem::size_of::<FillBatchPrimitive>()),
                mask_tiles: (batch.mask_tiles.len() * mem::size_of::<MaskTileBatchPrimitive>()),
            }
        }).collect();

        let total_batch_sizes: usize = batch_sizes.iter().map(|sizes| 8 + sizes.total()).sum();

        let shaders_size = self.shaders.len() * mem::size_of::<ObjectShader>();

        writer.write_u32::<LittleEndian>((4 +
                                          8 + header_size +
                                          8 + solid_tiles_size +
                                          8 + shaders_size +
                                          total_batch_sizes) as u32)?;

        writer.write_all(b"PF3S")?;

        writer.write_all(b"head")?;
        writer.write_u32::<LittleEndian>(header_size as u32)?;
        writer.write_u32::<LittleEndian>(FILE_VERSION)?;
        writer.write_u32::<LittleEndian>(self.batches.len() as u32)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.x)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.y)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.width)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.height)?;

        writer.write_all(b"shad")?;
        writer.write_u32::<LittleEndian>(shaders_size as u32)?;
        for &shader in &self.shaders {
            let fill_color = shader.fill_color;
            writer.write_all(&[fill_color.r, fill_color.g, fill_color.b, fill_color.a])?;
        }

        writer.write_all(b"soli")?;
        writer.write_u32::<LittleEndian>(solid_tiles_size as u32)?;
        for &tile_primitive in &self.solid_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile_y)?;
            writer.write_u16::<LittleEndian>(tile_primitive.shader.0)?;
        }

        for (batch, sizes) in self.batches.iter().zip(batch_sizes.iter()) {
            writer.write_all(b"batc")?;
            writer.write_u32::<LittleEndian>(sizes.total() as u32)?;

            writer.write_all(b"fill")?;
            writer.write_u32::<LittleEndian>(sizes.fills as u32)?;
            for fill_primitive in &batch.fills {
                writer.write_u16::<LittleEndian>(fill_primitive.px.0)?;
                writer.write_u32::<LittleEndian>(fill_primitive.subpx.0)?;
                writer.write_u16::<LittleEndian>(fill_primitive.mask_tile_index)?;
            }

            writer.write_all(b"mask")?;
            writer.write_u32::<LittleEndian>(sizes.mask_tiles as u32)?;
            for &tile_primitive in &batch.mask_tiles {
                writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_x)?;
                writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_y)?;
                writer.write_i16::<LittleEndian>(tile_primitive.tile.backdrop)?;
                writer.write_u16::<LittleEndian>(tile_primitive.shader.0)?;
            }
        }

        return Ok(());

        fn write_point2d_u8<W>(writer: &mut W, point: Point2D<u8>)
                               -> io::Result<()> where W: Write {
            writer.write_u8(point.x)?;
            writer.write_u8(point.y)?;
            Ok(())
        }

        const FILE_VERSION: u32 = 0;

        struct BatchSizes {
            fills: usize,
            mask_tiles: usize,
        }

        impl BatchSizes {
            fn total(&self) -> usize {
                8 + self.fills + 8 + self.mask_tiles
            }
        }
    }
}

impl Batch {
    fn new() -> Batch {
        Batch { fills: vec![], mask_tiles: vec![] }
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
    let tile_origin = Point2D::new((f32::floor(rect.origin.x) as i32 / TILE_WIDTH as i32) as i16,
                                   (f32::floor(rect.origin.y) as i32 / TILE_HEIGHT as i32) as i16);
    let tile_extent =
        Point2D::new(alignup_i32(f32::ceil(rect.max_x()) as i32, TILE_WIDTH as i32) as i16,
                     alignup_i32(f32::ceil(rect.max_y()) as i32, TILE_HEIGHT as i32) as i16);
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
// FIXME(pcwalton): SIMDify!
struct LineAxis { from: f32, to: f32 }
impl LineAxis {
    fn from_x(segment: &LineSegmentF32) -> LineAxis {
        LineAxis { from: segment.from_x(), to: segment.to_x() }
    }
    fn from_y(segment: &LineSegmentF32) -> LineAxis {
        LineAxis { from: segment.from_y(), to: segment.to_y() }
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
    // FIXME(pcwalton): SIMDify?
    fn partial_cmp(&self, other: &ActiveEdge) -> Option<Ordering> {
        let this_x = if self.segment.baseline.from_y() < self.segment.baseline.to_y() {
            self.segment.baseline.from_x()
        } else {
            self.segment.baseline.to_x()
        };
        let other_x = if other.segment.baseline.from_y() < other.segment.baseline.to_y() {
            other.segment.baseline.from_x()
        } else {
            other.segment.baseline.to_x()
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

#[derive(Clone, Copy, Debug)]
struct Point2DF32(<Sse41 as Simd>::Vf32);

impl Point2DF32 {
    fn new(x: f32, y: f32) -> Point2DF32 {
        unsafe {
            let mut data = Sse41::setzero_ps();
            data[0] = x;
            data[1] = y;
            return Point2DF32(data);
        }
    }

    fn splat(value: f32) -> Point2DF32 { unsafe { Point2DF32(Sse41::set1_ps(value)) } }

    fn from_euclid(point: &Point2D<f32>) -> Point2DF32 { Point2DF32::new(point.x, point.y) }
    fn as_euclid(&self) -> Point2D<f32> { Point2D::new(self.0[0], self.0[1]) }

    fn x(&self) -> f32 { self.0[0] }
    fn y(&self) -> f32 { self.0[1] }

    fn scale(&self, factor: f32) -> Point2DF32 {
        unsafe { Point2DF32(Sse41::mul_ps(self.0, Sse41::set1_ps(factor))) }
    }

    fn min(&self, other: &Point2DF32) -> Point2DF32 {
        unsafe { Point2DF32(Sse41::min_ps(self.0, other.0)) }
    }

    fn max(&self, other: &Point2DF32) -> Point2DF32 {
        unsafe { Point2DF32(Sse41::max_ps(self.0, other.0)) }
    }

    fn clamp(&self, min: &Point2DF32, max: &Point2DF32) -> Point2DF32 {
        self.max(min).min(max)
    }

    fn lerp(&self, other: &Point2DF32, t: f32) -> Point2DF32 {
        *self + (*other - *self).scale(t)
    }

    // TODO(pcwalton): Optimize this a bit.
    fn det(&self, other: &Point2DF32) -> f32 {
        self.0[0] * other.0[1] - self.0[1] * other.0[0]
    }

    fn floor(&self) -> Point2DF32 { unsafe { Point2DF32(Sse41::fastfloor_ps(self.0)) } }

    fn fract(&self) -> Point2DF32 { *self - self.floor() }

    // TODO(pcwalton): Have an actual packed u8 point type!
    fn to_u8(&self) -> Point2D<u8> {
        unsafe {
            let int_values = Sse41::cvtps_epi32(self.0);
            Point2D::new(int_values[0] as u8, int_values[1] as u8)
        }
    }
}

impl PartialEq for Point2DF32 {
    fn eq(&self, other: &Point2DF32) -> bool {
        unsafe {
            let results: <Sse41 as Simd>::Vi32 = mem::transmute(Sse41::cmpeq_ps(self.0, other.0));
            results[0] == -1 && results[1] == -1
        }
    }
}

impl Default for Point2DF32 {
    fn default() -> Point2DF32 { unsafe { Point2DF32(Sse41::setzero_ps()) } }
}

impl Add<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    fn add(self, other: Point2DF32) -> Point2DF32 { Point2DF32(self.0 + other.0) }
}

impl Sub<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    fn sub(self, other: Point2DF32) -> Point2DF32 { Point2DF32(self.0 - other.0) }
}

impl Mul<Point2DF32> for Point2DF32 {
    type Output = Point2DF32;
    fn mul(self, other: Point2DF32) -> Point2DF32 { Point2DF32(self.0 * other.0) }
}

#[derive(Clone, Copy, Debug)]
struct LineSegmentF32(pub <Sse41 as Simd>::Vf32);

impl LineSegmentF32 {
    fn new(from: &Point2DF32, to: &Point2DF32) -> LineSegmentF32 {
        unsafe {
            LineSegmentF32(Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(from.0),
                                                               Sse41::castps_pd(to.0))))
        }
    }

    fn from(&self) -> Point2DF32 {
        unsafe {
            Point2DF32(Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(self.0),
                                                           Sse41::setzero_pd())))
        }
    }
    fn to(&self) -> Point2DF32 {
        unsafe {
            Point2DF32(Sse41::castpd_ps(Sse41::unpackhi_pd(Sse41::castps_pd(self.0),
                                                           Sse41::setzero_pd())))
        }
    }

    fn from_x(&self) -> f32 { self.0[0] }
    fn from_y(&self) -> f32 { self.0[1] }
    fn to_x(&self)   -> f32 { self.0[2] }
    fn to_y(&self)   -> f32 { self.0[3] }

    fn clamp(&self, min: &Point2DF32, max: &Point2DF32) -> LineSegmentF32 {
        unsafe {
            let min_min = Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(min.0),
                                                            Sse41::castps_pd(min.0)));
            let max_max = Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(max.0),
                                                            Sse41::castps_pd(max.0)));
            LineSegmentF32(Sse41::min_ps(max_max, Sse41::max_ps(min_min, self.0)))
        }
    }

    fn scale(&self, factor: f32) -> LineSegmentF32 {
        unsafe {
            LineSegmentF32(Sse41::mul_ps(self.0, Sse41::set1_ps(factor)))
        }
    }

    fn floor(&self) -> LineSegmentF32 { unsafe { LineSegmentF32(Sse41::fastfloor_ps(self.0)) } }

    fn fract(&self) -> LineSegmentF32 {
        unsafe {
            LineSegmentF32(Sse41::sub_ps(self.0, self.floor().0))
        }
    }

    fn split(&self, t: f32) -> (LineSegmentF32, LineSegmentF32) {
        unsafe {
            let from_from = Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(self.0),
                                                                Sse41::castps_pd(self.0)));
            let to_to = Sse41::castpd_ps(Sse41::unpackhi_pd(Sse41::castps_pd(self.0),
                                                            Sse41::castps_pd(self.0)));
            let d_d = to_to - from_from;
            let mid_mid = from_from + d_d * Sse41::set1_ps(t);
            (LineSegmentF32(Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(from_from),
                                                                Sse41::castps_pd(mid_mid)))),
             LineSegmentF32(Sse41::castpd_ps(Sse41::unpackhi_pd(Sse41::castps_pd(mid_mid),
                                                                Sse41::castps_pd(to_to)))))
        }
    }

    fn to_line_segment_u4(&self) -> LineSegmentU4 {
        unsafe {
            let values = Sse41::cvtps_epi32(Sse41::fastfloor_ps(self.0));
            let mask = Sse41::set1_epi32(0x0c040800);
            let values_0213 = Sse41::shuffle_epi8(values, mask)[0] as u32;
            LineSegmentU4((values_0213 | (values_0213 >> 12)) as u16)
        }
    }

    fn to_line_segment_u8(&self) -> LineSegmentU8 {
        unsafe {
            let values = Sse41::cvtps_epi32(Sse41::fastfloor_ps(self.0));
            let mask = Sse41::set1_epi32(0x0c080400);
            LineSegmentU8(Sse41::shuffle_epi8(values, mask)[0] as u32)
        }
    }

    // FIXME(pcwalton): Eliminate all uses of this!
    fn as_lyon_line_segment(&self) -> LineSegment<f32> {
        LineSegment { from: self.from().as_euclid(), to: self.to().as_euclid() }
    }

    // FIXME(pcwalton): Optimize this!
    fn solve_y_for_x(&self, x: f32) -> f32 {
        self.as_lyon_line_segment().solve_y_for_x(x)
    }
}

impl PartialEq for LineSegmentF32 {
    fn eq(&self, other: &LineSegmentF32) -> bool {
        unsafe {
            let results = Sse41::castps_epi32(Sse41::cmpeq_ps(self.0, other.0));
            // FIXME(pcwalton): Is there a better way to do this?
            results[0] == -1 && results[1] == -1 && results[2] == -1 && results[3] == -1
        }
    }
}

impl Default for LineSegmentF32 {
    fn default() -> LineSegmentF32 { unsafe { LineSegmentF32(Sse41::setzero_ps()) } }
}

impl Sub<Point2DF32> for LineSegmentF32 {
    type Output = LineSegmentF32;
    fn sub(self, point: Point2DF32) -> LineSegmentF32 {
        unsafe {
            let point_point = Sse41::castpd_ps(Sse41::unpacklo_pd(Sse41::castps_pd(point.0),
                                                                  Sse41::castps_pd(point.0)));
            LineSegmentF32(self.0 - point_point)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LineSegmentU4(u16);

#[derive(Clone, Copy, Debug)]
struct LineSegmentU8(u32);

// Curve flattening

struct CubicCurveFlattener {
    curve: Segment,
}

impl Iterator for CubicCurveFlattener {
    type Item = Point2DF32;

    fn next(&mut self) -> Option<Point2DF32> {
        let s2inv;
        unsafe {
            let (baseline, ctrl) = (self.curve.baseline.0, self.curve.ctrl.0);
            let from_from = Sse41::shuffle_ps(baseline, baseline, 0b01000100);

            let v0102 = Sse41::sub_ps(ctrl, from_from);

            //      v01.x   v01.y   v02.x v02.y
            //    * v01.x   v01.y   v01.y v01.x
            //    -------------------------
            //      v01.x^2 v01.y^2 ad    bc
            //         |       |     |     |
            //         +-------+     +-----+
            //             +            -
            //         v01 len^2   determinant
            let products = Sse41::mul_ps(v0102, Sse41::shuffle_ps(v0102, v0102, 0b00010100));

            let det = products[2] - products[3];
            if det == 0.0 {
                return None;
            }

            s2inv = (products[0] + products[1]).sqrt() / det;
        }

        let t = 2.0 * ((FLATTENING_TOLERANCE / 3.0) * s2inv.abs()).sqrt();
        if t >= 1.0 - EPSILON || t == 0.0 {
            return None;
        }

        self.curve = self.curve.as_cubic_segment().split_after(t);
        return Some(self.curve.baseline.from());

        const EPSILON: f32 = 0.005;
    }
}

// Path utilities

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

// SIMD extensions

trait SimdExt: Simd {
    // TODO(pcwalton): Default scalar implementation.
    unsafe fn shuffle_epi8(a: Self::Vi32, b: Self::Vi32) -> Self::Vi32;
}

impl SimdExt for Sse41 {
    #[inline(always)]
    unsafe fn shuffle_epi8(a: Self::Vi32, b: Self::Vi32) -> Self::Vi32 {
        I32x4_41(x86_64::_mm_shuffle_epi8(a.0, b.0))
    }
}

// Trivial utilities

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn clamp(x: f32, min: f32, max: f32) -> f32 {
    f32::max(f32::min(x, max), min)
}

fn alignup_i32(a: i32, b: i32) -> i32 {
    (a + b - 1) / b
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
