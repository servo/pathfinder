use pathfinder_geometry::transform2d::Transform2DF;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_renderer::concurrent::rayon::RayonExecutor;
use pathfinder_renderer::gpu_data::RenderCommand;
use pathfinder_renderer::options::{BuildOptions, RenderCommandListener, RenderTransform};
use pathfinder_svg::BuiltSVG;
use std::env;
use std::fs::File;
use std::io::Read;
use std::time::Instant;
use usvg::{Options as UsvgOptions, Tree};

struct NoopListener;

impl RenderCommandListener for NoopListener {
    fn send(&self, _: RenderCommand) {}
}

fn main() {
    let mut data = vec![];
    let path = env::args().skip(1).next().unwrap();
    File::open(path).unwrap().read_to_end(&mut data).unwrap();
    let mut svg = BuiltSVG::from_tree(Tree::from_data(&data, &UsvgOptions::default()).unwrap());
    let original_view_box = svg.scene.view_box();
    let mut build_options = BuildOptions::default();
    println!("Scale,Time");
    for scale in 1..100 {
        let transform = Transform2DF::from_scale(Vector2F::splat(scale as f32));
        build_options.transform = RenderTransform::Transform2D(transform);
        let before_time = Instant::now();
        svg.scene.set_view_box(original_view_box.scale(scale as f32));
        svg.scene.build(build_options.clone(), Box::new(NoopListener), &RayonExecutor);
        let after_time = Instant::now();
        let elapsed_time = after_time - before_time;
        println!("{},{}", scale, elapsed_time.as_nanos() as f64 / 1000000.0);
    }
}
