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

import {DemoAppController} from './app-controller';
import {AntialiasingStrategy, AntialiasingStrategyName, NoAAStrategy} from "./aa-strategy";
import {OrthographicCamera} from "./camera";
import {ECAAStrategy, ECAAMulticolorStrategy} from "./ecaa-strategy";
import {PathfinderMeshData} from "./meshes";
import {ShaderMap, ShaderProgramSource} from './shader-loader';
import {panic, unwrapNull} from './utils';
import {PathfinderDemoView, Timings} from './view';
import SSAAStrategy from "./ssaa-strategy";
import PathfinderBufferTexture from "./buffer-texture";

const parseColor = require('parse-color');

const SVG_NS: string = "http://www.w3.org/2000/svg";

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

const BUILTIN_SVG_URI: string = "/svg/demo";

const DEFAULT_FILE: string = 'tiger';

/// The minimum size of a stroke.
const HAIRLINE_STROKE_WIDTH: number = 0.25;

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

interface PathInstance {
    element: SVGPathElement;
    stroke: number | 'fill';
}

class SVGDemoController extends DemoAppController<SVGDemoView> {
    start() {
        super.start();

        this.svg = document.getElementById('pf-svg') as Element as SVGSVGElement;

        this.pathInstances = [];

        this.loadInitialFile();
    }

    protected fileLoaded() {
        const decoder = new (window as any).TextDecoder('utf-8');
        const fileStringData = decoder.decode(new DataView(this.fileData));
        const svgDocument = (new DOMParser).parseFromString(fileStringData, 'image/svg+xml');
        const svgElement = svgDocument.documentElement as Element as SVGSVGElement;
        this.attachSVG(svgElement);
    }

    protected createView() {
        return new SVGDemoView(this,
                               unwrapNull(this.commonShaderSource),
                               unwrapNull(this.shaderSources));
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
        this.pathInstances.length = 0;
        const queue: Array<Element> = [this.svg];
        let element;
        while ((element = queue.pop()) != null) {
            let kid = element.lastChild;
            while (kid != null) {
                if (kid instanceof Element)
                    queue.push(kid);
                kid = kid.previousSibling;
            }

            if (element instanceof SVGPathElement) {
                const style = window.getComputedStyle(element);
                if (style.fill !== 'none')
                    this.pathInstances.push({ element: element, stroke: 'fill' });
                if (style.stroke !== 'none') {
                    this.pathInstances.push({
                        element: element,
                        stroke: parseInt(style.strokeWidth!),
                    });
                }
            }
        }

        const request: any = { paths: [] };
        let minX = 0, minY = 0, maxX = 0, maxY = 0;

        // Extract, normalize, and transform the path data.
        for (const instance of this.pathInstances) {
            const element = instance.element;
            const svgCTM = element.getCTM();
            const ctm = glmatrix.mat2d.fromValues(svgCTM.a, svgCTM.b,
                                                  svgCTM.c, svgCTM.d,
                                                  svgCTM.e, svgCTM.f);
            glmatrix.mat2d.scale(ctm, ctm, [1.0, -1.0]);

            const segments = element.getPathData({normalize: true}).map(segment => {
                const newValues = _.flatMap(_.chunk(segment.values, 2), coords => {
                    const point = glmatrix.vec2.create();
                    glmatrix.vec2.transformMat2d(point, coords, ctm);

                    minX = Math.min(point[0], minX);
                    minY = Math.min(point[1], minY);
                    maxX = Math.max(point[0], maxX);
                    maxY = Math.max(point[1], maxY);

                    return [point[0], point[1]];
                });
                return {
                    type: segment.type,
                    values: newValues,
                };
            });

            let kind;
            if (instance.stroke === 'fill')
                kind = 'Fill';
            else
                kind = { Stroke: Math.max(HAIRLINE_STROKE_WIDTH, instance.stroke) };

            request.paths.push({ segments: segments, kind: kind });
        }

        const bounds = glmatrix.vec4.clone([minX, minY, maxX, maxY]);

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
            this.meshesReceived(bounds);
        });
    }

    protected get builtinFileURI(): string {
        return BUILTIN_SVG_URI;
    }

    protected get defaultFile(): string {
        return DEFAULT_FILE;
    }

    private meshesReceived(bounds: glmatrix.vec4): void {
        this.view.then(view => {
            view.uploadPathColors(1);
            view.uploadPathTransforms(1);
            view.attachMeshes([this.meshes]);

            view.camera.bounds = bounds;
            view.camera.zoomToFit();
        })
    }

    pathInstances: PathInstance[];

    private svg: SVGSVGElement;
    private meshes: PathfinderMeshData;
}

