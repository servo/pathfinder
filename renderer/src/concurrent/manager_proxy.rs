// pathfinder/renderer/src/concurrent/scene_proxy.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A version of `SceneManager` that proxies all method calls out to a separate
//! thread.
//!
//! This is useful for:
//!
//!   * Avoiding GPU driver stalls on synchronous APIs such as OpenGL.
//!
//!   * Avoiding UI latency by building scenes off the main thread.
//!
//! You don't need to use this API to use Pathfinder; it's only a convenience.

use crate::command::RenderCommand;
use crate::concurrent::executor::Executor;
use crate::gpu::renderer::Renderer;
use crate::manager::{CachePolicy, RenderCommandListener, SceneManager};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::transform2d::Transform2F;
use pathfinder_geometry::transform3d::Perspective;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_gpu::Device;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

const MAX_MESSAGES_IN_FLIGHT: usize = 1024;

pub struct SceneManagerProxy {
    sender: Sender<MainToWorkerMsg>,
}

impl SceneManagerProxy {
    pub fn new<E>(executor: E) -> SceneManagerProxy where E: Executor + Send + 'static {
        SceneManagerProxy::from_scene_manager(SceneManager::new(), executor)
    }

    pub fn from_scene_manager<E>(scene_manager: SceneManager, executor: E) -> SceneManagerProxy
                                 where E: Executor + Send + 'static {
        let (main_to_worker_sender, main_to_worker_receiver) = mpsc::channel();
        thread::spawn(move || {
            scene_manager_thread(scene_manager, executor, main_to_worker_receiver)
        });
        SceneManagerProxy { sender: main_to_worker_sender }
    }

    #[inline]
    pub fn replace_scene_manager(&self, new_scene_manager: SceneManager) {
        self.sender.send(MainToWorkerMsg::ReplaceSceneManager(new_scene_manager)).unwrap();
    }

    #[inline]
    pub fn copy_scene_manager(&self) -> SceneManager {
        let (sender, receiver) = mpsc::channel();
        self.sender.send(MainToWorkerMsg::CopySceneManager(sender)).unwrap();
        receiver.recv().unwrap()
    }

    #[inline]
    pub fn set_cache_policy(&self, new_cache_policy: CachePolicy) {
        self.sender.send(MainToWorkerMsg::SetCachePolicy(new_cache_policy)).unwrap();
    }

    #[inline]
    pub fn set_view_box(&self, new_view_box: RectF) {
        self.sender.send(MainToWorkerMsg::SetViewBox(new_view_box)).unwrap();
    }

    #[inline]
    pub fn set_2d_transform(&self, new_transform: &Transform2F) {
        self.sender.send(MainToWorkerMsg::Set2DTransform(*new_transform)).unwrap();
    }

    #[inline]
    pub fn set_perspective_transform(&self, new_perspective: &Perspective) {
        self.sender.send(MainToWorkerMsg::SetPerspectiveTransform(*new_perspective)).unwrap();
    }

    #[inline]
    pub fn set_dilation(&self, new_dilation: Vector2F) {
        self.sender.send(MainToWorkerMsg::SetDilation(new_dilation)).unwrap();
    }

    #[inline]
    pub fn set_subpixel_aa_enabled(&self, enabled: bool) {
        self.sender.send(MainToWorkerMsg::SetSubpixelAAEnabled(enabled)).unwrap();
    }

    #[inline]
    pub fn build_with_listener(&self, listener: Box<dyn RenderCommandListener>) {
        self.sender.send(MainToWorkerMsg::Build(listener)).unwrap();
    }

    #[inline]
    pub fn build_with_stream(&self) -> RenderCommandStream {
        let (sender, receiver) = mpsc::sync_channel(MAX_MESSAGES_IN_FLIGHT);
        let listener = Box::new(move |command| drop(sender.send(command)));
        self.build_with_listener(listener);
        RenderCommandStream::new(receiver)
    }

    /// A convenience method to build a scene and send the resulting commands
    /// to the given renderer.
    ///
    /// Exactly equivalent to:
    ///
    ///     for command in scene_proxy.build_with_stream(options) {
    ///         renderer.render_command(&command)
    ///     }
    #[inline]
    pub fn build_and_render<D>(&self, renderer: &mut Renderer<D>) where D: Device {
        renderer.begin_scene();
        for command in self.build_with_stream() {
            renderer.render_command(&command);
        }
        renderer.end_scene();
    }
}

fn scene_manager_thread<E>(mut scene_manager: SceneManager,
                           executor: E,
                           main_to_worker_receiver: Receiver<MainToWorkerMsg>)
                           where E: Executor {
    while let Ok(msg) = main_to_worker_receiver.recv() {
        match msg {
            MainToWorkerMsg::ReplaceSceneManager(new_scene_manager) => {
                scene_manager = new_scene_manager
            }
            MainToWorkerMsg::CopySceneManager(sender) => {
                sender.send(scene_manager.clone()).unwrap()
            }
            MainToWorkerMsg::SetCachePolicy(new_cache_policy) => {
                scene_manager.set_cache_policy(new_cache_policy)
            }
            MainToWorkerMsg::SetViewBox(new_view_box) => {
                // FIXME(pcwalton): Invalidate caches?
                scene_manager.scene.set_view_box(new_view_box)
            }
            MainToWorkerMsg::Set2DTransform(new_transform) => {
                scene_manager.set_2d_transform(&new_transform)
            }
            MainToWorkerMsg::SetPerspectiveTransform(new_perspective) => {
                scene_manager.set_perspective_transform(&new_perspective)
            }
            MainToWorkerMsg::SetDilation(new_dilation) => scene_manager.set_dilation(new_dilation),
            MainToWorkerMsg::SetSubpixelAAEnabled(enabled) => {
                scene_manager.set_subpixel_aa_enabled(enabled)
            }
            MainToWorkerMsg::Build(listener) => scene_manager.build(listener, &executor)
        }
    }
}

enum MainToWorkerMsg {
    ReplaceSceneManager(SceneManager),
    CopySceneManager(Sender<SceneManager>),
    SetCachePolicy(CachePolicy),
    SetViewBox(RectF),
    Set2DTransform(Transform2F),
    SetPerspectiveTransform(Perspective),
    SetDilation(Vector2F),
    SetSubpixelAAEnabled(bool),
    Build(Box<dyn RenderCommandListener>),
}

pub struct RenderCommandStream {
    receiver: Receiver<RenderCommand>,
    done: bool,
}

impl RenderCommandStream {
    fn new(receiver: Receiver<RenderCommand>) -> RenderCommandStream {
        RenderCommandStream { receiver, done: false }
    }
}

impl Iterator for RenderCommandStream {
    type Item = RenderCommand;

    #[inline]
    fn next(&mut self) -> Option<RenderCommand> {
        if self.done {
            None
        } else {
            let command = self.receiver.recv().unwrap();
            if let RenderCommand::Finish { .. } = command {
                self.done = true;
            }
            Some(command)
        }
    }
}
