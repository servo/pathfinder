// pathfinder/resources/build.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("manifest.rs");
    let mut dest = File::create(dest_path).unwrap();
    let cwd = env::current_dir().unwrap();

    writeln!(&mut dest, "// Generated by `pathfinder/resources/build.rs`. Do not edit!\n").unwrap();
    writeln!(&mut dest,
             "pub static RESOURCES: &'static [(&'static str, &'static [u8])] = &[").unwrap();

    let mut add_manifest = |path: &str| {
        let src = BufReader::new(File::open(path).unwrap());
        for line in src.lines() {
            let line = line.unwrap();
            let line = line.trim_start().trim_end();
            if line.is_empty() || line.starts_with("#") {
                continue;
            }
    
            let escaped_path = line.escape_default().to_string();
            let mut full_path = cwd.clone();
            full_path.push(line);
            let escaped_full_path = full_path.to_str().unwrap().escape_default().to_string();
    
            writeln!(&mut dest,
                     "    (\"{}\", include_bytes!(\"{}\")),",
                     escaped_path,
                     escaped_full_path).unwrap();
    
            println!("cargo:rerun-if-changed={}", line);
        }
    };

    add_manifest("MANIFEST");

    for part in ["debug", "gl3", "gl4", "metal"] {
        let key = format!("CARGO_FEATURE_{}", part.to_ascii_uppercase());
        if env::var(&key).is_ok() {
            add_manifest(&format!("MANIFEST.{part}"));
        }
    }

    writeln!(&mut dest, "];").unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=MANIFEST");
}
