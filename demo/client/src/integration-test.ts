// pathfinder/client/src/integration-test.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from './aa-strategy';
import {SubpixelAAType} from './aa-strategy';
import {DemoAppController} from "./app-controller";
import {OrthographicCamera} from './camera';
import {UniformMap} from './gl-utils';
import {Renderer} from "./renderer";
import {ShaderMap} from "./shader-loader";
import SSAAStrategy from './ssaa-strategy';
import {DemoView} from "./view";
import {AdaptiveMonochromeXCAAStrategy} from './xcaa-strategy';

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    xcaa: AdaptiveMonochromeXCAAStrategy,
};

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    xcaa: typeof AdaptiveMonochromeXCAAStrategy;
}

class IntegrationTestAppController extends DemoAppController<IntegrationTestView> {
    protected builtinFileURI: string;
    protected defaultFile: string;
    protected createView(): IntegrationTestView {
        throw new Error("Method not implemented.");
    }
    protected fileLoaded(data: ArrayBuffer, builtinName: string | null): void {
        throw new Error("Method not implemented.");
    }
}

class IntegrationTestView extends DemoView {
    get camera(): OrthographicCamera {
        return this.renderer.camera;
    }
    readonly renderer: IntegrationTestRenderer;
}

class IntegrationTestRenderer extends Renderer {
    camera: OrthographicCamera;
    destFramebuffer: WebGLFramebuffer | null;
    destAllocatedSize: glmatrix.vec2;
    destUsedSize: glmatrix.vec2;
    protected objectCount: number;
    protected usedSizeFactor: glmatrix.vec2;
    protected worldTransform: glmatrix.mat4;

    pathBoundingRects(objectIndex: number): Float32Array {
        throw new Error("Method not implemented.");
    }

    setHintsUniform(uniforms: UniformMap): void {
        throw new Error("Method not implemented.");
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: SubpixelAAType):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {
        throw new Error("Method not implemented.");
    }

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        throw new Error("Method not implemented.");
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        throw new Error("Method not implemented.");
    }

    protected directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }
}

function main() {
    const controller = new IntegrationTestAppController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
