// pathfinder/renderer/src/serialization.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use byteorder::{LittleEndian, WriteBytesExt};
use crate::gpu_data::{BuiltScene, FillBatchPrimitive};
use crate::gpu_data::{MaskTileBatchPrimitive, SolidTileScenePrimitive};
use crate::paint::ObjectShader;
use std::io::{self, Write};
use std::mem;

pub trait RiffSerialize {
    fn write<W>(&self, writer: &mut W) -> io::Result<()> where W: Write;
}

impl RiffSerialize for BuiltScene {
    fn write<W>(&self, writer: &mut W) -> io::Result<()>
    where
        W: Write,
    {
        writer.write_all(b"RIFF")?;

        let header_size = 4 * 6;

        let solid_tiles_size = self.solid_tiles.len() * mem::size_of::<SolidTileScenePrimitive>();

        let batch_sizes: Vec<_> = self
            .batches
            .iter()
            .map(|batch| BatchSizes {
                fills: (batch.fills.len() * mem::size_of::<FillBatchPrimitive>()),
                mask_tiles: (batch.mask_tiles.len() * mem::size_of::<MaskTileBatchPrimitive>()),
            })
            .collect();

        let total_batch_sizes: usize = batch_sizes.iter().map(|sizes| 8 + sizes.total()).sum();

        let shaders_size = self.shaders.len() * mem::size_of::<ObjectShader>();

        writer.write_u32::<LittleEndian>(
            (4 + 8 + header_size + 8 + solid_tiles_size + 8 + shaders_size + total_batch_sizes)
                as u32,
        )?;

        writer.write_all(b"PF3S")?;

        writer.write_all(b"head")?;
        writer.write_u32::<LittleEndian>(header_size as u32)?;
        writer.write_u32::<LittleEndian>(FILE_VERSION)?;
        writer.write_u32::<LittleEndian>(self.batches.len() as u32)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.x)?;
        writer.write_f32::<LittleEndian>(self.view_box.origin.y)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.width)?;
        writer.write_f32::<LittleEndian>(self.view_box.size.height)?;

        writer.write_all(b"shad")?;
        writer.write_u32::<LittleEndian>(shaders_size as u32)?;
        for &shader in &self.shaders {
            let fill_color = shader.fill_color;
            writer.write_all(&[fill_color.r, fill_color.g, fill_color.b, fill_color.a])?;
        }

        writer.write_all(b"soli")?;
        writer.write_u32::<LittleEndian>(solid_tiles_size as u32)?;
        for &tile_primitive in &self.solid_tiles {
            writer.write_i16::<LittleEndian>(tile_primitive.tile_x)?;
            writer.write_i16::<LittleEndian>(tile_primitive.tile_y)?;
            writer.write_u16::<LittleEndian>(tile_primitive.shader.0)?;
        }

        for (batch, sizes) in self.batches.iter().zip(batch_sizes.iter()) {
            writer.write_all(b"batc")?;
            writer.write_u32::<LittleEndian>(sizes.total() as u32)?;

            writer.write_all(b"fill")?;
            writer.write_u32::<LittleEndian>(sizes.fills as u32)?;
            for fill_primitive in &batch.fills {
                writer.write_u16::<LittleEndian>(fill_primitive.px.0)?;
                writer.write_u32::<LittleEndian>(fill_primitive.subpx.0)?;
                writer.write_u16::<LittleEndian>(fill_primitive.mask_tile_index)?;
            }

            writer.write_all(b"mask")?;
            writer.write_u32::<LittleEndian>(sizes.mask_tiles as u32)?;
            for &tile_primitive in &batch.mask_tiles {
                writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_x)?;
                writer.write_i16::<LittleEndian>(tile_primitive.tile.tile_y)?;
                writer.write_i16::<LittleEndian>(tile_primitive.tile.backdrop)?;
                writer.write_u16::<LittleEndian>(tile_primitive.shader.0)?;
            }
        }

        return Ok(());

        const FILE_VERSION: u32 = 0;

        struct BatchSizes {
            fills: usize,
            mask_tiles: usize,
        }

        impl BatchSizes {
            fn total(&self) -> usize {
                8 + self.fills + 8 + self.mask_tiles
            }
        }
    }
}
