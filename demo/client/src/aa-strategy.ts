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

import {DemoView, Renderer} from './view';

export type AntialiasingStrategyName = 'none' | 'ssaa' | 'xcaa';

export type SubpixelAAType = 'none' | 'medium';

export type StemDarkeningMode = 'none' | 'dark';

export abstract class AntialiasingStrategy {
    // True if direct rendering should occur.
    shouldRenderDirect: boolean;

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

    // Called before direct rendering.
    //
    // Typically, this redirects direct rendering to a framebuffer of some sort.
    abstract prepare(renderer: Renderer): void;

    // Called after direct rendering.
    //
    // This usually performs the actual antialiasing.
    abstract antialias(renderer: Renderer): void;

    // Called after antialiasing.
    //
    // This usually blits to the real framebuffer.
    abstract resolve(renderer: Renderer): void;
}

export class NoAAStrategy extends AntialiasingStrategy {
    framebufferSize: glmatrix.vec2;

    constructor(level: number, subpixelAA: SubpixelAAType) {
        super();
        this.framebufferSize = glmatrix.vec2.create();
    }

    attachMeshes(renderer: Renderer) {}

    setFramebufferSize(renderer: Renderer) {
        this.framebufferSize = renderer.destAllocatedSize;
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    prepare(renderer: Renderer) {
        const renderContext = renderer.renderContext;
        renderContext.gl.bindFramebuffer(renderContext.gl.FRAMEBUFFER, renderer.destFramebuffer);
        renderContext.gl.viewport(0, 0, this.framebufferSize[0], this.framebufferSize[1]);
        renderContext.gl.disable(renderContext.gl.SCISSOR_TEST);
    }

    antialias(renderer: Renderer) {}

    resolve(renderer: Renderer) {}

    get shouldRenderDirect() {
        return true;
    }
}
