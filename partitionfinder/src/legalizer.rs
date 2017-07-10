// partitionfinder/legalizer.rs

use euclid::Point2D;
use geometry::{self, ApproxOrdered, SubdividedCubicBezier};
use std::u32;
use {ControlPoints, Endpoint, Subpath};

const MAX_SUBDIVISIONS: u8 = 16;

pub struct Legalizer {
    endpoints: Vec<Endpoint>,
    control_points: Vec<ControlPoints>,
    subpaths: Vec<Subpath>,
}

impl Legalizer {
    #[inline]
    pub fn new() -> Legalizer {
        Legalizer {
            endpoints: vec![],
            control_points: vec![],
            subpaths: vec![],
        }
    }

    #[inline]
    pub fn endpoints(&self) -> &[Endpoint] {
        &self.endpoints
    }

    #[inline]
    pub fn control_points(&self) -> &[ControlPoints] {
        &self.control_points
    }

    #[inline]
    pub fn subpaths(&self) -> &[Subpath] {
        &self.subpaths
    }

    pub fn move_to(&mut self, position: &Point2D<f32>) {
        self.subpaths.push(Subpath {
            first_endpoint_index: self.endpoints.len() as u32,
            last_endpoint_index: self.endpoints.len() as u32 + 1,
        });
        self.endpoints.push(Endpoint {
            position: *position,
            control_points_index: u32::MAX,
            subpath_index: (self.subpaths.len() - 1) as u32,
        })
    }

    #[inline]
    pub fn close_path(&mut self) {
        // All paths are implicitly closed.
    }

    pub fn line_to(&mut self, endpoint: &Point2D<f32>) {
        self.subpaths
            .last_mut()
            .expect("`line_to()` called with no current subpath")
            .last_endpoint_index += 1;
        self.endpoints.push(Endpoint {
            position: *endpoint,
            control_points_index: u32::MAX,
            subpath_index: (self.subpaths.len() - 1) as u32,
        })
    }

    #[inline]
    pub fn quadratic_curve_to(&mut self, control_point: &Point2D<f32>, endpoint: &Point2D<f32>) {
        self.bezier_curve_to(control_point, control_point, endpoint)
    }

    pub fn bezier_curve_to(&mut self,
                           point1: &Point2D<f32>,
                           point2: &Point2D<f32>,
                           endpoint: &Point2D<f32>) {
        self.bezier_curve_to_subdividing_if_necessary(point1, point2, endpoint, 0)
    }

    fn bezier_curve_to_subdividing_if_necessary(&mut self,
                                                point1: &Point2D<f32>,
                                                point2: &Point2D<f32>,
                                                endpoint: &Point2D<f32>,
                                                iteration: u8) {
        let last_endpoint_index = self.subpaths
                                      .last()
                                      .expect("`bezier_curve_to()` called with no current_subpath")
                                      .last_endpoint_index;
        let point0 = self.endpoints[last_endpoint_index as usize].position;
        if iteration >= MAX_SUBDIVISIONS ||
                [point0.x, point1.x, point2.x, endpoint.x].approx_ordered() {
            return self.monotone_bezier_curve_to(point1, point2, endpoint)
        }

        let t = geometry::newton_raphson(|t| {
                                            geometry::sample_cubic_bezier_deriv(t,
                                                                                &point0,
                                                                                point1,
                                                                                point2,
                                                                                endpoint).x
                                         },
                                         |t| {
                                            geometry::sample_cubic_bezier_deriv_deriv(t,
                                                                                      &point0,
                                                                                      point1,
                                                                                      point2,
                                                                                      endpoint).x
                                         },
                                         0.5);

        let subdivision = SubdividedCubicBezier::new(t, &point0, point1, point2, endpoint);

        self.bezier_curve_to_subdividing_if_necessary(&subdivision.ap1,
                                                      &subdivision.ap2,
                                                      &subdivision.ap3bp0,
                                                      iteration + 1);
        self.bezier_curve_to_subdividing_if_necessary(&subdivision.bp1,
                                                      &subdivision.bp2,
                                                      &subdivision.bp3,
                                                      iteration + 1);
    }

    fn monotone_bezier_curve_to(&mut self,
                                point1: &Point2D<f32>,
                                point2: &Point2D<f32>,
                                endpoint: &Point2D<f32>) {
        self.subpaths
            .last_mut()
            .expect("`bezier_curve_to()` called with no current subpath")
            .last_endpoint_index += 1;
        self.control_points.push(ControlPoints {
            point1: *point1,
            point2: *point2,
        });
        self.endpoints.push(Endpoint {
            position: *endpoint,
            control_points_index: (self.control_points.len() - 1) as u32,
            subpath_index: (self.subpaths.len() - 1) as u32,
        })
    }
}