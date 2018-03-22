// pathfinder/partitioner/src/mesh_pack.rs
//
// Copyright © 2018 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use bincode;
use byteorder::{LittleEndian, WriteBytesExt};
use mesh::Mesh;
use serde::Serialize;
use std::io::{self, ErrorKind, Seek, SeekFrom, Write};
use std::u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPack {
    pub meshes: Vec<Mesh>,
}

impl MeshPack {
    #[inline]
    pub fn new() -> MeshPack {
        MeshPack {
            meshes: vec![],
        }
    }

    #[inline]
    pub fn push(&mut self, mesh: Mesh) {
        self.meshes.push(mesh)
    }

    /// Writes this mesh pack to a RIFF file.
    /// 
    /// RIFF is a dead-simple extensible binary format documented here:
    /// https://msdn.microsoft.com/en-us/library/windows/desktop/ee415713(v=vs.85).aspx
    pub fn serialize_into<W>(&self, writer: &mut W) -> io::Result<()> where W: Write + Seek {
        // `PFMP` for "Pathfinder Mesh Pack".
        try!(writer.write_all(b"RIFF\0\0\0\0PFMP"));

        // NB: The RIFF spec requires that all chunks be padded to an even byte offset. However,
        // for us, this is guaranteed by construction because each instance of all of the data that
        // we're writing has a byte size that is a multiple of 4. So we don't bother with doing it
        // explicitly here.
        for mesh in &self.meshes {
            try!(write_chunk(writer, b"mesh", |writer| {
                try!(write_simple_chunk(writer, b"bqua", &mesh.b_quads));
                try!(write_simple_chunk(writer, b"bqvp", &mesh.b_quad_vertex_positions));
                try!(write_simple_chunk(writer, b"bqii", &mesh.b_quad_vertex_interior_indices));
                try!(write_simple_chunk(writer, b"bbox", &mesh.b_boxes));
                try!(write_simple_chunk(writer, b"sseg", &mesh.stencil_segments));
                try!(write_simple_chunk(writer, b"snor", &mesh.stencil_normals));
                Ok(())
            }));
        }

        let total_length = try!(writer.seek(SeekFrom::Current(0)));
        try!(writer.seek(SeekFrom::Start(4)));
        try!(writer.write_u32::<LittleEndian>((total_length - 8) as u32));
        return Ok(());

        fn write_chunk<W, F>(writer: &mut W, tag: &[u8; 4], mut closure: F) -> io::Result<()>
                             where W: Write + Seek, F: FnMut(&mut W) -> io::Result<()> {
            try!(writer.write_all(tag));
            try!(writer.write_all(b"\0\0\0\0"));

            let start_position = try!(writer.seek(SeekFrom::Current(0)));
            try!(closure(writer));

            let end_position = try!(writer.seek(SeekFrom::Current(0)));
            try!(writer.seek(SeekFrom::Start(start_position - 4)));
            try!(writer.write_u32::<LittleEndian>((end_position - start_position) as u32));
            try!(writer.seek(SeekFrom::Start(end_position)));
            Ok(())
        }

        fn write_simple_chunk<W, T>(writer: &mut W, tag: &[u8; 4], data: &[T]) -> io::Result<()>
                                    where W: Write + Seek, T: Serialize {
            write_chunk(writer, tag, |writer| {
                for datum in data {
                    try!(bincode::serialize_into(&mut *writer, datum).map_err(|_| {
                        io::Error::from(ErrorKind::Other)
                    }));
                }
                Ok(())
            })
        }
    }
}
