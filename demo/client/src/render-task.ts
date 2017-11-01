// pathfinder/client/src/render-task.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {Renderer} from "./renderer";
import {Range} from "./utils";

export type RenderTaskType = 'color' | 'clip';

export class RenderTask {
    type: RenderTaskType;
    instanceIndices: Range;
    compositingOperation: CompositingOperation | null;

    constructor(type: RenderTaskType,
                instanceIndices: Range,
                compositingOperation?: CompositingOperation) {
        this.type = type;
        this.instanceIndices = instanceIndices;
        this.compositingOperation = compositingOperation != null ? compositingOperation : null;
    }
}

export type CompositingOperation = AlphaMaskCompositingOperation;

export class AlphaMaskCompositingOperation {
    alphaFramebufferIndex: number;

    constructor(alphaFramebufferIndex: number) {
        this.alphaFramebufferIndex = alphaFramebufferIndex;
    }

    composite(renderer: Renderer, sourceTextureIndex: number, textures: WebGLTexture[]): void {
        const renderContext = renderer.renderContext;
        const gl = renderContext.gl;

        const program = renderContext.shaderPrograms.compositeAlphaMask;

        gl.useProgram(program.program);
        renderContext.initQuadVAO(program.attributes);

        // Composite to the current framebuffer.
        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, textures[sourceTextureIndex]);
        gl.uniform1i(program.uniforms.uSource, 0);
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, textures[this.alphaFramebufferIndex]);
        gl.uniform1i(program.uniforms.uMask, 1);
        renderer.setTransformAndTexScaleUniformsForDest(program.uniforms);
        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, renderContext.quadElementsBuffer);
        gl.drawElements(gl.TRIANGLES, 6, gl.UNSIGNED_BYTE, 0);
    }
}
