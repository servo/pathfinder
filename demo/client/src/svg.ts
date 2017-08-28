// pathfinder/client/src/svg.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as glmatrix from 'gl-matrix';
import 'path-data-polyfill.js';

import {ShaderMap, ShaderProgramSource} from './shader-loader';
import {PathfinderView} from './view';
import {panic} from './utils';
import AppController from './app-controller';

const SVG_NS: string = "http://www.w3.org/2000/svg";

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

declare class SVGPathSegment {
    type: string;
    values: number[];
}

declare class SVGPathElement {
    getPathData(settings: any): SVGPathSegment[];
}

class SVGDemoController extends AppController<SVGDemoView> {
    start() {
        super.start();

        this.svg = document.getElementById('pf-svg') as Element as SVGSVGElement;

        this.pathElements = [];

        this.loadFileButton = document.getElementById('pf-load-svg-button') as HTMLInputElement;
        this.loadFileButton.addEventListener('change', () => this.loadFile(), false);
    }

    protected fileLoaded() {
        const decoder = new (window as any).TextDecoder('utf-8');
        const fileStringData = decoder.decode(new DataView(this.fileData));
        const svgDocument = (new DOMParser).parseFromString(fileStringData, 'image/svg+xml');
        const svgElement = svgDocument.documentElement as Element as SVGSVGElement;
        this.attachSVG(svgElement);
    }

    protected createView(canvas: HTMLCanvasElement,
                         commonShaderSource: string,
                         shaderSources: ShaderMap<ShaderProgramSource>) {
        return new SVGDemoView(this, canvas, commonShaderSource, shaderSources);
    }

    private attachSVG(svgElement: SVGSVGElement) {
        // Clear out the current document.
        let kid;
        while ((kid = this.svg.firstChild) != null)
            this.svg.removeChild(kid);

        // Add all kids of the incoming SVG document.
        while ((kid = svgElement.firstChild) != null)
            this.svg.appendChild(kid);

        // Scan for geometry elements.
        this.pathElements.length = 0;
        const queue: Array<Element> = [this.svg];
        let element;
        while ((element = queue.pop()) != null) {
            for (const kid of element.childNodes) {
                if (kid instanceof Element)
                    queue.push(kid);
            }
            if (element instanceof SVGPathElement)
                this.pathElements.push(element);
        }

        // Extract and normalize the path data.
        let pathData = this.pathElements.map(element => element.getPathData({normalize: true}));

        // Build the partitioning request to the server.
        const request = {paths: pathData.map(segments => ({segments: segments}))};
        console.log(JSON.stringify(request));

        // Make the request.
        window.fetch(PARTITION_SVG_PATHS_ENDPOINT_URL, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(request),
        }).then(response => response.text()).then(responseText => {
            console.log(JSON.parse(responseText));
        });
    }

    private svg: SVGSVGElement;
    private pathElements: Array<SVGPathElement>;
}

class SVGDemoView extends PathfinderView {
    constructor(appController: SVGDemoController,
                canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(canvas, commonShaderSource, shaderSources);

        this.appController = appController;

        this.resized(false);
    }

    protected resized(initialSize: boolean) {}

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width, this.canvas.height);
    }

    get destDepthTexture() {
        return panic("TODO");
    }

    get destFramebuffer() {
        return panic("TODO");
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    setTransformAndTexScaleUniformsForDest() {
        panic("TODO");
    }

    setTransformSTAndTexScaleUniformsForDest() {
        panic("TODO");
    }

    private appController: SVGDemoController;
}

function main() {
    const controller = new SVGDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
