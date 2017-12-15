// pathfinder/client/src/aa-strategy.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {createFramebuffer, createFramebufferColorTexture} from './gl-utils';
import {createFramebufferDepthTexture} from './gl-utils';
import {Renderer} from './renderer';
import {unwrapNull} from './utils';
import {DemoView} from './view';

export type AntialiasingStrategyName = 'none' | 'ssaa' | 'xcaa';

export type DirectRenderingMode = 'none' | 'color';

export type SubpixelAAType = 'none' | 'medium';

export type GammaCorrectionMode = 'off' | 'on';

export type StemDarkeningMode = 'none' | 'dark';

export interface TileInfo {
    size: glmatrix.vec2;
    position: glmatrix.vec2;
}

export abstract class AntialiasingStrategy {
    // The type of direct rendering that should occur, if any.
    abstract readonly directRenderingMode: DirectRenderingMode;

    // How many rendering passes this AA strategy requires.
    abstract readonly passCount: number;

    // Prepares any OpenGL data. This is only called on startup and canvas resize.
    init(renderer: Renderer): void {
        this.setFramebufferSize(renderer);
    }

    // Uploads any mesh data. This is called whenever a new set of meshes is supplied.
    abstract attachMeshes(renderer: Renderer): void;

    // This is called whenever the framebuffer has changed.
    abstract setFramebufferSize(renderer: Renderer): void;

    // Returns the transformation matrix that should be applied when directly rendering.
    abstract get transform(): glmatrix.mat4;

    // Called before rendering.
    //
    // Typically, this redirects rendering to a framebuffer of some sort.
    abstract prepareForRendering(renderer: Renderer): void;

    // Called before directly rendering.
    //
    // Typically, this redirects rendering to a framebuffer of some sort.
    abstract prepareForDirectRendering(renderer: Renderer): void;

    // Called before directly rendering a single object.
    abstract prepareToRenderObject(renderer: Renderer, objectIndex: number): void;

    abstract finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void;

    // Called after direct rendering.
    //
    // This usually performs the actual antialiasing.
    abstract antialiasObject(renderer: Renderer, objectIndex: number): void;

    // Called after antialiasing each object.
    abstract finishAntialiasingObject(renderer: Renderer, objectIndex: number): void;

    // Called before rendering each object directly.
    abstract resolveAAForObject(renderer: Renderer, objectIndex: number): void;

    // Called after antialiasing.
    //
    // This usually blits to the real framebuffer.
    abstract resolve(pass: number, renderer: Renderer): void;

    worldTransformForPass(renderer: Renderer, pass: number): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }
}

export class NoAAStrategy extends AntialiasingStrategy {
    framebufferSize: glmatrix.vec2;

    get passCount(): number {
        return 1;
    }

    private renderTargetColorTextures: WebGLTexture[];
    private renderTargetDepthTextures: WebGLTexture[];
    private renderTargetFramebuffers: WebGLFramebuffer[];

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();
        this.framebufferSize = glmatrix.vec2.create();
        this.renderTargetColorTextures = [];
        this.renderTargetDepthTextures = [];
        this.renderTargetFramebuffers = [];
    }

    attachMeshes(renderer: Renderer) {}

    setFramebufferSize(renderer: Renderer) {
        this.framebufferSize = renderer.destAllocatedSize;
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    prepareForRendering(renderer: Renderer): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;
        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        gl.viewport(0, 0, this.framebufferSize[0], this.framebufferSize[1]);
        gl.disable(gl.SCISSOR_TEST);
    }

    prepareForDirectRendering(renderer: Renderer): void {}

    prepareToRenderObject(renderer: Renderer, objectIndex: number): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        if (renderer.usesIntermediateRenderTargets &&
            (renderer.renderTaskTypeForObject(objectIndex) === 'clip' ||
             renderer.compositingOperationForObject(objectIndex) != null)) {
            if (this.renderTargetColorTextures[objectIndex] == null) {
                this.renderTargetColorTextures[objectIndex] =
                    createFramebufferColorTexture(gl,
                                                  this.framebufferSize,
                                                  renderContext.colorAlphaFormat);
            }
            if (this.renderTargetDepthTextures[objectIndex] == null) {
                this.renderTargetDepthTextures[objectIndex] =
                    createFramebufferDepthTexture(gl, this.framebufferSize);
            }
            if (this.renderTargetFramebuffers[objectIndex] == null) {
                this.renderTargetFramebuffers[objectIndex] =
                    createFramebuffer(gl,
                                      this.renderTargetColorTextures[objectIndex],
                                      this.renderTargetDepthTextures[objectIndex]);
            }
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.renderTargetFramebuffers[objectIndex]);
        } else {
            gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        }

        gl.viewport(0, 0, this.framebufferSize[0], this.framebufferSize[1]);
        gl.disable(gl.SCISSOR_TEST);
    }

    finishDirectlyRenderingObject(renderer: Renderer, objectIndex: number): void {
        if (!renderer.usesIntermediateRenderTargets)
            return;

        const compositingOperation = renderer.compositingOperationForObject(objectIndex);
        if (compositingOperation == null)
            return;

        const gl = renderer.renderContext.gl;
        gl.bindFramebuffer(gl.FRAMEBUFFER, renderer.destFramebuffer);
        gl.viewport(0, 0, renderer.destAllocatedSize[0], renderer.destAllocatedSize[1]);
        gl.disable(gl.DEPTH_TEST);
        gl.disable(gl.BLEND);

        compositingOperation.composite(renderer, objectIndex, this.renderTargetColorTextures);
    }

    antialiasObject(renderer: Renderer, objectIndex: number): void {}

    finishAntialiasingObject(renderer: Renderer, objectIndex: number): void {}

    resolveAAForObject(renderer: Renderer): void {}

    resolve(pass: number, renderer: Renderer): void {}

    get directRenderingMode(): DirectRenderingMode {
        return 'color';
    }
}
