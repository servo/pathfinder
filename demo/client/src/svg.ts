// pathfinder/client/src/svg.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {ShaderMap, ShaderProgramSource} from './shader-loader';
import {PathfinderView} from './view';
import {panic} from './utils';
import AppController from './app-controller';

class SVGDemoController extends AppController<SVGDemoView> {
    protected createView(canvas: HTMLCanvasElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>) {
        const svg = document.getElementById('pf-svg') as Element as SVGSVGElement;
        return new SVGDemoView(this, svg, canvas, commonShaderSource, shaderSources);
    }
}

class SVGDemoView extends PathfinderView {
    constructor(appController: SVGDemoController,
                svg: SVGSVGElement,
                canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(canvas, commonShaderSource, shaderSources);

        this.svg = svg;
    }

    get destAllocatedSize() {
        return panic("TODO");
    }

    get destDepthTexture() {
        return panic("TODO");
    }

    get destFramebuffer() {
        return panic("TODO");
    }

    get destUsedSize() {
        return panic("TODO");
    }

    setTransformAndTexScaleUniformsForDest() {
        panic("TODO");
    }

    setTransformSTAndTexScaleUniformsForDest() {
        panic("TODO");
    }

    private svg: SVGSVGElement;
}
