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
import {PathfinderMeshData} from "./meshes";
import {panic, unwrapNull} from "./utils";

export const BUILTIN_SVG_URI: string = "/svg/demo";

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

export interface PathInstance {
    element: SVGPathElement;
    stroke: number | 'fill';
}

export class SVGLoader {
    pathInstances: PathInstance[];
    scale: number;
    bounds: glmatrix.vec4;

    private svg: SVGSVGElement;
    private fileData: ArrayBuffer;

    private paths: any[];

    constructor() {
        this.scale = 1.0;
        this.svg = unwrapNull(document.getElementById('pf-svg')) as Element as SVGSVGElement;
        this.pathInstances = [];
        this.paths = [];
        this.bounds = glmatrix.vec4.create();
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
        return window.fetch(PARTITION_SVG_PATHS_ENDPOINT_URL, {
            body: JSON.stringify({ paths: paths }),
            headers: {'Content-Type': 'application/json'},
            method: 'POST',
        }).then(response => response.text()).then(responseText => {
            const response = JSON.parse(responseText);
            if (!('Ok' in response))
                panic("Failed to partition the font!");
            const meshes = base64js.toByteArray(response.Ok.pathData);
            return new PathfinderMeshData(meshes.buffer as ArrayBuffer);
        });
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
        const queue: Element[] = [this.svg];
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
                        stroke: parseInt(style.strokeWidth!, 10),
                    });
                }
            }
        }

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

            this.paths.push({ segments: segments, kind: kind });
        }

        this.bounds = glmatrix.vec4.clone([minX, minY, maxX, maxY]);
    }
}
