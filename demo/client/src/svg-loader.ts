// pathfinder/client/src/svg-loader.ts
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

import {panic, unwrapNull} from "./utils";
import {PathfinderMeshData} from "./meshes";

const PARTITION_SVG_PATHS_ENDPOINT_URL: string = "/partition-svg-paths";

/// The minimum size of a stroke.
const HAIRLINE_STROKE_WIDTH: number = 0.25;

export interface PathInstance {
    element: SVGPathElement;
    stroke: number | 'fill';
}

export class SVGLoader {
    constructor() {
        this.svg = unwrapNull(document.getElementById('pf-svg')) as Element as SVGSVGElement;
        this.pathInstances = [];
        this.bounds = glmatrix.vec4.create();
    }

    loadFile(fileData: ArrayBuffer): Promise<PathfinderMeshData> {
        this.fileData = fileData;

        const decoder = new (window as any).TextDecoder('utf-8');
        const fileStringData = decoder.decode(new DataView(this.fileData));
        const svgDocument = (new DOMParser).parseFromString(fileStringData, 'image/svg+xml');
        const svgElement = svgDocument.documentElement as Element as SVGSVGElement;
        return this.attachSVG(svgElement);
    }

    private attachSVG(svgElement: SVGSVGElement): Promise<PathfinderMeshData> {
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

        this.bounds = glmatrix.vec4.clone([minX, minY, maxX, maxY]);

        // Make the request.
        return window.fetch(PARTITION_SVG_PATHS_ENDPOINT_URL, {
            method: 'POST',
            headers: {'Content-Type': 'application/json'},
            body: JSON.stringify(request),
        }).then(response => response.text()).then(responseText => {
            const response = JSON.parse(responseText);
            if (!('Ok' in response))
                panic("Failed to partition the font!");
            const meshes = response.Ok.pathData;
            return new PathfinderMeshData(meshes);
        });
    }

    private svg: SVGSVGElement;
    private fileData: ArrayBuffer;

    pathInstances: PathInstance[];
    bounds: glmatrix.vec4;
}
