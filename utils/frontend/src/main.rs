// pathfinder/utils/frontend/main.rs
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.// Copyright © 2017 Mozilla Foundation

extern crate app_units;
extern crate clap;
extern crate freetype_sys;
extern crate pathfinder_font_renderer;
extern crate pathfinder_partitioner;
extern crate pathfinder_path_utils;

use app_units::Au;
use clap::{App, Arg};
use freetype_sys::{FT_Init_FreeType, FT_New_Face};
use pathfinder_font_renderer::{FontContext, FontKey, FontInstance, GlyphKey, SubpixelOffset};
use pathfinder_partitioner::mesh_library::MeshLibrary;
use pathfinder_partitioner::partitioner::Partitioner;
use pathfinder_path_utils::monotonic::MonotonicPathCommandStream;
use pathfinder_path_utils::{PathBuffer, PathBufferStream};
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
    let font_key = FontKey::new();
    try!(font_context.add_font_from_memory(&font_key, Arc::new(font_data), 0));
    let font_instance = FontInstance {
        font_key: font_key,
        size: Au::from_f64_px(FONT_SIZE),
    };

    let mut path_buffer = PathBuffer::new();
    let mut partitioner = Partitioner::new(MeshLibrary::new());
    let subpath_ranges: Vec<_> = (0..glyph_count).map(|glyph_index| {
        let glyph_key = GlyphKey::new(glyph_index, SubpixelOffset(0));

        let subpath_start = path_buffer.subpaths.len() as u32;
        if let Ok(glyph_outline) = font_context.glyph_outline(&font_instance, &glyph_key) {
            path_buffer.add_stream(MonotonicPathCommandStream::new(glyph_outline.into_iter()))
        }
        let subpath_end = path_buffer.subpaths.len() as u32;

        let path_index = (glyph_index + 1) as u16;
        let stream = PathBufferStream::subpath_range(&path_buffer, subpath_start..subpath_end);
        partitioner.library_mut().push_segments(path_index, stream);
        let stream = PathBufferStream::subpath_range(&path_buffer, subpath_start..subpath_end);
        partitioner.library_mut().push_normals(stream);

        subpath_start..subpath_end
    }).collect();

    partitioner.init_with_path_buffer(&path_buffer);

    for (glyph_index, subpath) in subpath_ranges.iter().cloned().enumerate() {
        partitioner.partition((glyph_index + 1) as u16, subpath.start, subpath.end);
    }

    partitioner.library_mut().optimize();

    let mut output_file = try!(File::create(output_path).map_err(drop));
    partitioner.library().serialize_into(&mut output_file).map_err(drop)
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
