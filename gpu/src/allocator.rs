// pathfinder/gpu/src/gpu/allocator.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! GPU memory management.

use crate::{BufferData, BufferTarget, BufferUploadMode, Device, TextureFormat};
use instant::Instant;
use fxhash::FxHashMap;
use pathfinder_geometry::vector::Vector2I;
use std::collections::VecDeque;
use std::default::Default;
use std::mem;

// Everything above 16 MB is allocated exactly.
const MAX_BUFFER_SIZE_CLASS: u64 = 16 * 1024 * 1024;

// Number of seconds before unused memory is purged.
//
// TODO(pcwalton): jemalloc uses a sigmoidal decay curve here. Consider something similar.
const DECAY_TIME: f32 = 0.250;

// Number of seconds before we can reuse an object buffer.
//
// This helps avoid stalls. This is admittedly a bit of a hack.
const REUSE_TIME: f32 = 0.015;

pub struct GPUMemoryAllocator<D> where D: Device {
    general_buffers_in_use: FxHashMap<GeneralBufferID, BufferAllocation<D>>,
    index_buffers_in_use: FxHashMap<IndexBufferID, BufferAllocation<D>>,
    textures_in_use: FxHashMap<TextureID, TextureAllocation<D>>,
    framebuffers_in_use: FxHashMap<FramebufferID, FramebufferAllocation<D>>,
    free_objects: VecDeque<FreeObject<D>>,
    next_general_buffer_id: GeneralBufferID,
    next_index_buffer_id: IndexBufferID,
    next_texture_id: TextureID,
    next_framebuffer_id: FramebufferID,
    bytes_committed: u64,
    bytes_allocated: u64,
}

struct BufferAllocation<D> where D: Device {
    buffer: D::Buffer,
    size: u64,
    tag: BufferTag,
}

struct TextureAllocation<D> where D: Device {
    texture: D::Texture,
    descriptor: TextureDescriptor,
    tag: TextureTag,
}

struct FramebufferAllocation<D> where D: Device {
    framebuffer: D::Framebuffer,
    descriptor: TextureDescriptor,
    tag: FramebufferTag,
}

struct FreeObject<D> where D: Device {
    timestamp: Instant,
    kind: FreeObjectKind<D>,
}

