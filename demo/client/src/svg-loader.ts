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
import {lerp, panic, Range, unwrapNull, unwrapUndef} from "./utils";

export const BUILTIN_SVG_URI: string = "/svg/demo";

const parseColor = require('parse-color');

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

declare class SVGPathSegment {
    type: string;
    values: number[];
}

declare global {
    interface SVGPathElement {
        getPathData(settings: any): SVGPathSegment[];
    }
}

type FillRule = 'evenodd' | 'winding';

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
    fillRule: FillRule;

    get pathfinderFillRule(): string {
        return { evenodd: 'EvenOdd', winding: 'Winding' }[this.fillRule];
    }

    constructor(element: SVGPathElement) {
        super(element, 'fill');

        const style = window.getComputedStyle(element);
        this.fillRule = style.fillRule === 'evenodd' ? 'evenodd' : 'winding';
    }
}

export class SVGStroke extends SVGPath {
    width: number;

    constructor(element: SVGPathElement) {
        super(element, 'stroke');

        const style = window.getComputedStyle(element);
        const ctm = element.getCTM();

        const strokeWidthString = unwrapNull(style.strokeWidth);
        const matches = /^(\d+\.?\d*)(.*)$/.exec(strokeWidthString);
        let strokeWidth;
        if (matches == null) {
            strokeWidth = 0.0;
        } else {
            strokeWidth = parseFloat(matches[1]);
            if (matches[2] === 'px')
                strokeWidth *= lerp(ctm.a, ctm.d, 0.5);
        }

        this.width = strokeWidth;
    }
}

interface ClipPathIDTable {
    [id: string]: number;
}

export class SVGLoader {
    pathInstances: SVGPath[];
    scale: number;
    pathBounds: glmatrix.vec4[];
    svgBounds: glmatrix.vec4;
    svgViewBox: glmatrix.vec4;
    isMonochrome: boolean;

    private svg: SVGSVGElement;
    private fileData!: ArrayBuffer;

    private paths: any[];
    private clipPathIDs: ClipPathIDTable;

    constructor() {
        this.scale = 1.0;
        this.pathInstances = [];
        this.pathBounds = [];
        this.paths = [];
        this.clipPathIDs = {};
        this.svgBounds = glmatrix.vec4.create();
        this.svgViewBox = glmatrix.vec4.create();
        this.svg = unwrapNull(document.getElementById('pf-svg')) as Element as SVGSVGElement;
        this.isMonochrome = false;
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
            body: JSON.stringify({
                paths: paths,
                viewBoxHeight: this.svgViewBox[3] - this.svgViewBox[1],
                viewBoxWidth: this.svgViewBox[2] - this.svgViewBox[0],
            }),
            headers: {'Content-Type': 'application/json'} as any,
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

        const viewBox = svgElement.viewBox.baseVal;
        this.svg.setAttribute('width', "" + viewBox.width);
        this.svg.setAttribute('height', "" + viewBox.height);

        // Scan for geometry elements.
        this.pathInstances.length = 0;
        this.clipPathIDs = {};
        this.scanElement(this.svg);

        this.paths = [];

        for (const instance of this.pathInstances) {
            const element = instance.element;
            const svgCTM = element.getCTM();
            const ctm = glmatrix.mat2d.fromValues(svgCTM.a, -svgCTM.b,
                                                  svgCTM.c, -svgCTM.d,
                                                  svgCTM.e, viewBox.height - svgCTM.f);
            glmatrix.mat2d.scale(ctm, ctm, [this.scale, this.scale]);

            const bottomLeft = glmatrix.vec2.create();
            const topRight = glmatrix.vec2.create();

            const segments = element.getPathData({ normalize: true }).map(segment => {
                const newValues = _.flatMap(_.chunk(segment.values, 2), coords => {
                    const point = glmatrix.vec2.create();
                    glmatrix.vec2.transformMat2d(point, coords, ctm);

                    glmatrix.vec2.min(bottomLeft, bottomLeft, point);
                    glmatrix.vec2.max(topRight, topRight, point);

                    return [point[0], point[1]];
                });
                return {
                    type: segment.type,
                    values: newValues,
                };
            });

            const pathBounds = glmatrix.vec4.clone([
                bottomLeft[0], bottomLeft[1],
                topRight[0], topRight[1],
            ]);

            if (instance instanceof SVGFill) {
                this.paths.push({
                    kind: { Fill: instance.pathfinderFillRule },
                    segments: segments,
                });
                this.pathBounds.push(pathBounds);
            } else if (instance instanceof SVGStroke) {
                this.paths.push({ kind: { Stroke: instance.width }, segments: segments });
                this.pathBounds.push(pathBounds);
            }
        }

        this.isMonochrome = this.pathInstances.every(pathInstance => {
            return glmatrix.vec4.equals(pathInstance.color, this.pathInstances[0].color);
        });

        this.svgViewBox = glmatrix.vec4.clone([
            viewBox.x, viewBox.y, viewBox.x + viewBox.width, viewBox.y + viewBox.height,
        ]);
    }

    private scanElement(element: Element): void {
        const style = window.getComputedStyle(element);

        if (element instanceof SVGPathElement) {
            if (colorFromStyle(style.fill) != null)
                this.addPathInstance(new SVGFill(element));
            if (colorFromStyle(style.stroke) != null)
                this.addPathInstance(new SVGStroke(element));
        }

        for (const kid of element.childNodes) {
            if (kid instanceof Element)
                this.scanElement(kid);
        }
    }

    private addPathInstance(pathInstance: SVGPath): void {
        this.pathInstances.push(pathInstance);
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
