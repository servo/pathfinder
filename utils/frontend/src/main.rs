// pathfinder/utils/frontend/main.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Pathfinder is built as a set of modular Rust crates and accompanying shaders. Depending on how
//! you plan to use Pathfinder, you may need to link against many of these crates, or you may not
//! need to link against any of them and and only use the shaders at runtime.
//! 
//! Typically, if you need to generate paths at runtime or load fonts on the fly, then you will
//! need to use the `pathfinder_partitioner` and/or `pathfinder_font_renderer` crates. If your app
//! instead uses a fixed set of paths or fonts, then you may wish to consider running the
//! Pathfinder command-line tool as part of your build process. Note that in the latter case you
//! may not need to ship any Rust code at all!
//! 
//! This crate defines the `pathfinder` command line tool. It takes a font as an argument and
//! produces *mesh libraries* for the glyphs you wish to include. A *mesh library* is essentially a
//! simple storage format for VBOs. To render these paths, you can directly upload these VBOs to
//! the GPU and render them using the shaders provided.

extern crate app_units;
extern crate clap;
extern crate freetype_sys;
extern crate lyon_geom;
extern crate lyon_path;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate pathfinder_path_utils;

use app_units::Au;
use clap::{App, Arg};
use freetype_sys::{FT_Init_FreeType, FT_New_Face};
use lyon_path::PathEvent;
use lyon_path::builder::{FlatPathBuilder, PathBuilder};
use pathfinder_font_renderer::{FontContext, FontInstance, GlyphKey, SubpixelOffset};
use pathfinder_partitioner::FillRule;
use pathfinder_partitioner::mesh_pack::MeshPack;
use pathfinder_partitioner::partitioner::Partitioner;
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;
use std::ptr;
use std::sync::Arc;

const FONT_SIZE: f64 = 72.0;

fn convert_font(font_path: &Path, output_path: &Path) -> Result<(), ()> {
    let mut freetype_library = ptr::null_mut();
    let glyph_count;
    unsafe {
        if FT_Init_FreeType(&mut freetype_library) != 0 {
            return Err(())
        }

        // TODO(pcwalton): Allow the user to select a face by index.
        let mut freetype_face = ptr::null_mut();
        let font_path = match font_path.to_str() {
            None => return Err(()),
            Some(font_path) => font_path,
        };
        let font_path = try!(CString::new(font_path).map_err(drop));
        if FT_New_Face(freetype_library, font_path.as_ptr(), 0, &mut freetype_face) != 0 {
            return Err(())
        }

        glyph_count = (*freetype_face).num_glyphs as u32;
    }

    let mut font_data = vec![];
    let mut font_file = try!(File::open(font_path).map_err(drop));
    try!(font_file.read_to_end(&mut font_data).map_err(drop));

    // TODO(pcwalton): Allow the user to select a face by index.
    let mut font_context = try!(FontContext::new());
    try!(font_context.add_font_from_memory(&(), Arc::new(font_data), 0));
    let font_instance = FontInstance {
        font_key: (),
        size: Au::from_f64_px(FONT_SIZE),
    };

    let mut paths: Vec<(u16, Vec<PathEvent>)> = vec![];
    let mut mesh_pack = MeshPack::new();

    for glyph_index in 0..glyph_count {
        let glyph_key = GlyphKey::new(glyph_index, SubpixelOffset(0));

        let path = match font_context.glyph_outline(&font_instance, &glyph_key) {
            Ok(path) => path,
            Err(_) => continue,
        };

        let mut partitioner = Partitioner::new();

        let path_index = (glyph_index + 1) as u16;
        partitioner.mesh_mut().push_stencil_segments(path.iter());
        path.iter().for_each(|event| partitioner.builder_mut().path_event(event));
        partitioner.partition(FillRule::Winding);
        partitioner.builder_mut().build_and_reset();

        paths.push((path_index, path.iter().collect()));
        mesh_pack.push(partitioner.into_mesh());
    }

    let mut output_file = try!(File::create(output_path).map_err(drop));
    mesh_pack.serialize_into(&mut output_file).map_err(drop)
}

pub fn main() {
    let app = App::new("Pathfinder Build Utility")
        .version("0.1")
        .author("The Pathfinder Project Developers")
        .about("Builds meshes from fonts for use with Pathfinder")
        .arg(Arg::with_name("FONT-PATH").help("The `.ttf` or `.otf` font file to use")
                                        .required(true)
                                        .index(1))
        .arg(Arg::with_name("OUTPUT-PATH").help("The `.pfml` mesh library to produce").index(2));
    let matches = app.get_matches();

    let font_path = matches.value_of("FONT-PATH").unwrap();
    let font_path = Path::new(font_path);

    let output_path = match matches.value_of("OUTPUT-PATH") {
        Some(output_path) => PathBuf::from(output_path),
        None => {
            match font_path.file_stem() {
                None => {
                    eprintln!("error: No valid input path specified");
                    process::exit(1)
                }
                Some(output_path) => {
                    let mut output_path = PathBuf::from(output_path);
                    output_path.set_extension("pfml");
                    output_path
                }
            }
        }
    };

    if convert_font(font_path, &output_path).is_err() {
        // TODO(pcwalton): Better error handling.
        eprintln!("error: Failed");
        process::exit(1)
    }
}
