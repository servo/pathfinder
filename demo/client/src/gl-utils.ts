// pathfinder/client/src/gl-utils.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';

import {assert, UINT32_SIZE, unwrapNull} from './utils';
import {ColorAlphaFormat, DemoView} from './view';

export type WebGLVertexArrayObject = any;

export interface AttributeMap {
    [attributeName: string]: number;
}

export interface UniformMap {
    [uniformName: string]: WebGLUniformLocation;
}

export interface EXTDisjointTimerQuery {
    readonly QUERY_COUNTER_BITS_EXT: GLenum;
    readonly CURRENT_QUERY_EXT: GLenum;
    readonly QUERY_RESULT_EXT: GLenum;
    readonly QUERY_RESULT_AVAILABLE_EXT: GLenum;
    readonly TIME_ELAPSED_EXT: GLenum;
    readonly TIMESTAMP_EXT: GLenum;
    readonly GPU_DISJOINT_EXT: GLenum;
    createQueryEXT(): WebGLQuery;
    deleteQueryEXT(query: WebGLQuery): void;
    isQueryEXT(query: any): GLboolean;
    beginQueryEXT(target: GLenum, query: WebGLQuery): void;
    endQueryEXT(target: GLenum): void;
    queryCounterEXT(query: WebGLQuery, target: GLenum): void;
    getQueryEXT(target: GLenum, pname: GLenum): any;
    getQueryObjectEXT(query: WebGLQuery, pname: GLenum): any;
}

export class WebGLQuery {}

export const QUAD_ELEMENTS: Uint8Array = new Uint8Array([0, 1, 2, 1, 3, 2]);

export function createFramebufferColorTexture(gl: WebGLRenderingContext,
                                              size: glmatrix.vec2,
                                              colorAlphaFormat: ColorAlphaFormat,
                                              filter?: number):
                                              WebGLTexture {
    // Firefox seems to have a bug whereby textures don't get marked as initialized when cleared
    // if they're anything other than the first attachment of an FBO. To work around this, supply
    // zero data explicitly when initializing the texture.
    let format, type, internalFormat, bitDepth, zeroes;
    if (colorAlphaFormat === 'RGBA8') {
        format = internalFormat = gl.RGBA;
        type = gl.UNSIGNED_BYTE;
        bitDepth = 32;
        zeroes = new Uint8Array(size[0] * size[1] * 4);
    } else {
        format = internalFormat = gl.RGBA;
        type = gl.UNSIGNED_SHORT_5_5_5_1;
        bitDepth = 16;
        zeroes = new Uint16Array(size[0] * size[1] * 4);
    }

    const texture = unwrapNull(gl.createTexture());
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, internalFormat, size[0], size[1], 0, format, type, zeroes);
    setTextureParameters(gl, _.defaultTo(filter, gl.NEAREST));
    return texture;
}

export function createFramebufferDepthTexture(gl: WebGLRenderingContext, size: glmatrix.vec2):
                                              WebGLTexture {
    const texture = unwrapNull(gl.createTexture());
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D,
                  0,
                  gl.DEPTH_COMPONENT,
                  size[0],
                  size[1],
                  0,
                  gl.DEPTH_COMPONENT,
                  gl.UNSIGNED_INT,
                  null);
    setTextureParameters(gl, gl.NEAREST);
    return texture;
}

export function setTextureParameters(gl: WebGLRenderingContext, filter: number) {
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
}

export function createFramebuffer(gl: WebGLRenderingContext,
                                  colorAttachment: WebGLTexture,
                                  depthAttachment: WebGLTexture | null):
                                  WebGLFramebuffer {
    const framebuffer = unwrapNull(gl.createFramebuffer());
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);

    gl.framebufferTexture2D(gl.FRAMEBUFFER,
                            gl.COLOR_ATTACHMENT0,
                            gl.TEXTURE_2D,
                            colorAttachment,
                            0);

    if (depthAttachment != null) {
        gl.framebufferTexture2D(gl.FRAMEBUFFER,
                                gl.DEPTH_ATTACHMENT,
                                gl.TEXTURE_2D,
                                depthAttachment,
                                0);
        assert(gl.getFramebufferAttachmentParameter(gl.FRAMEBUFFER,
                                                    gl.DEPTH_ATTACHMENT,
                                                    gl.FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE) ===
                                                    gl.TEXTURE,
               "Failed to attach depth texture!");

    }

    assert(gl.checkFramebufferStatus(gl.FRAMEBUFFER) === gl.FRAMEBUFFER_COMPLETE,
           "Framebuffer was incomplete!");
    return framebuffer;
}
