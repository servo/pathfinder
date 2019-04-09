use crate::magicleap::MagicLeapLandscape;
use crate::magicleap::MagicLeapLogger;
use crate::magicleap::MagicLeapWindow;

use egl;
use egl::EGLContext;
use egl::EGLDisplay;
use egl::EGLSurface;

use log::debug;
use log::info;

use pathfinder_demo::Background;
use pathfinder_demo::DemoApp;
use pathfinder_demo::Options;
use pathfinder_demo::window::Event;
use pathfinder_demo::window::Mode;
use pathfinder_demo::window::SVGPath;
use pathfinder_demo::window::WindowSize;
use pathfinder_geometry::basic::point::Point2DI32;

use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use std::os::raw::c_void;

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

const SVG_FILENAMES: &[*const c_char] = &[
    &b"svg/Ghostscript_Tiger.svg\0"[0],
    &b"svg/paper.svg\0"[0],
    &b"svg/julius-caesar-with-bg.svg\0"[0],
    &b"svg/nba-notext.svg\0"[0],
    &b"svg/pathfinder_logo.svg\0"[0],
];

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_svg_filecount() -> usize {
    SVG_FILENAMES.len()
}

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_svg_filenames() -> *const *const c_char {
    &SVG_FILENAMES[0]
}

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_init() -> *mut c_void {
    unsafe { c_api::MLLoggingLog(c_api::MLLogLevel::Info, &b"Pathfinder Demo\0"[0], &b"Initializing\0"[0]) };

    let tag = CString::new("Pathfinder Demo").unwrap();
    let level = log::LevelFilter::Info;
    let logger = MagicLeapLogger::new(tag, level);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(level);
    info!("Initialized logging");

    let window = MagicLeapLandscape::new();
    let window_size = window.window_size();
    let options = Options::default();
    info!("Initializing app");
    let app = DemoApp::new(window, window_size, options);
    info!("Initialized app");

    Box::into_raw(Box::new(app)) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_render(app: *mut c_void, dpy: EGLDisplay, surf: EGLSurface, svg_filename: *const c_char) {
    let app = app as *mut DemoApp<MagicLeapLandscape>;
    if let Some(app) = app.as_mut() {
	let mut width = 0;
        let mut height = 0;
	egl::query_surface(dpy, surf, egl::EGL_WIDTH, &mut width);
	egl::query_surface(dpy, surf, egl::EGL_HEIGHT, &mut height);
	gl::Viewport(0, 0, width, height);
        let svg_filename = CStr::from_ptr(svg_filename).to_string_lossy().into_owned();
	info!("w={}, h={}.", width, height);
	app.window.set_size(width, height);
        let events = vec![
	    Event::WindowResized(app.window.window_size()),
            Event::OpenSVG(SVGPath::Resource(svg_filename)),
	];
        app.prepare_frame(events); 
        app.draw_scene(0);
        app.finish_drawing_frame();
        app.prepare_frame(vec![]); 
        app.draw_scene(0);
        app.finish_drawing_frame();
    }
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_deinit(app: *mut c_void) {
    Box::from_raw(app as *mut DemoApp<MagicLeapLandscape>);
}