class SVGDemoView extends PathfinderDemoView {
    constructor(appController: SVGDemoController,
                commonShaderSource: string,
                shaderSources: ShaderMap<ShaderProgramSource>) {
        super(commonShaderSource, shaderSources);

        this.appController = appController;

        this.camera = new OrthographicCamera(this.canvas);
        this.camera.onPan = () => this.setDirty();
        this.camera.onZoom = () => this.setDirty();
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

    protected pathColorsForObject(objectIndex: number): Uint8Array {
        const instances = this.appController.pathInstances;
        const pathColors = new Uint8Array(4 * (instances.length + 1));

        for (let pathIndex = 0; pathIndex < instances.length; pathIndex++) {
            const startOffset = (pathIndex + 1) * 4;

            // Set color.
            const style = window.getComputedStyle(instances[pathIndex].element);
            const property = instances[pathIndex].stroke === 'fill' ? 'fill' : 'stroke';
            const color: number[] =
                style[property] === 'none' ? [0, 0, 0, 0] : parseColor(style[property]).rgba;
            pathColors.set(color.slice(0, 3), startOffset);
            pathColors[startOffset + 3] = color[3] * 255;
        }

        return pathColors;
    }

    protected pathTransformsForObject(objectIndex: number): Float32Array {
        const instances = this.appController.pathInstances;
        const pathTransforms = new Float32Array(4 * (instances.length + 1));

        for (let pathIndex = 0; pathIndex < instances.length; pathIndex++) {
            // TODO(pcwalton): Set transform.
            const startOffset = (pathIndex + 1) * 4;
            pathTransforms.set([1, 1, 0, 0], startOffset);
        }

        return pathTransforms;
    }

    protected createAAStrategy(aaType: AntialiasingStrategyName,
                               aaLevel: number,
                               subpixelAA: boolean):
                               AntialiasingStrategy {
        return new (ANTIALIASING_STRATEGIES[aaType])(aaLevel, subpixelAA);
    }

    protected compositeIfNecessary(): void {}

    protected updateTimings(timings: Timings) {
        // TODO(pcwalton)
    }

    protected usedSizeFactor: glmatrix.vec2 = glmatrix.vec2.fromValues(1.0, 1.0);

    protected get worldTransform() {
        const transform = glmatrix.mat4.create();
        const translation = this.camera.translation;
        glmatrix.mat4.translate(transform, transform, [-1.0, -1.0, 0.0]);
        glmatrix.mat4.scale(transform,
                            transform,
                            [2.0 / this.canvas.width, 2.0 / this.canvas.height, 1.0]);
        glmatrix.mat4.translate(transform, transform, [translation[0], translation[1], 0]);
        glmatrix.mat4.scale(transform, transform, [this.camera.scale, this.camera.scale, 1.0]);
        if (this.antialiasingStrategy != null)
            glmatrix.mat4.mul(transform, transform, this.antialiasingStrategy.transform);
        console.log(transform);
        return transform;
    }

    protected get directCurveProgramName(): keyof ShaderMap<void> {
        return 'directCurve';
    }

    protected get directInteriorProgramName(): keyof ShaderMap<void> {
        return 'directInterior';
    }

    protected depthFunction: number = this.gl.GREATER;

    private appController: SVGDemoController;

    camera: OrthographicCamera;
}

function main() {
    const controller = new SVGDemoController;
    window.addEventListener('load', () => controller.start(), false);
}

main();
