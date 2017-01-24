// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use compute_shader::buffer::Protection;
use compute_shader::device::Device;
use compute_shader::texture::{ExternalTexture, Format, Texture};
use euclid::size::Size2D;
use gl::types::{GLint, GLuint};
use gl;

pub struct CoverageBuffer {
    pub texture: Texture,
    pub framebuffer: GLuint,
}

impl CoverageBuffer {
    pub fn new(device: &Device, size: &Size2D<u32>) -> Result<CoverageBuffer, ()> {
        let texture = try!(device.create_texture(Format::R32F, Protection::ReadWrite, size)
                                 .map_err(drop));

        let mut framebuffer = 0;
        unsafe {
            let mut gl_texture = 0;
            gl::GenTextures(1, &mut gl_texture);
            try!(texture.bind_to(&ExternalTexture::Gl(gl_texture)).map_err(drop));

            gl::BindTexture(gl::TEXTURE_RECTANGLE, gl_texture);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_S,
                              gl::CLAMP_TO_EDGE as GLint);
            gl::TexParameteri(gl::TEXTURE_RECTANGLE,
                              gl::TEXTURE_WRAP_T,
                              gl::CLAMP_TO_EDGE as GLint);

            gl::GenFramebuffers(1, &mut framebuffer);
            gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
            gl::FramebufferTexture2D(gl::FRAMEBUFFER,
                                     gl::COLOR_ATTACHMENT0,
                                     gl::TEXTURE_RECTANGLE,
                                     gl_texture,
                                     0);
        }

        Ok(CoverageBuffer {
            texture: texture,
            framebuffer: framebuffer,
        })
    }
}

impl Drop for CoverageBuffer {
    fn drop(&mut self) {
        unsafe {
            let mut gl_texture = 0;
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.framebuffer);
            gl::GetFramebufferAttachmentParameteriv(gl::FRAMEBUFFER,
                                                    gl::COLOR_ATTACHMENT0,
                                                    gl::FRAMEBUFFER_ATTACHMENT_OBJECT_NAME,
                                                    &mut gl_texture as *mut GLuint as *mut GLint);
            gl::DeleteTextures(1, &mut gl_texture);

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteFramebuffers(1, &mut self.framebuffer);
        }
    }
}

