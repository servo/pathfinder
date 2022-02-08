// pathfinder/renderer/src/concurrent/scene_proxy.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A version of `Scene` that proxies all method calls out to a separate
//! thread.
//!
//! This is useful for:
//!
//!   * Avoiding GPU driver stalls on synchronous APIs such as OpenGL.
//!
//!   * Avoiding UI latency by building scenes off the main thread.
//!
//! You don't need to use this API to use Pathfinder; it's only a convenience.

use crate::concurrent::executor::Executor;
use crate::gpu::options::RendererLevel;
use crate::gpu::renderer::Renderer;
use crate::gpu_data::RenderCommand;
use crate::options::{BuildOptions, RenderCommandListener};
use crate::scene::{Scene, SceneSink};
use crossbeam_channel::{self, Receiver, Sender};
use pathfinder_geometry::rect::RectF;
use pathfinder_gpu::Device;
use std::thread;

const MAX_MESSAGES_IN_FLIGHT: usize = 1024;

/// A version of `Scene` that proxies all method calls out to a separate thread.
pub struct SceneProxy {
    sender: Sender<MainToWorkerMsg>,
    receiver: Receiver<RenderCommand>,
}

impl SceneProxy {
    /// Creates a new scene proxy using the given renderer GPU API level and executor to execute
    /// CPU tasks.
    /// 
    /// If you want to use multiple threads, you typically pass in a `RayonExecutor` here. If you
    /// want to use a single thread (perhaps because you're in a WebAssembly environment), pass a
    /// `SequentialExecutor`.
    pub fn new<E>(renderer_level: RendererLevel, executor: E) -> SceneProxy
                  where E: Executor + Send + 'static {
        SceneProxy::from_scene(Scene::new(), renderer_level, executor)
    }

    /// Wraps an existing scene in a scene proxy using the given renderer GPU API level an executor
    /// to execute CPU tasks.
    /// 
    /// If you want to use multiple threads, you typically pass in a `RayonExecutor` here. If you
    /// want to use a single thread (perhaps because you're in a WebAssembly environment), pass a
    /// `SequentialExecutor`.
    pub fn from_scene<E>(scene: Scene, renderer_level: RendererLevel, executor: E)
                         -> SceneProxy
                         where E: Executor + Send + 'static {
        let (main_to_worker_sender, main_to_worker_receiver) =
            crossbeam_channel::bounded(MAX_MESSAGES_IN_FLIGHT);
        let (worker_to_main_sender, worker_to_main_receiver) =
            crossbeam_channel::bounded(MAX_MESSAGES_IN_FLIGHT);
        let listener = RenderCommandListener::new(Box::new(move |command| {
            drop(worker_to_main_sender.send(command))
        }));
        let sink = SceneSink::new(listener, renderer_level);
        thread::spawn(move || scene_thread(scene, executor, sink, main_to_worker_receiver));
        SceneProxy { sender: main_to_worker_sender, receiver: worker_to_main_receiver }
    }

    /// Replaces the wrapped scene with a new one, discarding the old scene.
    #[inline]
    pub fn replace_scene(&self, new_scene: Scene) {
        self.sender.send(MainToWorkerMsg::ReplaceScene(new_scene)).unwrap();
    }

    /// Sets the view box of the scene, which defines the visible rectangle.
    #[inline]
    pub fn set_view_box(&self, new_view_box: RectF) {
        self.sender.send(MainToWorkerMsg::SetViewBox(new_view_box)).unwrap();
    }

    /// Constructs a scene and queues up the commands needed to render it.
    #[inline]
    pub fn build(&self, options: BuildOptions) {
        self.sender.send(MainToWorkerMsg::Build(options)).unwrap();
    }

    /// Sends all queued commands to the given renderer to render the wrapped scene.
    #[inline]
    pub fn render<D>(&mut self, renderer: &mut Renderer<D>) where D: Device {
        renderer.begin_scene();
        while let Ok(command) = self.receiver.recv() {
            renderer.render_command(&command);
            match command {
                RenderCommand::Finish { .. } => break,
                _ => {}
            }
        }
        renderer.end_scene();
    }

    /// A convenience method to build a scene and send the resulting commands
    /// to the given renderer.
    ///
    /// Exactly equivalent to:
    ///
    /// ```ignore
    /// scene_proxy.build(build_options);
    /// scene_proxy.render(renderer);
    /// ```
    #[inline]
    pub fn build_and_render<D>(&mut self, renderer: &mut Renderer<D>, build_options: BuildOptions)
                               where D: Device {
        self.build(build_options);
        self.render(renderer);
    }

    /// Returns a copy of the wrapped scene.
    #[inline]
    pub fn copy_scene(&self) -> Scene {
        let (sender, receiver) = crossbeam_channel::bounded(MAX_MESSAGES_IN_FLIGHT);
        self.sender.send(MainToWorkerMsg::CopyScene(sender)).unwrap();
        receiver.recv().unwrap()
    }
}

fn scene_thread<E>(mut scene: Scene,
                   executor: E,
                   mut sink: SceneSink<'static>,
                   main_to_worker_receiver: Receiver<MainToWorkerMsg>)
                   where E: Executor {
    while let Ok(msg) = main_to_worker_receiver.recv() {
        match msg {
            MainToWorkerMsg::ReplaceScene(new_scene) => scene = new_scene,
            MainToWorkerMsg::CopyScene(sender) => sender.send(scene.clone()).unwrap(),
            MainToWorkerMsg::SetViewBox(new_view_box) => scene.set_view_box(new_view_box),
            MainToWorkerMsg::Build(options) => scene.build(options, &mut sink, &executor),
        }
    }
}

enum MainToWorkerMsg {
    ReplaceScene(Scene),
    CopyScene(Sender<Scene>),
    SetViewBox(RectF),
    Build(BuildOptions),
}
