// pathfinder/client/src/3d-demo.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import {AntialiasingStrategy, AntialiasingStrategyName} from "./aa-strategy";
import {mat4, vec2} from "gl-matrix";
import {ShaderMap, ShaderProgramSource} from "./shader-loader";
import {PathfinderView, Timings} from "./view";
import AppController from "./app-controller";

class ThreeDController extends AppController<ThreeDView> {
    protected fileLoaded(): void {
        throw new Error("Method not implemented.");
    }

    protected createView(canvas: HTMLCanvasElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         ThreeDView {
        throw new Error("Method not implemented.");
    }

    protected builtinFileURI: string;
}

class ThreeDView extends PathfinderView {
    protected resized(initialSize: boolean): void {
        throw new Error("Method not implemented.");
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName, aaLevel: number):
                               AntialiasingStrategy {
        throw new Error("Method not implemented.");
    }

    protected compositeIfNecessary(): void {
        throw new Error("Method not implemented.");
    }

    protected updateTimings(timings: Timings): void {
        throw new Error("Method not implemented.");
    }

    protected panned(): void {
        throw new Error("Method not implemented.");
    }

    destFramebuffer: WebGLFramebuffer | null;
    destAllocatedSize: vec2;
    destUsedSize: vec2;
    protected usedSizeFactor: vec2;
    protected scale: number;
    protected worldTransform: mat4;
}

function main() {
    const controller = new ThreeDController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
