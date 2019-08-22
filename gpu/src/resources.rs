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

use std::fs::File;
use std::io::{Error as IOError, Read};
use std::path::PathBuf;

pub trait ResourceLoader {
    /// This is deliberately not a `Path`, because these are virtual paths
    /// that do not necessarily correspond to real paths on a filesystem.
    fn slurp(&self, path: &str) -> Result<Vec<u8>, IOError>;
}

pub struct FilesystemResourceLoader {
    pub directory: PathBuf,
}

#[cfg(all(target_vendor = "apple", not(target_os = "macos")))]
fn parent_directory() -> PathBuf {
    use objc::runtime::Object;
    let cstr = unsafe {
        let main: *mut Object = msg_send![class!(NSBundle), mainBundle];
        let path: *mut Object = msg_send![main, resourcePath];
        let cstr: *const std::os::raw::c_char = msg_send![path, UTF8String];
        std::ffi::CStr::from_ptr(cstr)
    };
    PathBuf::from(cstr.to_str().unwrap().to_owned())
}

#[cfg(not(all(target_vendor = "apple", not(target_os = "macos"))))]
fn parent_directory() -> PathBuf {
    std::env::current_dir().unwrap()
}

impl FilesystemResourceLoader {
    pub fn locate() -> FilesystemResourceLoader {
        let mut parent_directory = parent_directory();
        loop {
            // So ugly :(
            let mut resources_directory = parent_directory.clone();
            resources_directory.push("res");
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
    fn slurp(&self, virtual_path: &str) -> Result<Vec<u8>, IOError> {
        let mut path = self.directory.clone();
        virtual_path
            .split('/')
            .for_each(|segment| path.push(segment));

        let mut data = vec![];
        File::open(&path)?.read_to_end(&mut data)?;
        Ok(data)
    }
}
