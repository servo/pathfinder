// pathfinder/utils/tile-svg/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(clippy::float_cmp)]

#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
extern crate rand;

use clap::{App, Arg};
use jemallocator;
use pathfinder_renderer::builder::SceneBuilder;
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::serialization::RiffSerialize;
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_svg::SceneExt;
use rayon::ThreadPoolBuilder;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use usvg::{Options as UsvgOptions, Tree};

#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
    let matches = App::new("tile-svg")
        .arg(
            Arg::with_name("runs")
                .short("r")
                .long("runs")
                .value_name("COUNT")
                .takes_value(true)
                .help("Run a benchmark with COUNT runs"),
        )
        .arg(
            Arg::with_name("jobs")
                .short("j")
                .long("jobs")
                .value_name("THREADS")
                .takes_value(true)
                .help("Number of threads to use"),
        )
        .arg(
            Arg::with_name("INPUT")
                .help("Path to the SVG file to render")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("OUTPUT")
                .help("Path to the output PF3 data")
                .required(false)
                .index(2),
        )
        .get_matches();
    let runs: usize = match matches.value_of("runs") {
        Some(runs) => runs.parse().unwrap(),
        None => 1,
    };
    let jobs: Option<usize> = matches
        .value_of("jobs")
        .map(|string| string.parse().unwrap());
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

    println!(
        "Scene bounds: {:?} View box: {:?}",
        scene.bounds, scene.view_box
    );
    println!(
        "{} objects, {} paints",
        scene.objects.len(),
        scene.paints.len()
    );

    let (mut elapsed_object_build_time, mut elapsed_scene_build_time) = (0.0, 0.0);

    let mut built_scene = BuiltScene::new(&scene.view_box);
    for _ in 0..runs {
        let z_buffer = ZBuffer::new(&scene.view_box);

        let start_time = Instant::now();
        let built_objects = match jobs {
            Some(1) => scene.build_objects_sequentially(&z_buffer),
            _ => scene.build_objects(&z_buffer),
        };
        elapsed_object_build_time += duration_to_ms(&(Instant::now() - start_time));

        let start_time = Instant::now();
        built_scene = BuiltScene::new(&scene.view_box);
        built_scene.shaders = scene.build_shaders();
        let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, &scene.view_box);
        built_scene.solid_tiles = scene_builder.build_solid_tiles();
        while let Some(batch) = scene_builder.build_batch() {
            built_scene.batches.push(batch);
        }
        elapsed_scene_build_time += duration_to_ms(&(Instant::now() - start_time));
    }

    elapsed_object_build_time /= runs as f64;
    elapsed_scene_build_time /= runs as f64;
    let total_elapsed_time = elapsed_object_build_time + elapsed_scene_build_time;

    println!(
        "{:.3}ms ({:.3}ms objects, {:.3}ms scene) elapsed",
        total_elapsed_time, elapsed_object_build_time, elapsed_scene_build_time
    );

    println!("{} solid tiles", built_scene.solid_tiles.len());
    for (batch_index, batch) in built_scene.batches.iter().enumerate() {
        println!(
            "Batch {}: {} fills, {} mask tiles",
            batch_index,
            batch.fills.len(),
            batch.mask_tiles.len()
        );
    }

    if let Some(output_path) = output_path {
        built_scene
            .write(&mut BufWriter::new(File::create(output_path).unwrap()))
            .unwrap();
    }
}

fn duration_to_ms(duration: &Duration) -> f64 {
    duration.as_secs() as f64 * 1000.0 + f64::from(duration.subsec_micros()) / 1000.0
}
