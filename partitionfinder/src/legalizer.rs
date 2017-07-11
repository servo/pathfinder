// partitionfinder/legalizer.rs

use euclid::Point2D;
use std::u32;
use {Endpoint, Subpath};

pub struct Legalizer {
    endpoints: Vec<Endpoint>,
    control_points: Vec<Point2D<f32>>,
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
    pub fn control_points(&self) -> &[Point2D<f32>] {
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
            control_point_index: u32::MAX,
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
            control_point_index: u32::MAX,
            subpath_index: (self.subpaths.len() - 1) as u32,
        })
    }

    #[inline]
    pub fn quadratic_curve_to(&mut self, control_point: &Point2D<f32>, endpoint: &Point2D<f32>) {
        self.subpaths
            .last_mut()
            .expect("`line_to()` called with no current subpath")
            .last_endpoint_index += 1;
        self.endpoints.push(Endpoint {
            position: *endpoint,
            control_point_index: self.control_points.len() as u32,
            subpath_index: (self.subpaths.len() - 1) as u32,
        });
        self.control_points.push(*control_point)
    }

    pub fn bezier_curve_to(&mut self,
                           point1: &Point2D<f32>,
                           point2: &Point2D<f32>,
                           endpoint: &Point2D<f32>) {
        // https://stackoverflow.com/a/2029695
        //
        // FIXME(pcwalton): Reimplement subdivision!
        let last_endpoint_index = self.subpaths
                                      .last()
                                      .expect("`bezier_curve_to()` called with no current subpath")
                                      .last_endpoint_index;
        let point0 = self.endpoints[last_endpoint_index as usize - 1].position;
        let control_point = ((point1.to_vector() + point2.to_vector()) * 0.75 -
                             (point0.to_vector() + endpoint.to_vector()) * 0.25).to_point();
        self.quadratic_curve_to(&control_point, endpoint)
    }
}