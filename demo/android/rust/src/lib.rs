// pathfinder/demo/android/rust/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use jni::{JNIEnv, JavaVM};
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JValue};
use pathfinder_demo::DemoApp;
use pathfinder_demo::window::{Event, Keycode, Window};
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_gl::GLVersion;
use pathfinder_gpu::resources::ResourceLoader;
use std::cell::RefCell;
use std::io::Error as IOError;
use std::os::raw::c_void;
use std::path::PathBuf;

thread_local! {
    static DEMO_APP: RefCell<Option<DemoApp<WindowImpl>>> = RefCell::new(None);
    static JAVA_RESOURCE_LOADER: RefCell<Option<JavaResourceLoader>> = RefCell::new(None);
}

static RESOURCE_LOADER: AndroidResourceLoader = AndroidResourceLoader;

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_init(env: JNIEnv,
                                                                            class: JClass,
                                                                            loader: JObject) {
    JAVA_RESOURCE_LOADER.with(|java_resource_loader| {
        *java_resource_loader.borrow_mut() = Some(JavaResourceLoader::new(env, loader))
    });
    DEMO_APP.with(|demo_app| *demo_app.borrow_mut() = Some(DemoApp::<WindowImpl>::new()));
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_runOnce(env: JNIEnv,
                                                                               class: JClass) {
    DEMO_APP.with(|demo_app| {
        if let Some(ref mut demo_app) = *demo_app.borrow_mut() {
            demo_app.run_once(vec![]);
        }
    });
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushMouseDown(env: JNIEnv,
                                                                                     class: JClass,
                                                                                     x: i32,
                                                                                     y: i32) {
}

struct WindowImpl;

impl Window for WindowImpl {
    fn new(default_framebuffer_size: Point2DI32) -> WindowImpl {
        gl::load_with(|name| egl::get_proc_address(name) as *const c_void);
        WindowImpl
    }

    fn gl_version(&self) -> GLVersion {
        GLVersion::GLES3
    }

    fn size(&self) -> Point2DI32 {
        Point2DI32::new(1080, 1920)
    }

    fn drawable_size(&self) -> Point2DI32 {
        Point2DI32::new(1080, 1920)
    }

    fn mouse_position(&self) -> Point2DI32 {
        Point2DI32::new(0, 0)
    }

    fn present(&self) {}

    fn resource_loader(&self) -> &dyn ResourceLoader {
        &RESOURCE_LOADER
    }

    fn create_user_event_id(&self) -> u32 {
        0
    }

    fn push_user_event(message_type: u32, message_data: u32) {
    }

    fn run_open_dialog(&self, extension: &str) -> Result<PathBuf, ()> {
        // TODO(pcwalton)
        Err(())
    }

    fn run_save_dialog(&self, extension: &str) -> Result<PathBuf, ()> {
        // TODO(pcwalton)
        Err(())
    }
}

struct AndroidResourceLoader;

impl ResourceLoader for AndroidResourceLoader {
    fn slurp(&self, path: &str) -> Result<Vec<u8>, IOError> {
        JAVA_RESOURCE_LOADER.with(|java_resource_loader| {
            let java_resource_loader = java_resource_loader.borrow();
            let java_resource_loader = java_resource_loader.as_ref().unwrap();
            let loader = java_resource_loader.loader.as_obj();
            let env = java_resource_loader.vm.get_env().unwrap();
            match env.call_method(loader,
                                  "slurp",
                                  "(Ljava/lang/String;)Ljava/nio/ByteBuffer;",
                                  &[JValue::Object(*env.new_string(path).unwrap())]).unwrap() {
                JValue::Object(object) => {
                    let byte_buffer = JByteBuffer::from(object);
                    Ok(Vec::from(env.get_direct_buffer_address(byte_buffer).unwrap()))
                }
                _ => panic!("Unexpected return value!"),
            }
        })
    }
}

struct JavaResourceLoader {
    loader: GlobalRef,
    vm: JavaVM,
}

impl JavaResourceLoader {
    fn new(env: JNIEnv, loader: JObject) -> JavaResourceLoader {
        JavaResourceLoader {
            loader: env.new_global_ref(loader).unwrap(),
            vm: env.get_java_vm().unwrap(),
        }
    }
}
