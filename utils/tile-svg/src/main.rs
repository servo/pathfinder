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

use euclid::{Point2D, Transform2D};
use quick_xml::Reader;
use quick_xml::events::Event;
use std::env;
use std::mem;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use svgtypes::{Color as SvgColor, PathParser, PathSegment as SvgPathSegment, TransformListParser};
use svgtypes::{TransformListToken};

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
    println!("{:#?}", scene);
}

#[derive(Debug)]
struct Scene {
    objects: Vec<PathObject>,
    styles: Vec<ComputedStyle>,
}

#[derive(Debug)]
struct PathObject {
    outline: Outline,
    style: StyleId,
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct StyleId(u32);

impl Scene {
    fn new() -> Scene {
        Scene {
            objects: vec![],
            styles: vec![],
        }
    }

    fn from_path(path: &Path) -> Scene {
        let mut reader = Reader::from_file(&path).unwrap();

        let mut xml_buffer = vec![];
        let mut group_styles = vec![];
        let mut style = None;

        let mut scene = Scene::new();

        loop {
            match reader.read_event(&mut xml_buffer) {
                Ok(Event::Start(ref event)) |
                Ok(Event::Empty(ref event)) if event.name() == b"path" => {
                    let attributes = event.attributes();
                    for attribute in attributes {
                        let attribute = attribute.unwrap();
                        if attribute.key != b"d" {
                            continue
                        }
                        let value = reader.decode(&attribute.value);
                        let style = scene.ensure_style(&mut style, &mut group_styles);
                        let path_parser = PathParser::from(&*value);
                        let outline =
                            Outline::from_svg_path_segments(path_parser, scene.get_style(style));
                        scene.objects.push(PathObject::new(outline, style));
                    }
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
}

impl PathObject {
    fn new(outline: Outline, style: StyleId) -> PathObject {
        PathObject {
            outline,
            style,
        }
    }
}

// Outlines

#[derive(Debug)]
struct Outline {
    contours: Vec<Contour>,
}

#[derive(Debug)]
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
        }
    }

    fn from_svg_path_segments<I>(segments: I, style: &ComputedStyle) -> Outline
                                 where I: Iterator<Item = SvgPathSegment> {
        let mut outline = Outline::new();
        let mut current_contour = Contour::new();
        let (mut first_point_in_path, mut last_ctrl_point, mut last_point) = (None, None, None);
        for segment in segments {
            match segment {
                SvgPathSegment::MoveTo { abs, x, y } => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()))
                    }
                    let to = compute_point(x, y, abs, &last_point);
                    first_point_in_path = Some(to);
                    last_point = Some(to);
                    last_ctrl_point = None;
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::LineTo { abs, x, y } => {
                    let to = compute_point(x, y, abs, &last_point);
                    last_point = Some(to);
                    last_ctrl_point = None;
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::HorizontalLineTo { abs, x } => {
                    let to = Point2D::new(compute_point(x, 0.0, abs, &last_point).x,
                                          last_point.unwrap_or(Point2D::zero()).y);
                    last_point = Some(to);
                    last_ctrl_point = None;
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::VerticalLineTo { abs, y } => {
                    let to = Point2D::new(last_point.unwrap_or(Point2D::zero()).x,
                                          compute_point(0.0, y, abs, &last_point).y);
                    last_point = Some(to);
                    last_ctrl_point = None;
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::Quadratic { abs, x1, y1, x, y } => {
                    let ctrl = compute_point(x1, y1, abs, &last_point);
                    last_ctrl_point = Some(ctrl);
                    let to = compute_point(x, y, abs, &last_point);
                    last_point = Some(to);
                    current_contour.push_transformed_point(&ctrl,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &style.transform);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::SmoothQuadratic { abs, x, y } => {
                    let ctrl = last_point.unwrap_or(Point2D::zero()) +
                        (last_point.unwrap_or(Point2D::zero()) -
                         last_ctrl_point.unwrap_or(Point2D::zero()));
                    last_ctrl_point = Some(ctrl);
                    let to = compute_point(x, y, abs, &last_point);
                    last_point = Some(to);
                    current_contour.push_transformed_point(&ctrl,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &style.transform);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
                    let ctrl0 = compute_point(x1, y1, abs, &last_point);
                    let ctrl1 = compute_point(x2, y2, abs, &last_point);
                    last_ctrl_point = Some(ctrl1);
                    let to = compute_point(x, y, abs, &last_point);
                    last_point = Some(to);
                    current_contour.push_transformed_point(&ctrl0,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &style.transform);
                    current_contour.push_transformed_point(&ctrl1,
                                                           PointFlags::CONTROL_POINT_1,
                                                           &style.transform);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
                    let ctrl0 = last_point.unwrap_or(Point2D::zero()) +
                        (last_point.unwrap_or(Point2D::zero()) -
                         last_ctrl_point.unwrap_or(Point2D::zero()));
                    let ctrl1 = compute_point(x2, y2, abs, &last_point);
                    last_ctrl_point = Some(ctrl1);
                    let to = compute_point(x, y, abs, &last_point);
                    last_point = Some(to);
                    current_contour.push_transformed_point(&ctrl0,
                                                           PointFlags::CONTROL_POINT_0,
                                                           &style.transform);
                    current_contour.push_transformed_point(&ctrl1,
                                                           PointFlags::CONTROL_POINT_1,
                                                           &style.transform);
                    current_contour.push_transformed_point(&to,
                                                           PointFlags::empty(),
                                                           &style.transform);
                }
                SvgPathSegment::ClosePath { abs: _ } => {
                    if !current_contour.is_empty() {
                        outline.contours.push(mem::replace(&mut current_contour, Contour::new()));
                        last_point = first_point_in_path;
                        last_ctrl_point = None;
                    }
                }
                SvgPathSegment::EllipticalArc { .. } => unimplemented!("arcs"),
            }
        }
        if !current_contour.is_empty() {
            outline.contours.push(current_contour)
        }
        return outline;

        fn compute_point(x: f64, y: f64, abs: bool, last_point: &Option<Point2D<f32>>)
                         -> Point2D<f32> {
            let point = Point2D::new(x, y).to_f32();
            match *last_point {
                Some(last_point) if !abs => last_point + point.to_vector(),
                _ => point,
            }
        }
    }
}

impl Contour {
    fn new() -> Contour {
        Contour {
            points: vec![],
            flags: vec![],
        }
    }

    fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    fn push_transformed_point(&mut self,
                              point: &Point2D<f32>,
                              flags: PointFlags,
                              transform: &Transform2D<f32>) {
        self.points.push(transform.transform_point(point));
        self.flags.push(flags);
    }
}