enum FreeObjectKind<D> where D: Device {
    GeneralBuffer { id: GeneralBufferID, allocation: BufferAllocation<D> },
    IndexBuffer { id: IndexBufferID, allocation: BufferAllocation<D> },
    Texture { id: TextureID, allocation: TextureAllocation<D> },
    Framebuffer { id: FramebufferID, allocation: FramebufferAllocation<D> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureDescriptor {
    width: u32,
    height: u32,
    format: TextureFormat,
}

// Vertex or storage buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GeneralBufferID(pub u64);

// Index buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexBufferID(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextureID(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FramebufferID(pub u64);

// For debugging and profiling.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct BufferTag(pub &'static str);

// For debugging and profiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureTag(pub &'static str);

// For debugging and profiling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramebufferTag(pub &'static str);

impl<D> GPUMemoryAllocator<D> where D: Device {
    pub fn new() -> GPUMemoryAllocator<D> {
        GPUMemoryAllocator {
            general_buffers_in_use: FxHashMap::default(),
            index_buffers_in_use: FxHashMap::default(),
            textures_in_use: FxHashMap::default(),
            framebuffers_in_use: FxHashMap::default(),
            free_objects: VecDeque::new(),
            next_general_buffer_id: GeneralBufferID(0),
            next_index_buffer_id: IndexBufferID(0),
            next_texture_id: TextureID(0),
            next_framebuffer_id: FramebufferID(0),
            bytes_committed: 0,
            bytes_allocated: 0,
        }
    }

    pub fn allocate_general_buffer<T>(&mut self, device: &D, size: u64, tag: BufferTag)
                                      -> GeneralBufferID {
        let mut byte_size = size * mem::size_of::<T>() as u64;
        if byte_size < MAX_BUFFER_SIZE_CLASS {
            byte_size = byte_size.next_power_of_two();
        }

        let now = Instant::now();

        for free_object_index in 0..self.free_objects.len() {
            match self.free_objects[free_object_index] {
                FreeObject {
                    ref timestamp,
                    kind: FreeObjectKind::GeneralBuffer { ref allocation, .. },
                } if allocation.size == byte_size &&
                    (now - *timestamp).as_secs_f32() >= REUSE_TIME => {}
                _ => continue,
            }

            let (id, mut allocation) = match self.free_objects.remove(free_object_index) {
                Some(FreeObject {
                    kind: FreeObjectKind::GeneralBuffer { id, allocation },
                    ..
                }) => {
                    (id, allocation)
                }
                _ => unreachable!(),
            };

            allocation.tag = tag;
            self.bytes_committed += allocation.size;
            self.general_buffers_in_use.insert(id, allocation);
            return id;
        }

        let buffer = device.create_buffer(BufferUploadMode::Dynamic);
        device.allocate_buffer::<u8>(&buffer,
                                     BufferData::Uninitialized(byte_size as usize),
                                     BufferTarget::Vertex);

        let id = self.next_general_buffer_id;
        self.next_general_buffer_id.0 += 1;

        debug!("mapping general buffer: {:?} {} ({}x{}) {:?}",
               id,
               byte_size,
               size,
               mem::size_of::<T>(),
               tag);

        self.general_buffers_in_use.insert(id, BufferAllocation { buffer, size: byte_size, tag });
        self.bytes_allocated += byte_size;
        self.bytes_committed += byte_size;

        id
    }

    pub fn allocate_index_buffer<T>(&mut self, device: &D, size: u64, tag: BufferTag)
                                    -> IndexBufferID {
        let mut byte_size = size * mem::size_of::<T>() as u64;
        if byte_size < MAX_BUFFER_SIZE_CLASS {
            byte_size = byte_size.next_power_of_two();
        }

        let now = Instant::now();

        for free_object_index in 0..self.free_objects.len() {
            match self.free_objects[free_object_index] {
                FreeObject {
                    ref timestamp,
                    kind: FreeObjectKind::IndexBuffer { ref allocation, .. },
                } if allocation.size == byte_size &&
                    (now - *timestamp).as_secs_f32() >= REUSE_TIME => {}
                _ => continue,
            }

            let (id, mut allocation) = match self.free_objects.remove(free_object_index) {
                Some(FreeObject { kind: FreeObjectKind::IndexBuffer { id, allocation }, .. }) => {
                    (id, allocation)
                }
                _ => unreachable!(),
            };

            allocation.tag = tag;
            self.bytes_committed += allocation.size;
            self.index_buffers_in_use.insert(id, allocation);
            return id;
        }

        let buffer = device.create_buffer(BufferUploadMode::Dynamic);
        device.allocate_buffer::<u8>(&buffer,
                                     BufferData::Uninitialized(byte_size as usize),
                                     BufferTarget::Index);

        let id = self.next_index_buffer_id;
        self.next_index_buffer_id.0 += 1;

        debug!("mapping index buffer: {:?} {} ({}x{}) {:?}",
               id,
               byte_size,
               size,
               mem::size_of::<T>(),
               tag);

        self.index_buffers_in_use.insert(id, BufferAllocation { buffer, size: byte_size, tag });
        self.bytes_allocated += byte_size;
        self.bytes_committed += byte_size;

        id
    }

    pub fn allocate_texture(&mut self,
                            device: &D,
                            size: Vector2I,
                            format: TextureFormat,
                            tag: TextureTag)
                            -> TextureID {
        let descriptor = TextureDescriptor {
            width: size.x() as u32,
            height: size.y() as u32,
            format,
        };
        let byte_size = descriptor.byte_size();

        for free_object_index in 0..self.free_objects.len() {
            match self.free_objects[free_object_index] {
                FreeObject { kind: FreeObjectKind::Texture { ref allocation, .. }, .. } if
                        allocation.descriptor == descriptor => {}
                _ => continue,
            }

            let (id, mut allocation) = match self.free_objects.remove(free_object_index) {
                Some(FreeObject { kind: FreeObjectKind::Texture { id, allocation }, .. }) => {
                    (id, allocation)
                }
                _ => unreachable!(),
            };

            allocation.tag = tag;
            self.bytes_committed += allocation.descriptor.byte_size();
            self.textures_in_use.insert(id, allocation);
            return id;
        }

        debug!("mapping texture: {:?} {:?}", descriptor, tag);

        let texture = device.create_texture(format, size);
        let id = self.next_texture_id;
        self.next_texture_id.0 += 1;

        self.textures_in_use.insert(id, TextureAllocation { texture, descriptor, tag });

        self.bytes_allocated += byte_size;
        self.bytes_committed += byte_size;

        id
    }

    pub fn allocate_framebuffer(&mut self,
                                device: &D,
                                size: Vector2I,
                                format: TextureFormat,
                                tag: FramebufferTag)
                                -> FramebufferID {
        let descriptor = TextureDescriptor {
            width: size.x() as u32,
            height: size.y() as u32,
            format,
        };
        let byte_size = descriptor.byte_size();

        for free_object_index in 0..self.free_objects.len() {
            match self.free_objects[free_object_index].kind {
                FreeObjectKind::Framebuffer { ref allocation, .. } if allocation.descriptor ==
                        descriptor => {}
                _ => continue,
            }

            let (id, mut allocation) = match self.free_objects.remove(free_object_index) {
                Some(FreeObject { kind: FreeObjectKind::Framebuffer { id, allocation }, .. }) => {
                    (id, allocation)
                }
                _ => unreachable!(),
            };

            allocation.tag = tag;
            self.bytes_committed += allocation.descriptor.byte_size();
            self.framebuffers_in_use.insert(id, allocation);
            return id;
        }

        debug!("mapping framebuffer: {:?} {:?}", descriptor, tag);

        let texture = device.create_texture(format, size);
        let framebuffer = device.create_framebuffer(texture);
        let id = self.next_framebuffer_id;
        self.next_framebuffer_id.0 += 1;

        self.framebuffers_in_use.insert(id, FramebufferAllocation {
            framebuffer,
            descriptor,
            tag,
        });

        self.bytes_allocated += byte_size;
        self.bytes_committed += byte_size;

        id
    }

    pub fn purge_if_needed(&mut self) {
        let now = Instant::now();
        loop {
            match self.free_objects.front() {
                Some(FreeObject { timestamp, .. }) if (now - *timestamp).as_secs_f32() >=
                    DECAY_TIME => {}
                _ => break,
            }
            match self.free_objects.pop_front() {
                None => break,
                Some(FreeObject {
                    kind: FreeObjectKind::GeneralBuffer { allocation, .. },
                    ..
                }) => {
                    debug!("purging general buffer: {}", allocation.size);
                    self.bytes_allocated -= allocation.size;
                }
                Some(FreeObject { kind: FreeObjectKind::IndexBuffer { allocation, .. }, .. }) => {
                    debug!("purging index buffer: {}", allocation.size);
                    self.bytes_allocated -= allocation.size;
                }
                Some(FreeObject { kind: FreeObjectKind::Texture { allocation, .. }, .. }) => {
                    debug!("purging texture: {:?}", allocation.descriptor);
                    self.bytes_allocated -= allocation.descriptor.byte_size();
                }
                Some(FreeObject { kind: FreeObjectKind::Framebuffer { allocation, .. }, .. }) => {
                    debug!("purging framebuffer: {:?}", allocation.descriptor);
                    self.bytes_allocated -= allocation.descriptor.byte_size();
                }
            }
        }
    }

    pub fn free_general_buffer(&mut self, id: GeneralBufferID) {
        let allocation = self.general_buffers_in_use
                             .remove(&id)
                             .expect("Attempted to free unallocated general buffer!");
        self.bytes_committed -= allocation.size;
        self.free_objects.push_back(FreeObject {
            timestamp: Instant::now(),
            kind: FreeObjectKind::GeneralBuffer { id, allocation },
        });
    }

    pub fn free_index_buffer(&mut self, id: IndexBufferID) {
        let allocation = self.index_buffers_in_use
                             .remove(&id)
                             .expect("Attempted to free unallocated index buffer!");
        self.bytes_committed -= allocation.size;
        self.free_objects.push_back(FreeObject {
            timestamp: Instant::now(),
            kind: FreeObjectKind::IndexBuffer { id, allocation },
        });
    }

    pub fn free_texture(&mut self, id: TextureID) {
        let allocation = self.textures_in_use
                             .remove(&id)
                             .expect("Attempted to free unallocated texture!");
        let byte_size = allocation.descriptor.byte_size();
        self.bytes_committed -= byte_size;
        self.free_objects.push_back(FreeObject {
            timestamp: Instant::now(),
            kind: FreeObjectKind::Texture { id, allocation },
        });
    }

    pub fn free_framebuffer(&mut self, id: FramebufferID) {
        let allocation = self.framebuffers_in_use
                             .remove(&id)
                             .expect("Attempted to free unallocated framebuffer!");
        let byte_size = allocation.descriptor.byte_size();
        self.bytes_committed -= byte_size;
        self.free_objects.push_back(FreeObject {
            timestamp: Instant::now(),
            kind: FreeObjectKind::Framebuffer { id, allocation },
        });
    }

    pub fn get_general_buffer(&self, id: GeneralBufferID) -> &D::Buffer {
        &self.general_buffers_in_use[&id].buffer
    }

    pub fn get_index_buffer(&self, id: IndexBufferID) -> &D::Buffer {
        &self.index_buffers_in_use[&id].buffer
    }

    pub fn get_texture(&self, id: TextureID) -> &D::Texture {
        &self.textures_in_use[&id].texture
    }

    pub fn get_framebuffer(&self, id: FramebufferID) -> &D::Framebuffer {
        &self.framebuffers_in_use[&id].framebuffer
    }

    #[inline]
    pub fn bytes_allocated(&self) -> u64 {
        self.bytes_allocated
    }

    #[inline]
    pub fn bytes_committed(&self) -> u64 {
        self.bytes_committed
    }

    #[allow(dead_code)]
    pub fn dump(&self) {
        println!("GPU memory dump");
        println!("---------------");

        println!("General buffers:");
        let mut ids: Vec<GeneralBufferID> = self.general_buffers_in_use.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let allocation = &self.general_buffers_in_use[&id];
            println!("id {:?}: {:?} ({:?} B)", id, allocation.tag, allocation.size);
        }

        println!("Index buffers:");
        let mut ids: Vec<IndexBufferID> = self.index_buffers_in_use.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let allocation = &self.index_buffers_in_use[&id];
            println!("id {:?}: {:?} ({:?} B)", id, allocation.tag, allocation.size);
        }

        println!("Textures:");
        let mut ids: Vec<TextureID> = self.textures_in_use.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let allocation = &self.textures_in_use[&id];
            println!("id {:?}: {:?} {:?}x{:?} {:?} ({:?} B)",
                     id,
                     allocation.tag,
                     allocation.descriptor.width,
                     allocation.descriptor.height,
                     allocation.descriptor.format,
                     allocation.descriptor.byte_size());
        }

        println!("Framebuffers:");
        let mut ids: Vec<FramebufferID> = self.framebuffers_in_use.keys().cloned().collect();
        ids.sort();
        for id in ids {
            let allocation = &self.framebuffers_in_use[&id];
            println!("id {:?}: {:?} {:?}x{:?} {:?} ({:?} B)",
                     id,
                     allocation.tag,
                     allocation.descriptor.width,
                     allocation.descriptor.height,
                     allocation.descriptor.format,
                     allocation.descriptor.byte_size());
        }
    }
}

impl TextureDescriptor {
    fn byte_size(&self) -> u64 {
        self.width as u64 * self.height as u64 * self.format.bytes_per_pixel() as u64
    }
}
