// pathfinder/demo/client/src/svg-demo.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';

import {DemoAppController} from './app-controller';
import {OrthographicCamera} from "./camera";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from './shader-loader';
import {BUILTIN_SVG_URI, SVGLoader} from './svg-loader';
import {SVGRenderer} from './svg-renderer';
import {DemoView} from './view';

const parseColor = require('parse-color');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const DEFAULT_FILE: string = 'tiger';

class SVGDemoController extends DemoAppController<SVGDemoView> {
    loader: SVGLoader;

    protected readonly builtinFileURI: string = BUILTIN_SVG_URI;

    private meshes: PathfinderMeshData;

    start() {
        super.start();

        this.loader = new SVGLoader;

        this.loadInitialFile(this.builtinFileURI);
    }

    protected fileLoaded(fileData: ArrayBuffer) {
        this.loader.loadFile(fileData);
        this.loader.partition().then(meshes => {
            this.meshes = meshes;
            this.meshesReceived();
        });
    }

    protected createView(gammaLUT: HTMLImageElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>):
                         SVGDemoView {
        return new SVGDemoView(this, gammaLUT, commonShaderSource, shaderSources);
    }

    protected get defaultFile(): string {
        return DEFAULT_FILE;
    }

    private meshesReceived(): void {
        this.view.then(view => {
            view.attachMeshes([this.meshes]);
            view.initCameraBounds(this.loader.svgViewBox);
        });
    }
}

class SVGDemoView extends DemoView {
    renderer: SVGDemoRenderer;
    appController: SVGDemoController;

    get camera(): OrthographicCamera {
        return this.renderer.camera;
    }

    constructor(appController: SVGDemoController,
                gammaLUT: HTMLImageElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(gammaLUT, commonShaderSource, shaderSources);

        this.appController = appController;
        this.renderer = new SVGDemoRenderer(this, {sizeToFit: true});

        this.resizeToFit(true);
    }

    initCameraBounds(viewBox: glmatrix.vec4): void {
        this.renderer.initCameraBounds(viewBox);
    }
}

class SVGDemoRenderer extends SVGRenderer {
    renderContext: SVGDemoView;

    protected get loader(): SVGLoader {
        return this.renderContext.appController.loader;
    }

    protected get canvas(): HTMLCanvasElement {
        return this.renderContext.canvas;
    }

    protected newTimingsReceived(): void {
        this.renderContext.appController.newTimingsReceived(this.lastTimings);
    }
}

function main() {
    const controller = new SVGDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
