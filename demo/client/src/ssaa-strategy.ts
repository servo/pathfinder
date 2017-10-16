// pathfinder/client/src/ssaa-strategy.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, SubpixelAAType} from './aa-strategy';
import {createFramebuffer, createFramebufferDepthTexture, setTextureParameters} from './gl-utils';
import {unwrapNull} from './utils';
import {DemoView, Renderer} from './view';

export default class SSAAStrategy extends AntialiasingStrategy {
    private level: number;
    private subpixelAA: SubpixelAAType;

    private destFramebufferSize: glmatrix.vec2;
    private supersampledFramebufferSize: glmatrix.vec2;
    private supersampledColorTexture: WebGLTexture;
    private supersampledDepthTexture: WebGLTexture;
    private supersampledFramebuffer: WebGLFramebuffer;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();
        this.level = level;
        this.subpixelAA = subpixelAA;
        this.destFramebufferSize = glmatrix.vec2.create();
        this.supersampledFramebufferSize = glmatrix.vec2.create();
    }

    attachMeshes(renderer: Renderer) {}

    setFramebufferSize(renderer: Renderer) {
        this.destFramebufferSize = glmatrix.vec2.clone(renderer.destAllocatedSize);

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.supersampledColorTexture = unwrapNull(renderer.gl.createTexture());
        renderer.gl.activeTexture(renderer.gl.TEXTURE0);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.supersampledColorTexture);
        renderer.gl.texImage2D(renderer.gl.TEXTURE_2D,
                               0,
                               renderer.colorAlphaFormat,
                               this.supersampledFramebufferSize[0],
                               this.supersampledFramebufferSize[1],
                               0,
                               renderer.colorAlphaFormat,
                               renderer.gl.UNSIGNED_BYTE,
                               null);
        setTextureParameters(renderer.gl, renderer.gl.LINEAR);

        this.supersampledDepthTexture =
            createFramebufferDepthTexture(renderer.gl, this.supersampledFramebufferSize);

        this.supersampledFramebuffer = createFramebuffer(renderer.gl,
                                                         renderer.drawBuffersExt,
                                                         [this.supersampledColorTexture],
                                                         this.supersampledDepthTexture);

        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, null);
    }

    get transform(): glmatrix.mat4 {
        const scale = glmatrix.vec2.create();
        glmatrix.vec2.div(scale, this.supersampledFramebufferSize, this.destFramebufferSize);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromScaling(transform, [scale[0], scale[1], 1.0]);
        return transform;
    }

    prepare(renderer: Renderer) {
        const framebufferSize = this.supersampledFramebufferSize;
        const usedSize = this.usedSupersampledFramebufferSize(renderer);
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, this.supersampledFramebuffer);
        renderer.gl.viewport(0, 0, framebufferSize[0], framebufferSize[1]);
        renderer.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        renderer.gl.enable(renderer.gl.SCISSOR_TEST);
    }

    antialias(renderer: Renderer) {}

    resolve(renderer: Renderer) {
        renderer.gl.bindFramebuffer(renderer.gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderer.gl.viewport(0, 0, renderer.destAllocatedSize[0], renderer.destAllocatedSize[1]);
        renderer.gl.disable(renderer.gl.DEPTH_TEST);
        renderer.gl.disable(renderer.gl.BLEND);

        // Set up the blit program VAO.
        let resolveProgram;
        if (this.subpixelAA !== 'none')
            resolveProgram = renderer.shaderPrograms.ssaaSubpixelResolve;
        else
            resolveProgram = renderer.shaderPrograms.blit;
        renderer.gl.useProgram(resolveProgram.program);
        renderer.initQuadVAO(resolveProgram.attributes);

        // Resolve framebuffer.
        renderer.gl.activeTexture(renderer.gl.TEXTURE0);
        renderer.gl.bindTexture(renderer.gl.TEXTURE_2D, this.supersampledColorTexture);
        renderer.gl.uniform1i(resolveProgram.uniforms.uSource, 0);
        renderer.gl.uniform2i(resolveProgram.uniforms.uSourceDimensions,
                          this.supersampledFramebufferSize[0],
                          this.supersampledFramebufferSize[1]);
        renderer.setTransformAndTexScaleUniformsForDest(resolveProgram.uniforms);
        renderer.gl.bindBuffer(renderer.gl.ELEMENT_ARRAY_BUFFER, renderer.quadElementsBuffer);
        renderer.gl.drawElements(renderer.gl.TRIANGLES, 6, renderer.gl.UNSIGNED_BYTE, 0);
    }

    get shouldRenderDirect() {
        return true;
    }

    private get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.clone([this.subpixelAA !== 'none' ? 3 : 2, this.level === 2 ? 1 : 2]);
    }

    private usedSupersampledFramebufferSize(renderer: Renderer): glmatrix.vec2 {
        const result = glmatrix.vec2.create();
        glmatrix.vec2.mul(result, renderer.destUsedSize, this.supersampleScale);
        return result;
    }
}
