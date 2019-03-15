// pathfinder/demo/android/rust/src/main.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use]
extern crate lazy_static;

use jni::{JNIEnv, JavaVM};
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JValue};
use pathfinder_demo::DemoApp;
use pathfinder_demo::window::{Event, Window, WindowSize};
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_gl::GLVersion;
use pathfinder_gpu::resources::ResourceLoader;
use std::cell::RefCell;
use std::io::Error as IOError;
use std::mem;
use std::os::raw::c_void;
use std::path::PathBuf;
use std::sync::Mutex;

lazy_static! {
    static ref EVENT_QUEUE: Mutex<Vec<Event>> = Mutex::new(vec![]);
}

thread_local! {
    static DEMO_APP: RefCell<Option<DemoApp<WindowImpl>>> = RefCell::new(None);
    static JAVA_RESOURCE_LOADER: RefCell<Option<JavaResourceLoader>> = RefCell::new(None);
}

static RESOURCE_LOADER: AndroidResourceLoader = AndroidResourceLoader;

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_init(env: JNIEnv,
                                                                            class: JClass,
                                                                            loader: JObject,
                                                                            width: i32,
                                                                            height: i32) {
    let logical_size = Point2DI32::new(width, height);
    let window_size = WindowSize { logical_size, backing_scale_factor: 1.0 };

    JAVA_RESOURCE_LOADER.with(|java_resource_loader| {
        *java_resource_loader.borrow_mut() = Some(JavaResourceLoader::new(env, loader));
    });
    DEMO_APP.with(|demo_app| {
        gl::load_with(|name| egl::get_proc_address(name) as *const c_void);
        *demo_app.borrow_mut() = Some(DemoApp::new(WindowImpl, window_size));
    });
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_runOnce(env: JNIEnv,
                                                                               class: JClass) {
    DEMO_APP.with(|demo_app| {
        let mut event_queue = EVENT_QUEUE.lock().unwrap();
        if let Some(ref mut demo_app) = *demo_app.borrow_mut() {
            demo_app.run_once(mem::replace(&mut *event_queue, vec![]));
        }
    });
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushWindowResizedEvent(
            env: JNIEnv,
            class: JClass,
            width: i32,
            height: i32) {
    EVENT_QUEUE.lock().unwrap().push(Event::WindowResized(WindowSize {
        logical_size: Point2DI32::new(width, height),
        backing_scale_factor: 1.0,
    }))
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushMouseDownEvent(
            _: JNIEnv,
            _: JClass,
            x: i32,
            y: i32) {
    EVENT_QUEUE.lock().unwrap().push(Event::MouseDown(Point2DI32::new(x, y)))
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushMouseDraggedEvent(
            _: JNIEnv,
            _: JClass,
            x: i32,
            y: i32) {
    EVENT_QUEUE.lock().unwrap().push(Event::MouseDragged(Point2DI32::new(x, y)))
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushLookEvent(
            _: JNIEnv,
            _: JClass,
            pitch: f32,
            yaw: f32) {
    EVENT_QUEUE.lock().unwrap().push(Event::Look { pitch, yaw })
}

struct WindowImpl;

impl Window for WindowImpl {
    fn gl_version(&self) -> GLVersion {
        GLVersion::GLES3
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
