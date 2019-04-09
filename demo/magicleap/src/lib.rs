use crate::magicleap::MagicLeapLogger;
use crate::magicleap::MagicLeapWindow;

use egl;
use egl::EGLContext;
use egl::EGLDisplay;
use egl::EGLSurface;

use gl::types::GLuint;

use log::info;

use pathfinder_demo::Background;
use pathfinder_demo::DemoApp;
use pathfinder_demo::Options;
use pathfinder_demo::UIVisibility;
use pathfinder_demo::window::Event;
use pathfinder_demo::window::Mode;
use pathfinder_demo::window::SVGPath;
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
use pathfinder_renderer::z_buffer::ZBuffer;
use pathfinder_renderer::gpu_data::BuiltScene;
use pathfinder_renderer::builder::SceneBuilder;
use pathfinder_renderer::builder::RenderOptions;
use pathfinder_renderer::builder::RenderTransform;
use pathfinder_svg::BuiltSVG;
use pathfinder_gpu::resources::ResourceLoader;

use std::collections::HashMap;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::sync::mpsc;

use usvg::Options as UsvgOptions;
use usvg::Tree;

mod c_api;
mod magicleap;

#[cfg(feature = "mocked")]
mod mocked_c_api;

struct ImmersiveApp {
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
    demo: DemoApp<MagicLeapWindow>,
}

