// pathfinder/client/src/svg-loader.ts
//
// Copyright © 2017 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

import * as base64js from 'base64-js';
import * as glmatrix from 'gl-matrix';
import * as _ from 'lodash';

import 'path-data-polyfill.js';
import {parseServerTiming, PathfinderMeshData} from "./meshes";
import {AlphaMaskCompositingOperation, RenderTask, RenderTaskType} from './render-task';
import {panic, Range, unwrapNull, unwrapUndef} from "./utils";

export const BUILTIN_SVG_URI: string = "/svg/demo";

const parseColor = require('parse-color');

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

/// The minimum size of a stroke.
const HAIRLINE_STROKE_WIDTH: number = 0.25;

declare class SVGPathSegment {
    type: string;
    values: number[];
}

declare global {
    interface SVGPathElement {
        getPathData(settings: any): SVGPathSegment[];
    }
}

export abstract class SVGPath {
    element: SVGPathElement;
    color: glmatrix.vec4;

    constructor(element: SVGPathElement, colorProperty: keyof CSSStyleDeclaration) {
        this.element = element;

        const style = window.getComputedStyle(element);
        this.color = unwrapNull(colorFromStyle(style[colorProperty]));
    }
}

export class SVGFill extends SVGPath {
    constructor(element: SVGPathElement) {
        super(element, 'fill');
    }
}

export class SVGStroke extends SVGPath {
    width: number;

    constructor(element: SVGPathElement) {
        super(element, 'stroke');

        const style = window.getComputedStyle(element);
        this.width = parseInt(style.strokeWidth!, 10);
    }
}

interface ClipPathIDTable {
    [id: string]: number;
}

export class SVGLoader {
    renderTasks: RenderTask[];
    pathInstances: SVGPath[];
    scale: number;
    bounds: glmatrix.vec4;

    private svg: SVGSVGElement;
    private fileData: ArrayBuffer;

    private paths: any[];
    private clipPathIDs: ClipPathIDTable;

    constructor() {
        this.scale = 1.0;
        this.renderTasks = [];
        this.pathInstances = [];
        this.paths = [];
        this.bounds = glmatrix.vec4.create();
        this.svg = unwrapNull(document.getElementById('pf-svg')) as Element as SVGSVGElement;
    }

    loadFile(fileData: ArrayBuffer) {
        this.fileData = fileData;

        const decoder = new (window as any).TextDecoder('utf-8');
        const fileStringData = decoder.decode(new DataView(this.fileData));
        const svgDocument = (new DOMParser).parseFromString(fileStringData, 'image/svg+xml');
        const svgElement = svgDocument.documentElement as Element as SVGSVGElement;
        this.attachSVG(svgElement);
    }

    partition(pathIndex?: number | undefined): Promise<PathfinderMeshData> {
        // Make the request.
        const paths = pathIndex == null ? this.paths : [this.paths[pathIndex]];
        let time = 0;
        return window.fetch(PARTITION_SVG_PATHS_ENDPOINT_URL, {
            body: JSON.stringify({ paths: paths }),
            headers: {'Content-Type': 'application/json'},
            method: 'POST',
        }).then(response => {
            time = parseServerTiming(response.headers);
            return response.arrayBuffer();
        }).then(buffer => new PathfinderMeshData(buffer));
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
        this.renderTasks.length = 0;
        this.pathInstances.length = 0;
        this.clipPathIDs = {};
        this.pushNewRenderTask('color');
        this.scanElement(this.svg);
        this.popTopRenderTaskIfEmpty();

        let minX = 0, minY = 0, maxX = 0, maxY = 0;
        this.paths = [];

        // Extract, normalize, and transform the path data.
        for (const instance of this.pathInstances) {
            const element = instance.element;
            const svgCTM = element.getCTM();
            const ctm = glmatrix.mat2d.fromValues(svgCTM.a, svgCTM.b,
                                                  svgCTM.c, svgCTM.d,
                                                  svgCTM.e, svgCTM.f);
            glmatrix.mat2d.scale(ctm, ctm, [1.0, -1.0]);
            glmatrix.mat2d.scale(ctm, ctm, [this.scale, this.scale]);

            const segments = element.getPathData({ normalize: true }).map(segment => {
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

            if (instance instanceof SVGFill) {
                this.paths.push({ segments: segments, kind: 'Fill' });
            } else if (instance instanceof SVGStroke) {
                this.paths.push({
                    kind: { Stroke: Math.max(HAIRLINE_STROKE_WIDTH, instance.width) },
                    segments: segments,
                });
            }
        }

        this.bounds = glmatrix.vec4.clone([minX, minY, maxX, maxY]);
    }

    private scanElement(element: Element): void {
        const currentRenderTask = unwrapUndef(_.last(this.renderTasks));
        const style = window.getComputedStyle(element);

        let hasClip = style.clipPath != null && style.clipPath !== 'none';
        if (hasClip) {
            const matches = /^url\("#([^"]+)"\)$/.exec(unwrapNull(style.clipPath));
            if (matches == null ||
                matches[1] == null ||
                !this.clipPathIDs.hasOwnProperty(matches[1])) {
                hasClip = false;
            } else {
                currentRenderTask.compositingOperation =
                    new AlphaMaskCompositingOperation(this.clipPathIDs[matches[1]]);
            }
        }

        if (element instanceof SVGPathElement) {
            if (colorFromStyle(style.fill) != null)
                this.addPathInstance(new SVGFill(element));
            if (colorFromStyle(style.stroke) != null)
                this.addPathInstance(new SVGStroke(element));
        }

        if (element instanceof SVGClipPathElement) {
            this.pushNewRenderTask('clip');
            this.clipPathIDs[element.id] = this.renderTasks.length - 1;
        }

        for (const kid of element.childNodes) {
            if (kid instanceof Element)
                this.scanElement(kid);
        }

        if (element instanceof SVGClipPathElement || hasClip)
            this.pushNewRenderTask('color');
    }

    private addPathInstance(pathInstance: SVGPath): void {
        const currentRenderTask = unwrapUndef(_.last(this.renderTasks));
        this.pathInstances.push(pathInstance);
        currentRenderTask.instanceIndices.end = Math.max(currentRenderTask.instanceIndices.end,
                                                         this.pathInstances.length + 1);
    }

    private popTopRenderTaskIfEmpty(): void {
        const lastRenderTask = _.last(this.renderTasks);
        if (lastRenderTask != null && lastRenderTask.instanceIndices.isEmpty)
            this.renderTasks.pop();
    }

    private pushNewRenderTask(taskType: RenderTaskType): void {
        this.popTopRenderTaskIfEmpty();
        const emptyRange = new Range(this.pathInstances.length + 1, this.pathInstances.length + 1);
        this.renderTasks.push(new RenderTask(taskType, emptyRange));
    }
}

function colorFromStyle(style: string | null): glmatrix.vec4 | null {
    if (style == null || style === 'none')
        return null;

    // TODO(pcwalton): Gradients?
    const color = parseColor(style);
    if (color.rgba == null)
        return glmatrix.vec4.clone([0.0, 0.0, 0.0, 1.0]);

    if (color.rgba[3] === 0.0)
        return null;
    return glmatrix.vec4.clone(color.rgba);
}
