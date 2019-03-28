use crate::magicleap::MagicLeapLogger;
use crate::magicleap::MagicLeapWindow;

use egl;
use egl::EGLContext;
use egl::EGLDisplay;

use log::debug;

use pathfinder_demo::Background;
use pathfinder_demo::DemoApp;
use pathfinder_demo::Options;
use pathfinder_demo::window::Mode;

use std::ffi::CString;

mod c_api;
mod magicleap;

#[cfg(feature = "mocked")]
mod mocked_c_api;

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_demo(egl_display: EGLDisplay, egl_context: EGLContext) {
    unsafe { c_api::MLLoggingLog(c_api::MLLogLevel::Info, &b"Pathfinder Demo\0"[0], &b"Initializing\0"[0]) };

    let tag = CString::new("Pathfinder Demo").unwrap();
    let level = log::LevelFilter::Warn;
    let logger = MagicLeapLogger::new(tag, level);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(level);
    debug!("Initialized logging");

    let window = MagicLeapWindow::new(egl_display, egl_context);
    let window_size = window.size();

    let mut options = Options::default();
    options.ui = false;
    options.background = Background::None;
    options.mode = Mode::VR;
    options.jobs = Some(3);
    options.pipeline = 0;

    let mut app = DemoApp::new(window, window_size, options);
    debug!("Initialized app");

    while app.window.running() {
        let mut events = Vec::new();
        while let Some(event) = app.window.try_get_event() {
            events.push(event);
        }

        let scene_count = app.prepare_frame(events);
        for scene_index in 0..scene_count {
            app.draw_scene(scene_index);
        }
        app.finish_drawing_frame();
    }
}