#[no_mangle]
pub extern "C" fn magicleap_pathfinder_demo_init(egl_display: EGLDisplay, egl_context: EGLContext) -> *mut c_void {
    unsafe { c_api::MLLoggingLog(c_api::MLLogLevel::Info, &b"Pathfinder Demo\0"[0], &b"Initializing\0"[0]) };

    let tag = CString::new("Pathfinder Demo").unwrap();
    let level = log::LevelFilter::Warn;
    let logger = MagicLeapLogger::new(tag, level);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(level);
    info!("Initialized logging");

    let window = MagicLeapWindow::new(egl_display, egl_context);
    let window_size = window.size();

    let mut options = Options::default();
    options.ui = UIVisibility::None;
    options.background = Background::None;
    options.mode = Mode::VR;
    options.jobs = Some(3);
    options.pipeline = false;
    
    let demo = DemoApp::new(window, window_size, options);
    info!("Initialized app");

    let (sender, receiver) = mpsc::channel();
    Box::into_raw(Box::new(ImmersiveApp { sender, receiver, demo })) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_demo_run(app: *mut c_void) {
    let app = app as *mut ImmersiveApp;
    if let Some(app) = app.as_mut() {
        while app.demo.window.running() {
            let mut events = Vec::new();
            while let Some(event) = app.demo.window.try_get_event() {
                events.push(event);
            }
            while let Ok(event) = app.receiver.try_recv() {
                events.push(event);
            }
            let scene_count = app.demo.prepare_frame(events);
            for scene_index in 0..scene_count {
                app.demo.draw_scene(scene_index);
            }
            app.demo.finish_drawing_frame();
	}
    }
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_demo_load(app: *mut c_void, svg_filename: *const c_char) {
    let app = app as *mut ImmersiveApp;
    if let Some(app) = app.as_mut() {
        let svg_filename = CStr::from_ptr(svg_filename).to_string_lossy().into_owned();
        info!("Loading {}.", svg_filename);
        let _ = app.sender.send(Event::OpenSVG(SVGPath::Resource(svg_filename)));
    }
}

struct MagicLeapPathfinder {
    renderers: HashMap<(EGLSurface, EGLDisplay), Renderer<GLDevice>>,
    svgs: HashMap<String, BuiltSVG>,
    resources: FilesystemResourceLoader,
}

#[repr(C)]
pub struct MagicLeapPathfinderRenderOptions {
    display: EGLDisplay,
    surface: EGLSurface,
    bg_color: [f32; 4],
    viewport: [u32; 4],
    svg_filename: *const c_char,    
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
    
    gl::load_with(|s| egl::get_proc_address(s) as *const c_void);
    info!("Initialized gl");

    let pf = MagicLeapPathfinder {
        renderers: HashMap::new(),
        svgs: HashMap::new(),
        resources: FilesystemResourceLoader::locate(),
    };
    info!("Initialized pf");

    Box::into_raw(Box::new(pf)) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_render(pf: *mut c_void, options: *const MagicLeapPathfinderRenderOptions) {
    let pf = pf as *mut MagicLeapPathfinder;
    if let (Some(pf), Some(options)) = (pf.as_mut(), options.as_ref()) {
        let resources = &pf.resources;

        let svg_filename = CStr::from_ptr(options.svg_filename).to_string_lossy().into_owned();
	let svg = pf.svgs.entry(svg_filename).or_insert_with(|| {
            let svg_filename = CStr::from_ptr(options.svg_filename).to_string_lossy();
	    let data = resources.slurp(&*svg_filename).unwrap();
	    let tree = Tree::from_data(&data, &UsvgOptions::default()).unwrap();
            BuiltSVG::from_tree(tree)
        });

        let mut width = 0;
    	let mut height = 0;
	egl::query_surface(options.display, options.surface, egl::EGL_WIDTH, &mut width);
	egl::query_surface(options.display, options.surface, egl::EGL_HEIGHT, &mut height);
        let size = Point2DI32::new(width, height);

        let viewport_origin = Point2DI32::new(options.viewport[0] as i32, options.viewport[1] as i32);
	let viewport_size = Point2DI32::new(options.viewport[2] as i32, options.viewport[3] as i32);
        let viewport = RectI32::new(viewport_origin, viewport_size);

	let renderer = pf.renderers.entry((options.display, options.surface)).or_insert_with(|| {
   	    let mut fbo = 0;
  	    gl::GetIntegerv(gl::DRAW_FRAMEBUFFER_BINDING, &mut fbo);
            let device = GLDevice::new(GLVersion::GLES3, fbo as GLuint);
            Renderer::new(device, resources, viewport, size)
        });

        let bg_color = F32x4::new(options.bg_color[0], options.bg_color[1], options.bg_color[2], options.bg_color[3]);

        svg.scene.view_box = viewport.to_f32();
        renderer.set_main_framebuffer_size(size);
        renderer.set_viewport(viewport);
	renderer.device.bind_default_framebuffer(viewport);
        renderer.device.clear(Some(bg_color), Some(1.0), Some(0));
        renderer.disable_depth();

        let z_buffer = ZBuffer::new(svg.scene.view_box);

        let scale = i32::min(viewport_size.x(), viewport_size.y()) as f32 /
	    f32::max(svg.scene.bounds.size().x(), svg.scene.bounds.size().y());
        let transform = Transform2DF32::from_translation(&svg.scene.bounds.size().scale(-0.5))
	    .post_mul(&Transform2DF32::from_scale(&Point2DF32::splat(scale)))
            .post_mul(&Transform2DF32::from_translation(&viewport_size.to_f32().scale(0.5)));
	    
        let render_options = RenderOptions {
            transform: RenderTransform::Transform2D(transform),
            dilation: Point2DF32::default(),
            barrel_distortion: None,
	    subpixel_aa_enabled: false,
	};

        let built_options = render_options.prepare(svg.scene.bounds);
        let quad = built_options.quad();
        let built_objects = svg.scene.build_objects(built_options, &z_buffer);
        let mut scene_builder = SceneBuilder::new(built_objects, z_buffer, svg.scene.view_box);
        let mut built_scene = BuiltScene::new(svg.scene.view_box, &quad, svg.scene.objects.len() as u32);

        built_scene.shaders = svg.scene.build_shaders();
        built_scene.solid_tiles = scene_builder.build_solid_tiles();
        while let Some(batch) = scene_builder.build_batch() {
            built_scene.batches.push(batch);
        }
	
        renderer.render_scene(&built_scene);
    }
}

#[no_mangle]
pub unsafe extern "C" fn magicleap_pathfinder_deinit(pf: *mut c_void) {
    Box::from_raw(pf as *mut MagicLeapPathfinder);
}