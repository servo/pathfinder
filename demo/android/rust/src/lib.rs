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
use jni::objects::{GlobalRef, JByteBuffer, JClass, JObject, JString, JValue};
use pathfinder_demo::DemoApp;
use pathfinder_demo::Options;
use pathfinder_demo::window::{Event, SVGPath, Window, WindowSize};
use pathfinder_geometry::basic::point::Point2DI32;
use pathfinder_geometry::basic::rect::RectI32;
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
    static JAVA_ACTIVITY: RefCell<Option<JavaActivity>> = RefCell::new(None);
    static JAVA_RESOURCE_LOADER: RefCell<Option<JavaResourceLoader>> = RefCell::new(None);
}

static RESOURCE_LOADER: AndroidResourceLoader = AndroidResourceLoader;

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_init(env: JNIEnv,
                                                                            class: JClass,
                                                                            activity: JObject,
                                                                            loader: JObject,
                                                                            width: i32,
                                                                            height: i32) {
    let logical_size = Point2DI32::new(width, height);
    let window_size = WindowSize { logical_size, backing_scale_factor: 1.0 };
    let window = WindowImpl { size: logical_size };

    JAVA_ACTIVITY.with(|java_activity| {
        *java_activity.borrow_mut() = Some(JavaActivity::new(env.clone(), activity));
    });
    JAVA_RESOURCE_LOADER.with(|java_resource_loader| {
        *java_resource_loader.borrow_mut() = Some(JavaResourceLoader::new(env, loader));
    });
    DEMO_APP.with(|demo_app| {
        gl::load_with(|name| egl::get_proc_address(name) as *const c_void);
        *demo_app.borrow_mut() = Some(DemoApp::new(window, window_size, options));
    });
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_prepareFrame(env: JNIEnv,
                                                                                    class: JClass)
                                                                                    -> i32 {
    DEMO_APP.with(|demo_app| {
        let mut event_queue = EVENT_QUEUE.lock().unwrap();
        match *demo_app.borrow_mut() {
            Some(ref mut demo_app) => {
                demo_app.prepare_frame(mem::replace(&mut *event_queue, vec![])) as i32
            }
            None => 0,
        }
    })
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_drawScene(
            env: JNIEnv,
            class: JClass,
            scene_index: i32) {
    DEMO_APP.with(|demo_app| {
        if let Some(ref mut demo_app) = *demo_app.borrow_mut() {
            demo_app.draw_scene(scene_index as u32)
        }
    })
}

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_finishDrawingFrame(
            env: JNIEnv,
            class: JClass) {
    DEMO_APP.with(|demo_app| {
        if let Some(ref mut demo_app) = *demo_app.borrow_mut() {
            demo_app.finish_drawing_frame()
        }
    })
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

#[no_mangle]
pub unsafe extern "system" fn
        Java_graphics_pathfinder_pathfinderdemo_PathfinderDemoRenderer_pushOpenSVGEvent(
            env: JNIEnv,
            _: JClass,
            string: JObject) {
    let string: String = env.get_string(JString::from(string)).unwrap().into();
    EVENT_QUEUE.lock().unwrap().push(Event::OpenSVG(SVGPath::Resource(string)))
}

struct WindowImpl {
    size: Point2DI32,
}

impl Window for WindowImpl {
    fn gl_version(&self) -> GLVersion {
        GLVersion::GLES3
    }

    fn mouse_position(&self) -> Point2DI32 {
        Point2DI32::new(0, 0)
    }

    fn view_box_size(&self, mode: Mode) -> Point2DI32 {
        let mut width = self.size.x();
        let height = self.size.y();
        if let Mode::VR = mode {
            width = width / 2;
        }
        Point2DI32::new(width as i32, height as i32)
    }

    fn make_current(&mut self, mode: Mode, index: Option<u32>) -> RectI32 {
        let mut width = self.size.x();
        let mut offset_x = 0;
        let height = self.size.y();
        if let (Mode::VR, Some(index)) = (mode, index) {
            width = width / 2;
            offset_x = (index as i32) * width;
        }
        let size = Point2DI32::new(width, height);
        let offset = Point2DI32::new(offset_x, 0);
        RectI32::new(offset, size)
    }

    fn present(&mut self) {}

    fn resource_loader(&self) -> &dyn ResourceLoader {
        &RESOURCE_LOADER
    }

    fn create_user_event_id(&self) -> u32 {
        0
    }

    fn push_user_event(message_type: u32, message_data: u32) {
    }

    fn present_open_svg_dialog(&mut self) {
        JAVA_ACTIVITY.with(|java_activity| {
            let mut java_activity = java_activity.borrow_mut();
            let java_activity = java_activity.as_mut().unwrap();
            let env = java_activity.vm.get_env().unwrap();
            env.call_method(java_activity.activity.as_obj(),
                            "presentOpenSVGDialog",
                            "()V",
                            &[]).unwrap();
        });
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

struct JavaActivity {
    activity: GlobalRef,
    vm: JavaVM,
}

impl JavaActivity {
    fn new(env: JNIEnv, activity: JObject) -> JavaActivity {
        JavaActivity {
            activity: env.new_global_ref(activity).unwrap(),
            vm: env.get_java_vm().unwrap(),
        }
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
