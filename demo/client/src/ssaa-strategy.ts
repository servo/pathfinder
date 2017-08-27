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

import {AntialiasingStrategy} from './aa-strategy';
import {createFramebufferDepthTexture, createFramebuffer, setTextureParameters} from './gl-utils';
import {unwrapNull} from './utils';
import {PathfinderView} from './view';

export default class SSAAStrategy implements AntialiasingStrategy {
    constructor(level: number) {
        this.level = level;
        this.destFramebufferSize = glmatrix.vec2.create();
        this.supersampledFramebufferSize = glmatrix.vec2.create();
    }

    init(view: PathfinderView) {}

    attachMeshes(view: PathfinderView) {}
    
    setFramebufferSize(view: PathfinderView, framebufferSize: glmatrix.vec2) {
        this.destFramebufferSize = framebufferSize;

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          framebufferSize,
                          this.supersampleScale);

        this.supersampledColorTexture = unwrapNull(view.gl.createTexture());
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.supersampledColorTexture);
        view.gl.texImage2D(view.gl.TEXTURE_2D,
                           0,
                           view.gl.RGBA,
                           this.supersampledFramebufferSize[0],
                           this.supersampledFramebufferSize[1],
                           0,
                           view.gl.RGBA,
                           view.gl.UNSIGNED_BYTE,
                           null);
        setTextureParameters(view.gl, view.gl.LINEAR);

        this.supersampledDepthTexture =
            createFramebufferDepthTexture(view.gl, this.supersampledFramebufferSize);

        this.supersampledFramebuffer = createFramebuffer(view.gl,
                                                         view.drawBuffersExt,
                                                         [this.supersampledColorTexture],
                                                         this.supersampledDepthTexture);

        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, null);
    }

    transform(): glmatrix.mat4 {
        const scale = glmatrix.vec2.create();
        glmatrix.vec2.div(scale, this.supersampledFramebufferSize, this.destFramebufferSize);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromScaling(transform, [scale[0], scale[1], 1.0]);
        return transform;
    }

    prepare(view: PathfinderView) {
        const framebufferSize = this.supersampledFramebufferSize;
        const usedSize = this.usedSupersampledFramebufferSize(view);
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, this.supersampledFramebuffer);
        view.gl.viewport(0, 0, framebufferSize[0], framebufferSize[1]);
        view.gl.scissor(0, 0, usedSize[0], usedSize[1]);
        view.gl.enable(view.gl.SCISSOR_TEST);

        // Clear.
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    resolve(view: PathfinderView) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, view.destFramebuffer);
        view.gl.viewport(0, 0, view.destAllocatedSize[0], view.destAllocatedSize[1]);
        view.gl.disable(view.gl.DEPTH_TEST);

        // Set up the blit program VAO.
        const blitProgram = view.shaderPrograms.blit;
        view.gl.useProgram(blitProgram.program);
        view.initQuadVAO(blitProgram.attributes);

        // Resolve framebuffer.
        view.gl.activeTexture(view.gl.TEXTURE0);
        view.gl.bindTexture(view.gl.TEXTURE_2D, this.supersampledColorTexture);
        view.gl.uniform1i(blitProgram.uniforms.uSource, 0);
        view.setTransformAndTexScaleUniformsForDest(blitProgram.uniforms);
        view.gl.bindBuffer(view.gl.ELEMENT_ARRAY_BUFFER, view.quadElementsBuffer);
        view.gl.drawElements(view.gl.TRIANGLES, 6, view.gl.UNSIGNED_BYTE, 0);
    }

    get shouldRenderDirect() {
        return true;
    }

    private get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(2, this.level == 2 ? 1 : 2);
    }

    private usedSupersampledFramebufferSize(view: PathfinderView): glmatrix.vec2 {
        const result = glmatrix.vec2.create();
        glmatrix.vec2.mul(result, view.destUsedSize, this.supersampleScale);
        return result;
    }

    private level: number;
    private destFramebufferSize: glmatrix.vec2;
    private supersampledFramebufferSize: glmatrix.vec2;
    private supersampledColorTexture: WebGLTexture;
    private supersampledDepthTexture: WebGLTexture;
    private supersampledFramebuffer: WebGLFramebuffer;
}

