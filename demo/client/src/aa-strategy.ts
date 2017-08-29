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

import {PathfinderView} from './view';

export type AntialiasingStrategyName = 'none' | 'ssaa' | 'ecaa';

export abstract class AntialiasingStrategy {
    // Prepares any OpenGL data. This is only called on startup and canvas resize.
    init(view: PathfinderView): void {
        this.setFramebufferSize(view);
    }

    // Uploads any mesh data. This is called whenever a new set of meshes is supplied.
    abstract attachMeshes(view: PathfinderView): void;

    // This is called whenever the framebuffer has changed.
    abstract setFramebufferSize(view: PathfinderView): void;

    // Returns the transformation matrix that should be applied when directly rendering.
    abstract get transform(): glmatrix.mat4;

    // Called before direct rendering.
    //
    // Typically, this redirects direct rendering to a framebuffer of some sort.
    abstract prepare(view: PathfinderView): void;

    // Called after direct rendering.
    //
    // This usually performs the actual antialiasing and blits to the real framebuffer.
    abstract resolve(view: PathfinderView): void;

    // True if direct rendering should occur.
    shouldRenderDirect: boolean;
}

export class NoAAStrategy extends AntialiasingStrategy {
    constructor(level: number) {
        super();
        this.framebufferSize = glmatrix.vec2.create();
    }

    attachMeshes(view: PathfinderView) {}

    setFramebufferSize(view: PathfinderView) {
        this.framebufferSize = view.destAllocatedSize;
    }

    get transform(): glmatrix.mat4 {
        return glmatrix.mat4.create();
    }

    prepare(view: PathfinderView) {
        view.gl.bindFramebuffer(view.gl.FRAMEBUFFER, view.destFramebuffer);
        view.gl.viewport(0, 0, this.framebufferSize[0], this.framebufferSize[1]);
        view.gl.disable(view.gl.SCISSOR_TEST);

        // Clear.
        view.gl.clearColor(1.0, 1.0, 1.0, 1.0);
        view.gl.clearDepth(0.0);
        view.gl.depthMask(true);
        view.gl.clear(view.gl.COLOR_BUFFER_BIT | view.gl.DEPTH_BUFFER_BIT);
    }

    resolve(view: PathfinderView) {}

    get shouldRenderDirect() {
        return true;
    }

    framebufferSize: glmatrix.vec2;
}
