use crate::magicleap::MagicLeapLandscape;
use crate::magicleap::MagicLeapLogger;
use crate::magicleap::MagicLeapWindow;

use egl;
use egl::EGLContext;
use egl::EGLDisplay;
use egl::EGLSurface;

use gl::types::GLuint;

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
use pathfinder_geometry::basic::point::Point2DF32;
use pathfinder_geometry::basic::rect::RectI32;
use pathfinder_geometry::basic::transform2d::Transform2DF32;
use pathfinder_gl::GLDevice;
use pathfinder_gl::GLVersion;
use pathfinder_gpu::Device;
use pathfinder_gpu::resources::FilesystemResourceLoader;
use pathfinder_renderer::gpu::renderer::Renderer;
use pathfinder_simd::default::F32x4;
use pathfinder_renderer::scene::Scene;
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::builder::SceneBuilder;
use pathfinder_renderer::builder::RenderOptions;
use pathfinder_renderer::builder::RenderTransform;
use pathfinder_svg::BuiltSVG;
use pathfinder_gpu::resources::ResourceLoader;

use rayon::ThreadPoolBuilder;

use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use std::os::raw::c_void;

use usvg::Options as UsvgOptions;
use usvg::Tree;

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
    &b"svg/nba-notext.svg\0"[0],
    &b"svg/paper.svg\0"[0],
    &b"svg/Ghostscript_Tiger.svg\0"[0],
    &b"svg/julius-caesar-with-bg.svg\0"[0],
    &b"svg/julius-caesar.svg\0"[0],
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

struct MagicLeapPathfinder {
    surfaces: HashMap<EGLSurface, MagicLeapPathfinderSurface>,
    resources: FilesystemResourceLoader,
}

struct MagicLeapPathfinderSurface {
    surf: EGLSurface,
    size: Point2DI32,
    viewport: RectI32,
    bg_color: F32x4,
    renderer: Renderer<GLDevice>,
    scene: Scene,
}

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_init(dpy: EGLDisplay, surf: EGLSurface) -> *mut c_void {
    unsafe { c_api::MLLoggingLog(c_api::MLLogLevel::Info, &b"Pathfinder Demo\0"[0], &b"Initializing\0"[0]) };

    let tag = CString::new("Pathfinder Demo").unwrap();
    let level = log::LevelFilter::Info;
    let logger = MagicLeapLogger::new(tag, level);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(level);
    info!("Initialized logging");

    let mut thread_pool_builder = ThreadPoolBuilder::new()
        .build_global().unwrap();
    info!("Initialized rayon");
    
    gl::load_with(|s| egl::get_proc_address(s) as *const c_void);
    info!("Initialized gl");

    let pf = MagicLeapPathfinder {
        surfaces: HashMap::new(),
        resources: FilesystemResourceLoader::locate(),
    };
    info!("Initialized pf");

    Box::into_raw(Box::new(pf)) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_render(pf: *mut c_void, dpy: EGLDisplay, surf: EGLSurface, svg_filename: *const c_char) {
    let pf = pf as *mut MagicLeapPathfinder;
    if let Some(pf) = pf.as_mut() {
        let resources = &pf.resources;
        let pfs = pf.surfaces.entry(surf).or_insert_with(|| {
	    let mut width = 0;
            let mut height = 0;
	    let mut fbo = 0;
	    egl::query_surface(dpy, surf, egl::EGL_WIDTH, &mut width);
	    egl::query_surface(dpy, surf, egl::EGL_HEIGHT, &mut height);
	    gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut fbo);
            let device = GLDevice::new(GLVersion::GLES3, fbo as GLuint);
	    let size = Point2DI32::new(width, height);
	    let viewport = RectI32::new(Point2DI32::default(), size);
            let renderer = Renderer::new(device, resources, viewport, size);
	    let bg_color = F32x4::new(0.5, 0.5, 0.5, 1.0);
	    let svg_filename = CStr::from_ptr(svg_filename).to_string_lossy();
	    let data = resources.slurp(&*svg_filename).unwrap();
            let svg = BuiltSVG::from_tree(Tree::from_data(&data, &UsvgOptions::default()).unwrap());
	    let mut scene = svg.scene;
	    scene.view_box = viewport.to_f32();
	    MagicLeapPathfinderSurface {
	        surf,
		size,
		viewport,
		bg_color,
		renderer,
		scene,
	    }
	});
        pfs.renderer.set_main_framebuffer_size(pfs.size);
        pfs.renderer.set_viewport(pfs.viewport);
	pfs.renderer.device.bind_default_framebuffer(pfs.viewport);
        pfs.renderer.device.clear(Some(pfs.bg_color), Some(1.0), Some(0));
        pfs.renderer.disable_depth();

        let z_buffer = ZBuffer::new(pfs.scene.view_box);

        let scale = i32::min(pfs.viewport.size().x(), pfs.viewport.size().y()) as f32 /
	    f32::max(pfs.scene.bounds.size().x(), pfs.scene.bounds.size().y());
        let transform = Transform2DF32::from_translation(&pfs.scene.bounds.size().scale(-0.5))
	    .post_mul(&Transform2DF32::from_scale(&Point2DF32::splat(scale)))
            .post_mul(&Transform2DF32::from_translation(&pfs.viewport.size().to_f32().scale(0.5)));
	    
        let render_options = RenderOptions {
            transform: RenderTransform::Transform2D(transform),
            dilation: Point2DF32::default(),
            barrel_distortion: None,
	};

        let built_options = render_options.prepare(pfs.scene.bounds);
        let quad = built_options.quad();

        let built_objects = pfs.scene.build_objects(built_options, &z_buffer);

        let mut built_scene = BuiltScene::new(pfs.scene.view_box, &quad, pfs.scene.objects.len() as u32);
        built_scene.shaders = pfs.scene.build_shaders();

        let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, pfs.scene.view_box);
        built_scene.solid_tiles = scene_builder.build_solid_tiles();
        while let Some(batch) = scene_builder.build_batch() {
            built_scene.batches.push(batch);
        }
	
        pfs.renderer.render_scene(&built_scene);

	// let mut width = 0;
        // let mut height = 0;
	// let mut fbo = 0;
	// egl::query_surface(dpy, surf, egl::EGL_WIDTH, &mut width);
	// egl::query_surface(dpy, surf, egl::EGL_HEIGHT, &mut height);
	// gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut fbo);
	// gl::Viewport(0, 0, width, height);
        // let svg_filename = CStr::from_ptr(svg_filename).to_string_lossy().into_owned();
	// info!("svg={}, w={}, h={}, fbo={}.", svg_filename, width, height, fbo);
	// if (app.window.dpy != dpy) || (app.window.surf != surf) {
   	//     app.render_to_current_gl_context();
	//     app.window.surf = surf;
	//     app.window.dpy = dpy;
	// }
	// app.window.fbo = fbo as GLuint;
	// app.window.set_size(width, height);
	// app.options.background = if let Background::Dark = app.options.background {
	//     Background::Light
	// } else {
	//     Background::Dark
	// };
        // let events = vec![
	//     Event::WindowResized(app.window.window_size()),
        //     Event::OpenSVG(SVGPath::Resource(svg_filename)),
	// ];
        // app.prepare_frame(events); 
        // app.draw_scene(0);
        // app.finish_drawing_frame();
	// while app.dirty {
        //     app.prepare_frame(vec![]); 
        //     app.draw_scene(0);
        //     app.finish_drawing_frame();
	// }
    }
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_deinit(pf: *mut c_void) {
    Box::from_raw(pf as *mut MagicLeapPathfinder);
}