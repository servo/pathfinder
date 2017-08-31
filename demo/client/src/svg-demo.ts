// pathfinder/client/src/svg-demo.ts
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
import 'path-data-polyfill.js';

import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {ECAAStrategy, ECAAMulticolorStrategy} from "./ecaa-strategy";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from './shader-loader';
import {panic} from './utils';
import {PathfinderView, Timings} from './view';
import AppController from './app-controller';
import SSAAStrategy from "./ssaa-strategy";

require('../html/svg-demo.html');

const parseColor = require('parse-color');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

const BUILTIN_SVG_URI: string = "/svg/demo";

const ANTIALIASING_STRATEGIES: AntialiasingStrategyTable = {
    none: NoAAStrategy,
    ssaa: SSAAStrategy,
    ecaa: ECAAMulticolorStrategy,
};

declare class SVGPathSegment {
    type: string;
    values: number[];
}

declare global {
    interface SVGPathElement {
        getPathData(settings: any): SVGPathSegment[];
    }
}

interface AntialiasingStrategyTable {
    none: typeof NoAAStrategy;
    ssaa: typeof SSAAStrategy;
    ecaa: typeof ECAAStrategy;
}

class SVGDemoController extends AppController<SVGDemoView> {
    start() {
        super.start();

        this.svg = document.getElementById('pf-svg') as Element as SVGSVGElement;

        this.pathElements = [];

        this.loadInitialFile();
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
            let kid = element.lastChild;
            while (kid != null) {
                if (kid instanceof Element)
                    queue.push(kid);
                kid = kid.previousSibling;
            }

            if (element instanceof SVGPathElement)
                this.pathElements.push(element);
        }

        // Extract, normalize, and transform the path data.
        let pathData = [];
        for (const element of this.pathElements) {
            const svgCTM = element.getCTM();
            const ctm = glmatrix.mat2d.fromValues(svgCTM.a, svgCTM.b,
                                                  svgCTM.c, svgCTM.d,
                                                  svgCTM.e, svgCTM.f);
            glmatrix.mat2d.scale(ctm, ctm, [1.0, -1.0]);

            pathData.push(element.getPathData({normalize: true}).map(segment => {
                const newValues = _.flatMap(_.chunk(segment.values, 2), coords => {
                    const point = glmatrix.vec2.create();
                    glmatrix.vec2.transformMat2d(point, coords, ctm);
                    return [point[0], point[1]];
                });
                return {
                    type: segment.type,
                    values: newValues,
                };
            }));
        }

        // Build the partitioning request to the server.
        const request = {paths: pathData.map(segments => ({segments: segments}))};

        // Make the request.
        window.fetch(PARTITION_SVG_PATHS_ENDPOINT_URL, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(request),
        }).then(response => response.text()).then(responseText => {
            const response = JSON.parse(responseText);
            if (!('Ok' in response))
                panic("Failed to partition the font!");
            const meshes = response.Ok.pathData;
            this.meshes = new PathfinderMeshData(meshes);
            this.meshesReceived();
        });
    }

    protected get builtinFileURI(): string {
        return BUILTIN_SVG_URI;
    }

    private meshesReceived() {
        this.view.then(view => {
            view.uploadPathData(this.pathElements);
            view.attachMeshes(this.meshes);
        })
    }

    private svg: SVGSVGElement;
    private pathElements: Array<SVGPathElement>;
    private meshes: PathfinderMeshData;
}

class SVGDemoView extends PathfinderView {
    constructor(appController: SVGDemoController,
                canvas: HTMLCanvasElement,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(canvas, commonShaderSource, shaderSources);

        this.appController = appController;

        this._scale = 1.0;
    }

    protected resized(initialSize: boolean) {
        this.antialiasingStrategy.init(this);
        this.setDirty();
    }

    get destAllocatedSize(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(this.canvas.width, this.canvas.height);
    }

    get destFramebuffer(): WebGLFramebuffer | null {
        return null;
    }

    get destUsedSize(): glmatrix.vec2 {
        return this.destAllocatedSize;
    }

    protected panned(): void {
        this.setDirty();
    }

    uploadPathData(elements: SVGPathElement[]) {
        const pathColors = new Uint8Array(4 * (elements.length + 1));
        const pathTransforms = new Float32Array(4 * (elements.length + 1));
        for (let pathIndex = 0; pathIndex < elements.length; pathIndex++) {
            const startOffset = (pathIndex + 1) * 4;

            // Set color.
            const style = window.getComputedStyle(elements[pathIndex]);
            const fillColor: number[] =
                style.fill === 'none' ? [0, 0, 0, 0] : parseColor(style.fill).rgba;
            pathColors.set(fillColor.slice(0, 3), startOffset);
            pathColors[startOffset + 3] = fillColor[3] * 255;

            // TODO(pcwalton): Set transform.
            pathTransforms.set([1, 1, 0, 0], startOffset);
        }

        this.pathColorsBufferTexture.upload(this.gl, pathColors);
        this.pathTransformBufferTexture.upload(this.gl, pathTransforms);
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName, aaLevel: number):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel);
    }

    protected compositeIfNecessary(): void {}

    protected updateTimings(timings: Timings) {
        // TODO(pcwalton)
    }

    protected get usedSizeFactor(): glmatrix.vec2 {
        return glmatrix.vec2.fromValues(1.0, 1.0);
    }

    protected get scale(): number {
        return this._scale;
    }

    protected set scale(newScale: number) {
        this._scale = newScale;
        this.setDirty();
    }

    protected get worldTransform() {
        const transform = glmatrix.mat4.create();
        glmatrix.mat4.fromTranslation(transform, [this.translation[0], this.translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.scale, this.scale, 1.0]);
        return transform;
    }

    private _scale: number;

    private appController: SVGDemoController;
}

function main() {
    const controller = new SVGDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
