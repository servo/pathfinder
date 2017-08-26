// pathfinder/client/src/buffer-texture.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {setTextureParameters, UniformMap} from './gl-utils';
import {expectNotNull} from './utils';

export default class PathfinderBufferTexture {
    constructor(gl: WebGLRenderingContext, uniformName: string) {
        this.texture = expectNotNull(gl.createTexture(), "Failed to create buffer texture!");
        this.size = glmatrix.vec2.create();
        this.capacity = glmatrix.vec2.create();
        this.uniformName = uniformName;
        this.glType = 0;
    }

    upload(gl: WebGLRenderingContext, data: Float32Array | Uint8Array) {
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.texture);

        const glType = data instanceof Float32Array ? gl.FLOAT : gl.UNSIGNED_BYTE;
        const area = Math.ceil(data.length / 4);
        if (glType != this.glType || area > this.capacityArea) {
            const width = Math.ceil(Math.sqrt(area));
            const height = Math.ceil(area / width);
            this.size = glmatrix.vec2.fromValues(width, height);
            this.capacity = this.size;
            this.glType = glType;

            gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, glType, null);
            setTextureParameters(gl, gl.NEAREST);
        }

        const mainDimensions = glmatrix.vec4.fromValues(0,
                                                        0,
                                                        this.capacity[0],
                                                        Math.floor(area / this.capacity[0]));
        const remainderDimensions = glmatrix.vec4.fromValues(0,
                                                             mainDimensions[3],
                                                             area % this.capacity[0],
                                                             1);
        const splitIndex = mainDimensions[2] * mainDimensions[3] * 4;

        if (mainDimensions[2] > 0 && mainDimensions[3] > 0) {
            gl.texSubImage2D(gl.TEXTURE_2D,
                             0,
                             mainDimensions[0],
                             mainDimensions[1],
                             mainDimensions[2],
                             mainDimensions[3],
                             gl.RGBA,
                             this.glType,
                             data.slice(0, splitIndex));
        }

        if (remainderDimensions[2] > 0) {
            // Round data up to a multiple of 4 elements if necessary.
            let remainderLength = data.length - splitIndex;
            let remainder: Float32Array | Uint8Array;
            if (remainderLength % 4 == 0) {
                remainder = data.slice(splitIndex);
            } else {
                remainderLength += 4 - remainderLength % 4;
                remainder = new (data.constructor as any)(remainderLength);
                remainder.set(data.slice(splitIndex));
            }

            gl.texSubImage2D(gl.TEXTURE_2D,
                             0,
                             remainderDimensions[0],
                             remainderDimensions[1],
                             remainderDimensions[2],
                             remainderDimensions[3],
                             gl.RGBA,
                             this.glType,
                             remainder);
        }
    }

    bind(gl: WebGLRenderingContext, uniforms: UniformMap, textureUnit: number) {
        gl.activeTexture(gl.TEXTURE0 + textureUnit);
        gl.bindTexture(gl.TEXTURE_2D, this.texture);
        gl.uniform2i(uniforms[`${this.uniformName}Dimensions`],
                     this.capacity[0],
                     this.capacity[1]);
        gl.uniform1i(uniforms[this.uniformName], textureUnit);
    }

    private get area() {
        return this.size[0] * this.size[1];
    }

    private get capacityArea() {
        return this.capacity[0] * this.capacity[1];
    }

    readonly texture: WebGLTexture;
    readonly uniformName: string;
    private size: glmatrix.vec2;
    private capacity: glmatrix.vec2;
    private glType: number;
}

