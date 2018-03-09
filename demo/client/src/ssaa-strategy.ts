// pathfinder/demo/client/src/ssaa-strategy.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, DirectRenderingMode, SubpixelAAType, TileInfo} from './aa-strategy';
import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture, setTextureParameters} from './gl-utils';
import {Renderer} from './renderer';
import {unwrapNull} from './utils';
import {DemoView} from './view';

export default class SSAAStrategy extends AntialiasingStrategy {
    get passCount(): number {
        switch (this.level) {
        case 16:
            return 4;
        case 8:
            return 2;
        }
        return 1;
    }

    private level: number;
    private subpixelAA: SubpixelAAType;

    private destFramebufferSize: glmatrix.vec2;
    private supersampledFramebufferSize: glmatrix.vec2;
    private supersampledColorTexture!: WebGLTexture;
    private supersampledDepthTexture!: WebGLTexture;
    private supersampledFramebuffer!: WebGLFramebuffer;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();

        this.level = level;
        this.subpixelAA = subpixelAA;
        this.destFramebufferSize = glmatrix.vec2.create();
        this.supersampledFramebufferSize = glmatrix.vec2.create();
    }

    attachMeshes(renderer: Renderer): void {}

    setFramebufferSize(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        this.destFramebufferSize = glmatrix.vec2.clone(renderer.destAllocatedSize);

        this.supersampledFramebufferSize = glmatrix.vec2.create();
        glmatrix.vec2.mul(this.supersampledFramebufferSize,
                          this.destFramebufferSize,
                          this.supersampleScale);

        this.supersampledColorTexture =
            createFramebufferColorTexture(gl,
                                          this.supersampledFramebufferSize,
                                          renderContext.colorAlphaFormat,
                                          gl.LINEAR);

        this.supersampledDepthTexture =
            createFramebufferDepthTexture(gl, this.supersampledFramebufferSize);

        this.supersampledFramebuffer = createFramebuffer(gl,
                                                         this.supersampledColorTexture,
                                                         this.supersampledDepthTexture);

        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    }

    get transform(): glmatrix.mat4 {
        const scale = glmatrix.vec2.create();
        glmatrix.vec2.div(scale, this.supersampledFramebufferSize, this.destFramebufferSize);

        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromScaling(transform, [scale[0], scale[1], 1.0]);
        return transform;
    }

    prepareForRendering(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const framebufferSize = this.supersampledFramebufferSize;
        const usedSize = this.usedSupersampledFramebufferSize(renderer);
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.supersampledFramebuffer);
        gl.viewport(0, 0, framebufferSize[0], framebufferSize[1]);
        gl.scissor(0, 0, usedSize[0], usedSize[1]);
        gl.enable(gl.SCISSOR_TEST);

        const clearColor = renderer.backgroundColor;
        gl.clearColor(clearColor[0], clearColor[1], clearColor[2], clearColor[3]);
        gl.clearDepth(0.0);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    }

    prepareForDirectRendering(renderer: Renderer): void {}

    prepareToRenderObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.bindFramebuffer(gl.FRAMEBUFFER, this.supersampledFramebuffer);
        gl.viewport(0,
                    0,
                    this.supersampledFramebufferSize[0],
                    this.supersampledFramebufferSize[1]);
        gl.disable(gl.SCISSOR_TEST);
    }

    finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void {}

    antialiasObject(renderer: Renderer): void {}

    finishAntialiasingObject(renderer: Renderer, objectIndex: number): void {}

    resolveAAForObject(renderer: Renderer): void {}

    resolve(pass: number, renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderer.setDrawViewport();
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

        // Set up the blit program VAO.
        let resolveProgram;
        if (this.subpixelAA !== 'none')
            resolveProgram = renderContext.shaderPrograms.ssaaSubpixelResolve;
        else
            resolveProgram = renderContext.shaderPrograms.blitLinear;
        gl.useProgram(resolveProgram.program);
        renderContext.initQuadVAO(resolveProgram.attributes);

        // Resolve framebuffer.
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, this.supersampledColorTexture);
        gl.uniform1i(resolveProgram.uniforms.uSource, 0);
        gl.uniform2i(resolveProgram.uniforms.uSourceDimensions,
                     this.supersampledFramebufferSize[0],
                     this.supersampledFramebufferSize[1]);
        const tileInfo = this.tileInfoForPass(pass);
        renderer.setTransformAndTexScaleUniformsForDest(resolveProgram.uniforms, tileInfo);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);
        gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0);
    }

    worldTransformForPass(renderer: Renderer, pass: number): glmatrix.mat4 {
        const tileInfo = this.tileInfoForPass(pass);
        const usedSize = renderer.destUsedSize;
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform, transform, [tileInfo.size[0], tileInfo.size[1], 1.0]);
        glmatrix.mat4.translate(transform, transform, [
            -tileInfo.position[0] / tileInfo.size[0] * 2.0,
            -tileInfo.position[1] / tileInfo.size[1] * 2.0,
            0.0,
        ]);
        glmatrix.mat4.translate(transform, transform, [1.0, 1.0, 0.0]);
        return transform;
    }

    private tileInfoForPass(pass: number): TileInfo {
        const tileSize = this.tileSize;
        return {
            position: glmatrix.vec2.clone([pass % tileSize[0], Math.floor(pass / tileSize[0])]),
            size: tileSize,
        };
    }

    get directRenderingMode(): DirectRenderingMode {
        return 'color';
    }

    private get supersampleScale(): glmatrix.vec2 {
        return glmatrix.vec2.clone([this.subpixelAA !== 'none' ? 3 : 2, this.level === 2 ? 1 : 2]);
    }

    private get tileSize(): glmatrix.vec2 {
        switch (this.level) {
        case 16:
            return glmatrix.vec2.clone([2.0, 2.0]);
        case 8:
            return glmatrix.vec2.clone([2.0, 1.0]);
        }
        return glmatrix.vec2.clone([1.0, 1.0]);
    }

    private usedSupersampledFramebufferSize(renderer: Renderer): glmatrix.vec2 {
        const result = glmatrix.vec2.create();
        glmatrix.vec2.mul(result, renderer.destUsedSize, this.supersampleScale);
        return result;
    }
}
