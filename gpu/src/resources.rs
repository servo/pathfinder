// pathfinder/gpu/src/resources.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An abstraction for reading resources.
//!
//! We can't always count on a filesystem being present.

use std::env;
use std::fs;
use std::io::Error as IOError;
use std::path::PathBuf;
use std::borrow::Cow;

pub trait ResourceLoader {
    /// This is deliberately not a `Path`, because these are virtual paths
    /// that do not necessarily correspond to real paths on a filesystem.
    fn slurp(&self, path: &str) -> Result<Cow<'static, [u8]>, IOError>;
}

pub struct FilesystemResourceLoader {
    pub directory: PathBuf,
}

impl FilesystemResourceLoader {
    pub fn locate() -> FilesystemResourceLoader {
        let mut parent_directory = env::current_dir().unwrap();
        loop {
            // So ugly :(
            let mut resources_directory = parent_directory.clone();
            resources_directory.push("resources");
            if resources_directory.is_dir() {
                let mut shaders_directory = resources_directory.clone();
                let mut textures_directory = resources_directory.clone();
                shaders_directory.push("shaders");
                textures_directory.push("textures");
                if shaders_directory.is_dir() && textures_directory.is_dir() {
                    return FilesystemResourceLoader {
                        directory: resources_directory,
                    };
                }
            }

            if !parent_directory.pop() {
                break;
            }
        }

        panic!("No suitable `resources/` directory found!");
    }
}

impl ResourceLoader for FilesystemResourceLoader {
    fn slurp(&self, virtual_path: &str) -> Result<Cow<'static, [u8]>, IOError> {
        let mut path = self.directory.clone();
        virtual_path
            .split('/')
            .for_each(|segment| path.push(segment));

        fs::read(&path)
            .map(|v| v.into())
            .map_err(|e| IOError::new(e.kind(), format!("trying to read {}", virtual_path)))
    }
}

